# Follow-ups and deferred work

Completed implementation plans and numbered bug reports have been removed from
the working tree; Git history retains them. Current behavior and commands belong
in the [testing guides](../TESTING.md), [design guide](../DESIGN.md),
[backend embed notes](../EMBEDS.md) and [webhook notes](../WEBHOOKS.md).

## Investigations that need current evidence

- **Windows inline-emoji pause assertion:** the [step 7 record](testing/evidence/windows-media-2026-10-23/README.md)
  retains one standard-user VM failure comparing frames immediately after a
  scroll; the unchanged complete replay, native CI matrices and earlier VM
  diagnostic passed. Capture allocated clipping bounds and after-paint frame
  samples before attributing this to the animation scheduler or changing the
  assertion. The visibility code already schedules a check after allocation;
  premature sampling is plausible, not a demonstrated root cause or fix.

- **Linux CI qualification:** the [build-provenance record](testing/build-provenance.md#verification-record-2026-10-21)
  identifies the repaired sandbox/loader setup, the explicit codec dependency
  correction and individual remaining GUI failures. Keep those cases failing until
  their assertions or product defects are resolved with mapped-layout/decoder
  evidence; an icon-only pass does not qualify the complete GUI workflow.

- **Native playback teardown:** the [2026-10-13 crash analysis](archive/CRASH_ANALYSIS_2026-10-13.md)
  records a GStreamer cleanup SIGSEGV after leaving fullscreen and jumping to the
  latest messages. Synthetic passing runs did not establish its root cause or
  prove it fixed. Preserve the matching executable for any new dump; reproduce
  the actual fullscreen-to-history-eviction path before attributing the fault.
- **Shutdown diagnostics:** the earlier warning-cleanup run recorded GTK
  measurement/focus diagnostics and an `AccessLoaded` callback after a fixture
  component shut down. Those observations were separate from compiler warnings.
  Reproduce on the current code before treating them as confirmed current bugs.
  The [backend guide](testing/backend.md) describes the relevant GUI checks.
- **Dependency advisory review:** the [security review](../SECURITY_REVIEW.md)
  covers the implemented frontend boundaries, but RustSec scanning and a review
  of bundled native codec/library advisories remain outstanding.
- **Desktop and device acceptance:** private displays and synthetic media do not
  validate every compositor, GPU driver, physical microphone/camera or long
  multi-user call. Keep the [voice/device checks](testing/voice.md) and
  [AppImage runtime checks](../APPIMAGE.md) separate from fixture results.

## Deferred product and backend decisions

- **Disk thumbnails/avatars:** retain the current in-memory caches until measured
  repeated-launch traffic justifies persistence. Any future disk cache needs an
  explicit size budget, account/server isolation, expiry, clear-cache controls
  and permission/logout invalidation. Use [performance diagnostics](../PERFORMANCE.md)
  to establish that benefit first.
- **Further history indexing:** larger lookup/height indexes remain conditional
  on a measured bottleneck in the bounded renderer, rather than a prerequisite
  for current functionality.
- **Closed-app desktop notifications:** mobile push registration does not provide
  a Linux desktop transport. Receiving notifications after exit needs an agreed
  backend/desktop background-delivery design.
- **Authoritative notification totals:** channel/sender badges describe loaded
  unread notices. Exact totals across unloaded pages need backend aggregates;
  fetching every page just to calculate a badge would increase request pressure.
- **Webhook author role colors:** the current `ExternalAuthor` model has identity,
  name, avatar and profile URL fields, with no role-color field. Such colors need
  an explicit backend contract; names and embed accents are not substitutes.
  Native members continue to use their supplied role colors.
- **Recovery-link desktop integration:** opening recovery links directly in the
  app can follow the existing pasted-token/link form. Do not invent an email
  recovery API that the backend does not expose.

The old plan's missing read-marker limitation is intentionally not carried
forward: the frontend now uses the explicit channel-read endpoint. Historical
route inventories and provider-scraping instructions are also superseded by the
current API implementation and backend GIF resolver.
