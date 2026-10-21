# Clean Windows VM runtime acceptance

This harness installs a disposable Windows guest from an official ISO and runs the
portable ZIP as a standard local account. It adds no MSYS2, Python, Rust, GTK or
GStreamer installation to the guest. Only the application's bundled runtime is
extracted. The guest has no network interface, host filesystem mount or guest agent.

Host requirements: Linux with accessible KVM, QEMU, swtpm, xorriso, mtools,
dosfstools and an OVMF firmware descriptor providing Secure Boot/SMM with Microsoft
keys enrolled. The defaults use Fedora's current 4 MiB OVMF descriptor and TPM 2.0
CRB interface. The scripts do not install or reconfigure host services. Reserve
about 6 GiB RAM and enough disk space for the ISO plus a sparse 80 GiB guest disk.

## Prepare and run

Obtain an ISO from the [Microsoft Evaluation Center](https://www.microsoft.com/en-us/evalcenter/download-windows-11-enterprise)
and verify its hash against [Microsoft's published values](https://support.microsoft.com/en-us/servicing/os/windows/docs/2026/09/verify-the-authenticity-of-a-windows-11-enterprise-evaluation-iso-file).
Use the archive's adjacent `.zip.sha256` too. The evaluation installs in the
new guest disk, with fixture-only local accounts. Nothing is installed on the host.

```sh
python3 packaging/windows/clean-vm/prepare.py \
  --iso target/windows-clean-vm/windows11.iso \
  --iso-sha256 <official-sha256> \
  --archive dist/Papo-1.0.2-windows-x86_64-dev.zip \
  --directory target/windows-clean-vm/acceptance

python3 packaging/windows/clean-vm/run.py \
  --directory target/windows-clean-vm/acceptance --install
```

Preparation refuses to overwrite an existing `os.qcow2`. Select a fresh directory
for another clean installation. QEMU opens only task-owned disk images, read-only
installation/input ISOs and firmware; it receives no host physical disk. The
unattended XML partitions only guest disk 0. Partition modification orders are
consecutive, as required by Windows Setup. The results disk is attached only after
Windows has begun writing its own volume, preventing removable storage from
claiming `C:` before setup. A resumed, already-installed guest can omit `--install`.

The run stays active until Windows shuts down. Installation can take several
minutes and reboot repeatedly. Inspect the guest without opening a host desktop
window:

```sh
python3 packaging/windows/clean-vm/control.py \
  --screenshot target/windows-clean-vm/acceptance/progress.png
```

The initial DVD boot prompt is normally handled by `--install`; if it remains
visible, send `--key spc` after inspecting the screen. If Windows Setup requires
confirmation, inspect its screen before sending a key with `--key ret`.
`--powerdown` sends the guest's ACPI power button. Reserve `--quit` for aborting a
failed disposable installation. Diagnostic commands can be typed into the guest
console from an ASCII file using `--type-file`; this does not execute them on the
host. The QMP and TPM sockets are private and local to the host.

## Acceptance and results

The guest starts the acceptance script from its desktop under the standard
`PapoTester` account, with no Administrators membership. It records OS edition,
full build/UBR, installed updates, account/profile, network adapters, application
inventory and runtime discovery commands before extracting the ZIP. It rejects
external development/runtime installations. It then runs the same portable
relocation/integrity/missing-component checks as CI, from an unrelated working
directory, and opens/closes the normal application while recording a screenshot.
The work directory is kept short to avoid turning this runtime check into the
separate long-path acceptance planned for step 5. The package paths themselves
include spaces and accented characters. No Papo account, credentials or backend
is used.

After the test writes `acceptance.json`, logs and `application.png`, Windows
shuts down to flush the results volume. Only then extract it on the host:

```sh
mkdir -p target/windows-clean-vm/evidence
mcopy -s -i target/windows-clean-vm/acceptance/results.img \
  ::/results target/windows-clean-vm/evidence/
```

Require `status: pass`, the expected ZIP/source/runtime-manifest hashes and the
normal-window screenshot. Preserve the input manifest, exact QEMU arguments,
results-attachment record and relevant output with the verification record.
Never count an incomplete installation, a failed test or CI-only result as this
clean-VM pass. Test disks, ISO files and unattended credentials stay in ignored
`target/`; commit only the intentionally selected evidence.

A runtime pass from an Enterprise evaluation guest does not qualify the Home/Pro
release matrix, other servicing revisions, real devices or hardware acceleration.
Those desktop/release checks remain separate steps of the port plan.
