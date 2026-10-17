# Security boundaries and private diagnostics

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

## Private terminal diagnostics

The logging subscriber defaults to `warn`, writes to stderr and accepts
`RUST_LOG` for application diagnostics. Its independent target filter disables
third-party HTTP/WebSocket dumps and component state/message dumps, including
foreign span fields, even when a more specific transport directive requests
`trace`. Use `RUST_LOG=papo_gtk=debug cargo run` for HTTP method/status/timing and
attempt counts. No request URLs, headers or bodies are included in these logs.

Authentication request models redact passwords in Debug while preserving JSON
serialization. Session cookie headers are marked sensitive for both HTTP and
WebSocket requests. Request errors strip their URL; terminal transport and
backend diagnostics retain typed error categories or status rather than raw
server text. Invalid WebSocket frames report only parse positions.

```sh
cargo test --locked --offline logging
cargo test --locked --offline authentication_debug
cargo test --locked --offline verbose_authentication_logs
cargo test --locked --offline
```

Captured-output checks use fake passwords/cookies and hostile response/error
payloads. They verify safe diagnostics survive debug logging, default/invalid
filters remain quiet, verbose dependency filters and spans cannot expose
credentials, and real login/server-password bodies and cookies still reach the
local mock server unchanged. Existing session rotation, authentication,
WebSocket connection and request pacing/retry tests exercise the same paths.

## Security boundaries

The fixes and remaining limitations are documented in [SECURITY_REVIEW.md](../../SECURITY_REVIEW.md).
Local video conversion requires Bubblewrap (`bubblewrap` on Fedora/Arch) and
working user namespaces. The AppImage already bundles Bubblewrap. Helpers fail
safely when isolation is unavailable; voice workers retain their own device and
network access. Inline file playback supports self-contained MP4/QuickTime and
WebM; other files can still be downloaded. Remote APIs require HTTPS, with HTTP
allowed only on loopback for development.

```sh
cargo test --locked
python3 tests/security/media_network_probe.py
PAPO_UI_CASE=security-icons dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The suite checks redirect destinations without sending them credentials, bounded
declared/chunked/close-delimited response bodies, session-error status preservation,
JSON text/collection limits, public-only avatar DNS, external media references,
and native worker network/filesystem isolation. The playlist probe must report
`request_count: 0` and a nonzero helper exit. GUI checks use a synthetic local
server and verify asynchronous icon decoding, bounded thumbnails, stale-result
rejection and saving while an icon is pending. They never read a saved session.

Media compatibility regressions include `draft-previews`, `pins-media`,
`attachment-media` and `backend-gifs` using the same `PAPO_UI_CASE` command above.
No test here establishes that all native codecs or dependencies are vulnerability-free.
