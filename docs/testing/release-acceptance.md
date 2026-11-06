# Regression budgets and release acceptance

Automated checks and physical acceptance have separate results. Passing a
synthetic GUI suite is not proof of native screen-reader speech, audible devices,
portal permission behavior or an installed upgrade. Pending rows must remain
pending; the implementation does not publish or label a release stable.

## Automated sequence

1. Run `cargo check --locked --all-targets`, regular Rust tests and Python tool tests.
2. Run focused GTK workflows: `multi-server`, `multi-account-restart`, `recovery`,
   `outgoing-queue`, `draft-recovery`, `account-scope`, `disk-thumbnails`,
   `media-policies`, `media-preferences`, `notification-controls`, accessibility and
   existing notification/voice/media-browser checks. These jointly cover reconnect,
   pending/uncertain sends, cross-account isolation, cache eviction/deletion and
   permission revocation. All use private synthetic services/configuration.
3. Run three fresh-process optimized baselines:

   ```sh
   python3 scripts/performance-baseline.py --profile release --runs 3 \
     --output target/performance/release.json
   python3 scripts/check-performance-budgets.py target/performance/release.json \
     --output target/performance/budgets.json
   ```

   The [budgets](performance-budgets.json) compare medians with the original three
   Linux runs. Each time limit is reference median × declared ratio + absolute
   slack. Renderer, geometry and CPU architecture must match; timings are enabled
   only for Linux release builds with at least three runs. The initial samples
   describe one machine, so these are deliberately broad synthetic regression
   envelopes, not statistically portable hardware benchmarks. Windows timing
   limits require a measured native reference first.

   Structural checks run on Linux and Windows, including debug CI: eight cold/
   zero warm thumbnail requests, at least 30 scroll frames, at most 202 mounted
   rows, 96 MiB decoded/64 MiB compressed cache checkpoints, and a 1 GiB synthetic
   process-RSS checkpoint envelope. RSS includes native runtime/allocator state;
   this is not a real-session peak-memory or leak guarantee. Missing scenarios,
   missing checkpoints, zero tests, malformed/non-finite measurements and failed
   reports fail the check. Release GTK CPU paint p95 must stay within 16.667 ms;
   this excludes compositor/GPU presentation latency.

   For an already-built test executable, the baseline tool records unverified
   source/profile status. `--assert-profile release` on the checker records an
   explicit operator assertion; it **does not** turn that into source verification.
   Retain the actual compiler artifact/binary hash with local evidence.
4. Keep report/log artifacts and compare regressions before changing a budget.
   Correct a behavioral regression first. Budget changes need measured evidence
   and an explanation, rather than automatic rebaselining.

Windows desktop cases use `python scripts/run-gui-case.py CASE`: one exact GTK
test executable runs with a 240-second bound and uncaptured Rust output. Reports
and logs are retained under `target/windows-build/gui/`; a nonzero exit, timeout
or zero-test result fails the case. Timeout cleanup terminates the test's process
tree. Four real subprocess tests cover success, false success and descendant
cleanup. The account-restart workflow also prints its progress checkpoints.

Linux CI now runs structural debug checks and three optimized timing runs.
Windows CI enforces structural checks and includes the new account workflows;
that does not mark native device acceptance passed. Existing Windows packaging
checks still cover installer rollback/cleanup fixtures and runtime isolation.

## Real desktop and artifact checks

Copy [release-acceptance-template.json](release-acceptance-template.json). Record
an operator, date, OS/desktop/architecture and exact artifact SHA-256 for AppImage,
Flatpak, Windows installer and Windows portable builds. Use controlled test
accounts and a disposable installation/profile for upgrade/rollback tests.
An isolated config directory alone does not isolate a native keyring account.
Never put credentials or private message content in a receipt.

For **each** format record results/evidence for:

- **multi_server:** two servers and two users on one backend; switch with drafts,
  media and an active call, inspect separate unread indicators, disconnect only one.
- **reconnect_pending_permissions:** interrupt one connection while sending; keep
  the other usable; verify no automatic uncertain resend, reauthentication,
  deleted messages and loss/restoration of channel permissions.
- **keyboard_reader_scaling:** keyboard-only login/settings/search/notifications/
  composer/outbox/media/call workflows; focus return, high contrast, large text/
  mixed DPI and reduced motion; Orca on Linux and NVDA/Narrator on Windows.
- **microphone_output:** real input level, explicit record/replay, selected output,
  mute, live device change and device removal; verify audible join/leave cues.
- **direct_turn_call:** two real participants, direct ICE and TURN paths, audio
  after device changes, switch servers without losing the original call, then leave.
- **capture_permission_cleanup:** camera/screen picker, visible local sharing
  preview/indicator, peer output, deny permission, stop sharing and close/quit;
  verify capture is released. Record absent hardware as a limitation, not a
  successful physical capture check.
- **upgrade_and_rollback:** retain a backup; launch previous published artifact,
  create controlled settings/drafts, close, upgrade, verify keyring/account/draft
  retention and new format launch, then exercise the documented rollback path.
  Flatpak must use its actual installation scope/remote. Test both Windows installer
  replacement and a separately extracted portable package.

A row needs `status: pass` and evidence describing an actual execution. A template,
source review, historical CI result or unavailable device is not a passing row.
Validate against the exact source fingerprint in the automated evidence:

```sh
python3 scripts/check-release-acceptance.py path/to/receipt.json \
  --source-fingerprint SHA256 --output target/release-acceptance.json
```

The command exits nonzero with `pending` when a format/check is missing, stale,
failed or lacks evidence. It validates operator-attested records; it does not
install packages, grant permissions, infer certification or replace the existing
[Windows support policy](../../WINDOWS_SUPPORT.md). Windows stays experimental
until its existing qualification requirements are satisfied.


## Windows automation accepted for this implementation

The user explicitly chose automated Windows acceptance on 2026-11-05 and waived
manual Windows qualification for these changes. A successful source-matched
`Windows release checks` run, including its packaged portable/installer checks,
can therefore be supplied with `--windows-ci-report path/to/github-run.json`.
The receipt must include that run's exact `source_commit`; the tool verifies the
repository, workflow name, run URL, commit and successful conclusion. Windows
rows then record `pass-automated`, rather than inventing device/speech results.
Physical Windows checks remain prepared for later and do not block this work.
This does not change the project's Windows experimental support/signing policy.
