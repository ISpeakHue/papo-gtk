# Windows support policy

Policy date: 2026-10-21. Scope: step 1 of [the port plan](MULTIPLATFORM_PLAN.md).

**Windows currently has an experimental developer preview, not a supported Papo
release.** This document defines the targets and evidence required for the first
supported release. Completing this policy does not complete desktop validation.
Linux releases retain their existing support policy; macOS remains groundwork.

## Release targets

The initial Windows release targets native x86_64 Windows 11 Home and Pro, with
a standard, non-administrator user account. The minimum target is **25H2, build
26200.9711**. **26H2, build 26300.9711** is the second required release family.
These are acceptance baselines selected on the policy date, not builds on which
Papo has already passed desktop acceptance.

| OS / architecture | Role | Initial build baseline | Current Papo evidence | Release requirement |
| --- | --- | --- | --- | --- |
| Windows 11 25H2 Home, native x86_64 | Minimum desktop target | 10.0.26200.9711 | Desktop tests pending | Required clean-machine acceptance |
| Windows 11 25H2 Pro, native x86_64 | Minimum desktop target | 10.0.26200.9711 | Desktop tests pending | Required clean-machine acceptance |
| Windows 11 26H2 Home, native x86_64 | Current desktop target | 10.0.26300.9711 | Desktop tests pending | Required clean-machine acceptance |
| Windows 11 26H2 Pro, native x86_64 | Current desktop target | 10.0.26300.9711 | Desktop tests pending | Required clean-machine acceptance |
| Windows Server 2022, x86_64 (`windows-2022`) | Build and synthetic-test CI | Runner image and exact revision must be recorded per run | Historical compile/synthetic checks passed | Does not qualify a Windows 11 desktop row |
| Windows 11 Enterprise/Education | Additional editions | No accepted release baseline yet | Enterprise Evaluation 26H2 x64, 26300.9457: [runtime/startup VM checks passed](docs/testing/evidence/windows-runtime-2026-10-21/README.md); full acceptance pending | Separate full acceptance before adding a support claim |
| Windows 11 24H2 and earlier; Windows 10 and earlier | Outside the initial release scope | None | Not validated | No legacy compatibility work in this port milestone |
| Windows 11 26H1; ARM64; x64 emulation on ARM64; 32-bit Windows | Additional OS/architecture targets | None | Not validated | Separate toolchain/runtime and acceptance work |
| Windows Insider builds, Wine and compatibility layers | Outside the initial release scope | None | Not validated | No inferred support from native Windows results |

Microsoft lists 25H2 and 26H2 as distinct release families with these October
2026 revisions. It also describes 26H1 as a release for particular new devices;
its higher build number is not a reason to include it automatically.
See [Windows 11 release information](https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information).

At release qualification, test the latest available security servicing revision
of both target families and record its full build/UBR and installed KB. If the
minimum revisions above cannot also be exercised, raise the published minimum to
the revisions actually qualified. Later servicing updates within a qualified
family are the maintenance target; a new feature-update family or architecture
requires a new matrix row and acceptance results. Recheck this policy before a
release if Microsoft servicing or dependency requirements have changed.

## Dependency and API constraints

An upstream library's Windows floor is not a Papo support claim. The application
policy above is deliberately narrower. Keep a consistent UCRT64 runtime and use
`Cargo.lock`; [build provenance](docs/testing/build-provenance.md) records actual native
package versions and retained archive hashes. The rolling repository itself is not a version pin.

| Component | Selected requirement / constraint | What remains to qualify |
| --- | --- | --- |
| Rust / native toolchain | MSYS2 UCRT64 Rust, GCC and pkg-config; `x86_64-pc-windows-gnu`. The Rust target's published client floor is Windows 10. | Record actual Rust/GCC/package revisions; keep all native libraries in the same UCRT64 environment. |
| GTK / libadwaita | `Cargo.toml` requires GTK >= 4.12 and libadwaita >= 1.6; `Cargo.lock` currently selects gtk4-rs 0.9.7 and libadwaita-rs 0.7.2. Rust binding versions are not DLL versions. | Capture the selected native DLL versions and validate the Win32 backend on the desktop rows. The GTK Windows overview supplies no complete OS floor for this particular package set. |
| Python / GI | Use the matching UCRT64 Python/GI packages. The preview's recorded worker trace used Python 3.14; upstream CPython 3.14 documents Windows 10+. `build.py` also uses `tomllib`, a Python 3.11+ build-tool requirement. | Record the distribution's exact Python version and bundle its matching GI modules and typelibs. The python.org installer does not establish MSYS2 GI compatibility. |
| GStreamer / codecs | Use UCRT64 core, base/good/bad plugins, libav, gst-python and libnice from the selected package set. | Pin versions, inventory factories/typelibs and test the complete codec closure. Upstream Windows documentation covers multiple distributions; its broad OS list does not certify this bundle. |
| Windows Credential Store | `keyring` 3.6.3 with `windows-native`, already selected by target. | Disposable native-store round trips, rotation/revocation and unavailable-store handling on Windows desktops. |
| Isolated converters | Windows sandbox backend is absent; keep conversions disabled. | Select and test the isolation implementation in step 6; recheck its OS floor and runtime permissions. |
| Screen capture | Windows backend is absent. Windows Graphics Capture is the planned integration candidate. Microsoft's desktop `CreateForWindow` API documents Windows 10 1903 / build 18362 as its minimum. | Picker/worker ownership prototype and runtime support/consent checks in step 10. The API floor alone does not establish a working Papo implementation. |
| Desktop notifications | Windows backend is absent; the exact API/binding and installer identity are not selected. | Resolve identity/activation in steps 4 and 9, document any SDK/runtime floor, and verify it against both desktop targets before enabling the capability. |

Requirements checked against the [MSYS2 Windows policy](https://www.msys2.org/docs/windows_support/),
[UCRT64 environment documentation](https://www.msys2.org/docs/environments/),
[Rust platform support](https://doc.rust-lang.org/rustc/platform-support.html),
[Python 3.14 Windows documentation](https://docs.python.org/3.14/using/windows.html),
[GTK Win32 documentation](https://docs.gtk.org/gtk4/windows.html),
[GStreamer Windows installation guide](https://gstreamer.freedesktop.org/documentation/installing/on-windows.html),
and [capture interop requirements](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow).
These references inform the constraints; desktop qualification remains pending.

## Distribution contract

| Current developer preview | First supported Windows release |
| --- | --- |
| ZIP bundles its UCRT64 runtime and launches through `Papo.exe`; clean Windows 11 standard-user runtime/startup checks passed. Full desktop acceptance remains pending. | Installer and self-contained ZIP run without MSYS2, Rust, Git, system Python or separately installed GStreamer. |
| Some native integrations are intentionally unavailable. | Every required feature in the acceptance matrix passes, including isolated conversions, native notifications and consent-based screen sharing. |
| Packaging rejects unexpected source changes and retains build/native-input manifests; byte-identical rebuilding is unproven. | Release provenance identifies the tested source and dependency inputs, with unexpected source changes rejected. |
| Server CI exercises selected synthetic cases. | Packaged artifacts pass desktop, clean-machine, real-device and real-backend checks. |
| No stable Windows support promise. | Published support is limited to qualified rows and documented runtime/device conditions. |

Packaging-only CI is useful evidence for packaging, but cannot replace the full
release gate. Hardware acceleration may be used when available; functioning
software rendering/decoding must remain a tested fallback. A machine without a
camera or with denied capture permission must retain working text chat and show
a recoverable device/capability state.

## Acceptance and evidence

[The Windows acceptance matrix](docs/testing/windows.md) is the release checklist.
Its evidence ledger separates historical Windows CI results, Linux-only coverage,
manual Windows results and missing implementations. Empty/pending results are
never counted as passes. Every advertised feature and supported OS row needs
evidence for the final distributed artifact.

The Windows port becomes supported only after that checklist passes and steps
12–14 of the implementation plan are complete. Later target additions must update
both this policy and the acceptance matrix. Preserve the shared platform adapters
and macOS compile/link checks throughout that work.
