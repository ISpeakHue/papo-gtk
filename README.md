# Papo GTK

A native desktop frontend for [papo-backend](https://github.com/Papo-Chat/papo-backend),
built with Rust, GTK 4, libadwaita and Relm4.

The application supports server and account authentication, channels and direct
messages, attachments and rich link/custom embeds, reactions and mentions, member profiles,
notifications, moderation, webhook management, voice calls, cameras and screen sharing. The interface
uses an adaptive server/channel sidebar and native GNOME controls.

Webhook integration and permissions are described in [WEBHOOKS.md](WEBHOOKS.md).

## Diagnostics

Terminal logging defaults to warnings and errors, written to stderr. Enable
application debug diagnostics with `RUST_LOG=papo_gtk=debug cargo run`.
HTTP diagnostics contain only method, status, elapsed time and attempt number;
URLs, headers, cookies, passwords and request/response bodies are excluded.
Raw dependency dumps remain disabled even with `RUST_LOG=trace` or an explicit
transport logging directive. Authentication models and cookie header values
also redact secrets in debug output.

## Build and run

Requires Rust, a C compiler, pkg-config, GLib development tools (`glib-compile-resources`), OpenSSL development files, GTK 4.12+
and libadwaita 1.6+. On Fedora 44:

```sh
sudo dnf install rust cargo gcc pkgconf-pkg-config glib2-devel openssl-devel gtk4-devel libadwaita-devel
cargo build --locked
cargo run --locked
```

Enter the **backend API URL** in the login screen, for example
`http://localhost:8080`. For a local server, start papo-backend separately and
keep it running while using the frontend. For a remote server, only the frontend
needs to run on your computer. Remote servers require HTTPS; HTTP is permitted
only for localhost/loopback development. Enter the server password when required,
then use your account credentials.

The pelican icon is embedded in the executable, including when using `cargo run`.
To also register Papo in the desktop app list and give its dock entry the same icon:

```sh
cargo build --locked
python3 scripts/install-desktop.py
```

This creates a per-user launcher pointing at this checkout's debug executable;
rebuilding updates the app it launches. For a release build, use
`cargo build --release --locked` and
`python3 scripts/install-desktop.py --binary target/release/papo-gtk`.
Keep the checkout at that path, or rerun the installer after moving it.

Voice, video playback, cameras and screen sharing use the native media runtime.
Install it on Fedora with:

```sh
sudo dnf install bubblewrap python3-gobject python3-gstreamer1 gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free libnice-gstreamer1
```

Screen sharing also requires a working desktop portal. Media playback depends
on the installed codecs; a backend configured for voice is required for calls.
Video preview/animation helpers require Bubblewrap and working user namespaces.
See [SECURITY_REVIEW.md](SECURITY_REVIEW.md) for media isolation and supported containers.

## Downloads

[Version 1.0.2](https://github.com/ISpeakHue/papo-gtk/releases/tag/v1.0.2) provides
an x86_64 AppImage and Flatpak bundle, SHA-256 checksums and matching source.
See [APPIMAGE.md](APPIMAGE.md) for the modern Fedora/Arch baseline and
[FLATPAK.md](FLATPAK.md) for installation and runtime details.

## Tests

For a self-contained Linux test build to share with others, see
[APPIMAGE.md](APPIMAGE.md). The build script produces an x86_64 AppImage with
the GTK and native media runtime included, plus tester instructions and source.

```sh
cargo test --locked
```

The default suite uses local HTTP and WebSocket fixtures and requires no live
backend or account credentials. GitHub Actions also runs GTK workflows under
a virtual display, native voice cleanup and private screen portal checks.
Real SFU integration can be enabled through the workflow's manual inputs.

See [TESTING.md](TESTING.md) for dependencies, commands and manual checks,
[DESIGN.md](DESIGN.md) for interface decisions, and
[follow-ups](docs/FOLLOW_UPS.md) for unresolved investigations and deferred work.
See [EMBEDS.md](EMBEDS.md) for the updated embed API, custom-card controls and
current backend media limitations.

## License

GNU General Public License version 3. See [LICENSE](LICENSE).
