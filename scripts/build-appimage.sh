#!/usr/bin/env bash
set -euo pipefail
PAPO_PROJECT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$PAPO_PROJECT"
if [[ $(uname -m) != x86_64 ]]; then
    echo "This recipe currently builds x86_64 AppImages only." >&2
    exit 1
fi
command -v podman >/dev/null
command -v curl >/dev/null
mkdir -p target/appimage-tools dist
PAPO_TOOL="$PAPO_PROJECT/target/appimage-tools/appimagetool-1.9.1-x86_64.AppImage"
if [[ ! -f "$PAPO_TOOL" ]]; then
    curl --fail --location --retry 3 --output "$PAPO_TOOL" \
        https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
fi
printf '%s  %s\n' ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0 "$PAPO_TOOL" | sha256sum --check
chmod +x "$PAPO_TOOL"
PAPO_RUNTIME="$PAPO_PROJECT/target/appimage-tools/runtime-x86_64"
if [[ ! -f "$PAPO_RUNTIME" ]]; then
    curl --fail --location --retry 3 --output "$PAPO_RUNTIME" \
        https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64
fi
printf '%s  %s\n' 617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46 "$PAPO_RUNTIME" | sha256sum --check
if [[ ! -d target/appimage-tools/stable-tool ]]; then
    (cd target/appimage-tools; "$PAPO_TOOL" --appimage-extract >/dev/null; mv squashfs-root stable-tool)
fi

podman build --security-opt label=disable -t papo-appimage-builder:fedora44 packaging/appimage
PAPO_MOUNTS=(--security-opt label=disable --volume "$PAPO_PROJECT:/work" \
    --volume "${CARGO_HOME:-$HOME/.cargo}/registry:/cargo/registry:ro")
podman run --rm "${PAPO_MOUNTS[@]}" --env CARGO_BUILD_JOBS=2 \
    papo-appimage-builder:fedora44 cargo build --release --locked --offline
podman run --rm "${PAPO_MOUNTS[@]}" papo-appimage-builder:fedora44 \
    python3 packaging/appimage/bundle.py
podman run --rm "${PAPO_MOUNTS[@]}" --env XDG_CACHE_HOME=/tmp/papo-bundle-cache \
    papo-appimage-builder:fedora44 target/appimage-fedora44/AppDir/AppRun --runtime-check

PAPO_VERSION=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
PAPO_IMAGE="$PAPO_PROJECT/dist/Papo-${PAPO_VERSION}-x86_64.AppImage"
LC_ALL=C.UTF-8 LANG=C.UTF-8 ARCH=x86_64 VERSION="$PAPO_VERSION" \
    target/appimage-tools/stable-tool/AppRun --no-appstream --runtime-file target/appimage-tools/runtime-x86_64 \
    target/appimage-fedora44/AppDir "$PAPO_IMAGE"
(cd dist; sha256sum "$(basename "$PAPO_IMAGE")") > "$PAPO_IMAGE.sha256"
python3 packaging/appimage/source-archive.py "$PAPO_VERSION"
sed -e "s/@VERSION@/$PAPO_VERSION/g" \
    packaging/appimage/TESTER_README.txt > "dist/Papo-${PAPO_VERSION}-AppImage-README.txt"
ls -lh "$PAPO_IMAGE"
