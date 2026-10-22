# Step 5 Windows storage and file acceptance

**Result: PASS**, 2026-10-22 17:15:14–17:15:46 UTC. This completes the storage/files
gate in [step 5](../../../../MULTIPLATFORM_PLAN.md). Windows remains a preview;
the [implementation contract](../../windows-storage.md) explains the adapters and
how to rerun the disposable checks.

## Package and source

- [CI run 39809350784](https://github.com/ISpeakHue/papo-gtk/actions/runs/39809350784)
  passed all three jobs: Windows native compilation/tests/package/storage probe,
  independent ZIP-only runtime acceptance, and macOS compilation/test linking.
  [CI conclusions](CI-RESULT.json) and [selected log lines](native-ci-excerpt.txt)
  retain the results. Windows checks include 24 packaging tests, 14 platform tests,
  synthetic voice, icon/startup/composer-clipboard GUI cases, 10 session tests
  (one opt-in test ignored) and four upload-validation tests.
- Clean source: `23fd2b7e00b6966c299c365f2b3554c4b882012c`, validated through the
  temporary `ci/windows-storage-step5` branch. [Source comparison](SOURCE-COMPARISON.json)
  verifies 188 current native application/package inputs byte for byte against
  that checkout. Documentation and the desktop fixture are recorded separately.
- ZIP: `Papo-1.0.2-windows-x86_64-dev.zip`, SHA-256
  `af5094352341ae9c1da611f608dd7f1e920e238a4514b905071bc5446b81f794`.
  [Package verification](PACKAGE-VERIFICATION.json) checked all 7,828 manifest
  files by size and hash, with manifest SHA-256
  `533b3cf39467b7453b335599325826410d380ac885a578e27b626634100f4043`.

## Guest and isolation

Windows 11 Enterprise Evaluation 26H2 x64, build **26300.9457**, ran as the
standard `PapoTester` account in the reused offline QEMU/KVM fixture: four vCPUs,
6 GiB RAM, 1280×800 display. No MSYS2, Rust, SDK, Python, GStreamer, guest agent,
network adapter or host directory was supplied. Cairo software rendering was
selected for this storage test. The expired evaluation watermark was not bypassed.

[VM-BASE-INPUTS.json](VM-BASE-INPUTS.json) describes the original step 3 OS install
and its **older** package; that package was not tested here. This run used a fresh
results disk and updated read-only FAT input, identified in
[STORAGE-INPUTS.json](STORAGE-INPUTS.json). Staged PowerShell files have a UTF-8 BOM
for Windows PowerShell 5 and match current source after BOM removal.
[QEMU-COMMAND.json](QEMU-COMMAND.json) and [QMP-ADDITIONS.json](QMP-ADDITIONS.json)
record the guest devices and read-only input attachment. A virtual USB keyboard
was present at boot. Fixture login and launching the test used QMP input only.

The package was extracted into a fresh directory containing spaces and `áéç`,
then launched from the Windows directory. The explicit storage command used only
randomly namespaced disposable Credential Manager entries, temporary preferences
and files, a synthetic clipboard texture and a loopback HTTP fixture. Existing
settings, real saved sessions and other users' credentials were not read or
enumerated. Ordinary Rust tests continue to use memory-only credential storage.

## Observed results

[storage.json](storage.json) records `status=pass`, native process exit code 0,
standard-user identity, package provenance and extracted-package cleanup.
[storage.stdout.json](storage.stdout.json) is the unmodified native assertion
report; [storage-child-exit.txt](storage-child-exit.txt) is 0.

| Check | Result |
| --- | --- |
| Native remembered sessions | Fifteen child-process phases passed: save/restart/resume/rotation, logout/empty restart, revocation at whoami and refresh, transient offline failure, session ownership, Remember opt-out and safe unavailable-store behavior. Six loopback HTTP requests; no real backend. |
| Native credential format | UTF-8 blob, larger ASCII session, long account key and real oversized-write rejection passed. Oversized failure retained the previous entry and did not expose its value. |
| Account identity and migration | Case-distinct account targets, UTF-16 legacy read/migration, rejection of a colliding legacy account, and malformed-legacy ownership handling passed. Probe entries were removed. |
| Unavailable store | Scoped injection failed safely without persistence or a plaintext fallback. This does not claim an OS-wide credential service outage was induced. |
| Native directories/config | RoamingAppData config and LocalAppData cache lookup passed. Preferences were round-tripped only under temporary scratch; atomic replacement and locked-config preservation passed. |
| Downloads | Unicode path of 345 UTF-16 units, replacement, locked-destination preservation and reserved/alternate-stream filename rules passed. |
| Cancellation and ownership | Cancelled staged copy preserved the old destination and removed staging/source files. Clipboard upload clones retained the file until the final owner dropped. |
| Native desktop | Synthetic clipboard texture round-trip and an actual GTK save-dialog cancellation passed without creating an output file. |
| Cleanup | Scratch directory, probe credentials and extracted package removed; guest shut down normally before host extraction of the results. |

[storage.stderr.txt](storage.stderr.txt) retains one nonfatal GTK warning while
the cancelled native save dialog enumerated a missing shell shortcut. The dialog
still returned the expected cancellation error, and all native assertions passed.
The driver log was empty. No diagnostic contained credential payloads.

The earlier interrupted attempt produced no report and is not counted as a pass.
This is the completed run with normal guest shutdown. Linux all-target compilation
and focused platform/session/upload/authenticated-download checks also passed;
native Windows CI supplies the GUI clipboard/upload regression evidence.

## Remaining scope

This validates step 5 on the recorded standard-user Enterprise Evaluation VM and
hosted Windows runner. It does not qualify the newer Home/Pro release rows or
real-backend, input-method, physical-device, media-isolation or installer flows.
Unavailable-store behavior is explicitly failure-injected; native capacity failure
is tested separately. macOS remains compile/link groundwork.

`SHA256SUMS` covers the retained evidence bytes. ISO, VM/TPM images, fixture
passwords, clipboard contents and packaged binaries are not included here.
