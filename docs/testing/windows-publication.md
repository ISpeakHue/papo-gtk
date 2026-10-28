# Windows signing, qualification and publication — step 14

Updated 2026-10-28. Source version **1.1.0** has an unsigned
[Windows preview](windows-preview.md) with a matching installer, portable ZIP,
checksums and corresponding source. Its committed revision passed the full
Windows/Linux/macOS candidate workflow and fresh-runner package checks.
**No supported Windows release has been qualified.** No signing certificate is
available, and native Windows 11 desktop/device/beta acceptance remains pending.
Step 14 stays unchecked. Linux 1.0.2 remains the latest stable release; macOS
remains compile/link groundwork without a supported bundle.

## Required inputs and trust boundaries

Complete the preceding plan steps and their actual acceptance before qualifying
a supported release. Keep the full Windows/Linux/macOS workflow green for one
clean, committed revision. `packaging_only`, historical results, partial GUI
runs and a Server runner do not replace that gate or native Windows 11 testing.
The qualifier rejects open steps 1–13, pending/failed/skipped feature evidence,
an incomplete matrix, beta blockers and unverified final files.

The signing adapter uses a specifically selected certificate in **CurrentUser/My**
on a trusted native Windows signing machine, through Windows SDK SignTool. A
hardware-backed key exposed by its CSP/KSP can use that store entry; unattended
provider/PIN behavior still needs validation with the owner's actual key.
Cloud-specific signing integrations are not implemented or inferred.

Configure these references outside source and ordinary CI logs:

- `PAPO_SIGNING_THUMBPRINT`: the selected public certificate's 40-hex store
  thumbprint. SHA-1 is only the store lookup identifier; file and timestamp
  digests use SHA-256.
- `PAPO_SIGNTOOL`: absolute path to the genuine Windows SDK `signtool.exe`.
- `PAPO_TIMESTAMP_URL`: the owner's CA/provider HTTPS RFC3161 endpoint, with
  no credentials, query or fragment in its URL.

No PFX/password import, private-key export, self-signed workaround, automatic
certificate selection or secret-bearing command line is provided. Keys remain
in the owner's external protected store. `.gitignore` excludes common private
key containers; candidate preparation also rejects tracked key containers and
PEM private-key blocks before producing source. Keep signing jobs isolated from
untrusted PR code. These tools do not provision certificates, CI secrets or
unprotected signing runners automatically.

SignTool uses `/fd SHA256 /tr ... /td SHA256`, followed by
`verify /pa /all /tw`. Nonzero results, including warnings, fail the operation.
PowerShell independently requires a valid embedded signature, the selected
signer and a timestamp. Receipts contain hashes/public certificate identifiers,
not private key material, account data or a full environment dump.

Authenticode identifies a trusted publisher and detects signed-file changes.
It does not guarantee SmartScreen reputation or the absence of prompts. Do not
document an unsigned preview as a signed or supported Windows release.

## Prepare the signed candidate

After a clean full CI run, download its exact portable ZIP/checksum, the
`Windows-release-gate` artifact, build diagnostics and source-matched
`Windows-media-acceptance` instrumented test artifact. Save the run's public API
metadata separately:

```sh
gh api repos/ISpeakHue/papo-gtk/actions/runs/RUN_ID > base-run.json
```

Run from that exact clean source revision on the trusted signing machine:

```sh
python packaging/windows/release.py candidate --archive INPUT_ZIP --base-gate gate.json --run-info base-run.json --output dist/windows-signed-candidate
```

Replace placeholders with actual files. Candidate preparation uses authenticated
GitHub API/CLI reads to verify the successful full run, repository, revision,
attempt and gate artifact. It rejects a locally substituted gate and requires
the input ZIP/build hash to match the tested candidate.

It creates a separate candidate directory and signs `Papo.exe`, `Papo.com`,
`runtime/bin/papo-gtk.exe`, `papo-shell.exe` and `papo-capture.dll`. Upstream runtime
DLLs are retained, rather than re-signed as Papo's own code. The Python PE check
requires signing to preserve every byte except the PE checksum/security entry
and an aligned appended certificate table; it is **not** a cryptographic
signature verifier. Actual cryptographic/trust/revocation verification uses
Windows SignTool and Authenticode.

The tool regenerates the runtime identity/hash manifest and BUILD-INFO signature
references, then rebuilds the ZIP and NSIS installer from that exact signed
payload. NSIS's `!uninstfinalize` signs the embedded uninstaller **before** it is
compressed into the installer. The final installer is then signed and verified.
The same signer must cover all owned binaries, installer and uninstaller.
Candidate labels/readme defer support claims to final qualification; the normal
step 13 build continues to produce explicitly unsigned preview artifacts.

`candidate.json` records the new archive/installer/source identities and remains
`release_qualified: false`. A failed preparation does not leave a candidate
manifest that can qualify a release. Corresponding source comes from
`git archive` of the tested commit, with dependency notices/licenses preserved.
Byte-identical rebuilding is still unproven. Signing changes file hashes, so the
original unsigned package's UI/VM results cannot qualify the new bytes.

## Retest the final distributed files

Use a fresh disposable native Windows account, with the original instrumented
test artifact. Verification requires only the public signer thumbprint and SDK
tool; the private signing key is not required on the test machine.

```powershell
.\packaging\windows\check-release.ps1 -CandidateDirectory .\windows-signed-candidate -TestDirectory .\media-checks -OutputDirectory .\signed-final-results -DisposableAccount
```

The driver runs installer acceptance first, including verification of the actual
installed uninstaller against the compiler's signing receipt. It then reruns the
portable runtime/storage/isolation/missing-component probes, ten Cairo/four
automatic media cases and the desktop/notification/lifecycle suite. Finally it
verifies all five owned application signatures and the installer, retaining the
uninstaller result in `signed-recheck.json`. The final gate binds every report
to the signed ZIP, installer, BUILD-INFO and runtime manifest.

These fixtures remain distinct from attended Home/Pro, hardware, input, mixed-DPI,
accessibility and real-backend acceptance. Complete the
[Windows matrix](windows.md) and [beta](windows-beta.md) on the **final candidate**.
Do not change/rebuild/re-sign files after recording acceptance.

Create a deliberately unqualified acceptance skeleton:

```sh
python packaging/windows/release.py acceptance-template --output acceptance.json
```

Later, add `--candidate dist/windows-signed-candidate` to bind a new skeleton to
the actual signed files. Never overwrite a completed record with a new template.
All template cases start `not_run`; no tool fills in a manual pass.

The completed record must identify the source, signed ZIP/installer, exact
backend revision, current servicing review, all four native standard-user x86_64
Home/Pro rows and W01–W26 for each. It also requires physical-device success
paths, PT-BR/real IME and mixed-DPI/accessibility evidence, and a beta result with
no open blockers. Each pass references local reviewed evidence by safe relative
filename and SHA-256. Missing or altered attachments fail qualification. Keep
credentials, production message text and raw private logs out of these records.

## Qualify and stage publication

After closing and recording the preceding plan steps, run:

```sh
python packaging/windows/release.py qualify --candidate dist/windows-signed-candidate --base-diagnostics BUILD_DIAGNOSTICS --final-reports signed-final-results --acceptance acceptance.json --evidence ACCEPTANCE_FILES --signer-certificate-sha256 OWNER_PUBLIC_CERTIFICATE_SHA256 --output dist/windows-signed-candidate/qualification.json
```

The explicitly pinned SHA-256 fingerprint is of the public signing certificate.
The qualifier checks authenticated base-source test identities, exact final
runtime/installer reports, all seven final signatures, the native matrix and
current servicing review. It generates a public qualification record with actual
tested minimum OS builds and fills the release-notes template from those validated
rows. The unqualified draft paragraph is removed only then. A failed qualifier
writes a failure record rather than retaining a successful result at that output.

Stage the qualified assets locally:

```sh
python packaging/windows/publish.py --candidate dist/windows-signed-candidate --qualification dist/windows-signed-candidate/qualification.json --output dist/windows-publication
```

Default behavior performs **no upload**. It requires the same clean revision,
unchanged candidate/notes/artifacts and a qualification less than seven days old.
It stages the signed installer/ZIP, exact source, dependency notices/licenses,
public qualification and release notes, and generates `SHA256SUMS`. Notices are
read from the distributed ZIP, not unrelated mutable files. Raw tester logs,
acceptance attachments, private keys and the instrumented UI executable are not
release assets.

## Explicit GitHub publication

When the owner authorizes publication and all gates have passed, create/push
`v1.1.0` at the exact qualified commit using the repository's normal tag review
process. The publisher **does not create or move tags** and refuses an existing
release rather than overwriting it. Run the same staging command with a fresh
output directory and `--publish` on native Windows.

Before writing, it reauthenticates the full GitHub run/gate and performs fresh
native signature/revocation checks on the final installer and ZIP's owned PEs.
It requires the remote tag to resolve to the tested source. It creates a **draft**
release, uploads only the approved files and validates every server-reported
asset SHA-256 before publishing. A missing digest, partial upload or mismatch
leaves a draft for investigation. Do not delete/replace evidence to make a failed
run appear successful. No GitHub release/tag has been created by this implementation.

README/Windows policy remain explicit about today's experimental state. The
published qualification and generated notes define the actually accepted Windows
rows; after successful publication, update the repository's current-support
summary and plan ledger to cite that record. Do not relabel Linux 1.0.2 binaries
or add a macOS support claim.

## Verification and remaining blockers

- The 1.1.0 all-target Rust check passes locally, with the locked offline inputs.
- 79 Python tests: **76 passed, three native Windows compiler checks skipped on
  Linux**. Eighteen new tests cover PE signing invariants, complete/invalid native
  evidence, safe publication staging, stale/changed artifacts, wrong tags/existing
  releases, draft promotion after matching digests and tampering/digest failures
  preventing publication. No test uses a real key or creates a GitHub release.
- PowerShell parser, workflow lint and NSIS recipe compilation are checked locally;
  both preview and candidate recipes compile with warnings treated as errors.
  A synthetic callback confirms the embedded-uninstaller hook executes, without
  producing a signature. These checks
  do not establish native SDK signing, certificate-provider behavior,
  Windows PowerShell 5.1 execution, revocation/timestamp service success or beta
  acceptance. No signing/reputation/native Windows pass is claimed.
- **Current supported-release blockers:** no owner signing certificate and open
  native Windows 11 acceptance/beta steps. The committed unsigned preview now
  passes the complete automated workflow and is published with matching assets;
  see its [build record](windows-preview.md). This does not qualify a supported
  Windows release.

References: [Microsoft SignTool](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool),
[NSIS signing hooks](https://nsis.sourceforge.io/Docs/Chapter5.html#uninstfinalize),
and [GitHub release asset metadata](https://docs.github.com/en/rest/releases/assets).
