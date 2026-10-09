# Papo GTK

A native desktop frontend for [papo-backend](https://github.com/Papo-Chat/papo-backend),
built with Rust, GTK 4, libadwaita and Relm4.

The application supports server and account authentication, channels and direct
messages, attachments and rich link/custom embeds, reactions and mentions, member profiles,
notifications, moderation, voice calls, cameras and screen sharing. The interface
uses an adaptive server/channel sidebar and native GNOME controls.

## Build and run

Requires Rust, a C compiler, pkg-config, OpenSSL development files, GTK 4.12+
and libadwaita 1.6+. On Fedora 44:

```sh
sudo dnf install rust cargo gcc pkgconf-pkg-config openssl-devel gtk4-devel libadwaita-devel
cargo build --locked
cargo run --locked
```

Enter the **backend API URL** in the login screen, for example
`http://localhost:8080`. For a local server, start papo-backend separately and
keep it running while using the frontend. For a remote server, only the frontend
needs to run on your computer. Enter the server password when required, then use
your account credentials.

Voice, video playback, cameras and screen sharing use the native media runtime.
Install it on Fedora with:

```sh
sudo dnf install python3-gobject python3-gstreamer1 gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free libnice-gstreamer1
```

Screen sharing also requires a working desktop portal. Media playback depends
on the installed codecs; a backend configured for voice is required for calls.

## Tests

```sh
cargo test --locked
```

The default suite uses local HTTP and WebSocket fixtures and requires no live
backend or account credentials. GitHub Actions also runs GTK workflows under
a virtual display, native voice cleanup and private screen portal checks.
Real SFU integration can be enabled through the workflow's manual inputs.

See [TESTING.md](TESTING.md) for dependencies, commands and manual checks,
[DESIGN.md](DESIGN.md) for interface decisions, and
[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) for feature implementation notes.
See [EMBEDS.md](EMBEDS.md) for the updated embed API, custom-card controls and
current backend media limitations.

## License

GNU General Public License version 3. See [LICENSE](LICENSE).
