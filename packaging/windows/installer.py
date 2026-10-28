#!/usr/bin/env python3
"""EN-US: Compile a per-user NSIS installer from the exact standalone ZIP payload.
PT-BR: Compila um instalador NSIS por usuário com o conteúdo exato do ZIP independente.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import re
import shutil
import stat
import subprocess
import sys
import zipfile

from build import ROOT, require_source, sha256, source_state, write_json
from resources import generate

RESERVED = re.compile(r"^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)", re.I)


def compiler_path(path):
    # EN-US: NSIS splits File paths only at its host separator; UCRT64 Python emits forward slashes.
    # PT-BR: O NSIS separa caminhos File só pelo separador nativo; o Python UCRT64 emite barras normais.
    return str(PureWindowsPath(path)) if sys.platform == "win32" else str(path)


def literal(value):
    if any(ord(c) < 32 for c in value):
        raise ValueError("Control character in installer input")
    return value.replace("$", "$$").replace('"', '$\\"')


def safe_name(name):
    parts = name.split("/")
    if (not name or len(name) > 220 or any(not part or part in (".", "..")
            or part[-1] in " ." or RESERVED.match(part)
            or any(c in part for c in '\\:<>"|?*') or any(ord(c) < 32 for c in part) for part in parts)):
        raise ValueError(f"Unsafe Windows payload name: {name}")
    return name


def inventory(stage, archive):
    stage, archive = Path(stage).resolve(), Path(archive).resolve()
    expected = archive.with_suffix(".zip.sha256").read_text(encoding="utf-8").split()[0]
    if sha256(archive) != expected:
        raise ValueError("Standalone ZIP checksum mismatch")
    entries = {}
    seen = set()
    total = 0
    with zipfile.ZipFile(archive) as bundle:
        for item in bundle.infolist():
            name = item.filename.rstrip("/")
            safe_name(name)
            if name.casefold() in seen:
                raise ValueError("Duplicate/case-colliding ZIP entry")
            seen.add(name.casefold())
            if stat.S_IFMT(item.external_attr >> 16) == stat.S_IFLNK or item.flag_bits & 1:
                raise ValueError("Symlink or encrypted installer payload")
            parts = PurePosixPath(name).parts
            if parts[0] != stage.name:
                raise ValueError("ZIP root differs from the staged package")
            if item.is_dir():
                continue
            relative = safe_name("/".join(parts[1:]))
            if relative.casefold() in {"install.ini", "uninstall.exe"}:
                raise ValueError("Payload overwrites installer metadata")
            path = stage / relative
            if path.is_symlink() or not path.resolve().is_relative_to(stage) or not path.is_file():
                raise ValueError("Missing/redirected staged payload")
            total += item.file_size
            if total > 4 * 1024**3 or len(entries) >= 60000:
                raise ValueError("Installer payload exceeds size/file limits")
            with bundle.open(item) as stream:
                digest = hashlib.file_digest(stream, "sha256").hexdigest()
            if path.stat().st_size != item.file_size or sha256(path) != digest:
                raise ValueError(f"Staged payload differs from ZIP: {relative}")
            entries[relative] = {"sha256": digest, "size": item.file_size}
    actual = {path.relative_to(stage).as_posix() for path in stage.rglob("*") if path.is_file()}
    if actual != entries.keys() or any(path.is_symlink() for path in stage.rglob("*")):
        raise ValueError("Unrecorded or redirected files in staged payload")
    info = json.loads((stage / "BUILD-INFO.json").read_text(encoding="utf-8"))
    if (info["schema"] != 1 or info["source_dirty"] or info["source"]["status"]
            or info["source"]["untracked_sha256"] or info["build"]["allow_dirty"]
            or not re.fullmatch(r"[0-9a-f]{40}", info["source"]["commit"])):
        raise ValueError("Installer requires a clean, identified source payload")
    if not re.fullmatch(r"\d+\.\d+\.\d+", info["version"]):
        raise ValueError("Invalid installer version")
    required = {"Papo.exe", "Papo.com", "BUILD-INFO.json", "LICENSE", "runtime/bin/papo-gtk.exe",
                "runtime/bin/papo-shell.exe", "runtime/bin/python.exe", "runtime/RUNTIME-MANIFEST.json"}
    if not required <= entries.keys():
        raise ValueError("Incomplete standalone installer payload")
    if entries["runtime/bin/papo-gtk.exe"]["sha256"] != info["binary_sha256"]:
        raise ValueError("Release executable identity mismatch")
    if entries["runtime/RUNTIME-MANIFEST.json"]["sha256"] != info["runtime"]["manifest_sha256"]:
        raise ValueError("Runtime manifest identity mismatch")
    return info, entries


def prepare(stage, archive, work, output, uninstaller_command=None, release_candidate=False):
    stage, archive, work, output = (Path(p).resolve() for p in (stage, archive, work, output))
    info, files = inventory(stage, archive)
    work.mkdir(parents=True, exist_ok=True)
    generate(work, info["version"], "Papo.exe")
    shutil.copy2(ROOT / "packaging/windows/installer.nsi", work / "installer.nsi")
    identity = json.loads((ROOT / "data/windows/identity.json").read_text(encoding="utf-8"))
    constants = {"PAPO_VERSION": info["version"], "PAPO_ARCHIVE_SHA256": sha256(archive),
                 "PAPO_INSTALLER_LABEL": f"Papo {info['version']}" + ("" if release_candidate else " (Windows preview)"),
                 "PAPO_PROGRAM_NAME": "Papo" if release_candidate else "Papo (Windows preview)",
                 "PAPO_APP_ID": identity["app_id"], "PAPO_PROTOCOL": identity["protocol"],
                 "PAPO_OUTPUT": compiler_path(output), "PAPO_ICON": compiler_path(work / "papo.ico"),
                 "PAPO_LICENSE": compiler_path(stage / "LICENSE")}
    if uninstaller_command is not None:
        constants["PAPO_UNINSTALLER_SIGN_COMMAND"] = uninstaller_command
    (work / "config.nsh").write_text("\n".join(f'!define {key} "{literal(value)}"' for key, value in constants.items()) + "\n", encoding="utf-8")
    directories = {str(parent) for name in files for parent in PurePosixPath(name).parents if str(parent) != "."}
    install = ["!macro InstallPayload"]
    for name in sorted(files):
        parent = PurePosixPath(name).parent.as_posix()
        target = "$INSTDIR" + ("\\" + literal(parent.replace("/", "\\")) if parent != "." else "")
        install += [f'SetOutPath "{target}"', f'File "{literal(compiler_path(stage / name))}"', "IfErrors install_failed"]
    install += ["!macroend"]
    check = ["!macro CheckPayload"]
    for name in sorted(directories | set(files)):
        target = "$INSTDIR\\" + literal(name.replace("/", "\\"))
        check += [f'System::Call \'kernel32::GetFileAttributesW(w "{target}") i.r0\'',
                  "${If} $0 != -1", "IntOp $0 $0 & 0x400", "${If} $0 != 0", "StrCpy $Failure 1", "${EndIf}", "${EndIf}"]
        if name in files:
            check += [f'System::Call \'kernel32::CreateFileW(w "{target}", i 0x10000, i 7, p 0, i 3, i 0, p 0) p.r0 ?e\'',
                      "Pop $1", "${If} $0 == -1", "${If} $1 != 2", "${AndIf} $1 != 3",
                      "StrCpy $Failure 1", "${EndIf}", "${Else}", "System::Call 'kernel32::CloseHandle(p r0)'", "${EndIf}"]
    check += ["!macroend"]
    remove = ["!macro RemovePayload"]
    for name in sorted(files):
        target = "$INSTDIR\\" + literal(name.replace("/", "\\"))
        remove += ["ClearErrors", f'Delete "{target}"', "${If} ${Errors}", "StrCpy $Failure 1", "${EndIf}"]
    for name in sorted(directories, key=lambda n: (n.count("/"), n), reverse=True):
        remove += [f'RMDir "$INSTDIR\\{literal(name.replace(chr(47), chr(92)))}"']
    remove += ["!macroend"]
    (work / "payload.nsh").write_text("\n".join(install + check + remove) + "\n", encoding="utf-8")
    return info, files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=ROOT / "target/windows-installer")
    parser.add_argument("--makensis", default="makensis")
    args = parser.parse_args()
    before = source_state()
    require_source(before)
    info = json.loads((args.stage / "BUILD-INFO.json").read_text(encoding="utf-8"))
    if info["source"]["commit"] != before["commit"]:
        raise SystemExit("Installer recipe must match the packaged source revision")
    # EN-US: Avoid setup/install/update filename heuristics that load Windows compatibility shims.
    # PT-BR: Evita heurísticas de nomes setup/install/update que carregam shims de compatibilidade.
    output = (ROOT / "dist" / f"Papo-{info['version']}-windows-x86_64.exe").resolve()
    info, files = prepare(args.stage, args.archive, args.work, output)
    compiler = subprocess.check_output([args.makensis, "-VERSION"], text=True).strip()
    # EN-US: A failed rebuild must not leave an older manifest looking like a new artifact.
    # PT-BR: Uma recompilação com falha não pode deixar um manifesto antigo parecendo um novo artefato.
    for stale in (output, output.with_suffix(".exe.sha256"), output.with_suffix(".exe.json")):
        stale.unlink(missing_ok=True)
    subprocess.run([args.makensis, "-INPUTCHARSET", "UTF8", "-WX", "-V3", "installer.nsi"], cwd=args.work.resolve(), check=True)
    require_source(before, source_state())
    output.with_suffix(".exe.sha256").write_text(f"{sha256(output)}  {output.name}\n", encoding="utf-8")
    write_json(output.with_suffix(".exe.json"), {
        "schema": 1, "kind": "nsis-per-user-preview", "version": info["version"],
        "source_commit": info["source"]["commit"], "source_dirty": False,
        "archive": {"name": args.archive.name, "sha256": sha256(args.archive)},
        "installer": {"name": output.name, "sha256": sha256(output)},
        "build_info_sha256": sha256(args.stage / "BUILD-INFO.json"),
        "runtime_manifest_sha256": info["runtime"]["manifest_sha256"],
        "compiler": {"name": "NSIS", "version": compiler, "sha256": sha256(Path(shutil.which(args.makensis) or args.makensis))},
        "recipe_sha256": {name: sha256(ROOT / "packaging/windows" / name) for name in ("installer.py", "installer.nsi")},
        "payload_files": len(files), "signed": False, "release_qualified": False,
    })
    print(f"Per-user preview installer: {output}")


if __name__ == "__main__":
    main()
