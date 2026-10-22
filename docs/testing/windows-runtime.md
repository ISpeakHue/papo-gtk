# Windows portable runtime (step 3)

The Windows preview bundles its own UCRT64 runtime. It remains experimental:
Windows 11 desktop acceptance, native application identity, isolated conversion,
real devices, screen capture and desktop notifications have separate port gates.

## Layout and discovery

```text
Papo.exe                       system-only native bootstrap
Papo.cmd                       optional command-line forwarding
BUILD-INFO.json                source, tools and native input identity
THIRD-PARTY-NOTICES.txt
runtime/
  runtime.id                   build identity for per-user registry separation
  RUNTIME-MANIFEST.json         versions, licenses, hashes and PE import graph
  bin/papo-gtk.exe              GTK executable beside its DLL closure
  bin/python.exe               matching UCRT64 Python
  lib/python3.x/               standard library, GI and Cairo extensions
  lib/girepository-1.0/        typelibs
  lib/gstreamer-1.0/           plugins including software codecs
  libexec/gstreamer-1.0/       native plugin scanner
  lib/gdk-pixbuf-2.0/          image loaders and relocatable cache
  share/                      schemas, themes, MIME data, locales, licenses
```

`bundle.py` copies package-owned inputs and recursively inspects AMD64 PE normal
and delay imports without executing them. An explicit Windows-system DLL allowlist
leaves OS components external; missing other imports fail packaging. Typelib DLLs
and plugin directories seed dynamic dependencies. GPU drivers stay with Windows
or the vendor. Development headers, static libraries, unrelated Python modules,
stdlib tests and prebuilt GStreamer registries are omitted.

The native bootstrap depends only on Windows DLLs. It starts the GTK executable
from `runtime/bin`, beside its complete import closure, so initial DLL resolution
works before Rust `main()`. Environment paths are child-local. It restricts PATH
to packaged binaries and System32 and sets absolute runtime data/helper paths.
Packaged Windows never searches PATH for an alternative Python. Source builds
must explicitly select their UCRT64 prefix via `PAPO_RUNTIME_DIR`; `PAPO_PYTHON`
remains an explicit developer override. Neither override is needed by end users.

The UCRT64 linker currently fails with accented output paths. Build in an ASCII
workspace; the native regression compiles there before moving its executables to
an accented installation directory. This is a build-tool limitation, not an
installation-path restriction.

The bootstrap preserves Unicode argument boundaries and supports package paths
with spaces and accents. It creates its GStreamer registry beneath LocalAppData,
keyed by runtime identity and location. Relocation therefore gets a fresh registry
instead of stale absolute plugin paths. Python writes no bytecode into the package.
GSettings, MIME, available icon-theme and GIO caches are generated during staging; GdkPixbuf loader filenames
are relative to the runtime prefix. Machine-wide PATH/settings are never modified.

[MSYS2 Python](https://www.msys2.org/docs/python/) uses a Unix-style standard-library
layout despite being a native Windows interpreter. Preserve that matching layout.
GdkPixbuf's [loader implementation](https://gitlab.gnome.org/GNOME/gdk-pixbuf/-/blob/master/gdk-pixbuf/gdk-pixbuf-io.c)
resolves relocatable cache entries against its prefix on Windows.
[GStreamer environment documentation](https://gstreamer.freedesktop.org/documentation/gstreamer/running.html)
describes the separate plugin/scanner and writable registry controls.

## Automated checks

```sh
python3 -m unittest discover -s tests -p 'windows_*test.py'
cargo test --locked platform:: -- --test-threads=1
```

Native packaging runs `Papo.exe --platform-info` and
`Papo.exe --check-packaged-runtime` before archiving. The second command checks
all manifest hashes; bundled Python prefix/import paths; GI/Cairo/GTK/Adw/media
namespaces; WebRTC/ICE/DTLS/Opus/VP8/WASAPI factories; MP4/MOV/WebM and WAV-to-Opus
decoding without devices; embedded SVG icons across three theme names and two
scales; compiled schemas; loaded Python-process DLL origins and plugin origins.
Rust also maps a small GTK window with accented text. It reads no session/settings
and contacts no server. This is runtime assembly evidence, not complete playback,
call or isolation acceptance. `--check-media-runtime` still requires the future
Windows sandbox backend and must remain unavailable.

Windows may load its Microsoft HEVC/VP9 codec extensions from `Program Files\WindowsApps`
while enumerating Media Foundation factories. The DLL audit recognizes only the
expected registered package families, Microsoft publisher ID, Store/System
signature kind, healthy package status and matching WindowsApps installation
directory. It reports those OS modules separately. Other external runtime DLLs
still fail the audit; no Microsoft codec extension is redistributed in the ZIP.

A separate `portable-runtime` Windows job downloads the ZIP without MSYS2 setup,
clears inherited developer lookup paths, sets a minimal process PATH and runs from
an unrelated working directory. It repeats the probe after relocation to paths
with spaces/accents, requires failure with bundled Python or the GTK DLL removed, verifies file
immutability and confirms no machine-wide PATH change. Hosted Windows Server
runners include development tools; restricted discovery and DLL-origin auditing
are stronger than an ordinary build-job check but do not substitute for a pristine
Windows 11 VM.

## Clean Windows 11 acceptance

**Passed on 2026-10-21:** the rebuilt ZIP ran under a standard account on an
offline Windows 11 Enterprise Evaluation 26H2 x64 VM, build 26300.9457, without
MSYS2, Rust, system Python or separately installed GTK/GStreamer. Both runtime
probes, accented/spaced relocation, unrelated CWD, missing-component rejection,
file integrity, unchanged machine PATH and normal application startup/shutdown
passed. See the [report, logs, screenshot and input hashes](evidence/windows-runtime-2026-10-21/README.md).
This completes step 3's runtime gate; this edition/revision does not qualify the
Home/Pro release matrix or full feature acceptance.

The [offline VM harness](../../packaging/windows/clean-vm/README.md) provisions a
fresh QEMU/KVM guest from a hash-verified official ISO. It uses enrolled Secure
Boot firmware and TPM 2.0, a new guest disk and a standard local account. No network
interface, host filesystem share, guest agent or external SDK/runtime is provided.
Its report includes OS/build, application inventory, artifact/source hashes,
runtime/relocation results and a screenshot of normal application startup. The
guest shuts down before results are copied from its dedicated virtual disk.

The portable driver uses the inbox .NET ZIP reader to avoid PowerShell 5.1's slow
progress rendering. The guest driver retains process handles so that version of
PowerShell reports child exit codes reliably. No system policy or PATH change is
needed.

To repeat runtime acceptance, copy the exact artifact and its checksum to a fresh
standard-user Windows 11 VM with no MSYS2, Rust, Python or GStreamer installed.
Release qualification additionally requires the [support matrix](../../WINDOWS_SUPPORT.md)
editions and servicing revisions. Record full OS build/UBR and artifact/source hashes. Extract
under a path such as `Downloads\Papo á é ç`, open Papo.exe, close it, then invoke
both diagnostic commands from an unrelated working directory. Move the package
and repeat. Confirm no installer/runtime prompt, correct text/icons and unchanged
machine PATH. The same automated script can be run there from PowerShell:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File packaging/windows/check-portable.ps1 -Archive C:\Downloads\Papo-1.0.2-windows-x86_64-dev.zip
```

Keep the adjacent `.zip.sha256` file with the archive. The script verifies it,
uses a unique temporary extraction directory, preserves the original ZIP and
modifies only the current shell's environment. Script permission applies only to
that process. Keep clean-VM evidence separate from hosted CI and rerun the check
when changing the distributed runtime.

## Verification record

Local: 20 packaging/provenance/VM-policy tests pass; the native launcher test is
skipped on Linux. All 21 execute successfully on Windows. The earlier five
platform-policy tests and warning-free all-target Rust check remain recorded in
the application baseline below.

Native application baseline: [run 39621763048](https://github.com/ISpeakHue/papo-gtk/actions/runs/39621763048)
compiled every Windows target with warnings denied and passed the platform,
synthetic voice, embedded icon and startup checks. Its macOS job compiled all
targets and linked tests. That run failed later during runtime staging; its
application tests are evidence only for their named cases, not a passing workflow.

The [portable build for `8c6444e`](https://github.com/ISpeakHue/papo-gtk/actions/runs/39625384337)
passed all 17 native packaging tests, optimized compilation and the offline
packaged runtime probe. It records Python 3.14.8, GTK 4.24.1 and GStreamer 1.28.8;
7,824 manifest files, 574 PE images and 206 attributed packages. The build is
clean-source and packaging-only; earlier application checks are recorded above.
The ZIP's independent CRC, full manifest and SHA-256 inspection passed locally.
The archive and checksum are also staged under `dist/` (ignored by Git).

Initial ZIP SHA-256:
`3f65b9fd44aac15685b6842097a1878ec9f8671c25226fed2a94f539abf1cf43`

Initial runtime manifest SHA-256:
`0752d87abd547f28ddbd9ae8cc589d3428aa60d3a243fd96de46967c0ad6eb10`

The original separate-runner probe passed relocation, manifest immutability and
missing Python/GTK assertions, but the shell incorrectly propagated the expected
negative test's exit code. The driver now consumes that code only after every
assertion succeeds. A `runtime_artifact_run` workflow input rechecks an existing
ZIP without rebuilding; it records no new build and cannot qualify a release.

[Final ZIP-only acceptance](https://github.com/ISpeakHue/papo-gtk/actions/runs/39627584697)
**passed** on a separate `windows-2022` runner using the unchanged ZIP above and
corrected driver `9f3fd4d`. No MSYS2 setup ran there. With developer paths cleared,
it passed from an unrelated working directory, repeated after relocation under
spaces/accents, rejected removed Python and GTK files, kept the manifest files
unchanged, wrote no package-local bytecode/registry and left machine PATH unchanged.
The probe audited all loaded Python-process DLLs and GStreamer plugin origins.
This is independent-runtime CI evidence; it does not itself qualify a Windows 11
desktop.

The [rebuilt package and independent acceptance for `b556628`](https://github.com/ISpeakHue/papo-gtk/actions/runs/39666153467)
**passed** all 21 native packaging tests, optimized compilation, the packaged
probe and the separate ZIP-only job. This revision fixes the Windows 11 codec
ownership false positive and the PowerShell 5.1 test-driver issues. It retains
7,824 runtime files, 574 PE images and 206 package owners. Local verification
checked the archive checksum, every file's hash/size, the runtime manifest and
the exact packaged probe source. The resulting archive is staged under `dist/`.

Current ZIP SHA-256:
`aa8a4e10f1bae62a74d022ce125099c588cec0770ef3e7ea99a4d04ed62078ef`

Current runtime manifest SHA-256:
`f05833e598867363b42e605ac96418b05835f2238bb27f915880048cd5bee0da`
