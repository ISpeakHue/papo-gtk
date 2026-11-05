# Implementation-plan steps 10–12 validation

These checks validate the local implementation working tree on Linux, using
private configuration directories, loopback backends and virtual displays.
Native audio checks use generated sources and fake outputs, never a physical
microphone or a saved account/session. No backend source, remote CI run, commit,
push or release is included. Native Windows and physical hardware acceptance
remain pending.

Feature details: [disk thumbnails, media browser and call
setup](../../../media-browser-and-call-setup.md). Exact counts, source fingerprint,
binary hash and focused-case outcomes are in [checks.json](checks.json).

## Coverage

- Disk-cache tests cover account/revision separation, restart admission,
  deletion/permission gates, stale writes, refreshed previews, hard byte/LRU
  limits, corrupt/expired/oversized entries and private Unix file modes.
- Media-history tests cover independent attachment/link cursors, timestamp/UUID
  ties, descending merge order, bounded exact-message lookups and rejecting
  cross-channel results before requesting history.
- `disk-thumbnails` reconstructs the API/client and GTK component tree for the
  same simulated readable account. Eight cold image requests become **zero
  additional thumbnail requests** with disk reuse. It also covers moderation
  invalidation and clearing. Timings and the fixture's encoded image size are
  retained in [disk-comparison.json](disk-comparison.json).
- `media-browser` covers image/file filtering, incremental discovery, native
  image viewer navigation, conservative invalidation and closing after access
  loss. Search-hit resolution uses one-message pages alongside independent chat
  refreshes; the browser does not scan ordinary text-history pages.
- `audio-setup` verifies explicit local record/replay and output tests,
  no ICE requests, stop-on-close, late-event rejection, confirmed device changes,
  mute preservation and failure rollback presentation.
- `voice-controls` includes the third device selector, scrolling to camera
  controls above the fixed action footer, narrow layouts, existing camera/screen
  state transitions and media cleanup.
- Python native-device checks cover bounded record/replay, output testing without
  opening a microphone, cancellation, permission/startup failure, microphone
  changes with stable WebRTC/SSRC/mute, output replacement and rollback. Existing
  synthetic device-removal/Windows endpoint tests remain included.
- The ignored native-worker cleanup check verifies process termination after
  dropping the Rust owner.

The local Go SFU probe ran with `--transport both --switch-audio`: both direct ICE
and TURN verify acknowledged live synthetic microphone changes and resumed
decoded/audible audio, followed by active-speaker, mute, routing and leave checks.
This is not a physical microphone/output or OS permission-dialog acceptance test.

Focused compatibility cases cover media loading/cache reuse, backend GIFs,
attachments, original viewers, pinned media, channel caches, account ownership,
translations, outgoing messages and existing call controls. The wider aggregate
native GStreamer stress case was not rerun; its earlier limitation is recorded
in [steps 7–9](../steps789-2026-11-05/README.md).

## Performance scope

The fresh-client disk fixture uses eight deterministic 640×320 PNGs, not a
live-server connection. Its timings are individual observations; request reuse
is the functional assertion. The earlier baseline's warm returns already made
zero extra requests **inside the same process**. This change additionally tests
reuse after rebuilding the client/component ownership tree.

| Isolated disk fixture | Cold | Fresh client using disk |
| --- | ---: | ---: |
| Eight decoded textures installed in the model | 541 ms | 59 ms |
| Thumbnail HTTP requests | 8 | 0 |

Readiness excludes compositor presentation. Each encoded fixture image is
819,648 bytes; thirty images of that shape use about 23.45 MiB before cache
headers. The 32 MiB account default accommodates that opening-batch scenario
while retaining a hard eviction bound. It does not guarantee retention of every
possible thirty-image batch or original file.

A post-change run of the existing performance fixture is retained in
[baseline.json](baseline.json). It uses private Xvfb/Openbox and Cairo in the
cached Fedora CI container; the earlier three samples used private Xvnc. Treat
timings across those environments as diagnostic observations, not a controlled
speedup/regression claim. No timing regression budget is introduced before
step 17. Cache bytes and process RSS remain separate measurements.

## Backend and packaging limits

By user preference, media discovery uses the current backend search API and
targeted history lookups. Historically unindexed custom/bot embeds without
text URLs remain outside that API's coverage. No speculative media endpoint or
frontend scraper is enabled.

Linux/Windows CI include the new GUI workflows; the optional local-SFU CI job
also enables the synthetic live-switch probe. Updated source/workflow checks
do not constitute a Windows run, packaged acceptance or release validation.
Development failures from obsolete two-selector/without-scrolling expectations
were corrected to exercise the expanded layout. The fresh-client disk check
does not assume immediate destruction of GTK's last native snapshot after
closing a window.

Raw development logs stay under `target/steps101112-*` and are excluded from Git.
