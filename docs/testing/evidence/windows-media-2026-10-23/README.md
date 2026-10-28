# Step 7 packaged Windows media acceptance

Native gate: **PASS-CI**, 2026-10-23. Final standard-user VM gate: **PASS-VM**.
The [media contract](../../windows-media.md) describes the fixture matrix and
[step 7](../../../../MULTIPLATFORM_PLAN.md) defines the completion gate.

## Source and package

The application, native bridge, runtime collector and instrumented UI executable
come from `ee3097dc525d3f3797b5270fecd563eda894d8e9`.
[Full native CI](https://github.com/ISpeakHue/papo-gtk/actions/runs/39879166819)
passed Windows all-target compilation with warnings denied, native/platform,
session, media/cache/classification and packaging tests, the packaged security
probe, standalone ZIP relocation, ten software-rendered media UI cases and four
automatic-renderer cases. macOS compilation and test linking also passed.
[Conclusions](CODE-CI-RESULT.json), [native excerpt](native-ci-excerpt.txt),
[runtime excerpt](portable-ci-excerpt.txt) and [media security output](ci-media.stdout.txt)
retain those results.

The optional driver subsequently needed a Windows PowerShell 5.1 correction:
resolve its default directory in the script body, after parameter initialization.
Only that external script and the validation workflow changed, at
`0029ecd8e4afa40a5655c45181165b5a6dfefad5` (see the exact revision in
[the driver CI record](DRIVER-CI-RESULT.json)).
[Driver-only CI](https://github.com/ISpeakHue/papo-gtk/actions/runs/39896033772)
reused the fully tested ZIP and matching harness; all ten Cairo and four automatic
cases passed under Windows PowerShell 5.1, alongside ZIP-only runtime acceptance.
It did not rebuild or requalify application code. Raw reports and individual
stdout/stderr are in [native-ui](native-ui/cairo/media-ui-cairo.json) and
[powershell5-ui](powershell5-ui/cairo/media-ui-cairo.json).

[Source comparison](SOURCE-COMPARISON.json) checks 234 inputs: application,
runtime and compiled fixture inputs against the full native revision, and the
two external driver/workflow files against their later revision. There are no
unexplained differences. Documentation and captured evidence are separate. This
is source equivalence, not a claim of byte-identical builds.

[Package verification](PACKAGE-VERIFICATION.json) checked all **7,828** manifest
files by size and SHA-256. The ZIP is `Papo-1.0.2-windows-x86_64-dev.zip`, SHA-256
`0539a4bb623595d93513c8bd8cdc0a6242c6c07f1300722dd26f45b3654021b6`;
runtime manifest SHA-256 is
`b6fb63b893cc2212b0e205f2e70da2887e98761bb4937760112179df592d913d`.
[BUILD-INFO](BUILD-INFO.json) reports clean source. The separately distributed
[instrumented harness](MEDIA-TESTS.json) matches that revision; it is not the
normal release executable and is not included in the user ZIP.

## Verified behavior

The shared draft classifier recognizes supported video suffixes without a host
MIME database. MP4/H.264/AAC, generic-MIME MOV, QuickTime MOV and WebM/VP8/Vorbis
produce real posters and play in native GTK streams. Reopening reuses the cached
source; access invalidation releases streams and temporary sources. Still images,
animated custom emojis, bare GIFs, backend GIF/video renditions, staged previews,
viewer zoom/pan/fullscreen, byte-exact downloads and video fullscreen/state
preservation use the actual shared widgets and synthetic loopback backend.

Cache fixtures verify opening batches, retained recent first frames, three
channel round trips without extra media requests, animation disposal, bounded
cache policies and suppression of repeated invalid-preview downloads. Windows
converters share two slots across queues; cancellation retains a slot until native
job/profile cleanup finishes. The [native probe](ci-media.stdout.txt) verifies
that bound alongside access denial, output/memory/time limits, descendant and
parent-exit cleanup, and three poster plus three animation conversions. Deletion
sharing applies only to the original source read handle; separately staged media
remain sealed. Container/playlist guards and backend-only GIF resolution remain.

[Decoder results](RUNTIME-DECODE-RESULT.json) record seven successful pipelines:
automatic and explicit software decoding for MP4/MOV/WebM, plus WAV-to-Opus.
Observed codecs include `avdec_h264`, `avdec_aac`, `vp8dec`, `vorbisdec` and
`opusdec`. This runner advertises no hardware decoder factories. Cairo and GTK's
automatic selection both resolved to `GskCairoRenderer`; neither proves physical
GPU acceleration. No driver is forced or redistributed.

UI checks select the bundled test audio sink only in the test process. They
require decoded audio tracks, prepared video and advancing playback timestamps;
they do not establish audible output. Runtime/software audio decoding is separate
from physical device acceptance. Sampled working-set peaks and instrumented test
durations are diagnostics, not release-build performance benchmarks.

## Clean VM gate

The [final VM driver](vm/VM-RESULT.json) and [ten-case report](vm/cairo/media-ui-cairo.json)
record a complete unchanged-matrix replay with all ten cases passed. Runtime
manifest contents and machine PATH remained unchanged, and the extracted package
was removed. The guest shut down normally before host extraction of the result
disk. Raw stdout/stderr and the driver log are retained in `vm/`. The run began
at 2026-10-23T03:53:36.8250971Z and finished at 2026-10-23T04:00:08.5810971Z.

The reused fixture is offline Windows
11 Enterprise Evaluation 26H2 x64 build 26300.9457, standard `PapoTester`, four
vCPUs, 6 GiB RAM and 1280×800 virtual VGA. There is no network adapter, host share,
guest agent or external SDK/runtime. The evaluation watermark is not bypassed.
[Original installation inputs](VM-BASE-INPUTS.json) identify an older package;
that older package is not the candidate accepted here.
[Updated inputs](MEDIA-INPUTS.json), [QEMU arguments](QEMU-COMMAND.json) and
[read-only device additions](QMP-ADDITIONS.json) identify the current ZIP,
matching test executable, external driver and fresh result disk. Copied scripts
use a UTF-8 BOM for Windows PowerShell 5.1; their text matches source after BOM
removal. Only synthetic fixture credentials and media are used.

## Earlier failures and regression scope

An initial MSYS2 mirror timeout stopped before compilation. The first standalone
UI candidate passed eight of ten cases; its [unmodified reports](initial-ui-failure/cairo/media-ui-cairo.json)
retain the checkout-relative draft fixture failure and missing audio-output
device error. Embedding fixture bytes and using a test-only audio sink corrected
the harness without weakening playback assertions or changing production audio.
The [two-case offline diagnostic](driver-diagnostic/results/media-ui-auto.json)
passed on the earlier package with the corrected driver; it is not the final
source-matched VM gate.

The first final-package VM attempt stopped during driver preflight, before any
media case, on the PowerShell 5.1 directory-default issue. Its
[failure report](powershell5-initial-failure/cairo/media-ui-cairo.json) is retained
and is not counted as a pass.

The next full VM run passed nine cases but failed the inline emoji offscreen-pause
frame comparison. Its [original report](initial-vm-media-failure/cairo/media-ui-cairo.json)
and stderr are retained. The same assertion passed in both native CI matrices and
the earlier VM diagnostic. The unchanged full-matrix replay passed all ten cases. No assertion
was removed and no root cause is claimed from the single failure. The intermittent
frame comparison remains a [tracked investigation](../../../FOLLOW_UPS.md); this
is not evidence that its timing issue has been fixed.

Local all-target compilation passed, as did 296 headless tests (four intentional
display/device skips), 26 Python packaging tests (24 passed, two Windows-native skips), and
all ten focused media GUI cases under rootless Fedora/Xvfb/Openbox. The final
[full Linux workflow](https://github.com/ISpeakHue/papo-gtk/actions/runs/39879167086)
still fails eight of 38 GUI cases. [Results and original assertions](linux-ci/results.tsv)
match the existing [tracked baseline](../../build-provenance.md#verification-record-2026-10-21);
this step does not claim full Linux GUI qualification. Focused media logs are in
`linux-ui/`; the full headless result is [retained](local-final-unit.log).

Nonfatal font/scanner diagnostics remain in raw logs. Font fallback, input,
accessibility, physical GPU/audio/camera devices, real-backend acceptance and the
Home/Pro release matrix remain later qualification work. macOS remains compile
and link groundwork, with no enabled isolated converter or supported bundle.
Inline GTK playback is not claimed to be sandboxed. No release was published.

`SHA256SUMS` covers retained evidence bytes after finalization. Binaries, ISO,
VM/TPM disks and fixture passwords are excluded.
