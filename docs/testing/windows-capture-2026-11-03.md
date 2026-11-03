# Attended Windows capture checks — 2026-11-03

Local-only tests on Windows 11 Home 23H2, build 22631.6199, AMD Radeon RX 6700 XT,
driver 32.0.21045.16010, one observed 1920×1080 display. This host is outside the
Home/Pro release rows in [WINDOWS_SUPPORT.md](../../WINDOWS_SUPPORT.md).
No production account or backend was used. Capture always required the user's
explicit Windows picker selection; no images or recordings were saved.

## Corrections

- The picker owner is explicitly made visible. Capture waits for foreground
  activation rather than allowing a background picker to immediately self-cancel.
  The Rust application grants foreground rights only to its owned voice worker.
  Activation and selection share the existing 90-second deadline.
- A changed WGC content size can temporarily accompany a smaller old texture.
  Such frames are discarded until a fitting texture arrives. Copy bounds and
  frame-pool recreation remain enforced.
- After functional capture/resize tests, the process crashed with `0xC0000005`
  while executing offset `0x6efd` in an **unloaded** System32 GraphicsCapture.dll.
  The private debugger recorded the module range/unload and fault address without
  saving capture content or a memory dump. Pinning this system module for the
  worker lifetime corrected the observed exit failure; sessions and pools still
  close normally. This is separate from the existing timeline-history GUI crash.
- Optional ABI diagnostics retain HRESULT, stage and owner visibility/foreground
  flags. Tests require the actual cancelled state, so a native failure cannot
  count as successful cancellation. The peer probe checks child exit codes as
  well as successful frame delivery.

## Results

The standalone runtime previously verified in the [scrolling record](windows-scroll-2026-10-30.md)
was used with the updated source worker and separately built native bridge.
These are local development checks, not acceptance of a final release artifact.
Base commit: `feb687665f00854706349b620c99f04cd52594b7`, with the recorded working
tree changes. Tested diagnostic bridge SHA-256:
`2785711840fe3f0425c82f01dcd2596518afb68c9837ae0133fd69292fba3344`.

| Check | Result |
| --- | --- |
| Real picker cancellation | Native state 2, HRESULT 0, zero buffers, no delivered source; exit 0. |
| Disposable selected window | 120 buffers; all 120 matched the four-color source; sizes 884×569 and 1056×749; joined native close and process exit 0. |
| Explicitly selected monitor | 120 buffers at 1920×1080; no recording or transmission; joined close and process exit 0. |
| Direct local three-peer SFU | First picker cancelled without publication, then two native screen sessions; two receivers decoded the correct source, both visually confirmed by the user. Resize, stop/restart, source closure, camera/screen subscription changes, reverse publisher, backpressure, concurrent synthetic audio and leave passed. Probe exit 0. |
| Forced local TURN | Same native screen/peer checks passed; every peer produced relay candidates. Both receiver panels visually confirmed. SFU and all three workers exited 0. |
| Capture regressions | 17 adapter tests, one real GStreamer BGRA/appsrc/VP8 resize/restart test and one native ABI/ownership/frame/close-bound test passed. |
| Voice worker regressions | 19 device/error/capture lifecycle tests passed. |
| Rust regressions | 310 passed, six ignored; no failures. |

Local logs and machine-readable results are retained under
`target/windows-dev/screen/`: `cancel-foreground`, `frames-resize-fixed`,
`monitor-final`, `direct-peers-retry`, `relay-peers-fixed`. Earlier failed runs
remain there and are not counted as passes. `frames-unload-map` identifies the
pre-fix OS module failure. Local test peers never use `api.cyberasilo.online`.

The reusable source fixture is `tests/windows_screen_test_window.py`. It renders
generated colors and responds only to its explicitly supplied control file.
`tests/video_probe.py --windows-capture --preview` displays decoded receiver
panels; `PAPO_CAPTURE_TEST_CONTROL` enables generated-source pixel verification,
controlled resize/closure and an explicit visual confirmation barrier.

## User acceptance and default enablement

After these checks, the user reported: “I've tested it locally and it all worked.”
This is recorded as user-reported local acceptance and used to enable Windows
screen sharing by default. No additional test logs, account/backend identity,
monitor configuration or OS matrix rows were supplied; none are inferred.
The launch-time preview flag has been removed from the application and attended
probes, and the portable capability check now requires `screen_capture: true`.
Each share still requires an explicit Windows picker selection.

## Remaining release qualification

Default feature availability does not qualify a supported Windows release row.
The following still need detailed, artifact-bound release evidence:

- Physical mixed-DPI/multiple-monitor, minimize/restore, occlusion/static-source,
  monitor disconnect, GPU loss and 4K performance checks.
- Ten consecutive real start/stop cycles and full application leave/disconnect/
  exit while picking and while sharing, including handle/working-set observation.
- The actual GTK viewer and authenticated deployed backend with two test accounts;
  local engine receiver panels do not establish full application acceptance.
- Repeating required desktop checks with the final relocated package on supported
  Home/Pro OS rows and a clean machine.

The updated preview is built by `packaging/windows/build.py --allow-dirty` using
the retained native inputs. Its generated `BUILD-INFO.json`, runtime manifest
and adjacent `.zip.sha256` identify the actual output; the previous scrolling
ZIP hash does not identify this change.
