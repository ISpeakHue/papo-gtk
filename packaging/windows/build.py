#!/usr/bin/env python3
"""EN-US: Stage a developer preview using one consistent MSYS2 UCRT64 runtime.
PT-BR: Prepara uma prévia para desenvolvimento usando um único runtime MSYS2 UCRT64.
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def main():
    if sys.platform != "win32" or os.environ.get("MSYSTEM") != "UCRT64":
        raise SystemExit("Run this script with UCRT64 Python from an MSYS2 UCRT64 terminal.")
    # EN-US: Reject MSVCRT/MSVC compilers mixed with UCRT GTK/Python dependencies.
    # PT-BR: Rejeita compiladores MSVCRT/MSVC misturados com dependências GTK/Python UCRT.
    prefix = Path(sys.executable).resolve().parent
    for name in ("cargo.exe", "rustc.exe", "pkg-config.exe", "glib-compile-resources.exe"):
        executable = shutil.which(name)
        if not executable or Path(executable).resolve().parent != prefix:
            raise SystemExit(f"{name} must come from the same UCRT64 bin directory as Python.")
    subprocess.run(["cargo", "build", "--release", "--locked"], cwd=ROOT, check=True)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    stage = ROOT / "dist" / f"Papo-{version}-windows-x86_64-dev"
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    shutil.copy2(ROOT / "target/release/papo-gtk.exe", stage)
    for name in ("Papo.cmd", "README.txt"):
        shutil.copy2(ROOT / "packaging/windows" / name, stage)
    shutil.copy2(ROOT / "LICENSE", stage)
    subprocess.run([str(stage / "papo-gtk.exe"), "--platform-info"], check=True)
    shutil.make_archive(str(stage), "zip", stage.parent, stage.name)
    print(f"Developer preview: {stage}.zip (requires an installed MSYS2 UCRT64 runtime)")


if __name__ == "__main__":
    main()
