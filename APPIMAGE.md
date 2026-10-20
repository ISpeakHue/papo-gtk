# Modern GNOME/KDE AppImage

This x86_64 recipe builds the current frontend in an isolated **Fedora 44**
container. It requires **glibc 2.43 or newer**, as found in Fedora 44 and current
compatible Arch installations. It deliberately does not target Ubuntu 24.04 or
older Linux releases. Desktop environment choice does not change the package.

The runtime bundles Fedora's GTK 4.22, libadwaita 1.9, current Pango,
Python 3.14/PyGObject, GStreamer 1.28, software H.264/AAC codecs, optional VA-API/NVIDIA video decoding, WebRTC/libnice,
PipeWire clients, icons, certificates and fallback fonts. Exact versions, source
RPM names and the frontend binary hash are recorded inside the package.

The desktop supplies glibc, its Fontconfig/FreeType/HarfBuzz libraries, graphics
drivers, X11/Wayland, audio services, D-Bus,
and its desktop portal. Screen sharing requires a ScreenCast portal backend and
PipeWire (usually `xdg-desktop-portal-kde` on KDE). Remembered login uses a Secret
Service keyring. These desktop services are not bundled daemons.

## Hardware video decoding

The AppImage includes GStreamer's `va` and `nvcodec` plugins and their runtime
libraries. VA-API can use compatible AMD/Intel hardware through the desktop's
video driver; NVIDIA decoding requires the host's compatible NVIDIA driver and
its CUDA/NVDEC libraries. The package does not distribute vendor GPU drivers or
Mesa's DRI/VA driver modules. Existing `LIBVA_*`, CUDA and decoder-rank overrides
are preserved, and host GStreamer plugins are not mixed with the bundle.

GStreamer selects compatible decoders automatically. Software codecs, including
`avdec_h264`, remain available when hardware decoding is unavailable. This adds
backends for chat attachments and embeds; calls and screen sharing still use
the application's explicit software VP8 pipeline.

`--runtime-check` verifies both plugin files and their dependencies, reports the
hardware video decoders registered on that machine, and decodes a synthetic
H.264 clip explicitly through the software fallback. An empty registered-decoder
list is valid on a machine without the corresponding hardware or driver. A
registered decoder is not proof that a particular video used it: codec/profile
support and GStreamer's selection still apply. Hardware plugin discovery may
probe GPU capabilities; the check does not capture microphone, camera or screen
contents. A separate registry cache is used for hardware-enabled builds.

References: [GStreamer VA-API](https://gstreamer.freedesktop.org/documentation/va/index.html),
[GStreamer NVIDIA codecs](https://gstreamer.freedesktop.org/documentation/nvcodec/index.html).

## Font rendering

The launcher preserves the complete desktop Fontconfig configuration, including
user font rules, and adds bundled Adwaita/Noto/DejaVu fonts as fallbacks. Explicit
`FONTCONFIG_FILE`/`FONTCONFIG_PATH` overrides remain intact. It does not force a
font family, hinting mode, GDK scale, XWayland backend or software renderer.
GTK uses the desktop's native backend and monitor scaling.
Core font libraries come from the desktop to keep newer font configuration rules
compatible on rolling distributions such as Arch. The MIME database is bundled
so image identification does not depend on other GTK apps being installed.
Current GTK's font rendering behavior is described in its
[settings documentation](https://docs.gtk.org/gtk4/property.Settings.gtk-font-rendering.html).

Modern PNG/SVG decoding includes Glycin's loaders and a Bubblewrap wrapper that
adds the read-only AppDir and its libraries to the existing decoder sandbox.
Loader configuration is relocated in the user's cache for each mount path.
Namespaces, seccomp and network isolation are retained; the launcher does not
disable sandboxing. The self-check exercises real PNG and SVG decoding.

IBus and Fcitx5 GTK input-method clients are included, along with their library
dependencies. The launcher preserves the desktop's input-method selection;
input-method daemons remain supplied by the desktop. GTK's native compose
handling supports dead-key accents in the editor.

## Build

Install Podman, curl and Python 3.11+ on the build machine. Cache the locked Rust
dependencies with `cargo fetch --locked`, then run:

```sh
bash scripts/build-appimage.sh
```

The first run downloads Fedora packages and RPM Fusion's free codec library.
Later builds reuse the container and `target/appimage-fedora44` Cargo cache.
The AppImage builder 1.9.1 and type-2 runtime are checksum verified. No GTK or
libadwaita downgrade or source rebuild is performed.

Versioned release outputs in ignored `dist/` are:

- `Papo-1.0.0-x86_64.AppImage`
- Its `.sha256` checksum
- `papo-gtk-1.0.0-source.tar.gz`
- `Papo-1.0.0-AppImage-README.txt`, installation and runtime notes

Version 1.0.0 is available from [GitHub Releases](https://github.com/ISpeakHue/papo-gtk/releases/tag/v1.0.0).
The package retains the modern desktop/font policy described above.

The AppDir is assembled under `target/appimage-fedora44/AppDir`, containing only
build outputs and selected builder resources. Personal settings, caches, tokens
and account credentials are not copied. Development launches keep using the
system Python interpreter; the AppImage sets its own `PAPO_PYTHON`.

## Test and distribute

Make the new AppImage executable and run:

```sh
./Papo-1.0.0-x86_64.AppImage --runtime-check
./Papo-1.0.0-x86_64.AppImage --font-check
./Papo-1.0.0-x86_64.AppImage
```

The runtime check uses synthetic audio/video and validates imports, codecs,
certificates, fallback fonts, desktop Fontconfig rules and PNG/SVG loaders. It
opens no microphone, camera, screen picker or server connection; hardware
plugins can probe GPU capabilities. `--font-check` briefly shows a
sample window and reports the backend, font match, renderer and scale; add
`--snapshot /tmp/papo-fonts.png` to save its rendering.

When FUSE is unavailable, put `--appimage-extract-and-run` before app arguments.
Share the versioned AppImage, checksum, tester instructions and matching source
archive. Testers enter their own backend URL and credentials. A backend is not
included. Real calls/devices and the friends' GNOME/KDE setups need their own
checks; desktop configuration and graphics drivers can still affect rendering.

## Validation

Version 1.0.0 (2026-10-20): 280 regular tests and six packaging checks passed.
The package passed bundled runtime checks and actual isolated MP4/MOV/WebM
poster/animation conversion. Native Wayland startup and font rendering passed
on a private display with temporary settings. The upstream type-2 runtime was
refreshed and checksum verified for this release.


Validation for the 2026-10-12 build: 186 release tests passed in the Fedora builder.
Mapped mention, avatar, clipboard and chat regressions passed with the bundle.
The final AppImage passed runtime checks on Fedora (FUSE and extraction) and on
current Arch with glibc 2.44 and its native font libraries. PNG/SVG, 37 media
elements, synthetic Opus/VP8 decoding, certificates and Fontconfig were checked.
Native Wayland font diagnostics passed at 100%, 125% and 200%; the mounted final
image selected Adwaita Sans and Vulkan at true 125% on an isolated GNOME compositor.
The login window/pelican icon were checked on a private X11 display. Actual KDE
desktop sessions and physical calls/camera/screen sharing remain tester checks.

The 20261013-input update adds the multiline, wrapping composer and input-method
clients. All 196 regular tests passed. Focused mention/clipboard regressions and
real keyboard checks passed, including dead-key accents, Shift+Enter, Unicode
commit without sending, autocomplete, bounded input height and failed-send retry.
The keyboard checks also passed with bundled GTK using `simple`, `ibus` and
`fcitx`, without running desktop input-method daemons. The mounted final AppImage
passed its runtime check, including loading both clients and all 37 media elements.

The 20261013-hardware update adds the VA-API and NVIDIA decoder plugins. Four
packaging boundary regressions passed. The finished AppImage passed runtime
checks on the Fedora desktop and in isolated Fedora/Arch environments without
GPU devices, including the explicit software H.264 decode. Using the bundled
runtime, automatic decoding on the AMD GPU selected `vah264dec` and produced all
48 frames of the fixture; the same pipeline without GPU access selected
`avdec_h264` and also produced 48 frames. NVIDIA plugin dependency loading was
checked without an NVIDIA driver; decoding on physical NVIDIA hardware remains
unverified. GPU driver files were checked to be absent from the AppDir.

## Runtime sources and licenses

The frontend is GPL-3.0-only. Runtime license notices are in `usr/share/licenses`;
exact package versions/source RPM names are in
`usr/share/papo-gtk/fedora-packages.tsv`; build information and the binary hash are
in `usr/share/papo-gtk/build-info.json`.

- [Fedora sources](https://src.fedoraproject.org/)
- [RPM Fusion free sources](https://download1.rpmfusion.org/free/fedora/)
- [Glycin source](https://github.com/GNOME/glycin)
- [AppImage builder/runtime sources](https://github.com/AppImage/appimagetool)
