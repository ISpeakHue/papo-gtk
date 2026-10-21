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

The bootstrap preserves Unicode argument boundaries and supports package paths
with spaces and accents. It creates its GStreamer registry beneath LocalAppData,
keyed by runtime identity and location. Relocation therefore gets a fresh registry
instead of stale absolute plugin paths. Python writes no bytecode into the package.
GSettings and GIO caches are generated during staging; GdkPixbuf loader filenames
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

A separate `portable-runtime` Windows job downloads the ZIP without MSYS2 setup,
clears inherited developer lookup paths, sets a minimal process PATH and runs from
an unrelated working directory. It repeats the probe after relocation to paths
with spaces/accents, requires failure with bundled Python removed, verifies file
immutability and confirms no machine-wide PATH change. Hosted Windows Server
runners include development tools; restricted discovery and DLL-origin auditing
are stronger than an ordinary build-job check but do not substitute for a pristine
Windows 11 VM.

## Clean Windows 11 acceptance (pending)

Copy the exact artifact and its checksum to a fresh standard-user Windows 11 VM
from the [support matrix](../../WINDOWS_SUPPORT.md), with no MSYS2, Rust, Python or
GStreamer installed. Record full OS build/UBR and artifact/source hashes. Extract
under a path such as `Downloads\Papo á é ç`, open Papo.exe, close it, then invoke
both diagnostic commands from an unrelated working directory. Move the package
and repeat. Confirm no installer/runtime prompt, correct text/icons and unchanged
machine PATH. Keep this evidence separate from hosted CI; the VM pass is required
before checking off step 3's full completion criterion.

## Verification record

Local: 16 packaging/provenance tests and five platform-policy tests pass; all Rust
targets compile without warnings. Native CI evidence will be recorded after its
checks finish. No clean Windows 11 VM result is claimed.
