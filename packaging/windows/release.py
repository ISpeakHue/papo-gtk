#!/usr/bin/env python3
"""EN-US: Prepare signed Windows candidates and qualify exact tested files before publication.
PT-BR: Prepara candidatos Windows assinados e qualifica os arquivos exatos antes de publicar.
"""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

from authenticode import signing_only
from build import ROOT, require_source, sha256, source_state, write_json
from installer import inventory, prepare, safe_name
from release_gate import aggregate, authorize_rebuild, read, require

OWNED = ("Papo.exe", "Papo.com", "runtime/bin/papo-gtk.exe", "runtime/bin/papo-shell.exe", "runtime/bin/papo-capture.dll")
TARGETS = {"W25H": ("Home", "25H2", (10, 0, 26200, 9711)), "W25P": ("Pro", "25H2", (10, 0, 26200, 9711)),
           "W26H": ("Home", "26H2", (10, 0, 26300, 9711)), "W26P": ("Pro", "26H2", (10, 0, 26300, 9711))}
FEATURES = {f"W{i:02}" for i in range(1, 27)}
DEVICES = {"audible_audio", "direct_and_turn_calls", "camera", "screen_picker_and_peer", "pt_br_and_ime", "mixed_dpi_and_accessibility"}


def acceptance_template(args):
    candidate = read(args.candidate / "candidate.json") if args.candidate else None
    record = {"schema": 1, "source_commit": candidate["source_commit"] if candidate else "UNQUALIFIED",
              "archive_sha256": candidate["archive"]["sha256"] if candidate else "UNQUALIFIED",
              "installer_sha256": candidate["installer"]["sha256"] if candidate else "UNQUALIFIED",
              "backend_revision": "UNQUALIFIED", "servicing_reviewed_utc": None,
              "blockers": ["Complete signing, final native checks and attended acceptance before qualification"],
              "targets": {key: {"native": False, "standard_user": False, "architecture": "x86_64", "edition": edition,
                                "family": family, "build": ".".join(map(str, minimum)), "kb": "",
                                "features": {feature: {"status": "not_run", "evidence": []} for feature in sorted(FEATURES)}}
                          for key, (edition, family, minimum) in TARGETS.items()},
              "device_success_paths": {name: {"status": "not_run", "evidence": []} for name in sorted(DEVICES)},
              "beta": {"status": "not_run", "blockers": ["Attended beta pending"], "evidence": []}}
    require(not args.output.exists(), "Do not overwrite an acceptance record")
    write_json(args.output, record)


def artifact(path):
    return {"name": Path(path).name, "sha256": sha256(path)}


def authenticate_gate(gate_path, run_path, repository, commit):
    gate, expected = read(gate_path), read(run_path)
    require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository), "Invalid GitHub repository")
    run_id = str(expected["id"])
    require(run_id.isdecimal(), "Invalid tested run ID")
    actual = json.loads(subprocess.check_output(["gh", "api", f"repos/{repository}/actions/runs/{run_id}"], text=True))
    authorize_rebuild(gate, actual, commit, repository)
    require(actual["run_attempt"] == expected["run_attempt"], "Workflow attempt changed; fetch current evidence")
    with tempfile.TemporaryDirectory(prefix="papo-release-gate-") as directory:
        subprocess.run(["gh", "run", "download", run_id, "--repo", repository,
                        "--name", "Windows-release-gate", "--dir", directory], check=True)
        require(sha256(Path(directory) / "gate.json") == sha256(gate_path), "Local gate differs from the authenticated GitHub artifact")
    return gate, actual


def powershell(script, arguments):
    executable = Path(os.environ.get("SystemRoot", "C:/Windows")) / "System32/WindowsPowerShell/v1.0/powershell.exe"
    subprocess.run([str(executable), "-NoProfile", "-NonInteractive", "-File", str(ROOT / "packaging/windows" / script), *arguments], check=True)


def signature(file, receipt, role):
    before = file.read_bytes()
    powershell("sign.ps1", ["-Path", str(file), "-Receipt", str(receipt), "-Role", role])
    signing_only(before, file.read_bytes())
    result = read(receipt)
    require(result["status"] == "pass" and result["before_sha256"] == hashlib.sha256(before).hexdigest()
            and result["sha256"] == sha256(file) and result["timestamped"] is True and result["trusted"] is True,
            "Missing trusted/timestamped signing receipt")
    return result


def extract(archive, destination):
    # EN-US: Validate all names/types/limits before extracting; never trust ZIP paths.
    # PT-BR: Valida nomes/tipos/limites antes de extrair; nunca confia nos caminhos do ZIP.
    with zipfile.ZipFile(archive) as bundle:
        names = set()
        total = 0
        roots = set()
        for row in bundle.infolist():
            name = safe_name(row.filename.rstrip("/"))
            require(name.casefold() not in names and (row.external_attr >> 16) & 0o170000 != 0o120000
                    and not row.flag_bits & 1, "Duplicate/redirected/encrypted ZIP payload")
            names.add(name.casefold())
            roots.add(PurePosixPath(name).parts[0])
            total += row.file_size
        require(len(roots) == 1 and len(names) < 60000 and total <= 4 * 1024**3, "Invalid ZIP roots/limits")
        bundle.extractall(destination)
    return destination / roots.pop()


def nsis_sign_command(receipt):
    shell = Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"
    script = ROOT / "packaging/windows/sign.ps1"
    for path in (shell, script, receipt):
        require(not any(c in str(path) for c in '%!&|<>^\r\n\"\''), "Use signing paths without command-processor metacharacters")
    return f'"{shell}" -NoProfile -NonInteractive -File "{script}" -Path "%1" -Receipt "{receipt}" -Role uninstaller'


def make_candidate(args):
    require(sys.platform == "win32", "Signing a candidate requires native Windows and the owner's store key")
    require(os.environ.get("PAPO_SIGNING_THUMBPRINT") and os.environ.get("PAPO_SIGNTOOL")
            and os.environ.get("PAPO_TIMESTAMP_URL"), "No owner-provided signing configuration; unsigned previews cannot be qualified")
    before = source_state()
    require_source(before)
    tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT, text=True).split("\0")
    require(not any(Path(name).suffix.lower() in {".pfx", ".p12", ".key"} for name in tracked if name),
            "Signing material must stay outside tracked source archives")
    for name in tracked:
        if name and (ROOT / name).is_file() and (ROOT / name).stat().st_size <= 4 * 1024**2:
            content = (ROOT / name).read_bytes()
            require(not re.search(rb"-----BEGIN (?:RSA |EC |OPENSSH |ENCRYPTED )?PRIVATE KEY-----", content),
                    "A tracked private key must be removed before preparing release source")
    gate, run = authenticate_gate(args.base_gate, args.run_info, args.repository, before["commit"])
    require(artifact(args.archive) == gate["archive"], "Input ZIP differs from the full tested candidate")
    require(not args.output.exists(), "Use a new empty candidate output directory")
    args.output.mkdir(parents=True)
    try:
        receipts = args.output / "signatures"
        receipts.mkdir()
        stage = extract(args.archive, args.output / "staging")
        info, files = inventory(stage, args.archive)
        require(sha256(stage / "BUILD-INFO.json") == gate["build_info_sha256"], "Base build information mismatch")
        original_manifest = sha256(stage / "runtime/RUNTIME-MANIFEST.json")
        (stage / "README.txt").write_text(
            "Papo Windows x86_64\n\nExtract the complete ZIP and open Papo.exe, or use the per-user installer.\n"
            "No external MSYS2, Rust, Python or GStreamer installation is required.\n"
            "Support is defined by the matching published release's Windows-qualification.json and OS matrix.\n"
            "A candidate without that qualification remains experimental. macOS has no supported bundle.\n"
            "Uninstall preserves settings, caches and saved sessions. ZIP desktop registration is optional:\n"
            "  Papo.com --register-desktop\n  Papo.com --unregister-desktop\n"
            "Offline diagnostics: Papo.com --check-packaged-runtime and --check-media-runtime.\n"
            "BUILD-INFO.json, SOURCE.txt and runtime/RUNTIME-MANIFEST.json identify source/native inputs.\n"
            "Dependency notices/licenses are included. Byte-identical rebuilding is unproven.\n\n"
            "Extraia o ZIP completo e abra Papo.exe, ou use o instalador por usuario.\n"
            "Nao requer MSYS2, Rust, Python ou GStreamer externos. O suporte depende da qualificacao\n"
            "e da matriz de sistemas da mesma versao publicada. Sem qualificacao, o candidato e experimental.\n"
            "A desinstalacao preserva configuracoes, caches e sessoes. O registro do desktop no ZIP e opcional.\n",
            encoding="utf-8")
        signed = {name: signature(stage / name, receipts / f"application-{i}.json", "application") for i, name in enumerate(OWNED)}
        certificate = next(iter(signed.values()))["signer_certificate_sha256"]
        require(all(row["signer_certificate_sha256"] == certificate for row in signed.values()), "Mixed signing certificates")
        manifest_path = stage / "runtime/RUNTIME-MANIFEST.json"
        manifest = read(manifest_path)
        runtime_id = stage / "runtime/runtime.id"
        runtime_id.write_text(hashlib.sha256((runtime_id.read_text() + certificate + sha256(stage / OWNED[2])).encode()).hexdigest(), encoding="ascii")
        for row in manifest["files"]:
            file = stage / row["path"]
            row.update(size=file.stat().st_size, sha256=sha256(file))
        manifest["signing"] = {"signer_certificate_sha256": certificate, "unsigned_manifest_sha256": original_manifest}
        write_json(manifest_path, manifest)
        info["binary_sha256"] = sha256(stage / OWNED[2])
        info["runtime"]["manifest_sha256"] = sha256(manifest_path)
        info["signing"] = {"schema": 1, "status": "signed-candidate", "signer_certificate_sha256": certificate,
                           "base_gate_sha256": sha256(args.base_gate), "unsigned_archive": gate["archive"],
                           "unsigned_build_info_sha256": gate["build_info_sha256"], "applications": signed}
        write_json(stage / "BUILD-INFO.json", info)
        # EN-US: Copy the same readme/license payload; only signatures and identity manifests may change.
        # PT-BR: Preserva readme/licenças; apenas assinaturas e manifestos de identidade podem mudar.
        allowed = set(OWNED) | {"runtime/runtime.id", "runtime/RUNTIME-MANIFEST.json", "BUILD-INFO.json", "README.txt"}
        require(all(name in allowed or sha256(stage / name) == row["sha256"] for name, row in files.items()), "Unexpected payload mutation during signing")
        version = info["version"]
        archive = Path(shutil.make_archive(str(args.output / f"Papo-{version}-windows-x86_64"), "zip", stage.parent, stage.name))
        archive.with_suffix(".zip.sha256").write_text(f"{sha256(archive)}  {archive.name}\n", encoding="utf-8")
        output = args.output / f"Papo-{version}-windows-x86_64.exe"
        work = args.output / "installer-work"
        uninstaller_receipt = receipts / "uninstaller.json"
        prepare(stage, archive, work, output, nsis_sign_command(uninstaller_receipt), release_candidate=True)
        subprocess.run([args.makensis, "-INPUTCHARSET", "UTF8", "-WX", "-V3", "installer.nsi"], cwd=work, check=True)
        uninstall = read(uninstaller_receipt)
        require(uninstall["status"] == "pass" and uninstall["signer_certificate_sha256"] == certificate
                and uninstall["trusted"] is True and uninstall["timestamped"] is True, "Embedded uninstaller is not signed by the selected owner")
        installed = signature(output, receipts / "installer.json", "installer")
        require(installed["signer_certificate_sha256"] == certificate, "Installer signer differs from the application signer")
        output.with_suffix(".exe.sha256").write_text(f"{sha256(output)}  {output.name}\n", encoding="utf-8")
        metadata = {"schema": 1, "kind": "nsis-per-user-signed-candidate", "version": version,
                    "source_commit": before["commit"], "source_dirty": False, "archive": artifact(archive),
                    "installer": artifact(output), "build_info_sha256": sha256(stage / "BUILD-INFO.json"),
                    "runtime_manifest_sha256": sha256(manifest_path), "signed": True,
                    "signer_certificate_sha256": certificate, "uninstaller": uninstall, "release_qualified": False}
        write_json(output.with_suffix(".exe.json"), metadata)
        for name in ("BUILD-INFO.json",):
            shutil.copy2(stage / name, args.output / name)
        write_json(args.output / "INSTALLER-INFO.json", metadata)
        source = args.output / f"papo-gtk-{version}-source.tar.gz"
        subprocess.run(["git", "archive", "--format=tar.gz", f"--prefix=papo-gtk-{version}/", "-o", str(source), before["commit"]], cwd=ROOT, check=True)
        shutil.copy2(stage / "THIRD-PARTY-NOTICES.txt", args.output / "THIRD-PARTY-NOTICES.txt")
        shutil.copytree(stage / "licenses", args.output / "licenses")
        shutil.copy2(args.base_gate, args.output / "base-gate.json")
        write_json(args.output / "base-run.json", run)
        write_json(args.output / "candidate.json", {"schema": 1, "status": "awaiting-final-tests", "version": version,
                    "source_commit": before["commit"], "repository": args.repository, "signer_certificate_sha256": certificate,
                    "archive": artifact(archive), "installer": artifact(output), "source": artifact(source),
                    "base_gate_sha256": sha256(args.base_gate), "base_run_sha256": sha256(args.output / "base-run.json"),
                    "release_qualified": False})
        require_source(before, source_state())
    except Exception:
        (args.output / "candidate.json").unlink(missing_ok=True)
        raise


def evidence(row, root):
    require(row["status"] == "pass" and row["evidence"], "Missing/failed/skipped acceptance evidence")
    for entry in row["evidence"]:
        name = safe_name(entry["file"])
        path = (root / name).resolve()
        require(path.is_relative_to(root.resolve()) and not (root / name).is_symlink()
                and path.is_file() and sha256(path) == entry["sha256"], "Missing, redirected or altered acceptance attachment")


def desktop_acceptance(record, candidate, root):
    require(record["schema"] == 1 and record["source_commit"] == candidate["source_commit"]
            and record["archive_sha256"] == candidate["archive"]["sha256"]
            and record["installer_sha256"] == candidate["installer"]["sha256"], "Desktop evidence belongs to another candidate")
    require(record["blockers"] == [] and re.fullmatch(r"[0-9a-f]{40}", record["backend_revision"]), "Open blockers or unidentified backend")
    require(isinstance(record["servicing_reviewed_utc"], str), "Record the current Windows servicing review date")
    reviewed = dt.datetime.fromisoformat(record["servicing_reviewed_utc"].replace("Z", "+00:00"))
    require(reviewed.tzinfo is not None and dt.timedelta(0) <= dt.datetime.now(dt.timezone.utc) - reviewed <= dt.timedelta(days=30), "Review current Windows servicing before release")
    require(set(record["targets"]) == set(TARGETS), "Incomplete supported Windows matrix")
    for key, (edition, family, minimum) in TARGETS.items():
        target = record["targets"][key]
        require(re.fullmatch(r"\d+\.\d+\.\d+\.\d+", target["build"]), "Invalid native build/UBR record")
        build = tuple(int(part) for part in target["build"].split("."))
        require(target["native"] is True and target["standard_user"] is True and target["architecture"] == "x86_64"
                and target["edition"] == edition and target["family"] == family
                and len(build) == 4 and build[:3] == minimum[:3] and build[3] >= minimum[3]
                and re.fullmatch(r"KB\d{6,9}", target["kb"]), f"Unqualified desktop target: {key}")
        require(set(target["features"]) == FEATURES, f"Missing feature checks: {key}")
        for row in target["features"].values():
            evidence(row, root)
    require(set(record["device_success_paths"]) == DEVICES, "Missing native hardware/input success paths")
    for row in record["device_success_paths"].values():
        evidence(row, root)
    evidence(record["beta"], root)
    require(record["beta"]["blockers"] == [], "Beta blockers remain open")


def qualify(args):
    plan = (ROOT / "MULTIPLATFORM_PLAN.md").read_text(encoding="utf-8")
    require(all(re.search(rf"^{step}\. \[x\]", plan, flags=re.M) for step in range(1, 14)),
            "Previous implementation-plan acceptance steps remain open")
    candidate = read(args.candidate / "candidate.json")
    gate = read(args.candidate / "base-gate.json")
    run = read(args.candidate / "base-run.json")
    authorize_rebuild(gate, run, candidate["source_commit"], candidate["repository"])
    require(sha256(args.candidate / "base-gate.json") == candidate["base_gate_sha256"]
            and sha256(args.candidate / "base-run.json") == candidate["base_run_sha256"], "Base gate/run changed")
    require(re.fullmatch(r"[0-9a-f]{64}", args.signer_certificate_sha256)
            and args.signer_certificate_sha256 == candidate["signer_certificate_sha256"], "Owner certificate fingerprint must be explicitly pinned")
    for key in ("archive", "installer", "source"):
        require(artifact(args.candidate / safe_name(candidate[key]["name"])) == candidate[key], "Distributed artifact changed; retest the new bytes")
    # EN-US: Base CI covers the code; post-signing reports must cover the new manifests and exact signed bytes.
    # PT-BR: A CI base cobre o código; relatórios posteriores devem cobrir os manifestos e bytes assinados finais.
    for row in gate["source_checks"]:
        require(sha256(args.base_diagnostics / row["file"]) == row["sha256"], "Base source-test summary mismatch")
        shutil.copy2(args.base_diagnostics / row["file"], args.candidate / row["file"])
    final = aggregate(args.candidate / "BUILD-INFO.json", args.final_reports, gate["jobs"], candidate["source_commit"],
                      candidate["repository"], gate["run_id"], gate["run_attempt"])
    require(final["archive"] == candidate["archive"] and final["installer"] == candidate["installer"], "Final package rechecks used different files")
    signature_report = read(args.final_reports / "signed-recheck.json")
    require(signature_report["schema"] == 1 and signature_report["status"] == "pass"
            and signature_report["archive"] == candidate["archive"] and signature_report["installer"] == candidate["installer"], "Missing final signature verification")
    require(set(signature_report["signatures"]) == set(OWNED) | {"installer", "uninstaller"}, "Incomplete final signature inventory")
    info = read(args.candidate / "BUILD-INFO.json")
    installer_info = read(args.candidate / "INSTALLER-INFO.json")
    expected = {name: row["sha256"] for name, row in info["signing"]["applications"].items()}
    expected.update(installer=candidate["installer"]["sha256"], uninstaller=installer_info["uninstaller"]["sha256"])
    for name, row in signature_report["signatures"].items():
        require(row["status"] == "pass" and row["trusted"] is True and row["timestamped"] is True
                and row["verify_only"] is True and row["sha256"] == expected[name]
                and row["signer_certificate_sha256"] == args.signer_certificate_sha256, f"Invalid final signature: {name}")
    desktop_acceptance(read(args.acceptance), candidate, args.evidence)
    current = source_state()
    require_source(current)
    require(current["commit"] == candidate["source_commit"], "Qualification docs must belong to the tested clean revision")
    notes = ROOT / f"packaging/releases/{candidate['version']}.md"
    require(notes.is_file(), "Matching release notes are missing")
    body = notes.read_text(encoding="utf-8")
    marker = "<!-- WINDOWS_QUALIFICATION -->"
    require(body.count(marker) == 1, "Release notes need one qualification marker")
    acceptance = read(args.acceptance)
    support = "Windows x86_64 installer and ZIP qualified for the following native Windows 11 targets:\n\n"
    support += "| Edition | Family | Tested minimum build |\n| --- | --- | --- |\n"
    for key in TARGETS:
        target = acceptance["targets"][key]
        support += f"| {target['edition']} | {target['family']} | {target['build']} |\n"
    support += f"\nSigning certificate SHA-256: `{args.signer_certificate_sha256}`. Authenticode and timestamp validation passed for the final files.\n"
    body = re.sub(r"<!-- BEGIN_UNRELEASED -->.*?<!-- END_UNRELEASED -->\s*", "", body, flags=re.S)
    (args.candidate / "RELEASE-NOTES.md").write_text(body.replace(marker, support), encoding="utf-8")
    write_json(args.output, {"schema": 1, "status": "pass", "release_qualified": True,
               "source_commit": candidate["source_commit"], "version": candidate["version"], "repository": candidate["repository"],
               "signer_certificate_sha256": args.signer_certificate_sha256, "candidate_sha256": sha256(args.candidate / "candidate.json"),
               "acceptance_sha256": sha256(args.acceptance), "final_report_sha256": sha256(args.final_reports / "signed-recheck.json"),
               "artifacts": {key: candidate[key] for key in ("archive", "installer", "source")},
               "supported_targets": {key: {"edition": target["edition"], "family": target["family"],
                                           "minimum_build": target["build"], "architecture": "x86_64", "standard_user": True}
                                     for key, target in acceptance["targets"].items()},
               "features": sorted(FEATURES), "beta": "pass",
               "release_notes_sha256": sha256(args.candidate / "RELEASE-NOTES.md"), "qualified_utc": dt.datetime.now(dt.timezone.utc).isoformat()})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    template = commands.add_parser("acceptance-template")
    template.add_argument("--candidate", type=Path)
    template.add_argument("--output", type=Path, required=True)
    candidate = commands.add_parser("candidate")
    for name in ("archive", "base-gate", "run-info", "output"):
        candidate.add_argument(f"--{name}", type=Path, required=True)
    candidate.add_argument("--repository", default="ISpeakHue/papo-gtk")
    candidate.add_argument("--makensis", default="makensis")
    qualification = commands.add_parser("qualify")
    for name in ("candidate", "base-diagnostics", "final-reports", "acceptance", "evidence", "output"):
        qualification.add_argument(f"--{name}", type=Path, required=True)
    qualification.add_argument("--signer-certificate-sha256", required=True)
    args = parser.parse_args()
    for key, value in vars(args).items():
        if isinstance(value, Path):
            setattr(args, key, value.resolve())
    if args.command == "qualify" and (args.output.name != "qualification.json" or args.output == args.acceptance):
        parser.error("Use a separate output named qualification.json; never overwrite acceptance evidence")
    try:
        {"candidate": make_candidate, "qualify": qualify, "acceptance-template": acceptance_template}[args.command](args)
    # EN-US: Malformed evidence must replace any previous success with failure, including unexpected parser errors.
    # PT-BR: Evidências inválidas devem substituir qualquer sucesso anterior por falha, inclusive erros inesperados de leitura.
    except Exception as error:
        if args.command == "qualify" and args.output.name == "qualification.json":
            write_json(args.output, {"schema": 1, "status": "fail", "release_qualified": False, "error": str(error)})
        raise SystemExit(f"Windows release is not qualified: {error}") from error


if __name__ == "__main__":
    main()
