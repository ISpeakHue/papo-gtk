# Windows GStreamer executable paths

This is a local correction to the MSYS2 GStreamer core 1.28.8 recipe, not an
upstream release or a GPU-driver workaround. `GSTREAMER-PKGBUILD` retains the
source archive checksum and the patch checksum. Developer setup and Windows CI
build package revision `1.28.8-1.1`; packaging rejects the stock revision.

On a Windows system whose ANSI code page is not UTF-8, core's
`find_executable_path()` reads an ANSI executable path, while the external plugin
loader interprets that path as UTF-8. A fresh registry under an accented path
reproduced `Couldn't build cmd string`, followed by scanning inside the host.
Warm registry results concealed this defect. The patch uses `GetModuleFileNameW`,
checks truncation and converts the counted UTF-16 path with GLib. The separate
scanner, plugin policy and hardware selection remain enabled.

`tests/windows_gstreamer_test.py` compiles a small native host, moves it into an
accented directory and checks its executable path plus fresh scanner discovery.
It failed on the original core and passes with this patch. Only `coreelements`
is allowed in this unit regression, so it opens no devices or vendor drivers.
Whole-package relocation and full-plugin discovery require separate results.

Keep both local native packages (GTK and GStreamer), their recipes, patches and
source archives when capturing native inputs. The recipe is retained under its
distinct `GSTREAMER-PKGBUILD` name to avoid colliding with GTK's `PKGBUILD`.
Do not carry this patch to a new core version without reviewing upstream behavior
and rerunning both the native regression and packaged fresh-cache checks.

Sources: [GStreamer initialization](https://github.com/GStreamer/gstreamer/blob/1.28.8/subprojects/gstreamer/gst/gst.c),
[Windows plugin loader](https://github.com/GStreamer/gstreamer/blob/1.28.8/subprojects/gstreamer/gst/gstpluginloader-win32.c),
[MSYS2 recipe](https://github.com/msys2/MINGW-packages/tree/master/mingw-w64-gstreamer).
