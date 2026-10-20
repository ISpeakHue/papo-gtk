#!/usr/bin/env bash
set -euo pipefail
PAPO_PROJECT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$PAPO_PROJECT"
[[ $(uname -m) == x86_64 ]] || { echo 'This release recipe builds x86_64 only.' >&2; exit 1; }
command -v flatpak-builder >/dev/null
mkdir -p target/flatpak/source/.cargo dist
# Copy only repository source, never the checkout's settings, build outputs or credentials.
python3 - <<'PY'
from pathlib import Path
import shutil, subprocess
root=Path('target/flatpak/source')
if root.exists():
    shutil.rmtree(root)
(root/'.cargo').mkdir(parents=True)
for name in subprocess.check_output(['git','ls-files','-co','--exclude-standard','-z']).decode().split('\0'):
    source=Path(name)
    if name and source.is_file():
        target=root/source
        target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(source,target)
PY
cargo vendor --locked --offline --versioned-dirs target/flatpak/source/vendor | sed 's|directory = "target/flatpak/source/vendor"|directory = "vendor"|' > target/flatpak/source/.cargo/config.toml
# Both SDK/runtime and Rust extension must be installed (see FLATPAK.md).
flatpak-builder --force-clean --disable-rofiles-fuse --state-dir=target/flatpak/state --repo=target/flatpak/repo \
    target/flatpak/build packaging/flatpak/br.com.papo.gtk.json
PAPO_VERSION=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
PAPO_BUNDLE="dist/Papo-${PAPO_VERSION}-x86_64.flatpak"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    target/flatpak/repo "$PAPO_BUNDLE" br.com.papo.gtk stable
(cd dist; sha256sum "$(basename "$PAPO_BUNDLE")") > "$PAPO_BUNDLE.sha256"
python3 packaging/appimage/source-archive.py "$PAPO_VERSION"
ls -lh "$PAPO_BUNDLE"
