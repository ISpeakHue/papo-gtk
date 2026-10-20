"""Write the matching frontend source alongside the distributable binary."""

from pathlib import Path
import subprocess
import sys
import tarfile

version = sys.argv[1]
files = subprocess.check_output(["git", "ls-files", "-co", "--exclude-standard", "-z"]).decode().split("\0")
with tarfile.open(f"dist/papo-gtk-{version}-source.tar.gz", "w:gz") as archive:
    for filename in sorted(set(files)):
        if filename and Path(filename).is_file():
            archive.add(filename, arcname=f"papo-gtk-{version}/{filename}", recursive=False)
