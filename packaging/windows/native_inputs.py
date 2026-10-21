#!/usr/bin/env python3
"""EN-US: Retain exact installed MSYS2 packages instead of relying on rolling mirrors.
PT-BR: Preserva os pacotes MSYS2 instalados em vez de depender de espelhos contínuos.
"""
import argparse
import json
import re
from pathlib import Path
import shutil
import sys

from build import ROOT, run, sha256, write_json


def msys_path(path):
    return run("cygpath", "-u", str(Path(path).resolve()))


def package_set(output):
    pairs = [line.split() for line in output.splitlines() if line.strip()]
    if any(len(pair) != 2 for pair in pairs) or len({p[0] for p in pairs}) != len(pairs):
        raise RuntimeError("Malformed or duplicate package inventory")
    return dict(pairs)


def verify_archive(directory, installed=None):
    directory = Path(directory)
    manifest = json.loads((directory / "NATIVE-INPUTS.json").read_text(encoding="utf-8"))
    if manifest["schema"] != 1 or not manifest["files"]:
        raise RuntimeError("Unsupported or empty native input archive")
    expected_files = set()
    for entry in manifest["files"]:
        name = entry["file"]
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+@~-]*", name) or name in expected_files:
            raise RuntimeError("Unsafe or duplicate archive filename")
        expected_files.add(name)
        path = directory / name
        if path.is_symlink() or path.stat().st_size != entry["size"] or sha256(path) != entry["sha256"]:
            raise RuntimeError(f"Native input checksum mismatch: {name}")
    actual_files = {path.name for path in directory.iterdir() if path.name != "NATIVE-INPUTS.json"}
    if expected_files != actual_files:
        raise RuntimeError("Native input archive has missing or unrecorded files")
    if installed is not None and package_set(installed) != manifest["installed"]:
        raise RuntimeError("Installed MSYS2 versions differ from the retained inputs; archive again before building")
    return {"manifest_sha256": sha256(directory / "NATIVE-INPUTS.json"), **manifest}


def capture(directory):
    if sys.platform != "win32":
        raise RuntimeError("Archive native inputs from MSYS2 on Windows")
    directory = directory.resolve()
    if directory.exists():
        raise RuntimeError("Choose a new/empty archive path; never mix package snapshots")
    directory.mkdir(parents=True)
    installed = package_set(run("pacman", "-Q"))
    # EN-US: Download and validate in one transaction; reject repository drift after provisioning.
    # PT-BR: Baixa e valida em uma transação; rejeita divergências do repositório após preparar o ambiente.
    import subprocess
    subprocess.run(["pacman", "-Sw", "--noconfirm", "--cachedir", msys_path(directory), *sorted(installed)],
                   cwd=ROOT, check=True)
    packages = sorted(path for path in directory.iterdir()
                      if ".pkg.tar." in path.name and not path.name.endswith(".sig"))
    retained = package_set(run("pacman", "-Qp", *(msys_path(path) for path in packages)))
    if retained != installed:
        raise RuntimeError("Rolling repository versions changed; retained packages do not match installed versions. "
                           "Re-provision the full environment, then start a new archive.")
    for name in ("msys.db", "ucrt64.db"):
        source = Path(run("cygpath", "-w", f"/var/lib/pacman/sync/{name}"))
        shutil.copy2(source, directory / name)
        if source.with_suffix(source.suffix + ".sig").exists():
            shutil.copy2(source.with_suffix(source.suffix + ".sig"), directory / (name + ".sig"))
    manifest = {
        "schema": 1, "installed": installed,
        "files": [{"file": path.name, "size": path.stat().st_size, "sha256": sha256(path)}
                  for path in sorted(directory.iterdir())],
        "validation": "pacman download integrity/signature checks; exact installed/archived name-version equality",
        "byte_identical_rebuild_demonstrated": False,
    }
    write_json(directory / "NATIVE-INPUTS.json", manifest)
    verify_archive(directory, installed=run("pacman", "-Q"))
    print(f"Retained {len(packages)} exact MSYS2 package archives in {directory}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("capture", "verify"))
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    if args.command == "capture":
        capture(args.directory)
    else:
        manifest = verify_archive(args.directory)
        print(f"Verified {len(manifest['files'])} retained native inputs")


if __name__ == "__main__":
    main()
