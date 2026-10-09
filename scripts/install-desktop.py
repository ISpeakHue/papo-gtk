#!/usr/bin/env python3
"""Register a built Papo executable and its icons in the desktop app list."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess


def desktop_exec(binary: Path) -> str:
    # Exec has two escaping layers: a desktop string, then a quoted argument.
    argument = str(binary).replace("%", "%%")
    for character in ("\\", '"', "`", "$"):
        argument = argument.replace(character, "\\" + character)
    return '"' + argument.replace("\\", "\\\\") + '"'


def main() -> None:
    project = Path(__file__).resolve().parent.parent
    xdg_data = Path(os.environ.get("XDG_DATA_HOME", ""))
    default_data = xdg_data if xdg_data.is_absolute() else Path.home() / ".local/share"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=project / "target/debug/papo-gtk")
    parser.add_argument("--data-dir", type=Path, default=default_data)
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error(f"Build the application first; executable not found: {binary}")
    if any(character in str(binary) for character in "\n\r\t"):
        parser.error("The executable path must not contain control characters")

    data = args.data_dir.resolve()
    applications = data / "applications"
    applications.mkdir(parents=True, exist_ok=True)
    icon_theme = data / "icons/hicolor"
    for source in (project / "assets/icons/hicolor").glob("*/apps/br.com.papo.gtk.png"):
        target = icon_theme / source.relative_to(project / "assets/icons/hicolor")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)

    entry = (project / "data/br.com.papo.gtk.desktop").read_text(encoding="utf-8")
    entry = entry.replace("Exec=papo-gtk\n", f"Exec={desktop_exec(binary)}\n")
    launcher = applications / "br.com.papo.gtk.desktop"
    launcher.write_text(entry, encoding="utf-8")
    for command in (
        ["gtk4-update-icon-cache", "--force", "--ignore-theme-index", str(icon_theme)],
        ["update-desktop-database", str(applications)],
    ):
        if shutil.which(command[0]):
            subprocess.run(command, check=True)
    print(f"Installed Papo launcher: {launcher}")
    print(f"Launcher executable: {binary}")


if __name__ == "__main__":
    main()
