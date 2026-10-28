# Clean Windows runtime acceptance — 2026-10-21

**PASS for implementation-plan step 3.** This is portable-runtime assembly and
startup acceptance, not a supported Windows release or full feature acceptance.

## Artifact and environment

- Source: `b5566289e586c9047b5765271d8ed1d4014a6fea`, clean checkout.
- [Native build and separate ZIP-only acceptance](https://github.com/ISpeakHue/papo-gtk/actions/runs/39666153467): passed, including all 21 native packaging tests.
- Artifact: `Papo-1.0.2-windows-x86_64-dev.zip`.
- ZIP SHA-256: `aa8a4e10f1bae62a74d022ce125099c588cec0770ef3e7ea99a4d04ed62078ef`.
- Runtime manifest SHA-256: `f05833e598867363b42e605ac96418b05835f2238bb27f915880048cd5bee0da`.
- Observed guest: Windows 11 Enterprise Evaluation, 26H2, native x64,
  **10.0.26300.9457**. Installed KBs are in [acceptance.json](acceptance.json).
  The older shared-base `BuildLabEx`/desktop watermark is also preserved; the
  version/build/UBR above come from the guest's OS and registry queries.
- QEMU 10.2.2/KVM, four CPUs, 6 GiB RAM, sparse 80 GiB guest disk,
  enrolled Secure Boot firmware and TPM 2.0 CRB. Display: 1280×800, Cairo software
  rendering. No network adapter, host filesystem share or guest agent.
- Standard local fixture account `PapoTester`, outside Administrators. No MSYS2,
  Rust, system Python, separately installed GTK or GStreamer. The two zero-byte
  Windows Store Python aliases in the report are not interpreters.

The official ISO came from the [Microsoft Evaluation Center](https://www.microsoft.com/en-us/evalcenter/download-windows-11-enterprise),
with SHA-256 `bc3f24086ebadc94489066b5ad78089e2cf5c3491e90e790bb81a2b199c10e38`
checked against [Microsoft's published hashes](https://support.microsoft.com/en-us/servicing/os/windows/docs/2026/09/verify-the-authenticity-of-a-windows-11-enterprise-evaluation-iso-file).

## Observed results

[acceptance.json](acceptance.json) records `status: pass` from
20:10:58–20:12:20 UTC. [Runtime output](runtime.stdout.txt) records both probes:

- Python 3.14.8, GTK 4.24.1, GStreamer 1.28.8; all 7,824 runtime files verified.
- Four offline media fixtures decoded. All 47 SVG controls rendered under three
  theme names and two scales.
- Package paths contain `á é ç` and spaces. The second probe runs after moving
  the package to a different directory; its per-user registry location changes.
- Unrelated working directory and restricted developer discovery paths.
- Missing bundled Python and GTK rejected; all original files restored and their
  hashes unchanged. No package-local bytecode or plugin registry generated.
- No change to machine-wide PATH.
- Normal `Papo.exe` launch from the Windows directory displayed the login window.
  [Screenshot](application.png) was inspected; closing its GTK window ended the
  application and bootstrap successfully. No Papo session or backend was used.

[Runtime stderr](runtime.stderr.txt) contains the expected missing-Python error
from the negative test. [Application stderr](application.stderr.txt) retains the
nonfatal GLib warning about an unavailable Win32 session D-Bus binary. Startup
and shutdown passed despite it; native application identity/single-instance
behavior remains step 4. The screenshot is startup evidence, not complete window
sizing, DPI or input acceptance.

## Test sequence and provenance

The [reusable harness](../../../../packaging/windows/clean-vm/README.md) created
the new Windows guest from the ISO. Earlier disposable installation attempts
exposed fixture issues: firmware/TPM presentation, partition modification order,
removable drive-letter assignment and an accented unattended account name. Those
failed installations are not counted as passing evidence; the harness fixes are
covered by its provisioning tests.

The successful OS installation was retained while correcting the test driver and
probe. Only Papo ZIPs and test scripts were extracted in it; no development tools
or external runtimes were added. The first archive's [audit failure](initial-audit-failure.txt)
identified Microsoft HEVC/VP9 extensions outside the Windows directory. The
revised probe checks their registered package family, publisher, signature kind,
status and installation location. Its final relocated probe records exactly
those two OS modules; [registered codec metadata](os-codecs.json) is preserved.
Other external runtime DLLs remain rejected.

[installation-inputs.json](installation-inputs.json) records the original ISO,
firmware and initial archive. Its archive hash is intentionally different from
the rebuilt ZIP in [final-inputs.json](final-inputs.json). The latter identifies
the final read-only FAT input disk and exact script/archive hashes. The
[fixture rerun script](fixture-rerun.ps1.txt) stopped only this fixture's obsolete
staged PowerShell test attempts and invoked the revised acceptance script as the
same standard account. It installed nothing. A fresh results disk prevented
earlier logs from being mistaken for the final result.

[qemu-command.json](qemu-command.json) records the final resumed guest command.
The final input disk was attached through `control.py --input-disk` as a read-only
raw block device named `papo_fixture_update`, using USB storage on `usb.0`.
[Installation results attachment](installation-results-attachment.json) records
the delayed results disk insertion during the original installation. Workspace
prefixes in host paths are replaced with `${WORKSPACE}`; other evidence is copied
unchanged. Generated ISOs, VM disks, TPM state and answer files stay in ignored
`target/`. Windows shut down normally before the results disk was read on the host.

## Limits

This Enterprise evaluation edition and servicing revision do not qualify the
Home/Pro release rows or their newer `.9711` baselines. Real devices, hardware
acceleration, live accounts/backends, Windows credential operations, native
notifications, sandboxed converters, capture, installers, input and DPI remain
subject to the later port gates. The evaluation watermark is visible; no
activation or licensing workaround was applied.

Use the current harness with the exact archive/checksum to reproduce this runtime
test on another fresh guest. [SHA256SUMS](SHA256SUMS) covers the retained evidence.
