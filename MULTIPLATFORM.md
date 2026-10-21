# Multiplatform development

The `multiplatform` branch starts a Windows-first port of the shared Rust/GTK
application. Linux remains the supported release platform. Windows is an
experimental developer build; macOS has platform boundaries prepared but no
supported build or `.app` bundle yet. There is one application and protocol
implementation, rather than separate operating-system forks.

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
| Voice/camera GStreamer engine and remote stream viewing | Existing implementation | Shared engine; synthetic worker lifecycle tested by Windows CI | Shared engine, not yet validated |
| Screen publication | Consent through desktop portal/PipeWire | Disabled; native source picker pending | Disabled; native capture/permission integration pending |
| Desktop notification alerts/actions | freedesktop D-Bus | Pending; in-app inbox/badges still active | Pending; in-app inbox/badges still active |
| Isolated video poster/GIF conversion | Bubblewrap / nested Flatpak sandbox | Disabled until a Windows isolation backend exists | Disabled until a macOS isolation backend exists |

`src/platform` owns OS policy, native credential access, helper/runtime paths,
primary shortcut modifiers, decoder isolation and screen capture selection.
The UI selects a desktop notification adapter separately. Unsupported adapters
never fall back to Linux commands or execute media converters without isolation.
Backend-provided images/thumbnails and ordinary image/GIF decoding remain shared.
Some providers supply GIFs as video; those animations need the pending conversion
backend on Windows/macOS. Video playback still depends on GTK/GStreamer codecs;
real audio devices, cameras, fullscreen playback and live calls need native testing.

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

The build script produces `dist/Papo-1.0.2-windows-x86_64-dev.zip`.
It includes an application-owned UCRT64 runtime: GTK/libadwaita, Python/GI,
GStreamer plugins/codecs, schemas, icons, loaders and dependency licenses. Extract
the whole archive and launch `Papo.exe`; `Papo.cmd` forwards CLI arguments too.
The native bootstrap establishes DLL lookup before the GTK executable starts.
Neither a default `C:\msys64` installation nor a system Python is searched.
`PAPO_RUNTIME_DIR` explicitly selects a complete alternate packaged runtime;
source-tree development uses an explicit UCRT64 prefix as above. `PAPO_PYTHON`
remains an explicit helper interpreter override.

Run `Papo.exe --check-packaged-runtime` for the offline dependency, Python/GI,
icon, schema and codec probe. See [runtime packaging](docs/testing/windows-runtime.md)
for layout, relocation, manifest and validation limits.

The Windows workflow also compiles the macOS foundation against Homebrew GTK
and libadwaita, without claiming runtime or bundle validation. Its Windows job
checks all Rust targets with warnings treated as errors,
runs platform policy, synthetic voice worker, native icon and chat/member startup
tests, and uploads
a developer preview artifact. A manual `packaging_only` workflow dispatch rebuilds packaging for a source
revision already covered by the native checks. This does not replace testing on real Windows
hardware. Native credential integration is compiled but ordinary tests never
read or change the developer's real keyring. GUI tests use an isolated
configuration directory on every OS, including systems that ignore XDG variables.

## Roadmap and macOS groundwork

Before a supported Windows release: implement an isolated decoder process,
permission-aware screen/window selection, native notifications with activation
and revocation, clean Windows 11 runtime acceptance, an installer,
application icon/metadata and signing. Validate real media devices, dead-key/IME
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
