# Steps 16–17 validation — 2026-11-05

The interrupted implementation left this directory's four original files empty.
Its raw logs and optimized baseline runs were recovered from `target/steps1617-*`;
the recorded source fingerprint matched the working tree before continuation.
The [machine-readable record](checks.json) keeps recovered, historical and final
results separate, with source identities, executable hashes and log receipts.

The planned features and regression tooling were already implemented. This
continuation repaired issues exposed by broader and native validation:

- The media browser appended a status label while it still belonged to the old
  loading layout. Windows' fatal GTK critical policy aborted here. Image/GIF
  viewers now move it explicitly and install their titlebar before realization.
  The workflow covers still images and animated GIFs, including next-viewer load.
- Keyboard context-menu requests were deferred on replaceable toolbar widgets.
  Requests now resolve the current message row/menu by message identity after
  layout. The new regression replaces either the toolbar or the whole row while
  opening is queued: the original code failed; the corrected code passes both.
  The accessibility fixture also follows the current row after reconciliation.
- Stale fixtures rejected workspace/preference outputs, reset notifications
  without their account owner, expected an animation under reduced motion, or
  asserted focus before native activation settled. Focus-dependent Linux cases
  now use a private window manager.
- The Windows cache-eviction fixture assumed rapid writes had distinct timestamps.
  It now seeds distinct past modification times while still checking that a real
  cache hit refreshes recency. Runtime cache behavior and limits are unchanged.
- Windows GUI cases now run as exact, bounded test processes with uncaptured Rust
  output, per-case logs/JSON and process-tree cleanup. Zero-test output, nonzero
  exits and timeouts cannot pass. Four subprocess tests exercise those gates.
  Account-restart checkpoints identify any future stall. Verified GTK dependency
  bytes can be reused only when the full inventory and recipes match; test
  results are always rerun.

The working branch remains uncommitted. Source snapshots were pushed only to
`ci/finish-plan-20261105-73a7e4a`; no merge, release or publication was performed.
Final native validation is [run 42123069510](https://github.com/ISpeakHue/papo-gtk/actions/runs/42123069510),
commit `c847ed119fbe1974ea2232144912acb47d68ff1c`. Its actual result is retained
in [ci-run.json](ci-run.json) and `checks.json`; an incomplete/failed run is not
accepted as successful Windows evidence.

## Implementation handoff

The user explicitly permitted ignoring the remaining Windows error on 2026-11-05
and requested that further retry work stop. The final Windows run failed before
application compilation/testing because the MSYS2 mirror timed out downloading a
`libwmf` package signature. No further retry is required for this implementation
handoff, and that failure is **not** recorded as a passing Windows check.
macOS passed; Linux units and both performance checks passed, with its full GTK
suite still running at handoff. The final source's local checks below all pass.
Full release qualification, physical acceptance and the recorded intermittent
native playback risk remain separate and unresolved.

## Local final-source checks

- Rust: **347 passed**, zero failed, six ignored; all-targets compilation clean.
- Python: **185 tests**, zero failed, seven platform-dependent skips; **19**
  implementation-tool tests pass; **1,010** matching translation entries.
- Nine focused GTK cases pass with fatal critical checks: accessibility, high
  contrast, native Linux AT-SPI, account restart, media browser, report8, report9,
  clipboard and pinned media. The accessibility cases include both new context
  replacement regressions. Native reader speech is not inferred from AT-SPI.
- Three optimized performance runs are retained in [baseline.json](baseline.json),
  with the unchanged regression limits evaluated in [budgets.json](budgets.json).
- Earlier in this continuation, three native worker/call cleanup tests and both
  direct ICE/TURN audio paths passed, including synthetic live microphone changes.
  Their exact source scope remains recorded separately in `checks.json`.

Using `--binary` intentionally retains `binary_source_verified: false`; the
release profile is explicitly asserted and the compiler artifact/executable hash
is recorded separately. The baseline's inherited `native` display label refers
to a private Fedora/Xvfb/Openbox session, not the user's physical desktop.

## Failed attempts and remaining runtime risk

The [original remote run](https://github.com/ISpeakHue/papo-gtk/actions/runs/42076952395)
failed on the media-browser GTK parent assertion and stale Linux fixtures.
[The first continuation](https://github.com/ISpeakHue/papo-gtk/actions/runs/42096295223)
exposed the timestamp-dependent Windows fixture and passed **57/58 Linux GTK
cases**, failing only `recent-media` with SIGSEGV. Its [receipt](linux-ci-first-results.tsv)
records that the aggregate `default` case passed in 310.09 seconds.

[The next run](https://github.com/ISpeakHue/papo-gtk/actions/runs/42100245205)
passed **all 58 Linux GTK cases** and macOS. Windows passed its Rust/lifecycle and
structural checks, then stalled in `multi-account-restart`; the run was cancelled
after more than twelve minutes to obtain diagnostics.
[The bounded rerun](https://github.com/ISpeakHue/papo-gtk/actions/runs/42105865219)
passed account restart in about two seconds, all Linux GTK cases and macOS, then
identified the Windows accessibility context-menu failure addressed above.
The earlier restart stall has not been reproduced; bounded execution now retains
its checkpoint/output and stops its process tree if it occurs again.

The full **local** GTK run earlier passed **56/58**, with native playback SIGSEGVs
in `default` and `recent-media`. The [retained native stack](native-playback.txt)
matches earlier evidence (`gst_play_main`, `g_object_unref`,
`g_type_check_instance_is_fundamentally_a`). Simpler standalone probes did not
reproduce the problem; legacy playbin also crashed. This is an intermittent,
unresolved playback risk. Later successful CI runs do not establish a root cause
or erase those failures. No speculative playback workaround was enabled.

Raw logs remain under `target/resume-*`; retained result lines and hashes bind
the summarized outcomes. GTK/native suites from before the final context-menu
fix keep their own source/binary scope.

## Physical acceptance

[acceptance-receipt.json](acceptance-receipt.json) and [acceptance.json](acceptance.json)
retain source-bound acceptance status. The user's Windows waiver requires
successful source-matched automation, as described in the
[acceptance guide](../../release-acceptance.md). Windows remains experimental.

Physical Linux audio/camera/screen permissions, real screen-reader speech and
installed AppImage/Flatpak upgrade/rollback checks remain pending. Synthetic tests
and automated receipts do not establish those outcomes or certify a stable release.
