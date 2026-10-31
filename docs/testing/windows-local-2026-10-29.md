# Local Windows automatic verification, 2026-10-29

This continues steps 8–14 with the user's **automatic local tests only** boundary.
No production account or call used `https://api.cyberasilo.online`. There is no
camera, no live pair of test accounts and no permission to perform attended
picker, toast-click, native-keyboard or device/privacy changes in this run.

The host is native x86_64 Windows 11 Home 23H2, build 22631.6199. This is useful
physical-machine evidence but does not qualify the policy's 25H2/26H2 Home/Pro
matrix. Source is the modified `multiplatform` checkout based on
`52df9a90ac95120042eaa4b11c099ae3070164cf`; no new clean-source CI candidate or
supported release is claimed. Full source state is recorded in build metadata.

## Executed automatic checks

| Area | Observed result | Remaining acceptance |
| --- | --- | --- |
| Regular/native tests | `windows-dev.ps1 Test -ExcludeHip`: 138 Python checks, 137 passed and one explicitly Linux-only PulseAudio skip; 310 regular Rust tests passed, six intentionally ignored; explicit native worker/cue lifecycle tests passed (one voice, two call). | HIP developer exclusion is recorded; this does not qualify vendor drivers. |
| Step 9 notifications | Focused Windows notification routing/window/filter fixtures pass. Real WinRT construction and DPAPI encrypted storage, cold restoration, owner/session rejection and replay checks pass without registration or visible alerts. | Actual Windows delivery, click/minimized/cold-start routing and removal; production permissions and account flows. |
| Step 10 capture | Native WGC bridge compilation, ABI, frame bounds/ownership and real BGRA/appsrc/VP8 resize/restart checks pass. | System picker consent/cancel, selected source, second client, DPI/source closure and physical cleanup. `screen_capture` remains false; preview is explicit. |
| Step 11 desktop | Thirteen focused source GUI cases pass: desktop policy, login, multiline, edit mentions, search recycling, account settings, voice controls, icons, status/viewer, three notification cases and voice text chat. Timeline controls and corrected rich-width/backend-GIF regressions also pass. | Real keyboard events, dead keys, IME, Narrator, file-dialog interaction and fractional/mixed-monitor DPI. The SendInput helper refused a lost foreground window and sent no input; that attempt is not a pass. |
| Step 12 packaged runtime | Development ZIP built with 7,890 manifest-owned files. Direct clean-environment runtime, WinRT/DPAPI, disposable native storage and AppContainer checks pass. MP4/MOV auto decoding selected `d3d12h264dec`; explicit software decoding also passed, with WebM and WAV/Opus fixtures. | Final updated ZIP's fresh-cache/relocation results and clean-source workflow must be recorded separately. |
| Step 13 installer | Ten Windows installer tests pass, including compiling the real NSIS recipe with Unicode payload paths. NSIS 3.13 is included in developer setup. | No new installation replaces the owner's existing Papo. Current modified-source payloads remain rejected by the installer gate. Clean-account install/upgrade/uninstall and multi-machine beta remain pending. |
| Step 14 release | Signing/publication gates remain enforced. Public metadata reports zero accessible current-user code-signing certificates with private keys; no private key was read or exported. | Signing certificate, committed/tested source, prior acceptance and supported OS/device beta matrix. No supported release was published. |

## Corrections found in this run

- Packaged verification now removes developer plugin exclusions, alternate Python
  paths and preview capability overrides before invoking the native bootstrap.
  Renderer/accessibility selections used by CI remain explicit.
- Rebuilding a populated Windows stage now handles copied read-only zoneinfo
  data. Cleanup verifies containment and rejects links/reparse points; genuine
  locks/ACL failures still stop the build. All 18 focused packaging regressions
  pass, including read-only cleanup and refusal to remove outside directories.
- Native-input capture can reuse exact cached package archives and signatures,
  then requires ordinary pacman validation and exact installed/archive equality.
  A mislabeled archive fails; local backports cannot be replaced by stock cache
  entries. The native snapshot contains 381 packages with corresponding local
  GTK/GStreamer sources and recipes.
- Fresh discovery under an accented executable path exposed a GStreamer ANSI
  path/UTF-8 command mismatch. The original package failed the isolated native
  regression; the local Unicode-path correction passes it. See the
  [retained recipe and rationale](../../packaging/windows/gstreamer/README.md).
- GIF tests now embed their WebM fixture, so an unrelated working directory does
  not require a checkout. The rich-card wrapping fixture now exceeds the narrow
  Windows text width while still fitting the wide card; both assertions remain.
  Focused `backend-gifs` and `rich-width` checks pass.
- The native fullscreen viewer fixture no longer assumes its 1800-pixel original
  exceeds every monitor's width. It checks 100% fit/overflow against the actual
  viewport, then establishes zoom overflow before testing panning. The original
  packaged attempt and subsequent replay results remain separate evidence.
- ZIP-only replay reproduced the inline emoji pause assertion. The fixture now
  waits for two after-paint frames, proves allocated clipping bounds and a stopped
  playback timer, then samples unchanged frames for 400ms and verifies scrolling
  back resumes them. Focused Cairo/default replays pass. This changes test sampling
  only; original VM qualification and visual/font acceptance remain separate.
- Test-only stage markers and `timeline-controls`/`rich-width` case selectors make
  native failures attributable to bounded fixtures without changing application
  behavior.

## Open automatic investigations

The broad `default` GUI fixture has previously exited with native access
violation `0xC0000005` after the integrated action fixture, during timeline history.
The isolated timeline fixture passes. Debugger replays instead exposed the
working-directory and font-dependent fixture defects above; neither establishes
the native crash's cause. Preserve the matching executable and failed logs.
After the fixture/native corrections, one complete unchanged automatic replay
passed in 337.67 seconds. This is retained as a pass for that execution, not proof
that an intermittent native fault is resolved. Profile GTK measurement/font
warnings remain unsuppressed. Do not classify this
as the older Linux GstPlay failure or infer a root cause from the last warning.

The first clean-environment ZIP replay failed without diagnostics, then passed
directly and through PowerShell with a warm cache. Relocation and three fresh
caches reproduced the scanner command error described above. Those failures are
retained and require replay against the corrected package; warm-cache success
alone does not close them.

With the corrected core, full-plugin checks pass before/after relocation and
with three fresh caches in a shorter accented extraction directory, without
excluding HIP. A deeper accented extraction root still causes several GPU plugin
scanner processes to terminate and loses `mfvideosrc`. The scanner command error
is gone there; the remaining plugin failures need a matching native fault
diagnosis. Keep the ZIP in a short writable folder for now. Short-root success
does not qualify the deeper path, and system long-path settings were not changed.
The bounded scanner-tree debugger captured native exception `0xC0000409`,
parameter 5, inside installed AMD `amdxx64.dll` (9.17.11.0082), `amdxc64.dll`
and `amdhip64_6.dll` (10.0.3652.0). Its log and minidump are under
`target/windows-dev/remaining/trace-tree/`. These native failure sites are useful
diagnostic evidence, not a complete driver/caller root-cause analysis or a fix.

Machine-local evidence is under `target/windows-dev/remaining/` and
`target/windows-dev/{python-tests.json,rust-tests.log,native-voice.log,native-call.log}`.
The development ZIP and build/runtime manifests are under `dist/`. These ignored
outputs are not a published candidate or a replacement for the historical CI
evidence. Final artifact hashes and subsequent packaged results are retained in
`target/windows-dev/remaining/verification.json` after packaging finishes, so the
record does not mutate the source snapshot recorded inside the ZIP.
