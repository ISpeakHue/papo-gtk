# Windows installer and beta — step 13

Prepared 2026-10-28. The per-user installer recipe, CI driver and beta tooling
are implemented. **No Windows installation or attended beta run of this revision
has been performed.** The user deferred Windows desktop testing until a release
candidate is available. Step 13 stays unchecked until clean-machine acceptance
and the beta below pass. Windows remains an unsigned experimental preview;
supported release qualification/signing/publication belongs to step 14.

## Packaging

Build the normal standalone ZIP first, then run from its clean source revision
in UCRT64:

```sh
pacman -S mingw-w64-ucrt-x86_64-nsis
python packaging/windows/installer.py --stage dist/Papo-1.1.0-windows-x86_64-dev --archive dist/Papo-1.1.0-windows-x86_64-dev.zip
```

Use the actual Cargo version. The compiler and its native package inputs are
retained by the Windows workflow. `installer.py` checks the ZIP checksum, every
staged file against the ZIP, clean source identity, executable/runtime hashes,
reserved/unsafe Windows names, case collisions and redirected paths before
generating NSIS input. The compiler runs with warnings treated as errors.
The installer contains the exact standalone files; it does not download a runtime
or rely on a system Python, GStreamer, Rust or MSYS2 installation.

Outputs are `Papo-<version>-windows-x86_64.exe`, its `.exe.sha256` and `.exe.json`
manifest. The filename deliberately avoids NSIS's unsafe setup/install/update
compatibility-shim heuristic. The manifest binds installer SHA, standalone ZIP,
source SHA, BUILD-INFO/runtime hashes, compiler version/hash and recipe hashes.
The original self-contained ZIP/checksum remains available as a separate artifact.
CI uploads these as preview artifacts, without publishing a GitHub release.

## Installation and lifecycle contract

- NSIS uses `RequestExecutionLevel user`, current-user shell folders and 64-bit
  HKCU registry entries. The default root is `%LOCALAPPDATA%\Programs\Papo`.
  There is no elevation request, machine PATH change, shared installation or
  custom destination page. Normal users open the installer or `Papo.exe` without
  a terminal. Its wizard supports English and Brazilian Portuguese.
- Each attempt extracts into a new private `versions/<unique-slot>` directory.
  The existing application files remain available while the candidate runs its
  offline packaged-runtime check. Running Papo blocks activation/removal; users
  close it and retry. Failed extraction/runtime checks leave the active copy
  unchanged. An incomplete slot can remain for inspection, rather than risking
  recursive deletion of untracked files.
- Before changing desktop identity, the installer journals the candidate,
  previous installed slot and previous owned desktop registration in HKCU.
  Only after shell registration and `--desktop-info` succeed does it publish the
  Installed Apps entry and current slot. A failed commit or the next invocation
  after interruption restores the previous registration and Installed Apps
  pointer. `Papo-<version>-windows-x86_64.exe /S /RECOVERONLY` explicitly performs
  only that recovery. An unavailable previous portable copy or conflicting owner
  fails recovery visibly instead of taking over a different registration.
- Shell registration uses the existing ownership-checked adapter: Start menu
  `Papo.lnk`, `br.com.papo.gtk` taskbar/notification identity and the opaque
  activation protocol. The Installed Apps entry points to the matching
  uninstaller. Uninstalling an older slot cannot unregister a newer or separately
  registered portable copy. Downgrades are refused; explicit uninstall is required.
- A successful upgrade keeps one previous complete slot. Older complete versions
  are removed using their own generated inventories. Locked/incomplete copies
  may remain for inspection. No cleanup follows a reparse directory.
- Uninstall checks file sharing/locks and reparse entries before changing the
  active identity, and uses each version's exact file inventory. It does not use
  recursive directory deletion or delete unknown files. A locked copy fails
  without deliberately forcing processes closed or scheduling a machine reboot.
  Unexpected permission/lock races can require retrying uninstall; its entry
  remains until tracked-file removal succeeds.
- Configuration under RoamingAppData, runtime caches/notification state under
  LocalAppData and Windows Credential Manager entries remain outside program
  files. **Uninstall preserves them.** No implicit logout, credential enumeration,
  telemetry upload or data-purge option is added. Account/session removal remains
  an explicit user action in Papo/Windows.

The installer's x64/API checks are not a promise of Windows 10, ARM64 or Server
desktop support. The [Windows 11 Home/Pro support matrix](../../WINDOWS_SUPPORT.md)
remains the release policy. CI's Windows Server runner is synthetic integration
coverage, with its administrator-token status recorded separately.

## Automated installer checks

`installer-runtime` runs on a fresh Windows runner, without installing MSYS2.
Its explicit `-DisposableAccount` driver refuses preexisting Papo installation,
registration, process or data directories. It uses only synthetic markers and
one randomly named disposable Credential Manager entry. It checks:

1. Install, offline runtime/hash checks, GUI shortcut and Installed Apps metadata.
2. Same-version replacement through a different slot, exercising the full upgrade
   transaction rather than overwriting the current files.
3. A stale uninstaller preserving the new registration and an unknown user file.
4. Locked-file uninstall rejection before identity removal.
5. A distinct interrupted-commit journal, explicit recovery to the previous slot,
   and a successful subsequent replacement.
6. Uninstall removing owned registration and tracked program files across slots.
7. Settings/cache markers, the disposable credential and unknown files surviving,
   with machine PATH unchanged.

The fixture journal exercises recovery at a durable checkpoint. It does not
simulate a physical power loss at every instruction. Different-version upgrades,
actual process-running refusal, first-install failure rollback, disk-full/
permission failures and power interruption remain attended beta checks.

Only in an empty disposable Windows account:

```powershell
.\packaging\windows\check-installer.ps1 -Installer .\Papo-1.1.0-windows-x86_64.exe -OutputDirectory .\installer-results -DisposableAccount
```

Keep adjacent `.exe.sha256`/`.exe.json` files. On failure the driver preserves the
program for inspection and removes only its own disposable credential/data
markers; discard the test VM/profile after reviewing evidence. It never inspects
an existing user's session.

The [candidate gate](windows-release-gate.md) now also requires this job, the
source/ZIP-bound installer metadata and all seven `installer-runtime.json` cases.
A passing ZIP report alone cannot qualify its installer. Packaging-only mode
still cannot produce a candidate gate. The macOS and Linux jobs remain required.

## Attended beta matrix — not run

Use the candidate's exact source SHA, installer/ZIP hashes and native package
manifest. Run under a standard user with disposable accounts/content. Record
edition, servicing build, GPU/driver, renderer, audio connection type, keyboard/
IME, display scale and network scenario. Keep private server URLs, message text,
session files, credentials and hardware/account identifiers out of shared reports.

| Area | Required acceptance | Status |
| --- | --- | --- |
| OS matrix | W25H/W25P/W26H/W26P from the support policy; normal per-user install and launch without development tools | NOT RUN |
| Installer | Install, different-version upgrade, same-version replacement, refused downgrade, running-app refusal, stale uninstaller and full uninstall | NOT RUN |
| Interrupted install | Interrupt extraction, runtime validation, identity registration and publication; retry/recover; previous copy remains usable; no ownership takeover | NOT RUN |
| Filesystem failures | Disk full, read-only/locked payload, unavailable previous slot and cancelled wizard; actionable error, no user-data loss, successful retry | NOT RUN |
| Rendering | Intel/AMD/NVIDIA where available; automatic renderer and Cairo; 100/125/150/200% plus mixed monitors; images/GIFs/video/fullscreen | NOT RUN |
| Audio/video | Internal/default, USB and Bluetooth where available; direct ICE and TURN; audible cues, mute/gain, camera changes/hotplug and ten call cycles | NOT RUN |
| Input | PT-BR dead keys, US/AltGr and a real IME; compose/edit/search focus and candidate commit; Narrator, file dialogs and keyboard-only use | NOT RUN |
| Capture | Explicit WGC picker consent/cancel/source closure, remote peer frames, permission changes and cleanup; preview stays disabled by default | NOT RUN |
| Recovery | Sleep/resume during chat/call/media; offline/packet loss/reconnect/backend restart; permission revocation; drafts and correct session/capture teardown | NOT RUN |
| Repetition | Ten channel round trips, older-media scrolling, viewer close/fullscreen cycles, ten call cycles and sustained idle; cache reuse, bounded work/helpers and responsive UI | NOT RUN |
| Cross-platform baseline | Same source/release profile, synthetic scenario, renderer/scale and content on Linux; repeated cold startup, cold/warm media and sustained working-set samples | NOT RUN |
| Data preservation | Upgrade/uninstall preserve settings, saved sessions, unknown files and another portable owner's registration; explicit logout/revocation remains correct | NOT RUN |

Existing pending call, notification, capture and desktop acceptance remains a
beta blocker. Do not count compilation, a Server/Wine run or synthetic frames as
physical-device/Windows 11 acceptance. Keep failed results and reproducible
steps in the beta record; repeat failures after fixing them before closing a row.

## Opt-in diagnostics and Linux comparison

There is no automatic collection or upload. A tester can opt into Papo's existing
aggregate performance output for one controlled scenario:

```powershell
$env:PAPO_PERFORMANCE='1'
.\Papo.com --diagnostics 2> beta-local.log
Remove-Item Env:PAPO_PERFORMANCE
```

Close Papo normally to flush counters. Keep the raw log local. Use the
`--startup-info` PID to sample that instance's working set/peak via PowerShell or
Task Manager; record samples before/after ten round trips and after returning to
idle. Separately time startup to the first usable window. Use a new process per
scenario, repeat at least three times and retain individual samples. On Linux,
use the same source/release profile and `PAPO_PERFORMANCE=1`, recording RSS with
the method described in [performance measurements](../../PERFORMANCE.md).
Working-set/RSS accounting differs between OSes; these comparisons are
observations, not a claim that one allocator/GPU has a proven regression.

The maintainer can export only approved numeric fields with the standalone
Python tool (testers do not need a Python installation to use Papo):

```sh
python packaging/windows/beta_report.py --consent --performance beta-local.log --output beta-windows.json --platform windows --source-commit FULL_SOURCE_SHA --package-sha256 INSTALLER_SHA256 --scenario channel-switch-10 --renderer cairo --scale 100 --gpu intel --keyboard pt-br --audio default --network normal --working-set-start-bytes 100000000 --working-set-end-bytes 110000000 --working-set-peak-bytes 150000000
```

Replace hashes and example measurements with actual observations; they are not
acceptance results. Omit `--startup-ms` if it was not measured. The exporter
requires `--consent`, accepts fixed scenario/device categories, strips unknown
metric labels/fields and raw lines, and rejects text/boolean/negative/oversized
counters. It reads no settings, credentials, environment dump or machine identity.
Review the JSON before sharing it. The tool never uploads it.

Create a Linux report with `--platform linux`, the same source/scenario/settings,
and its actual measurements. Add `--compare beta-linux.json` when exporting the
Windows report to include the two raw measurement sets. Mismatched source,
scenario, renderer, scale, network, keyboard or audio conditions reject the
comparison. The tool makes no causal/performance-percentage claim and cannot
mark a release qualified.

## Local verification

- 61 Python Windows/packaging/gate/beta tests: **58 passed, 3 native-Windows
  compiler checks skipped on Linux**, explicitly not Windows acceptance.
- NSIS 3.11 compiled the actual recipe and generated inventory with a synthetic,
  non-runnable payload, using `-WX` without warnings. This verifies compilation,
  resources, languages and script linkage, not installation execution.
- PowerShell 7.6.9 parsed the seven packaging drivers; actionlint validates both
  workflows. No native Windows/PowerShell 5.1 execution is claimed.

The [NSIS scripting reference](https://nsis.sourceforge.io/Docs/Chapter4.html)
defines execution level, per-user shell context, silent/uninstall behavior and
file/registry commands. [nsExec](https://nsis.sourceforge.io/Docs/nsExec/nsExec.txt)
runs CLI probes without a console; its timeout is an **idle-output timeout**,
not a process-tree security boundary. The fixture driver and CI also impose
overall execution limits. Media isolation continues to use AppContainer/Job Objects.
