Security review and remediation — 2026-10-16

The source review of revision `b989d35` identified the five issues below. All five
now have frontend corrections and regression coverage. Tests use synthetic local
fixtures only; no saved sessions, private messages or credentials were accessed.
These checks are not a live-server penetration test or security certification.

1. **[P1] Local video playlists could trigger network requests — fixed.**

   The original poster helper followed a synthetic HLS playlist and issued two
   HTTP GETs. [Container validation](src/media/container.rs) now runs on a worker
   before native inline playback, posters and GIF-video conversion. It accepts
   self-contained MP4/QuickTime and WebM, rejects manifests, reference movies and
   external MP4 data references. Other formats remain available for download.

   [File-media helpers](src/media/sandbox.rs) additionally run in Bubblewrap with
   private user/network/mount namespaces, no host network, home or desktop sockets,
   read-only input and private temporary storage. Only system/AppImage runtimes
   are mounted read-only. Missing Bubblewrap or denied namespaces fails safely;
   there is no unsandboxed fallback. AppImage already bundles Bubblewrap;
   development installations need `bubblewrap` and working user namespaces.

   Both Python helpers reject manifest headers before native discovery, even on
   direct invocation. The [playlist regression](tests/security/media_network_probe.py)
   asserts rejection and **zero HTTP requests**. A native regression checks host
   network isolation, hidden private files, read-only input and healthy MP4-poster/
   WebM-animation decoding.

   Inline GTK playback still runs native codecs in the application process after
   container validation. This does not sandbox every decoder or establish immunity
   to codec vulnerabilities. Voice/camera workers retain network/device access.

2. **[P1] Remote HTTP could expose passwords before cookie validation — fixed.**

   [Client construction](src/api/mod.rs) requires HTTPS before any remote request.
   HTTP is allowed only for literal loopback addresses and exactly `localhost`;
   HTTP clients pin localhost DNS to loopback. LAN servers also need HTTPS.
   Authenticated redirects must preserve scheme, host and effective port, with
   a five-hop ceiling. Tests reject remote HTTP, HTTPS downgrade and origin/port
   changes, and verify 301/302/307/308 destinations receive no password request.

3. **[P2] Unbounded API/error bodies could exhaust memory — fixed.**

   The [shared streaming reader](src/api/body.rs) checks declared lengths and actual
   chunks, including responses without Content-Length. JSON, registration and
   security/ordinary errors use it; existing media-download caps remain in force.
   Error/non-JSON acknowledgements: 64 KiB. JSON: normally 8 MiB, emoji lists 24 MiB,
   message/pin lists 32 MiB, profile batches 160 MiB (up to 50 base64 portraits).
   These are ceilings, not preallocated buffers.

   Before typed models reach GTK, rendered message/embed text is capped at 32 KiB,
   URLs at 4 KiB and media blobs at 8 MiB. Collection limits include 100 messages,
   50 profiles, 10 attachments/embeds and 25 embed fields; traversal is capped at
   100,000 nodes. Oversized data is rejected rather than truncated. WebSocket
   frames/messages are capped at 8 MiB and use the same parsed-data checks.
   Oversized error bodies preserve HTTP status for session recovery. Regressions
   cover declared, chunked and close-delimited responses, exact-limit success,
   session errors and rendered text/collections.

4. **[P2] External-avatar DNS could resolve to private services — fixed.**

   The [avatar client](src/api/webhooks.rs) uses a [validating resolver](src/api/public_dns.rs).
   Every resolved address must be public; the exact validated list goes to the
   connector, avoiding a second lookup. Empty/excessive/mixed public-private
   answers, loopback, LAN, link-local, CGNAT, mapped IPv6, multicast and reserved
   ranges are rejected. Literal IPs use the same check.

   External avatars require HTTPS on port 443, with no ambient proxy, cookies,
   Authorization or redirects. The only same-backend exception is the documented
   `/webhooks/{uuid}/avatar` route; arbitrary same-origin paths are rejected.
   Explicitly configured private/local backends retain that route. DNS/range,
   mixed-answer, URL and local backend tests cover this boundary.

   Public avatar hosts can still observe the viewer's public IP. Preventing that
   requires backend proxying or a provider allowlist, a separate product decision.

5. **[P2] Administration icons bypassed bounded decoding on GTK — fixed.**

   [Administration previews](src/ui/main_window/administration.rs) use the shared
   bounded asynchronous avatar decoder (two concurrent jobs maximum). Encoded
   input and source pixels are checked before full decoding; GTK receives only
   a thumbnail of at most 64×64 pixels. Unbounded texture helpers were removed.
   Local icon reads are capped at 2 MiB while streaming. Generation tokens reject
   late results after replacement/removal/reopening. Saving waits for a selected
   icon; cancellation and invalid input release the pending state.

   Tests reject corrupt base64, oversized encoded input and a valid PNG header
   advertising 50,000×50,000 pixels without allocating that image. The
   `security-icons` GUI test holds decoder workers, verifies responsiveness,
   pending-save handling, stale-result rejection and bounded texture installation.

Verification commands and runtime requirements are in [TESTING.md](TESTING.md).
CI includes native regressions, the playlist sentinel and the icon GUI case.
Its disposable container permits namespace creation so Bubblewrap can apply the
stricter inner sandbox; production desktop security policies are not changed.

Dependency advisory scanning remains **incomplete**: `cargo audit` is not installed;
no RustSec database scan or audit of bundled system codec libraries was performed.
Existing keyring storage, credential/log redaction, private temporary media files
and native Markdown rendering remain in place. Backend authorization and native
codec/dependency vulnerabilities require continued review.
