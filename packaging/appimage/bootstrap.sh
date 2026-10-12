#!/usr/bin/env bash
set -euo pipefail
mkdir -p /build/sources
cd /build/sources

fetch_gnome() {
    local module="$1" version="$2" series="$3"
    local archive="${module}-${version}.tar.xz"
    curl --fail --location --retry 3 --output "$archive" "https://download.gnome.org/sources/$module/$series/$archive"
    curl --fail --location --retry 3 --output "$archive.sha256sum" "https://download.gnome.org/sources/$module/$series/${module}-${version}.sha256sum"
    awk -v file="$archive" '$2 == file { print }' "$archive.sha256sum" > "$archive.checksum"
    test -s "$archive.checksum"
    sha256sum --check "$archive.checksum"
    tar -xf "$archive"
}

curl --fail --location --retry 3 --output rust.tar.xz https://static.rust-lang.org/dist/rust-1.90.0-x86_64-unknown-linux-gnu.tar.xz
curl --fail --location --retry 3 --output rust.sha256 https://static.rust-lang.org/dist/rust-1.90.0-x86_64-unknown-linux-gnu.tar.xz.sha256
awk '{ print $1 "  rust.tar.xz" }' rust.sha256 | sha256sum --check
tar -xf rust.tar.xz
./rust-1.90.0-x86_64-unknown-linux-gnu/install.sh --prefix=/opt/rust --without=rust-docs
rm -rf rust-1.90.0-x86_64-unknown-linux-gnu rust.tar.xz

fetch_gnome gtk 4.16.13 4.16
meson setup gtk-build gtk-4.16.13 --prefix=/usr/local --libdir=lib --buildtype=release \
    -Dbuild-tests=false -Dbuild-examples=false -Dbuild-demos=false \
    -Ddocumentation=false -Dintrospection=disabled -Dvulkan=disabled \
    -Dmedia-gstreamer=enabled -Dprint-cups=disabled -Dtracker=disabled
meson compile -C gtk-build -j 4
meson install -C gtk-build

export PKG_CONFIG_PATH=/usr/local/lib/pkgconfig
export LD_LIBRARY_PATH=/usr/local/lib
fetch_gnome libadwaita 1.6.10 1.6
meson setup adwaita-build libadwaita-1.6.10 --prefix=/usr/local --libdir=lib --buildtype=release \
    -Dtests=false -Dexamples=false -Dgtk_doc=false -Dintrospection=disabled -Dvapi=false
meson compile -C adwaita-build -j 4
meson install -C adwaita-build
ldconfig
rm -rf /build/sources
