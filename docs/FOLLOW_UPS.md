# Follow-ups and deferred work

Completed implementation plans and numbered bug reports have been removed from
the working tree; Git history retains them. Current behavior and commands belong
in the [testing guides](../TESTING.md), [design guide](../DESIGN.md),
[backend embed notes](../EMBEDS.md) and [webhook notes](../WEBHOOKS.md).

The [2026-10-30 Windows scrolling/text record](testing/windows-scroll-2026-10-30.md)
tracks the read-only IME layout fix and font rasterization defaults. It is
separate from the native crash and height-measurement investigations below.
The broad replay still reproduces `0xC0000005` during timeline history after
the action fixture; its current logs are in `target/windows-dev/scroll/regressions-final`.

## Investigations that need current evidence

- **Deep Windows extraction roots:** after the core Unicode-path correction,
  a deeply nested accented ZIP root still terminates several GPU plugin scanners
  and loses `mfvideosrc`. The same bytes pass full runtime/relocation checks and
  three fresh caches in a shorter accented root with HIP enabled. No Windows
  long-path setting or driver was changed. Preserve the failing scanner logs and
  runtime identities in `target/windows-dev/remaining/{portable-unicode,cold-runtime-unicode}`;
  short-root passing results are under `{portable-verified,cold-runtime-verified}`.
  The bounded scanner-tree debugger captured `0xC0000409`, parameter 5, at native
  exception sites in AMD `amdxx64.dll` (9.17.11.0082), `amdxc64.dll` and
  `amdhip64_6.dll` (10.0.3652.0); `trace-tree/trace.log` and its local minidump
  retain that evidence. The fast-fail sites do not prove the complete calling
  fault or a vendor fix, and are distinct from the older full-GUI access violation.
  This failure remains open; extract the current development ZIP to a short
  writable folder. See the [local record](testing/windows-local-2026-10-29.md).

- **Windows accented-path scanner startup:** fresh-cache portable relocation on
  2026-10-29 reproduced GStreamer's ANSI executable-path/UTF-8 command mismatch.
  Stock core 1.28.8-1 fails the native regression; the retained
  [Unicode-path correction](../packaging/windows/gstreamer/README.md) passes it.
  Developer setup, CI and packaging require the corrected revision and preserve
  its exact source. This is separate from the AMD HIP investigation below;
  [the local record](testing/windows-local-2026-10-29.md) tracks full-package replay.

- **Windows HIP plugin discovery on AMD:** a fresh developer SDK registry scan
  on Windows 11 Home 23H2 (22631.6199), with GStreamer 1.28.8, stalled in
  `libgsthip.dll` → `libgsthip-1.0-0.dll` → `amdhip64_6.dll`; another driver thread
  was waiting in `amd_comgr_2.dll`. An explicit HIP-only loading exclusion let
  discovery finish. The [developer wrapper](../MULTIPLATFORM.md#windows-developer-build-x86_64)
  now bounds the probe/scanner process tree and offers `-ExcludeHip`, recording
  the override separately. The packaged preview needs its own fresh-registry
  reproduction on qualified Windows/driver versions before choosing a release
  correction; this developer workaround does not establish a vendor-driver fix.
  **2026-10-29:** the installed preview from `474f256` completed fresh-registry
  discovery with and without HIP under a bounded process-tree probe. Its runtime
  manifest SHA-256 is `35d17d24c8dcd50932aec9b8a9e30fd2412300ed3e6df325353f33933d230170`.
  This did not reproduce the SDK stall; no release-wide HIP exclusion was added.

- **Windows inline-emoji pause assertion:** the [step 7 record](testing/evidence/windows-media-2026-10-23/README.md)
  retains one standard-user VM failure comparing frames immediately after a
  scroll; the unchanged complete replay, native CI matrices and earlier VM
  diagnostic passed. Capture allocated clipping bounds and after-paint frame
  samples before attributing this to the animation scheduler or changing the
  assertion. The visibility code already schedules a check after allocation;
  premature sampling is plausible, not a demonstrated root cause or fix.
  The unchanged `media-emojis` and `emoji-notifications` fixtures passed on the
  local Windows host on 2026-10-29; the intermittent VM failure remains open.
  Later ZIP-only replay reproduced the inline pause assertion. Its fixture now
  waits for two after-paint frames, asserts allocated bounds are actually clipped
  and verifies no active playback deadline before sampling, then still requires
  400ms of unchanged frames and resumed animation on scrolling back. Focused
  Cairo/default replays pass with these diagnostics. This corrects premature
  fixture sampling; no production scheduler change or original-VM rerun is claimed.

- **Linux CI qualification:** the [build-provenance record](testing/build-provenance.md#verification-record-2026-10-21)
  identifies the repaired sandbox/loader setup, the explicit codec dependency
  correction and individual remaining GUI failures. Keep those cases failing until
  their assertions or product defects are resolved with mapped-layout/decoder
  evidence; an icon-only pass does not qualify the complete GUI workflow.
  On 2026-10-29, Windows focused cases `webhook-chat`, `report10`, `emoji-grid`,
  `emoji-notifications`, `account-notices` and `backend-contracts` passed. These
  are not Linux reruns and do not close that platform's qualification record.

- **Native playback teardown:** the [2026-10-13 crash analysis](archive/CRASH_ANALYSIS_2026-10-13.md)
  records a GStreamer cleanup SIGSEGV after leaving fullscreen and jumping to the
  latest messages. Synthetic passing runs did not establish its root cause or
  prove it fixed. Preserve the matching executable for any new dump; reproduce
  the actual fullscreen-to-history-eviction path before attributing the fault.
  `recent-media` now exercises ten cycles with Papo's actual chat, native
  H.264/AAC playback, fullscreen restoration, the Latest action and history/player
  eviction. All ten passed on Windows on 2026-10-29. The original Linux crash is
  still unproven fixed; rerun this regression there with matching native symbols.
- **Shutdown diagnostics:** the earlier warning-cleanup run recorded GTK
  measurement/focus diagnostics and an `AccessLoaded` callback after a fixture
  component shut down. Those observations were separate from compiler warnings.
  Reproduce on the current code before treating them as confirmed current bugs.
  The [backend guide](testing/backend.md) describes the relevant GUI checks.
  **2026-10-29:** permission refreshes now belong to the account component and
  are cancelled on drop; late completions use a fallible channel send. The
  `recovery` fixture holds a real local HTTP response across component closure
  and verifies cancellation. The earlier callback log was not reproduced in the
  focused runs. GTK height-measurement warnings still reproduce in the profile/
  full-GUI fixtures and are not suppressed or claimed fixed.
- **Windows full-GUI native failure:** after correcting the fixture's assumption
  that local screen capture is enabled, the broad `default` UI case exits with
  `0xC0000005` on Windows 11 Home 23H2. The executable built with patched
  dependencies reproduces it; focused call, recovery and media cases pass.
  Preserve the matching test executable and `target/windows-dev/step8/ui-default.log`
  / `default-gdb.log` for native diagnosis. Do not classify this as the older
  Linux GstPlay failure or infer a cause from preceding layout/font warnings.
  The debugger replay did not capture that access violation: it reached a Rust
  panic when an older input fixture tried to run `setxkbmap`. Those fixtures now
  require Linux and `PAPO_NATIVE_INPUT_TEST=1`; the old presence check incorrectly
  treated `0` as enabled. Focused edit-mentions, multiline, full-app input,
  login input and report9 cases pass after the correction. This is a separate
  test-portability fix, not an established cause of the native failure.
  Current replays and test-only stage markers are retained under
  `target/windows-dev/remaining/`. Debugger replays also found relative WebM
  fixture paths and a rich-text sentence short enough to fit Windows' narrow
  card; those fixtures are corrected and focused cases pass. Neither establishes
  the access violation's cause. See the [current automatic record](testing/windows-local-2026-10-29.md).
  After the fixture/native corrections, a complete automatic replay passed in
  337.67 seconds. Preserve that execution result separately; one pass does not
  prove an intermittent native failure fixed without a matching fault diagnosis.
- **Dependency advisory review:** the [security review](../SECURITY_REVIEW.md)
  covers the implemented frontend boundaries. The 2026-10-29 RustSec scan and
  compatible lockfile fixes leave zero vulnerability findings and four explicit
  unmaintained-dependency notices. A targeted GStreamer advisory/version review
  is recorded there; the complete bundled native-library audit remains open.
- **Desktop and device acceptance:** private displays and synthetic media do not
  validate every compositor, GPU driver, physical microphone/camera or long
  multi-user call. Keep the [voice/device checks](testing/voice.md) and
  [AppImage runtime checks](../APPIMAGE.md) separate from fixture results.
  Current local Windows audio/video SFU evidence and the user's local-only test
  boundary are recorded in [Windows calls](testing/windows-calls.md).
  Physical Windows USB microphone capture/Opus/mute/gain/release, twenty real
  application-worker drop cycles, direct/TURN buffer transport and user-confirmed
  WASAPI call cues now pass. The user has no camera/live test accounts available
  and deferred attended unplug/privacy/default-device changes; those specific
  acceptance gaps remain open rather than claiming Windows hardware was not run.

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
