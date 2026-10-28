#!/usr/bin/env python3
"""EN-US: Stage qualified release assets; publishing requires an explicit flag and an existing exact tag.
PT-BR: Prepara artefatos qualificados; publicação exige opção explícita e tag existente e exata.
"""
import argparse
import datetime as dt
import json
import io
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile

from build import ROOT, require_source, sha256, source_state, write_json
from installer import safe_name
from release import OWNED, artifact, authenticate_gate, extract, powershell
from release_gate import read, require


def stage(candidate_dir, qualification, output):
    qualified, candidate = read(qualification), read(candidate_dir / "candidate.json")
    require(qualified["schema"] == 1 and qualified["status"] == "pass" and qualified["release_qualified"] is True,
            "A signed, fully qualified Windows release is required")
    require(re.fullmatch(r"\d+\.\d+\.\d+", candidate["version"])
            and candidate["version"] == tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"],
            "Release/Cargo version mismatch")
    require(qualified["candidate_sha256"] == sha256(candidate_dir / "candidate.json")
            and qualified["source_commit"] == candidate["source_commit"] and qualified["version"] == candidate["version"]
            and qualified["repository"] == candidate["repository"]
            and qualified["signer_certificate_sha256"] == candidate["signer_certificate_sha256"], "Qualification/candidate mismatch")
    date = dt.datetime.fromisoformat(qualified["qualified_utc"])
    require(date.tzinfo is not None and dt.timedelta(0) <= dt.datetime.now(dt.timezone.utc) - date <= dt.timedelta(days=7),
            "Qualification is stale; revalidate signatures/revocation and final evidence")
    require(qualified["release_notes_sha256"] == sha256(candidate_dir / "RELEASE-NOTES.md"), "Release notes changed")
    require(not output.exists(), "Use a new publication staging directory")
    for kind in ("archive", "installer", "source"):
        row = candidate[kind]
        require(row == qualified["artifacts"][kind] and artifact(candidate_dir / safe_name(row["name"])) == row,
                "Qualified artifact changed; do not rebuild between testing and publishing")
    current = source_state()
    require_source(current)
    require(current["commit"] == candidate["source_commit"], "Publish from the exact clean tested revision")
    output.mkdir(parents=True)
    for kind in ("archive", "installer", "source"):
        row = candidate[kind]
        shutil.copy2(candidate_dir / row["name"], output / row["name"])
    shutil.copy2(candidate_dir / "RELEASE-NOTES.md", output / "RELEASE-NOTES.md")
    shutil.copy2(qualification, output / "Windows-qualification.json")
    # EN-US: Publish notices/licenses, not raw private tester logs or signing credentials.
    # PT-BR: Publica avisos/licenças, não logs privados dos testes nem credenciais de assinatura.
    notice = output / f"Papo-{candidate['version']}-windows-notices.tar.gz"
    with zipfile.ZipFile(candidate_dir / candidate["archive"]["name"]) as package, tarfile.open(notice, "w:gz") as archive:
        count = 0
        for row in package.infolist():
            relative = "/".join(row.filename.split("/")[1:])
            if not row.is_dir() and (relative == "THIRD-PARTY-NOTICES.txt" or relative.startswith(("licenses/", "runtime/share/licenses/"))):
                safe_name(relative)
                data = package.read(row)
                entry = tarfile.TarInfo(relative)
                entry.size = len(data)
                entry.mode = 0o644
                archive.addfile(entry, io.BytesIO(data))
                count += 1
        require(count > 1, "Distributed dependency notices/licenses are missing")
    rows = {file.name: sha256(file) for file in output.iterdir() if file.is_file()}
    (output / "SHA256SUMS").write_text("".join(f"{digest}  {name}\n" for name, digest in sorted(rows.items())), encoding="utf-8")
    write_json(output / "publication.json", {"schema": 1, "status": "staged", "repository": candidate["repository"],
               "tag": f"v{candidate['version']}", "source_commit": candidate["source_commit"], "assets": rows})
    return candidate


def verify_before_publish(candidate_dir, candidate):
    require(sys.platform == "win32", "Publication requires fresh native Windows signature/revocation checks")
    with tempfile.TemporaryDirectory(prefix="papo-publish-signatures-") as directory:
        root = Path(directory)
        package = extract(candidate_dir / candidate["archive"]["name"], root / "package")
        files = {name: package / name for name in OWNED}
        files["installer"] = candidate_dir / candidate["installer"]["name"]
        for i, (name, file) in enumerate(files.items()):
            receipt = root / f"verify-{i}.json"
            powershell("sign.ps1", ["-Path", str(file), "-Receipt", str(receipt), "-Role",
                                    "installer" if name == "installer" else "application", "-VerifyOnly"])
            row = read(receipt)
            require(row["status"] == "pass" and row["sha256"] == sha256(file) and row["trusted"] is True
                    and row["timestamped"] is True and row["signer_certificate_sha256"] == candidate["signer_certificate_sha256"],
                    "Final publisher signature check failed")


def publish(output, candidate):
    repository, tag = candidate["repository"], f"v{candidate['version']}"
    ref = json.loads(subprocess.check_output(["gh", "api", f"repos/{repository}/git/ref/tags/{tag}"], text=True))["object"]
    for _ in range(4):
        if ref["type"] == "commit":
            break
        require(ref["type"] == "tag", "Release tag must resolve to a commit")
        ref = json.loads(subprocess.check_output(["gh", "api", f"repos/{repository}/git/tags/{ref['sha']}"], text=True))["object"]
    require(ref["type"] == "commit" and ref["sha"] == candidate["source_commit"], "Remote release tag differs from the tested source")
    existing = subprocess.run(["gh", "api", f"repos/{repository}/releases/tags/{tag}"], capture_output=True, text=True)
    require(existing.returncode != 0 and "HTTP 404" in existing.stderr, "Release exists or its availability could not be verified; nothing overwritten")
    manifest = read(output / "publication.json")
    require({file.name for file in output.iterdir()} == set(manifest["assets"]) | {"SHA256SUMS", "publication.json"}, "Publication directory contains unapproved files")
    for name, digest in manifest["assets"].items():
        require(sha256(output / name) == digest, "Staged release asset changed")
    checksums = "".join(f"{digest}  {name}\n" for name, digest in sorted(manifest["assets"].items()))
    require((output / "SHA256SUMS").read_text(encoding="utf-8") == checksums, "Staged checksums changed")
    assets = sorted(str(file) for file in output.iterdir() if file.is_file() and file.name != "publication.json")
    subprocess.run(["gh", "release", "create", tag, "--repo", repository, "--verify-tag", "--target", candidate["source_commit"],
                    "--draft", "--title", f"Papo {candidate['version']}", "--notes-file", str(output / "RELEASE-NOTES.md"), *assets], check=True)
    uploaded = json.loads(subprocess.check_output(["gh", "api", f"repos/{repository}/releases/tags/{tag}"], text=True))
    require(uploaded["draft"] is True and {row["name"] for row in uploaded["assets"]} == {Path(file).name for file in assets}, "Draft release assets are incomplete")
    for row in uploaded["assets"]:
        require(row.get("digest") == "sha256:" + sha256(output / row["name"]), "Uploaded asset digest mismatch; release remains a draft")
    subprocess.run(["gh", "release", "edit", tag, "--repo", repository, "--draft=false"], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--qualification", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--publish", action="store_true", help="Explicitly create a GitHub release after all checks pass")
    args = parser.parse_args()
    candidate = stage(args.candidate.resolve(), args.qualification.resolve(), args.output.resolve())
    if args.publish:
        authenticate_gate(args.candidate / "base-gate.json", args.candidate / "base-run.json", candidate["repository"], candidate["source_commit"])
        verify_before_publish(args.candidate, candidate)
        publish(args.output, candidate)
    else:
        print("Qualified assets staged locally. Nothing uploaded; --publish is required for GitHub publication.")


if __name__ == "__main__":
    main()
