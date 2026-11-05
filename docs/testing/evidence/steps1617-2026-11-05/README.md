# Steps 16–17 validation — 2026-11-05

The interrupted implementation left this directory empty. Its original raw logs
and three optimized baseline runs were recovered from `target/steps1617-*`.
The source fingerprint in those runs matched the working tree before this
continuation. The [machine-readable record](checks.json) separates recovered
results from the final checks and records source/binary identities.

The implementation includes saved accounts and independent live sessions,
account/conversation restoration, unread/call indicators, regression budgets,
and artifact-bound acceptance receipts. The final continuation corrected:

- A media-browser status label being appended while still parented to the old
  loading layout. Windows' fatal GTK critical policy aborted here. Still-image
  and animated-GIF viewers now move the label explicitly; their workflow covers
  both paths and waits for the replacement viewer to finish loading.
- Image-viewer titlebar setup after the browser window had been realized.
- Old aggregate tests rejecting the new workspace summary/preference outputs
  instead of checking that failures preserve login.
- A notification fixture resetting its state without its account owner.
- A reduced-motion fixture still expecting an animation object.
- The conversation-switcher focus fixture running without a window manager and
  checking focus before native activation settled.
- The disk-cache eviction fixture assuming rapid writes have distinct timestamps.
  Windows CI exposed tied clock ticks; the fixture now seeds distinct past
  modification times and still verifies that a real cache hit refreshes recency.
  Cache behavior and limits are unchanged.

The Windows workflow now prints each looped case name so native aborts identify
their fixture. Its retained GTK dependency source was refreshed to the first
continuation run; complete inventory and recipe checks still decide whether
reuse is allowed, and no test result is reused. The working branch remains uncommitted. The corrected source was
submitted on an isolated CI branch; no release or merge was performed.

Fresh cross-platform validation is [run 42100245205](https://github.com/ISpeakHue/papo-gtk/actions/runs/42100245205)
on commit `5c255b1196714c6635f37a3cc14c677f31c30144`, branch
`ci/finish-plan-20261105-73a7e4a`. The local snapshot's implementation fingerprint
and both workflow files match the tested checkout. The earlier
[interrupted run](https://github.com/ISpeakHue/papo-gtk/actions/runs/42076952395)
finished with failures; it was not accepted as passing Windows evidence. The
[first continuation run](https://github.com/ISpeakHue/papo-gtk/actions/runs/42096295223)
then exposed the timestamp-dependent Windows fixture before the final correction.
That run passed **57/58 Linux GTK cases**: its aggregate `default` case passed
in 310.09 seconds; only `recent-media` failed with native SIGSEGV. Its
[case receipt](linux-ci-first-results.tsv) is retained separately from the local
56/58 result.

## Automated evidence

- Regular Rust suite: **347 passed**, zero failed, six ignored.
- Python suite: **181 tests**, zero failed, seven platform-dependent skips;
  implementation-tool checks: **15 passed**; translations: **1,010 matching entries**.
- All-targets compilation, shell syntax and diff whitespace checks passed.
- Full local GTK run with fatal critical checks: **56/58 passed**. `default` and
  `recent-media` exited with SIGSEGV in the previously recorded native playback
  path. The [retained stack](native-playback.txt) matches `gst_play_main` →
  `g_object_unref` → `g_type_check_instance_is_fundamentally_a`; this is not a
  complete root-cause proof. Neither failure is counted as a pass.
- Three worker/call cleanup tests and both direct ICE/TURN decoded-audio paths,
  including synthetic live microphone changes, passed.
- Three fresh optimized baseline runs pass the existing timing, request, cache,
  row and process-memory budgets without relaxing limits.

Final measurements and budgets are retained as [baseline.json](baseline.json)
and [budgets.json](budgets.json). The optimized compiler artifact and executable
SHA-256 are recorded separately: using `--binary` intentionally keeps
`binary_source_verified: false`, and the release profile is an explicit operator
assertion. The native-display label describes the display inherited by the
runner; these local measurements used private Fedora/Xvfb/Openbox, not the user's
desktop.

Full GUI and native playback results, source-matched CI status, and any remaining
failures are recorded in `checks.json`; a passing focused case does not erase an
aggregate/native playback failure. Raw logs remain under `target/resume-*`, with
hashes and actual test-result lines retained in the record.

## Acceptance boundary

[acceptance-receipt.json](acceptance-receipt.json) and
[acceptance.json](acceptance.json) retain the exact source identity and pending
physical Linux checks. Successful source-matched Windows automation can satisfy
the user's Windows waiver, as documented in the
[acceptance guide](../../release-acceptance.md). A failed/incomplete workflow
cannot satisfy it. Windows remains experimental.

No physical microphone/output/camera/screen permissions, real screen-reader
speech, installed AppImage/Flatpak upgrade or rollback is inferred from synthetic
tests. Those operator checks remain pending. No stable release is certified by
this evidence bundle.
