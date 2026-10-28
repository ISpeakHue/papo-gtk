# Step 4 Windows desktop identity acceptance

**Result: PASS**, 2026-10-22 15:08:30–15:11:18 UTC. This records the identity and
activation gate in [step 4](../../../../MULTIPLATFORM_PLAN.md), not a supported
Windows release. See the [implementation contract](../../windows-identity.md).

## Package and source

- [Windows/macOS CI run 39703346154](https://github.com/ISpeakHue/papo-gtk/actions/runs/39703346154)
  passed all three jobs: Windows native compilation/tests/packaging, independent
  ZIP-only runtime validation, and macOS foundation compilation/test linking.
  [Selected log lines](native-ci-excerpt.txt) retain test counts.
- Artifact: `Papo-1.0.2-windows-x86_64-dev.zip`, SHA-256
  `d151f623bef5abd992d636ca6fcb386efd4a1c801cc48719facfd1e86640dca9`.
- Clean source: `a374e67743ab6211e0522c2b5b4d785d453c9c33` on the temporary CI
  validation branch. [Package verification](PACKAGE-VERIFICATION.json) checked
  7,828 runtime files. [Source comparison](SOURCE-COMPARISON.json) confirms the
  current native application/package inputs match that commit byte for byte.
- The final desktop test driver was refined after the CI build. Its staged input
  hashes are in [IDENTITY-INPUTS.json](IDENTITY-INPUTS.json); both staged PowerShell
  scripts match the current source after removing the UTF-8 BOM. No application
  binary changed for these test-driver corrections.

## Guest and isolation

The offline QEMU/KVM guest used Windows 11 Enterprise Evaluation 26H2 x64, build
26300.9457, a 1280×800 display, 6 GiB RAM and four vCPUs. `PapoTester` was a standard
user. No SDK, MSYS2, development tools, native Papo dependencies, guest agent,
network adapter or host directory were supplied. The package was extracted under
a fresh path containing spaces and `áéç`.

[VM-BASE-INPUTS.json](VM-BASE-INPUTS.json) describes the reused step 3 installation,
including its **older** package hash; that package is not the artifact tested here.
[IDENTITY-INPUTS.json](IDENTITY-INPUTS.json) and the report identify this run's
updated read-only FAT input and package. [QEMU-COMMAND.json](QEMU-COMMAND.json)
records startup; [QMP-ADDITIONS.json](QMP-ADDITIONS.json) records the updated input
and USB keyboard hot-plugs. The legacy guest keyboard stopped accepting input;
adding a virtual USB keyboard restored it. The harness now includes one at boot.

The evaluation watermark remains visible; no activation bypass was used. One
stale, protected startup entry from the original disposable test fixture was
removed with the fixture administrator before this run. Papo itself, desktop
registration and every acceptance command ran as the standard account without
elevation. The preparation script now grants that account removal rights to only
the fixture's one-shot startup file.

## Checked behavior

[identity.json](identity.json) reports `status=pass`, child exit code
[0](identity-child-exit.txt), registration removal and unchanged machine PATH.
[stages.txt](stages.txt) records completion through cleanup and report saving.

| Check | Observed result |
| --- | --- |
| Executable resources | All four native files have version 1.0.2.0, icon resources and `asInvoker`/system-DPI manifests. GUI files use subsystem 2; deliberate CLI helpers use subsystem 3. |
| Normal GUI launch | No console, system DPI awareness, 96 DPI; native Papo taskbar icon. |
| Shell identity | Correct Papo name/icon in Explorer, Start search and notification history; matching shortcut/AUMID/protocol/stub CLSID. |
| Shortcut and protocol | Existing process 600 reused; minimized window restored. |
| Warm notification click | Process 600 reused; GTK delivered count advanced to 4; no console. |
| Cold notification click | New process 1424; GTK delivered count 1; no console. |
| Account policy | Offline probe passed current-account/session binding, expiry, access denial, logout and bounded-ticket rules. No real login or message access. |
| Cleanup | Own test toast and registration removed; no machine PATH change. Guest shut down normally before host results extraction. |

The operator inspected the native notification history and sent actual mouse
clicks through QMP. [PHYSICAL-CLICKS.jsonl](PHYSICAL-CLICKS.jsonl) records coordinates,
timestamps and corresponding [warm](history-warm.png)/[cold](history-cold.png)
screenshots. IPC delivery and process state independently confirm activation.
The optional UI Automation click path was not used for acceptance because this
guest's shell accessibility names were unreliable.

Inspected captures: [normal window/taskbar](normal-window.png),
[warm activation](warm-toast-activated.png), [cold activation](cold-toast-activated.png),
[Explorer](explorer.png), and [Start search](start-menu.png). The normal capture
includes the already-open notification panel. PowerShell windows in other captures
belong to the explicit test driver; the application reports `console=false`.

The [runtime probe](cli-1.stdout.txt), [account policy probe](cli-2.stdout.txt) and
[desktop registration probe](cli-4.stdout.txt) are retained. Application
[stderr](application.stderr.txt) contains an optional GLib session-D-Bus warning;
native single-instance activation works independently of it. The driver log was
empty. Earlier attempts exposed fixture issues with PowerShell aliases, argument
binding, UTF-8 encoding, icon handle counts and provider metadata during JSON
serialization. They are not counted as successful runs; this report is the final
completed run using the corrected driver.

## Decision and remaining scope

Select **NSIS per-user** for step 13: the tested unpackaged protocol-only/stub-CLSID
registration supports notification history and warm/cold click activation under
UCRT64 without MSIX, a COM server or administrator privileges. Installer upgrade,
rollback and uninstall acceptance remain step 13.

Production Windows notifications remain disabled until step 9 implements trusted
ticket creation/restoration and preferences/account routing. Unknown cold-start
tickets only open the login/current window. This fixture does not qualify Windows
Home/Pro, real hardware, mixed-DPI monitors, IME, device/media acceptance, signing
or a supported installer. macOS has compile/link groundwork only.

`SHA256SUMS` covers the retained evidence bytes. ISO, disk/TPM images, fixture
passwords and repetitive activation polling logs are intentionally not included.
