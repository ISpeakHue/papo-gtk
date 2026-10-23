# Step 6 Windows media isolation acceptance

Final gate: **PASS-CI / PASS-VM**, 2026-10-22. The cold standard-user VM
run passed at 23:12:20–23:16:48 UTC after the startup correction below. See the
[implementation contract](../../windows-isolation.md) and
[step 6](../../../../MULTIPLATFORM_PLAN.md).

## Source and CI

The final candidate is `61da878a9366e90faa77782a484ca6905e8f7374`, under
[full native CI](https://github.com/ISpeakHue/papo-gtk/actions/runs/39868216963).
All three jobs passed: Windows all-target compilation with warnings treated as
errors, 24 packaging tests, 13 platform tests, synthetic voice and native GUI
cases, session/upload tests, packaged storage/isolation/conversion checks;
independent ZIP-only runtime acceptance; and macOS compile/test linking.
[Conclusions](CODE-CI-RESULT.json), [selected logs](native-ci-excerpt.txt) and the
[packaged media report](ci-media.stdout.txt) retain these results.
[Source comparison](SOURCE-COMPARISON.json) verifies 190 local native application
and package inputs against that snapshot, including the C++ bridge. Documentation
and fixture records are separate. This is source equivalence, not a claim of
byte-identical builds.

The earlier [full native run](https://github.com/ISpeakHue/papo-gtk/actions/runs/39838842551)
and [documentation/package run](https://github.com/ISpeakHue/papo-gtk/actions/runs/39864866170)
passed native security/conversion and ZIP-only checks; their reports are retained
under `initial-native-ci/`. The second package failed cold VM acceptance with a
deadline error; [the original report](initial-cold-failure/media.json) is retained.
It is not counted as a VM pass.

## Cold-start correction

The metadata-only [diagnostic](cold-start-diagnostic/DIAGNOSTIC.json) measured
3,562 ms for initial runtime ACL setup, then cold Python/GStreamer startup:
child resume at 3,968 ms, exit at 18,921 ms. The warmed probe exited at 3,687 ms.
The old packaged CLI subsequently passed all assertions after that warm-up.
These are diagnostic results, not final package acceptance: the instrumented
standalone prototype had a 90-second diagnostic budget.

Trusted runtime permission preparation now runs asynchronously before any helper
starts, with a 60-second await deadline. Windows callers give the **unchanged
25-second native limit** and cleanup 30 seconds to finish, instead of cancelling
a healthy cold poster at Linux's nine-second threshold. Linux's poster/animation
timeouts remain unchanged. CPU, memory, output, token and network policy are
unchanged. The final VM run used a fresh extraction, without a prototype or
manual decoder warm-up.

## Final package and clean VM

[Package verification](PACKAGE-VERIFICATION.json) checked all **7,828** manifest
files by size and SHA-256. The ZIP is `Papo-1.0.2-windows-x86_64-dev.zip`, SHA-256
`7914456e7fed0f469ca7f9b9fabad8438e1524d397c8fc2476d8eb7a5d4156b1`;
runtime manifest SHA-256 is
`152d7177bc4d7717072089803f849066283d09f66e36a928ebf26b1495aee3e1`.
Source metadata is clean and matches the full-tested candidate above.

The reused offline Windows 11 Enterprise Evaluation **26H2 build 26300.9457**
fixture ran as standard `PapoTester`: four vCPUs, 6 GiB RAM, 1280×800 display,
no network adapter, guest agent, host share or external developer runtime. The
evaluation watermark was not bypassed. The earlier disposable diagnostic runtimes
were removed before testing. A pre-test host QEMU crash required a restart;
`qemu-img check` reported no image errors. No Papo acceptance case ran in that
aborted boot, and it is not counted as a result.

[VM-BASE-INPUTS.json](VM-BASE-INPUTS.json) identifies the original OS installation
and its **older** package; that older archive was not accepted here.
[MEDIA-INPUTS.json](MEDIA-INPUTS.json), [QEMU-COMMAND.json](QEMU-COMMAND.json) and
[QMP-ADDITIONS.json](QMP-ADDITIONS.json) identify the updated read-only FAT fixture,
synthetic media, PowerShell scripts and guest devices. Script copies have a UTF-8
BOM and match source after BOM removal. Login and launching used fixture-only
QMP keyboard input, not a host or guest desktop agent. A fresh results disk
prevents stale diagnostic files from being mistaken for final evidence.

The driver verified the ZIP checksum, extracted beneath a new `WM<random>` path
with spaces and `áéç`, cleared developer/runtime overrides, and launched from the
Windows directory. [media.json](media.json) records `status=pass`, exit code 0,
three posters, three animations and every access/resource/lifecycle assertion.
[Raw stdout](media.stdout.txt), [stderr](media.stderr.txt),
[driver log](media-driver.log) and [child exit](media-child-exit.txt) are retained;
stderr and driver log are empty. Runtime contents remained byte-identical to the
manifest, machine PATH stayed unchanged and the extracted package was removed.
The guest shut down normally before host extraction of the results disk.

The check created/deleted only its own randomly namespaced synthetic credential;
it did not enumerate existing credentials or read saved sessions, user settings
or server messages. [Linux regression](LINUX-MEDIA-RESULT.json) separately passed
all isolation and conversion assertions with process exit code 0.

## Boundary and assertions

Every decoder starts suspended with a distinct AppContainer identity, one
runtime-read capability, a verified low-integrity token and restricted inherited
handles. Input is sealed outside its writable profile. A Job Object enforces
process-tree lifetime and resource quotas; it does not supply file/network
isolation by itself. There is no unrestricted conversion fallback.

| Check | Native result |
| --- | --- |
| Private files and original source | Reads denied; only the protected staged copy is granted. |
| Input and runtime mutation | Writes, attribute changes and staged-directory replacement denied. Runtime manifest contents remain unchanged. |
| Native credentials | One random synthetic entry cannot be read; no existing credentials enumerated. |
| Inherited handles | Private inheritable fixture handle unavailable. Only exact missing/wrong-handle native statuses after readiness count as denial; unrelated crashes fail. |
| Network | No TCP4/TCP6/UDP loopback listener receives traffic. No network capability granted. |
| IPC and memory | Oversized stdout/stderr rejected; a 600 MiB allocation fails under the 512 MiB process quota. |
| Deadlines/cancellation | Internal deadline terminates work; cancellation kills ready root/child and removes the profile/input. |
| Abrupt parent exit | Job close kills root/child; next invocation reaps the owned orphan profile/input. |
| Healthy conversions | MP4, QuickTime MOV and WebM produce three valid posters and three nonempty animations. |

## Scope

This completes only the decoder-backend gate. Full packaged inline/viewer media
UX, Home/Pro qualification, physical GPU/device paths, real-backend integration,
notifications, screen capture and installers remain later steps. Voice workers
need separate network/device permissions. Inline playback is not claimed to be
sandboxed. macOS remains compile/link groundwork with its converter capability
disabled. Scratch has a bounded lifetime, not a hard disk-space quota.

`SHA256SUMS` covers retained evidence bytes. Binaries, ISO, VM/TPM disks and
fixture passwords are excluded.
