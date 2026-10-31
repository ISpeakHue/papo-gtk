#!/usr/bin/env python3
"""EN-US: Build an auditable Windows preview with an application-owned UCRT64 runtime.
PT-BR: Compila uma prévia Windows auditável com runtime UCRT64 próprio do aplicativo.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
BUILD_COMMAND = ["cargo", "build", "--release", "--locked", "--target", "x86_64-pc-windows-gnu",
                 "--target-dir", "target/windows-build/cargo"]
# EN-US: Record only build controls; dumping the environment could expose CI secrets.
# PT-BR: Registra apenas controles de compilação; copiar o ambiente poderia expor segredos da CI.
BUILD_ENV = ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET",
             "CARGO_TARGET_DIR", "CARGO_PROFILE_RELEASE_OPT_LEVEL", "CARGO_PROFILE_RELEASE_LTO",
             "CARGO_PROFILE_RELEASE_STRIP", "SOURCE_DATE_EPOCH", "CC", "CXX", "AR",
             "PKG_CONFIG_PATH", "MSYSTEM", "MINGW_PREFIX")


def run(*command, root=ROOT, optional=False):
    result = subprocess.run(command, cwd=root, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace")
    if result.returncode and not optional:
        raise RuntimeError(f"{command[0]} failed ({result.returncode}): {result.stderr.strip()}")
    return result.stdout.rstrip()


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def source_state(root=ROOT):
    # EN-US: Keep file-mode/EOL/generated-file evidence; never hide a dirty worktree.
    # PT-BR: Preserva evidências de modos/fins de linha/arquivos gerados; nunca oculta alterações locais.
    untracked = run("git", "ls-files", "--others", "--exclude-standard", "-z", root=root)
    untracked_hashes = {}
    for name in untracked.split("\0"):
        if name:
            path = root / name
            untracked_hashes[name] = (hashlib.sha256(os.readlink(path).encode()).hexdigest()
                                      if path.is_symlink() else sha256(path))
    return {
        "untracked_sha256": untracked_hashes,
        "index_diff_sha256": hashlib.sha256(run("git", "diff", "--cached", "--no-ext-diff", "--binary", root=root).encode()).hexdigest(),
        "diff_sha256": hashlib.sha256(run("git", "diff", "--no-ext-diff", "--binary", "HEAD", root=root).encode()).hexdigest(),
        "commit": run("git", "rev-parse", "HEAD", root=root),
        "status": run("git", "status", "--porcelain=v1", "--untracked-files=all", root=root),
        "diff_stat": run("git", "diff", "--stat", "HEAD", root=root),
        "diff_summary": run("git", "diff", "--summary", "HEAD", root=root),
        "diff_numstat": run("git", "diff", "--numstat", "HEAD", root=root),
        "line_endings": run("git", "ls-files", "--eol", root=root),
        "git_config": run("git", "config", "--show-origin", "--get-regexp",
                          r"^core\.(autocrlf|eol|filemode|symlinks|ignorecase)$", root=root, optional=True),
        "git_version": run("git", "--version", root=root),
    }


def require_source(before, after=None, allow_dirty=False):
    if before["status"] and not allow_dirty:
        raise RuntimeError("Source checkout is dirty; inspect target/windows-build/source-before.json. "
                           "Commit intended changes, or explicitly use --allow-dirty for local previews.")
    if after is not None and after != before:
        raise RuntimeError("Source changed during compilation; inspect source-before/after.json. "
                           "No ZIP will be published.")


def write_json(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def packaged_environment(environment):
    # EN-US: SDK workarounds and capture opt-ins cannot qualify the default shipped runtime.
    # PT-BR: Ajustes do SDK e captura experimental não validam o runtime padrão distribuído.
    prefixes = ('PAPO_', 'PYTHON', 'GST_', 'GI_', 'GIO_', 'FONTCONFIG_', 'XDG_')
    return {key: value for key, value in environment.items()
            if not key.upper().startswith(prefixes) and key.upper() != 'GSETTINGS_SCHEMA_DIR'}


def require_gstreamer(inventory):
    recipe = (ROOT / 'packaging/windows/gstreamer/GSTREAMER-PKGBUILD').read_text(encoding='utf-8')
    fields = [re.search(rf'^{name}=([\d.]+)$', recipe, re.MULTILINE) for name in ('pkgver', 'pkgrel')]
    if not all(fields):
        raise RuntimeError('Cannot identify the reviewed GStreamer recipe.')
    expected = 'mingw-w64-ucrt-x86_64-gstreamer ' + '-'.join(field[1] for field in fields)
    if inventory.strip() != expected:
        raise RuntimeError(f'GStreamer must match the Unicode-path fixed recipe: {expected}. Run scripts/windows-dev.ps1 Setup.')


def remove_stage(stage, distribution):
    # EN-US: Only remove the generated direct child; never traverse redirected staging paths.
    # PT-BR: Remove apenas o filho gerado; nunca percorre caminhos de staging redirecionados.
    stage, distribution = Path(stage).absolute(), Path(distribution).absolute()
    if stage.parent != distribution or distribution.resolve() != distribution or stage.resolve() != stage:
        raise RuntimeError('Staging cleanup escaped the distribution directory or follows a redirect')
    for directory, folders, files in os.walk(stage, followlinks=False):
        for path in (Path(directory), *(Path(directory) / name for name in folders + files)):
            info = path.lstat()
            if stat.S_ISLNK(info.st_mode) or getattr(info, 'st_file_attributes', 0) & 0x400:
                raise RuntimeError('Staging cleanup refuses links/reparse points')

    def retry_readonly(function, path, error):
        info = os.lstat(path)
        if (os.name != 'nt' or not isinstance(error, PermissionError)
                or not stat.S_ISREG(info.st_mode) or not getattr(info, 'st_file_attributes', 0) & 1):
            raise error
        # EN-US: copy2 preserves native read-only data flags; a real lock/ACL failure must still fail.
        # PT-BR: copy2 preserva arquivos somente leitura; falhas reais de bloqueio/ACL continuam falhando.
        os.chmod(path, stat.S_IWRITE)
        function(path)

    shutil.rmtree(stage, onexc=retry_readonly)


def toolchain():
    prefix = Path(sys.executable).resolve().parent
    versions = {}
    for name, arguments in {
        "cargo.exe": ["--version", "--verbose"], "rustc.exe": ["-vV"],
        "gcc.exe": ["--version"], "g++.exe": ["--version"], "ar.exe": ["--version"], "pkg-config.exe": ["--version"],
        "glib-compile-resources.exe": ["--version"],
    }.items():
        executable = shutil.which(name)
        if not executable or Path(executable).resolve().parent != prefix:
            raise RuntimeError(f"{name} must come from the same UCRT64 bin directory as Python.")
        versions[name] = {"path": executable, "version": run(executable, *arguments)}
    if "host: x86_64-pc-windows-gnu" not in versions["rustc.exe"]["version"]:
        raise RuntimeError("The port requires native x86_64-pc-windows-gnu Rust.")
    versions["python"] = {"path": sys.executable, "version": sys.version}
    gstreamer = run('pacman', '-Q', 'mingw-w64-ucrt-x86_64-gstreamer')
    require_gstreamer(gstreamer)
    versions['gstreamer_package'] = {'version': gstreamer}
    return versions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-dirty", action="store_true", help="Explicitly label a local development build")
    parser.add_argument("--native-inputs", type=Path, required=True,
                        help="Verified archive directory produced by native_inputs.py")
    parser.add_argument("--preflight", action="store_true", help="Record source, tools and inputs without compiling")
    parser.add_argument("--packaging-only", action="store_true", help="Rebuild only an identical, fully tested source/toolchain")
    parser.add_argument("--tested-gate", type=Path, help="Successful full-run Windows-release-gate/gate.json")
    parser.add_argument("--tested-run", type=Path, help="Metadata obtained from the GitHub Actions run API")
    args = parser.parse_args()
    if args.packaging_only and (not args.tested_gate or not args.tested_run or args.allow_dirty):
        parser.error("--packaging-only requires --tested-gate and --tested-run; dirty rebuilds are forbidden")
    if not args.packaging_only and (args.tested_gate or args.tested_run):
        parser.error("Tested-run evidence is only valid with --packaging-only")
    if sys.platform != "win32" or os.environ.get("MSYSTEM") != "UCRT64":
        raise SystemExit("Run this script with UCRT64 Python from an MSYS2 UCRT64 terminal.")
    if not shutil.which("git"):
        raise SystemExit("Git is required for source metadata; install it with pacman -S git.")
    diagnostics = ROOT / "target/windows-build"
    before = source_state()
    write_json(diagnostics / "source-before.json", before)
    require_source(before, allow_dirty=args.allow_dirty)
    versions = toolchain()
    from native_inputs import verify_archive
    inputs = verify_archive(args.native_inputs, installed=run("pacman", "-Q"))
    if os.environ.get("CARGO_BUILD_TARGET") or os.environ.get("CARGO_TARGET_DIR"):
        raise RuntimeError("Custom Cargo target/output directories are not supported by this staging recipe.")
    environment = {
        "source": before, "toolchain": versions, "native_inputs_manifest_sha256": inputs["manifest_sha256"],
        "command": BUILD_COMMAND,
        "environment": {key: os.environ[key] for key in BUILD_ENV if key in os.environ},
    }
    rebuild = None
    if args.packaging_only:
        from release_gate import check_rebuild
        rebuild = check_rebuild(args.tested_gate, args.tested_run, {
            "source": before, "toolchain": versions, "native_inputs": inputs,
            "build": {"target": "x86_64-pc-windows-gnu", "profile": "release",
                      "environment": environment["environment"], "cargo_lock_sha256": sha256(ROOT / "Cargo.lock")},
        }, os.environ.get("GITHUB_REPOSITORY", "ISpeakHue/papo-gtk"))
    write_json(diagnostics / "environment.json", environment)
    if args.preflight:
        return
    subprocess.run(BUILD_COMMAND, cwd=ROOT, check=True)
    after = source_state()
    write_json(diagnostics / "source-after.json", after)
    require_source(before, after, allow_dirty=args.allow_dirty)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    stage = ROOT / "dist" / f"Papo-{version}-windows-x86_64-dev"
    if stage.exists():
        remove_stage(stage, ROOT / 'dist')
    stage.mkdir(parents=True)
    from bundle import Bundle
    runtime = Bundle(stage, Path(sys.executable).resolve().parent.parent).prepare(
        ROOT / "target/windows-build/cargo/x86_64-pc-windows-gnu/release/papo-gtk.exe",
        before, inputs["manifest_sha256"],
    )
    for name in ("Papo.cmd", "README.txt"):
        shutil.copy2(ROOT / "packaging/windows" / name, stage)
    shutil.copy2(ROOT / "LICENSE", stage)
    shutil.copytree(ROOT / "assets/icons/symbolic/licenses", stage / "licenses/symbolic-icons")
    dirty = bool(before["status"])
    source_label = "Base source (local changes present)" if dirty else "Matching source"
    (stage / "SOURCE.txt").write_text(
        f"{source_label}: https://github.com/ISpeakHue/papo-gtk/tree/{before['commit']}\n",
        encoding="utf-8",
    )
    info = {
        "schema": 1, "version": version, "source": before, "source_dirty": dirty,
        "toolchain": versions, "native_inputs": inputs,
        "build": {"command": BUILD_COMMAND, "profile": "release", "target": "x86_64-pc-windows-gnu",
                  "environment": {key: os.environ[key] for key in BUILD_ENV if key in os.environ},
                  "cargo_lock_sha256": sha256(ROOT / "Cargo.lock"),
                  "packaging_only": args.packaging_only, "allow_dirty": args.allow_dirty},
        "host": {"system": platform.system(), "release": platform.release(), "version": platform.version(),
                 "machine": platform.machine()},
        "ci": {key: os.environ.get(key) for key in
               ("GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_WORKFLOW", "GITHUB_SHA", "ImageOS", "ImageVersion")},
        "binary_sha256": sha256(stage / "runtime/bin/papo-gtk.exe"),
        "tested_rebuild": rebuild,
        "runtime": runtime,
        "byte_identical_rebuild_demonstrated": False,
    }
    write_json(stage / "BUILD-INFO.json", info)
    write_json(diagnostics / "BUILD-INFO.json", info)
    # EN-US: Remove developer runtime overrides; validate the real native bootstrap.
    # PT-BR: Remove substituições do ambiente de desenvolvimento e valida o iniciador nativo.
    clean_env = packaged_environment(os.environ)
    subprocess.run([str(stage / "Papo.com"), "--platform-info"], check=True, env=clean_env)
    subprocess.run([str(stage / "Papo.com"), "--check-packaged-runtime"], check=True, env=clean_env)
    # EN-US: Recheck after staging too; output is ignored, source mutations are not.
    # PT-BR: Confere novamente após preparar o pacote; saídas são ignoradas, alterações no código não.
    final = source_state()
    write_json(diagnostics / "source-final.json", final)
    require_source(before, final, allow_dirty=args.allow_dirty)
    archive = Path(shutil.make_archive(str(stage), "zip", stage.parent, stage.name))
    archive.with_suffix(".zip.sha256").write_text(f"{sha256(archive)}  {archive.name}\n", encoding="utf-8")
    print(f"Developer preview: {archive} (includes the application-owned UCRT64 runtime)")


if __name__ == "__main__":
    main()
