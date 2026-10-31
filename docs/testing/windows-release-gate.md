# Windows packaged CI gate — step 12

Updated 2026-10-28. The clean Windows preview revision passed the
[full automated workflow](https://github.com/ISpeakHue/papo-gtk/actions/runs/40756265548),
including every required packaged job and the candidate gate. Downloaded and
uploaded files were checked against that source-bound result; see the
[preview build record](windows-preview.md). Windows 11 Home/Pro, physical devices,
keyboard/IME, accessibility, displays and beta acceptance remain deferred as
requested. The remaining completion checkboxes do not claim that acceptance.

## Required workflow

`.github/workflows/windows.yml` runs the following jobs on pushes/PRs to
`multiplatform`, and on a full manual dispatch:

| Required job | Checks |
| --- | --- |
| `windows` | Clean checkout, retained native inputs, all-target compilation, **all regular Rust tests**, native synthetic voice/cue/process-tree checks, notifications, capture ABI and bounded frames, desktop policies, media policies, runtime staging, disposable native storage and AppContainer conversion. Warnings fail compilation. |
| `portable-runtime` | A separate Windows Server runner without MSYS2 provisioning. Extracts the shipped ZIP into an accented/spaced path, excludes developer discovery variables and prefixes, and launches `Papo.com` from an unrelated directory. Checks the runtime, capabilities, notification backend, disposable native credentials/files, real AppContainer denial/limits/cleanup plus healthy conversions, relocation, missing Python/GTK/capture/shell rejection, immutable files and machine PATH. |
| `media-ui-runtime` | Another fresh runner uses the shipped runtime with the source-matched instrumented test executable: ten Cairo cases and four automatic-renderer cases. Each case retains logs, exit/timeout status, timing and peak working set. Audio uses a test sink. |
| `desktop-ui-runtime` | Fresh extraction with the instrumented test executable, shipped runtime and hashed key helper: thirteen desktop/inbox/voice-channel-text cases plus three native synthetic call/cue/process-tree cases. Native key delivery is explicitly requested on the disposable runner. |
| `linux-regression` | Calls the existing `test.yml` workflow: rootless Fedora, all regular tests, the complete focused X11 GUI list, decoder/network isolation, portals and synthetic worker lifetime. Existing failing cases remain failures. |
| `macos-foundation` | Compiles all targets and links the tests with native GTK and Keychain dependencies. No macOS runtime or bundle claim. |
| `installer-runtime` | Step 13 adds a fresh-runner per-user NSIS install/replacement/uninstall fixture, stale-owner protection, file locks, interrupted-commit recovery and data preservation. See [installer/beta checks](windows-beta.md). |

`Windows candidate gate` requires all seven jobs to report **success**. Failure,
cancellation, missing results and skipped jobs cannot satisfy the gate. It also
validates the actual reports, rather than relying only on job status. Missing,
duplicate, empty, incomplete or failed reports fail the gate. Reported process
exit codes and expected missing-component failures must agree with the result.

X11/XTest key probes and screen portals run only in the Linux regression. Windows
input uses its own ownership-checked SendInput helper. The PulseAudio mixer
identity test explicitly reports a Linux-only skip on Windows; native cue
decoding and worker lifecycle still run. Python discovery writes separate
`python-native.json` and `python-workers.json` summaries, rejects empty discovery
and fails on any skip outside that single allowed PulseAudio test. Native
compiler-dependent checks cannot silently pass by skipping.

The final job uses `always()` to inspect failed dependencies. Report downloads
may continue after an artifact is missing so the validator can record the
failure; the validator itself must succeed. No test or validation failure is
converted into a successful candidate.

## Source and artifact binding

The `Windows-release-gate` artifact contains `gate.json` and `jobs.json`. A passing
gate binds:

- The exact clean source SHA, workflow run ID and attempt.
- The SHA-256 of `BUILD-INFO.json`, which identifies the release executable,
  retained dependencies, options, toolchain and runtime manifest.
- One ZIP name/hash shared by every packaged report, and the runtime manifest
  hash. All instrumented UI reports must also identify the same test executable.
- The hashes and complete expected case lists of the four ZIP runtime/UI reports,
  plus the source Python summaries and their explicit skips.
- The installer metadata/hash bound to that same ZIP and the complete seven-case
  installer acceptance report. Testing a portable ZIP does not qualify a different installer.
- A W01–W26 feature-to-fixture index and the **pending** native acceptance matrix.

Failed aggregation exits unsuccessfully and writes `automated_status: fail` when
the validator can run. Failure diagnostics remain downloadable. A successful
aggregate reports `automated_status: pass`, but **`release_qualified: false`**:
Windows Server 2022 CI is not a Windows 11 Home/Pro desktop. Synthetic capture
frames/ABI checks do not prove attended source selection; fake audio output does
not prove audible playback; GTK test accessibility does not prove Narrator.
The [acceptance matrix](windows.md) and steps 13–14 remain required before
publishing a supported Windows release. No release is published by this workflow.

Use `Windows candidate gate` as the full workflow's required status when setting
up branch protection. Publication must additionally check the downloaded gate's
source/ZIP hashes and outstanding native acceptance; a green packaging or
diagnostic run, or a skipped candidate job, is insufficient. This change does
not alter repository protection settings.

## Packaging-only rebuilds and diagnostics

For a packaging-only dispatch set `packaging_only: true` and `tested_run_id` to a
successful **full** Windows workflow for the exact same source revision. The
workflow reads that run from GitHub's authenticated API and checks its repository,
workflow path, event, completed/success result and SHA before downloading its
gate. PR/fork runs cannot authorize this shortcut. The gate's run ID/attempt must
match the API response. Expired or unavailable evidence requires a new full run.

`build.py` additionally requires the source, installed native versions and exact
package hashes, tool versions, Cargo.lock, target/profile and recorded build
options to match the tested gate. Changing only repository index databases or
the installation location of a tool does not change package identity. Dependency,
compiler or option drift requires the full workflow. Dirty rebuilds are rejected.
Both `--tested-gate` and `--tested-run` are mandatory with `--packaging-only`;
the API metadata is provided by CI, not trusted merely because someone supplied
a local JSON file. `BUILD-INFO.json` records the tested-run reference.

A packaging-only run produces a new ZIP and reruns the portable probes, but emits
**no `Windows-release-gate` artifact**. Its new ZIP is not automatically qualified
by the previous artifact's hash or UI results. `runtime_artifact_run` rechecks an
existing ZIP; `bridge_probe` is an API diagnostic. These modes likewise produce
no candidate gate and cannot authorize another packaging-only rebuild.

Use a full dispatch (all diagnostic/rebuild inputs at their defaults) for a new
candidate. Re-run the **whole** workflow after a failed attempt: reports from
different attempts are deliberately rejected.

## Local verification record

On Linux, with synthetic fixtures and no account/server access:

```sh
python3 -m unittest discover -s tests -p 'windows_*test.py' -v
python3 -m unittest discover -s tests -p windows_release_gate_test.py -v
```

The step 12 combined suite reported **47 tests, 44 passed, 3 native-Windows
checks skipped** on Linux. The 18 gate tests pass, including failed/skipped/
cancelled jobs, absent/duplicate reports, mixed source/build/ZIP/test executables,
missing native tests, dirty/package-only candidates, changed rebuild inputs,
trusted-run constraints, empty discovery and unexpected skips. Those three
Linux skips are not native acceptance evidence and would fail Windows CI.

`actionlint` 1.7.7 validates both workflow files without findings. PowerShell's
7.6.9 parser accepts all six packaging `.ps1` files on Linux; this is syntax
validation, not execution on Windows PowerShell 5.1. Native runtime/desktop probes
are deliberately left to the configured Windows jobs and attended acceptance.
No new Windows run, VM acceptance or supported-release qualification is claimed.
That statement describes the historical Linux implementation record above.
The [2026-10-29 local Windows automatic record](windows-local-2026-10-29.md)
adds 138 Python checks (one explicitly Linux-only skip), 310 regular Rust passes,
native lifecycle checks and modified-source portable diagnostics. It also records
the corrected GStreamer Unicode scanner defect. CI provisioning retains that
native correction; packaging removes SDK plugin exclusions before its runtime
probe. These local results do not replace a full clean-source workflow or change
the gate's dirty-source, missing-attended-evidence and artifact-identity rules.
Step 13's expanded verification record is in the [installer/beta guide](windows-beta.md).
The structured Python runner also passes 21 local worker/probe/cue/input checks;
both bundled call cues decode through their real GStreamer helper to a test sink.

GitHub documents [job dependencies and skipped results](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idneeds)
and [reusable workflows](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows).
