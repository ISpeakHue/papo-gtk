# Testing

Run the suite with:

```sh
cargo test --locked
```

Install Rust, a C compiler, pkg-config, GLib development tools, OpenSSL development files, GTK 4.12+
and libadwaita 1.6+ first. On Fedora 44:

```sh
sudo dnf install rust cargo gcc pkgconf-pkg-config glib2-devel openssl-devel gtk4-devel libadwaita-devel bubblewrap
```

The default suite runs without a display, a Papo backend, credentials, or a database.
The HTTP/WebSocket integration test binds an ephemeral loopback port, so the
execution environment must permit local sockets. Dependencies must already be
cached when using `cargo test --locked --offline`.

GitHub Actions runs the same suite on pushes, pull requests, and manual dispatches
using `.github/workflows/test.yml`. Its rootless Fedora container supplies the GTK and
libadwaita versions required by `Cargo.toml`. Compilation includes all UI components,
so errors such as the invalid application-window child assignment fail CI. Focused
CI cases run the GTK smoke test in Xvfb to verify the header bar, window controls,
separate account/server password fields, history pagination/retry controls,
presence status icons, login/main view transitions, and the session-expired login message.
It edits the actual login fields, including Unicode typing, paste, deletion,
queued edits, selection/caret preservation, and credential clearing, to catch
text-change feedback loops that synthetic component messages miss.
It also exercises the message/emoji workflows against a stateful local mock
HTTP server, including failures, retries, confirmations, and permission changes:

```sh
dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

Media helper tests require Bubblewrap and working user namespaces. Native media
and GUI dependencies are described in [Build and run](README.md#build-and-run).
All ordinary fixtures use synthetic data; real device/server checks are opt-in.

## Focused guides

| Guide | Coverage |
| --- | --- |
| [Build provenance and CI diagnostics](docs/testing/build-provenance.md) | Windows source/toolchain/native inputs, sandboxed Fedora runtime and per-case GUI failures |
| [Windows acceptance matrix](docs/testing/windows.md) | Target desktop builds, feature checklist, existing CI evidence and packaged-release gates |
| [Desktop integration and layout](docs/testing/desktop.md) | Icons, adaptive navigation, header geometry and AppImage media plugins |
| [Feature workflows and acceptance checks](docs/testing/workflows.md) | Requirements, manual scenarios and account/admin feature walkthroughs |
| [Accounts and profiles](docs/testing/account.md) | Login request budgets, remembered sessions, profiles, settings and status |
| [Messages, Markdown and reactions](docs/testing/chat.md) | Message rendering, mentions, emoji browsing, syntax colors and reaction details |
| [Composer, clipboard and input methods](docs/testing/input.md) | Image paste, multiline composition and accented input diagnostics |
| [Images, GIFs, embeds and video](docs/testing/media.md) | Media presentation, playback, viewers, pins and attachment previews |
| [Notifications and unread state](docs/testing/notifications.md) | Inbox tabs, channel/sender groups, replies and unread indicators |
| [Caching, scrolling and performance](docs/testing/performance.md) | Loading order, bounded work, history retention and media reuse |
| [Voice, camera and screen sharing](docs/testing/voice.md) | Native workers, device/call acceptance, screen portals and call sounds |
| [Backend integration and webhook identities](docs/testing/backend.md) | Protocol fixtures, moderation, webhooks and external author metadata |
| [Security boundaries and private diagnostics](docs/testing/security.md) | Credential redaction, response/DNS/media limits and isolated workers |

The [CI workflow](.github/workflows/test.yml) lists the current focused GUI cases.
Use `PAPO_UI_CASE=<case>` with the GUI command above to run one case.
[Performance measurements](PERFORMANCE.md), [security review](SECURITY_REVIEW.md)
and [remaining follow-ups](docs/FOLLOW_UPS.md) record scope and limitations.
