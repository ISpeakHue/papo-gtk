# Multiplatform development

The `multiplatform` branch starts a Windows-first port of the shared Rust/GTK
application. Linux remains the supported release platform. Windows is an
experimental developer build; macOS has platform boundaries prepared but no
supported build or `.app` bundle yet. There is one application and protocol
implementation, rather than separate operating-system forks.

Source version **1.1.0** is available as an unsigned
[Windows preview](https://github.com/ISpeakHue/papo-gtk/releases/tag/v1.1.0-preview.1),
with a self-contained installer and portable ZIP. Its clean source revision passed
the complete automated candidate workflow; see the
[build record and remaining acceptance](docs/testing/windows-preview.md). The
[signing/qualification/publication tools](docs/testing/windows-publication.md)
still require an owner signing certificate and completed native acceptance for a
supported release. Neither is currently available. Linux 1.0.2 remains the latest
stable release.

The first release targets and dependency constraints are defined in
[WINDOWS_SUPPORT.md](WINDOWS_SUPPORT.md). The
[Windows acceptance matrix](docs/testing/windows.md) records required cases,
historical CI evidence and pending desktop tests. These documents complete step 1
of [the implementation plan](MULTIPLATFORM_PLAN.md); Windows remains experimental.

## Current boundaries

| Integration | Linux | Windows | Future macOS |
| --- | --- | --- | --- |
| GTK UI, HTTP/WebSocket, history, image cache, Unicode/custom emoji | Existing implementation | Shared implementation, native build checked by Windows CI | Shared implementation, compile-only CI |
| Remembered sessions | Secret Service | Windows Credential Store | Apple Keychain selected |
| Configuration/cache directories | XDG | Native AppData directories | Native Application Support/cache directories |
| Worker interpreter | Existing packaged/system Python | UCRT64 Python; hidden helper consoles | Bundle Resources runtime or native Python |
| Voice/camera GStreamer engine and remote stream viewing | Existing implementation | Step 8 device/lifecycle implementation; real Windows device acceptance deferred ([checks](docs/testing/windows-calls.md)) | Shared engine and process-group cleanup; device behavior not yet validated |
| Screen publication | Consent through desktop portal/PipeWire | Native WGC system picker/worker bridge; opt-in preview, [desktop acceptance pending](docs/testing/windows-capture.md) | Disabled; native capture/permission integration pending |
| Desktop notification alerts/actions | freedesktop D-Bus | Protocol WinRT toasts, account/session-bound cold activation and removal; [desktop acceptance pending](docs/testing/windows-notifications.md) | Pending; in-app inbox/badges still active |
| Isolated video poster/GIF conversion | Bubblewrap / nested Flatpak sandbox | AppContainer + bounded Job Objects; native security/conversion checks passed | Disabled until a macOS isolation backend exists |

`src/platform` owns OS policy, native credential access, helper/runtime paths,
primary shortcut modifiers, decoder isolation and screen capture selection.
The UI selects a desktop notification adapter separately. Unsupported adapters
never fall back to Linux commands or execute media converters without isolation.
Backend-provided images/thumbnails and ordinary image/GIF decoding remain shared.
Some providers supply GIFs as video; Linux and Windows have isolated conversion
backends, while macOS still requires one. Video playback still depends on GTK/GStreamer codecs.
Packaged fixture playback/fullscreen is covered by step 7; real audio devices,
cameras and live calls still need native testing.

`papo-gtk --platform-info` prints the selected integration capabilities without
reading settings or credentials. A `true` value describes an implemented backend,
not a guarantee that the host supplies its dependencies or permissions.
`--check-media-runtime` continues to require isolation and fails on platforms
whose isolation adapter is not yet implemented.

## Windows developer build (x86_64)

Use [MSYS2 UCRT64](https://www.msys2.org/docs/environments/), including its Rust
package. Do not mix MSVC, MINGW64/MSVCRT or another Python runtime into this build.
The [GTK Rust Windows guide](https://gtk-rs.org/gtk4-rs/stable/latest/book/installation_windows.html)
describes the native GTK toolchain requirements.

From an **MSYS2 UCRT64** terminal:

```sh
pacman -Syu
pacman -S --needed git \
  mingw-w64-ucrt-x86_64-rust mingw-w64-ucrt-x86_64-gcc \
  mingw-w64-ucrt-x86_64-pkgconf mingw-w64-ucrt-x86_64-gtk4 \
  mingw-w64-ucrt-x86_64-libadwaita mingw-w64-ucrt-x86_64-librsvg \
  mingw-w64-ucrt-x86_64-python-gobject mingw-w64-ucrt-x86_64-gst-python \
  mingw-w64-ucrt-x86_64-gst-plugins-base mingw-w64-ucrt-x86_64-gst-plugins-good \
  mingw-w64-ucrt-x86_64-gst-plugins-bad mingw-w64-ucrt-x86_64-gst-libav \
  mingw-w64-ucrt-x86_64-libnice
export PAPO_RUNTIME_DIR="$(cygpath -w "$MINGW_PREFIX")"
cargo run --locked
python packaging/windows/native_inputs.py capture target/windows-native-inputs
python packaging/windows/build.py --native-inputs target/windows-native-inputs
```

Git is required to record the source revision. Packaging requires a clean checkout
by default; `--allow-dirty` is an explicit local-preview override. The source state
is checked before/after compilation and staging. Exact MSYS2 packages and their
hashes are retained separately from the preview ZIP. See
[build provenance and CI diagnostics](docs/testing/build-provenance.md) for the
original line-ending diagnosis, toolchain metadata, input restoration and limits.
Byte-identical rebuilds have not been demonstrated.

The build script produces `dist/Papo-1.1.0-windows-x86_64-dev.zip`.
It includes an application-owned UCRT64 runtime: GTK/libadwaita, Python/GI,
GStreamer plugins/codecs, schemas, icons, loaders and dependency licenses. Extract
the whole archive and launch `Papo.exe` without a console. Use `Papo.com` for
intentional CLI output and `Papo.com --diagnostics` for application diagnostics;
`Papo.cmd` forwards there for compatibility.
The native bootstrap establishes DLL lookup before the GTK executable starts.
Neither a default `C:\msys64` installation nor a system Python is searched.
`PAPO_RUNTIME_DIR` explicitly selects a complete alternate packaged runtime;
source-tree development uses an explicit UCRT64 prefix as above. `PAPO_PYTHON`
remains an explicit helper interpreter override.

Run `Papo.com --check-packaged-runtime` for the offline dependency, Python/GI,
icon, schema and codec probe. See [runtime packaging](docs/testing/windows-runtime.md)
for layout, relocation, manifest and validation limits.

Step 3's [clean-VM runtime acceptance](docs/testing/evidence/windows-runtime-2026-10-21/README.md)
passed with the self-contained ZIP under a standard Windows 11 account, including
accented/spaced relocation and normal startup/shutdown. This is runtime evidence;
the Home/Pro release matrix and remaining native integrations are still pending.

Step 4's [Windows identity and startup contract](docs/testing/windows-identity.md)
defines executable resources, the stable `br.com.papo.gtk` identity, explicit
per-user desktop registration, a native notification prototype and account-bound
activation policy. The [step 9 adapter](docs/testing/windows-notifications.md)
now integrates account-bound message alerts and durable cold activation. Actual
Windows desktop acceptance remains pending.

Step 5's [credentials, configuration and file contract](docs/testing/windows-storage.md)
passed native CI and offline standard-user Windows 11 VM acceptance. Disposable
Credential Manager entries cover restart, rotation, logout, revocation and safe
failure; native paths, Unicode/long filenames, locks, cancellation and clipboard
cleanup passed. Exact-account hashed targets avoid Windows case collisions;
legacy migration verifies account ownership. No plaintext credential fallback
exists. [Reports and hashes](docs/testing/evidence/windows-storage-2026-10-22/README.md)
record this gate independently of the remaining Home/Pro release qualification.

Step 6 implements [AppContainer media isolation](docs/testing/windows-isolation.md)
with bounded Job Objects, sealed input, private scratch and restricted handle
inheritance. [Native CI](https://github.com/ISpeakHue/papo-gtk/actions/runs/39868216963)
passed the security/lifecycle probes and three poster plus three GIF conversions
before Windows `isolated_media_helpers` was enabled. macOS converters remain
disabled. [Cold Windows VM acceptance](docs/testing/evidence/windows-isolation-2026-10-22/README.md)
also passed all security/conversion checks and runtime/package cleanup.

Step 7 passed [packaged media acceptance](docs/testing/windows-media.md): staged
previews, animated custom emojis and backend GIFs, native MP4/MOV/WebM playback,
software audio decoding, viewer zoom/fullscreen, downloads and cache reuse.
[Reports and hashes](docs/testing/evidence/windows-media-2026-10-23/README.md)
cover the ten-case standard-user Windows VM replay, native CI, automatic/software
renderer checks and Windows PowerShell 5.1 driver compatibility. An intermittent
inline-emoji pause assertion remains tracked; no fix is claimed from its passing
replay. Physical GPU/audio/device and Home/Pro release qualification remain open.

The [Windows packaged CI gate](docs/testing/windows-release-gate.md) requires all
regular Windows tests, focused native checks and separate ZIP-only runtime,
media and desktop jobs, alongside Linux regression and macOS compile/link.
It binds successful reports to one clean source and ZIP; missing or skipped
required checks cannot qualify a candidate. A manual `packaging_only` rebuild
requires an authenticated full-run gate for identical source, dependencies,
tools and build options, and produces no new candidate gate. Windows 11 desktop
acceptance remains separate and pending. Native credential acceptance runs only through the explicit disposable
storage probe; ordinary tests use memory and never
read or change the developer's real keyring. GUI tests use an isolated
configuration directory on every OS, including systems that ignore XDG variables.

## Roadmap and macOS groundwork

Step 13 adds the [per-user NSIS preview installer](docs/testing/windows-beta.md),
using the standalone ZIP's exact runtime, versioned upgrade/recovery and
inventory-based uninstall. The Windows workflow builds both installer and ZIP
artifacts and requires a separate installer acceptance job. Native installer and
attended beta qualification remain pending; this is not a supported Windows
release. Optional beta reports export only selected numeric measurements locally.

Before a supported Windows release: complete Home/Pro and physical media/device qualification,
native screen/window capture and notification activation/removal acceptance,
full Windows desktop acceptance, an installer,
signing. Application icon/metadata and the native identity/activation prototype
passed step 4's offline VM gate. Validate real media devices, dead-key/IME
input, DPI scaling, reconnects and fullscreen media on Windows.

The same adapters leave room for macOS Keychain, native notification activation,
ScreenCaptureKit/source selection and a restricted decoder helper. Runtime lookup
already recognizes `Papo.app/Contents/MacOS` with helpers under
`Contents/Resources/runtime`; primary window shortcuts use Command on macOS.
Build and validate GTK/libadwaita/GStreamer on Apple Silicon first, then address
`.app` resources, privacy usage descriptions, entitlements, signing and
notarization. Nothing in this branch claims those macOS integrations are ready.

Credential backend features are documented in
[keyring 3.6.3](https://docs.rs/keyring/3.6.3/keyring/).
The initial Windows toolchain is provisioned through
[msys2/setup-msys2](https://github.com/msys2/setup-msys2), using the
[UCRT64 Rust package](https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-rust).

## Initial verification (`ec8bf2b`)

The [native verification run](https://github.com/ISpeakHue/papo-gtk/actions/runs/39536236642)
passed Windows platform-policy tests, synthetic voice startup/gain/cleanup, icon
rendering without a host icon theme, and synthetic chat/member startup. The
macOS job compiled all targets and linked the test executable. These checks use
no real account, microphone, camera or desktop capture; native credential access
is compiled but the ordinary session tests use memory storage.

On Linux, all 286 regular tests passed (four integration tests remain opt-in),
as did the synthetic voice worker, four private portal checks, call-sound tests,
and focused icon/call-control GUI checks. Notification and backend-GIF GUI
assertions passed with GTK warnings nonfatal; the notification window still
produces a height-measurement warning under the host GTK version.

The initial [Linux container workflow](https://github.com/ISpeakHue/papo-gtk/actions/runs/39536236637)
failed during the icon test when its Glycin SVG loader exited inside Bubblewrap.
At that point the complete Linux GUI workflow was unverified. Step 2 below records
the subsequent runtime repair and complete case sweep; remaining GUI limitations
are tracked in [desktop testing](docs/testing/desktop.md) and the provenance record.

The [successful packaging rerun](https://github.com/ISpeakHue/papo-gtk/actions/runs/39537643269)
provides the **Papo-Windows-x86_64-developer-preview** artifact. The downloaded ZIP
passed integrity checks and contains an AMD64 executable, launcher, instructions,
license attribution and source metadata. It requires the external MSYS2 runtime.
Its `SOURCE.txt` points to `c911a1519259dd14a9845cf9e13b9cb64d7aecb4` and reports
local checkout changes; this preview has not been verified as a reproducible
clean-source release.

ZIP SHA-256:
`c44ca68449e8bd6eeb75115b4224f26cf0447defc3dfedf8a121c140c0708b3c`

## Build provenance (step 2)

The original dirty preview was reproduced as a Git for Windows/MSYS2 line-ending
configuration mismatch. `.gitattributes` now gives both implementations a clean
checkout; unexpected source edits fail packaging. The
[clean-source packaging run](https://github.com/ISpeakHue/papo-gtk/actions/runs/39615565000)
retained 352 exact MSYS2 package archives, toolchain/build metadata and checksums.
It was packaging-only and does not replace native application or desktop tests.
Byte-identical rebuilding remains unproven.

Linux CI now uses rootless Fedora with decoder isolation preserved and retains
every GUI case's result. See the [provenance and CI record](docs/testing/build-provenance.md)
for diagnosed container/codec failures, verification and remaining GUI assertions.
The initial results above remain historical evidence.
