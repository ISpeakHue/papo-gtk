# Recent changes: Windows verification — 2026-11-04

Result: the recent frontend changes pass the targeted native Windows source
checks after the fractional-wheel and decoration-coordinate fixes below.

Native checks use GitHub's Windows Server 2022 x86-64 runner, UCRT64,
the reviewed GTK IME backport and GStreamer Unicode-path backport. Fixtures
use synthetic messages, isolated settings and local mock backends. This is
source-build verification; it does not qualify a new installer or Windows 11
desktop release.

## Changes made during the review

- Preserve GDK's fractional wheel deltas. Win32 can report `Down` with a delta
  of `0.25`; converting every discrete direction to a whole step made these
  high-resolution wheels scroll too far. A native `-30` wheel event now tests
  a quarter detent alongside normal `-120` events.
- Correct decorated-window coordinates: subtract the native surface's offset
  when picking a widget; add it when converting widget coordinates for native
  input. GTK's own event routing uses this subtraction. Windows exposed a
  nonzero 25px decoration offset; the earlier handler/helper conversions were
  both inverted and cancelled each other in wheel-only checks. A native button
  click and descendant Ctrl+wheel handler independently check the hit position.
  The image viewer retains its existing native zoom controller.
- Extend the owned-window native-input helper with DPI-aware pointer actions,
  foreground/PID validation and balanced modifiers. Process activation,
  non-client frame changes and pointer motion before recalculating coordinates
  and sending a wheel event. Notification tabs otherwise intermittently missed
  the first synthetic event.
- Restore the read-acknowledgement fixture's host focus after modal dialogs on
  Windows. This preserves the application's existing active-window read policy.
- Check the local screen-preview pipeline's factories in both Windows developer
  preflight and packaged-runtime diagnostics: `tee`, `valve`, `videorate`,
  `videoscale`, `jpegenc` and `appsink`.
- Add native dialog/notification scrolling to the Windows workflow and packaged
  desktop suite, including its required report case list. Reuse the retained
  dependency build from the first run only when its full inventory and GTK
  recipe match; test results are always regenerated.

## Evidence

| Run | Source | Results and limits |
| --- | --- | --- |
| [Initial full workflow](https://github.com/ISpeakHue/papo-gtk/actions/runs/41774323962) | `536cd4a6f47ddd4bf07abb266f2adb88901c7cc8` | Windows all-target compilation with warnings denied; 320 Rust tests passed, six explicit display/native tests ignored by the regular suite and relevant native probes run separately. Native packaging/tooling Python checks: 108 passed. Worker suite: 28 run, one permitted Linux PulseAudio identity skip. Desktop input/login/edit/search/settings/voice fixtures passed before the new wheel fixture failed. macOS foundation compile/link passed. Overall workflow failed; no package acceptance is claimed. |
| [Independent recent GUI replay](https://github.com/ISpeakHue/papo-gtk/actions/runs/41777397903) | `16a17ac81f85c96c252a80fa9ab6b29029f0333c` | Emoji/header rendering and native Escape, all-channel preferences/mute actions, reply layout, mentions, completion, embeds and bounded decoded/encoded cache retention passed on Windows. The independent replay exposed Ctrl+wheel, notification first-event/focus and headless media-fixture failures. |
| [Affected GUI replay](https://github.com/ISpeakHue/papo-gtk/actions/runs/41779233005) | `d6af693` | Both notification tabs passed real wheel easing; backend/unread integration passed with explicit host activation; GIF/WebP and image-viewer fixtures passed. Quarter-detent input reached its correct fractional destination. H.264/AAC fullscreen/latest/history eviction passed ten cycles using the test audio sink. Descendant Ctrl+wheel still failed in this revision. |
| [Decoration-coordinate diagnostic](https://github.com/ISpeakHue/papo-gtk/actions/runs/41905057979) | `2200cd7e91de78e9e62bc1dbb125a010fa13262f` | Native fullscreen image-viewer checks passed. The independent native button-click check failed, exposing the reversed surface/widget conversions. The wheel-only checks had not revealed that error. This run did not pass. |
| [Corrected coordinate replay](https://github.com/ISpeakHue/papo-gtk/actions/runs/41907391563) | `b5332a26f39fe0179d2af2498a4e1a958881bb9f` | PASS: native vertical/nested/horizontal wheel input, a quarter detent, descendant Ctrl+wheel, native button activation, reduced motion and stopping on hide (5.36s); both notification tabs with real wheel input (9.16s); native fullscreen image-viewer checks (6.38s). Logs confirm the actual 25px surface offset. All three cases passed; the workflow is green. |

The temporary `ci/windows-recent-20261104` branch contains review snapshots and
a diagnostic-only workflow. It is not a release branch. The native replay
reads only wheel/input geometry in diagnostics, never credentials or message
text. The final local suite passes 318 tests (six explicit display/native skips);
the scrolling/image-viewer GUI fixtures and warning-free all-target check also
pass.
An additional private Linux display replay forces a 24px window margin and
observes a real `(24, 24)` native surface transform. Wheel easing, descendant
Ctrl+wheel, native button activation and reduced motion pass with that nonzero
offset, independently checking the surface/widget conversion.

## Separate Linux result

The initial workflow passed 40 of 41 focused Linux GUI cases. `recent-media`
terminated with SIGSEGV. A local replay reproduced the failure after seven
fullscreen/latest/history-eviction cycles; its core backtrace is
`g_type_check_instance_is_fundamentally_a → g_object_unref → gst_play_main`
on a GStreamer thread. The backtrace and compressed synthetic-fixture core are
retained under ignored `target/windows-review-linux-*` paths. This review does
not establish a frontend fix for that native playback failure, and the failed
full workflow must not be represented as a passing release gate.

## Remaining desktop acceptance

CI uses Cairo and a test audio sink. It verifies decoding, stream teardown and
screen-preview pipelines, not audible output, a physical camera/microphone,
attended screen selection or driver behavior. Windows 11 Home/Pro, physical
input/IME, multiple DPI settings, mixed monitors, real call devices and capture
remain covered by the existing [acceptance matrix](windows.md). These results
do not expand the experimental Windows support policy.
