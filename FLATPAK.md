# Flatpak

Papo's x86_64 Flatpak uses the GNOME 50 runtime (GTK 4.22 and libadwaita 1.9).
It works on GNOME and KDE desktops through Flatpak's runtime and desktop portals.
Application control icons are embedded in the executable with Papo-specific
names; they remain available when the desktop selects a theme absent from the
Flatpak runtime. GTK applies the active foreground colors to the symbolic SVGs.

## Install version 1.0.2

Download `Papo-1.0.2-x86_64.flatpak` from
[GitHub Releases](https://github.com/ISpeakHue/papo-gtk/releases/tag/v1.0.2), then:

```sh
flatpak install --user ./Papo-1.0.2-x86_64.flatpak
flatpak run br.com.papo.gtk
```

The bundle points to Flathub for its GNOME runtime. It is a GitHub download,
not a Flathub application listing. A bundle does not establish an app update
repository: download and install a newer bundle when upgrading.

Settings and caches are isolated under `~/.var/app/br.com.papo.gtk/`. Login
credentials use the desktop Secret Service. The app has network, display,
audio and device access for chat, playback, calls and cameras. It uses desktop
portals for file selection and screen sharing, without blanket home-directory
access or host-command execution. KDE needs its ScreenCast portal backend.

Video posters and video-to-GIF decoding run in a stricter child sandbox through
`flatpak-spawn --sandbox --no-network`. Only the selected media input is exposed
read-only through a temporary staging file, removed when the decoder completes
or is cancelled; account files and desktop sockets are concealed. The host's nested
Bubblewrap path stays in use for development and AppImage builds.

## Build

Install Flatpak, flatpak-builder, Cargo and Python 3.11+. Install the SDK/runtime
and Rust extension (system-installed SDK/runtime may also be used):

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.gnome.Platform//50 org.gnome.Sdk//50 org.freedesktop.Sdk.Extension.rust-stable//25.08
cargo fetch --locked
bash scripts/build-flatpak.sh
```

The script stages repository source and vendors exactly the locked Rust
crates. Compilation runs offline in the SDK; no host executable or AppImage
libraries are copied into `/app`. Build state remains in ignored `target/flatpak`.
Outputs in ignored `dist/` include the Flatpak, its checksum and matching source.

The runtime supplies Python/PyGObject/GStreamer, WebRTC/libnice, PipeWire and
codecs. Flatpak manages compatible GPU drivers and codec extensions. No vendor
GPU driver is included in the application.

## Check the installed package

```sh
flatpak run br.com.papo.gtk --version
flatpak run --command=papo-runtime-check br.com.papo.gtk
```

The offline runtime check verifies Python/GStreamer imports, 38 required media
elements, certificates, and the app's actual isolated MP4/MOV/WebM thumbnail
and animation conversion. It checks that decoder input is read-only, an adjacent
private marker is hidden and the parent network is unreachable. It opens no
microphone, camera, screen picker or account session. Real calls and physical
devices still need desktop acceptance testing.

[Flatpak bundles](https://docs.flatpak.org/en/latest/single-file-bundles.html)
and [sandbox permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html)
describe the distribution format and permission model.

## Version 1.0.2 validation (2026-10-20)

The installed versioned bundle reports 1.0.2 and passed its runtime check with
38 media elements and isolated MP4/MOV/WebM conversion. All 47 embedded control
icons resolved and decoded with Adwaita, Breeze and an unavailable desktop theme
selected, at 1x/2x scale. Private Wayland startup passed with separate test
settings and fatal GTK warnings. See [desktop testing](docs/testing/desktop.md#bundled-control-icons)
for source checks and the pre-existing broader GUI-test limitations.

## Version 1.0.1 validation (2026-10-20)

The installed release bundle reports 1.0.1. Its runtime check passed with all
38 required media elements and isolated MP4/MOV/WebM conversion. Startup passed
on a private Wayland display with separate test settings and fatal GTK warnings.
The source passed 282 regular tests, six Python checks and three focused GTK
cases; all-target compilation was warning-free.

## Version 1.0.0 validation (2026-10-20)

The installed release bundle reports version 1.0.0 and passed the runtime check
with GTK 4.22, libadwaita 1.9, GStreamer 1.26.11 and all 38 required elements.
Isolated MP4/MOV/WebM poster and animation conversion passed, as did a separate
check hiding synthetic private configuration and blocking writes, desktop-bus
inheritance and parent-network access. Startup passed on a private Wayland
display with fresh settings and fatal GTK warnings. The source passed 280
regular tests and six packaging checks; compilation was warning-free.
