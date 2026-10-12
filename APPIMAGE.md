# Distributable Linux test build

The x86_64 recipe builds on Ubuntu 24.04 (glibc 2.39) rather than using the host's
Fedora libraries. It bundles GTK 4.16.13, libadwaita 1.6.10, Python 3.12/PyGObject,
GStreamer 1.24, software media codecs, WebRTC/libnice, PipeWire client libraries,
fonts, standard icons, HTTPS certificates and the Papo pelican icon.

The host still supplies its kernel, graphics drivers, X11/Wayland, audio service,
D-Bus, and desktop portal. Screen sharing needs a ScreenCast portal backend and
PipeWire; on KDE this is normally `xdg-desktop-portal-kde`. Remembered login uses
a Secret Service keyring. These are desktop services, not bundled daemons.

## Build

Install Podman, curl and Python 3.11+ on the build machine. Cache the locked Rust
dependencies first with `cargo fetch --locked`; container compilation uses that
registry offline. Then run:

```sh
bash scripts/build-appimage.sh
```

The first run downloads the Ubuntu container, native build/runtime packages,
the checksum-verified Rust 1.90.0 toolchain, GNOME source archives and the pinned,
checksum-verified AppImage builder 1.9.1 and SHA-256-pinned type-2 runtime. Later runs reuse the container and Cargo
build cache. The GTK build also resolves its versioned Meson subprojects.

Outputs are kept in the ignored `dist/` directory:

- `Papo-0.1.0-preview-x86_64.AppImage`
- Its SHA-256 checksum
- `papo-gtk-0.1.0-source.tar.gz`, matching frontend source/build recipes
- `README.txt`, instructions for testers

The AppDir is assembled under `target/appimage/AppDir`. It contains only build
outputs and explicitly selected runtime resources. User settings, keyring data,
chat caches and account credentials are not copied. Development launches retain
the system Python interpreter; the AppImage supplies its bundled interpreter
through `PAPO_PYTHON`.

## Test and distribute

```sh
chmod +x dist/Papo-0.1.0-preview-x86_64.AppImage
dist/Papo-0.1.0-preview-x86_64.AppImage --runtime-check
dist/Papo-0.1.0-preview-x86_64.AppImage
```

The self-check verifies bundled imports/libraries, codec and capture element
availability, synthetic Opus/VP8 encoding/decoding, and HTTPS certificate loading.
It does not open a microphone, camera, screen picker or server connection.

When FUSE mounting is unavailable, use
`--appimage-extract-and-run` before application arguments. AppImage desktop
integration should point its launcher at the distributed AppImage file; the
development `install-desktop.py` helper instead points at a checkout's binary.

Share the AppImage, tester instructions and matching source archive. Testers
enter their own backend URL and credentials. A Papo backend is not bundled.

Local validation on 2026-10-09: 168 regular release tests passed on the Ubuntu
baseline; the complete GTK workflow passed in 145.13 seconds with the bundled
runtime, including native playback and chat/media regressions. Synthetic native
voice cleanup and all four private portal tests passed. The actual AppImage
self-check passed using both FUSE and extraction mode on Fedora 44, and its login
window/pelican icon were verified on a private display. Physical devices, real
multi-user calls and other desktops still need the testers' checks.

## Runtime sources and licenses

The frontend remains GPL-3.0-only. Runtime copyright/license notices are included
under `usr/share/licenses`, and the builder's exact Ubuntu package/source
versions are listed in `usr/share/papo-gtk/ubuntu-packages.tsv`.

- [GTK 4.16.13 source](https://download.gnome.org/sources/gtk/4.16/gtk-4.16.13.tar.xz)
- [libadwaita 1.6.10 source](https://download.gnome.org/sources/libadwaita/1.6/libadwaita-1.6.10.tar.xz)
- [Ubuntu package/source archive](https://archive.ubuntu.com/ubuntu/)
- [AppImage builder/runtime sources](https://github.com/AppImage/appimagetool)
- [AppImage packaging requirements](https://docs.appimage.org/reference/best-practices.html)
