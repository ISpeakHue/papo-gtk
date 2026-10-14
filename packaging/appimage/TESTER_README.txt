Papo GTK — modern Linux x86_64 test build

File: Papo-@VERSION@-preview-@BUILD@-x86_64.AppImage

1. Make the file executable in Properties/Permissions, or run:
   chmod +x Papo-@VERSION@-preview-@BUILD@-x86_64.AppImage
2. Double-click it, or run:
   ./Papo-@VERSION@-preview-@BUILD@-x86_64.AppImage
3. Enter the backend API URL, for example https://api.cyberasilo.online,
   and your own account/server credentials.

Requires x86_64 Linux with glibc 2.43 or newer: Fedora 44 and current compatible
Arch systems. This build targets current GNOME and KDE desktops; it does not
support Ubuntu 24.04/older distributions. Check with: getconf GNU_LIBC_VERSION

Current GTK/libadwaita, Python GI, software and optional hardware video codecs, voice/video components,
image decoders, fallback fonts and the pelican icon are included. No Rust compiler,
GTK development packages or local Papo backend are needed. The launcher uses the
desktop's fonts, rendering settings and native monitor scaling without forcing
XWayland or integer scaling. Core font libraries (Fontconfig/FreeType/HarfBuzz)
and graphics drivers remain supplied by your current desktop system.

VA-API and NVIDIA decoder plugins are included. Hardware video decoding needs a
compatible GPU and its video drivers installed on the desktop. GPU drivers are
not included. Software video decoding remains available when acceleration is
unavailable. --runtime-check reports the registered hardware decoders; an empty
list is normal when a corresponding GPU/driver is absent. It can probe GPU
capabilities but does not capture microphone, camera or screen contents.
Calls and screen sharing currently use software VP8 encoding/decoding.

Screen sharing needs PipeWire and a ScreenCast portal backend (normally
xdg-desktop-portal-kde on KDE). Remembered login requires a Secret Service keyring.
The package contains no account credentials.

GTK input-method clients for IBus and Fcitx5 are included. Your desktop chooses
the input method and supplies its service. Please check dead-key accents such as
acute followed by a/e, including login fields. In chat, Enter sends and Shift+Enter
adds a line break. Long text wraps and expands the editor before scrolling
vertically. Enter should commit an active input-method candidate before sending.

Without FUSE:
   ./Papo-@VERSION@-preview-@BUILD@-x86_64.AppImage --appimage-extract-and-run

Runtime check (no microphone, camera or screen picker):
   ./Papo-@VERSION@-preview-@BUILD@-x86_64.AppImage --runtime-check
Font/backend/scaling diagnostics (brief sample window):
   ./Papo-@VERSION@-preview-@BUILD@-x86_64.AppImage --font-check
   ./Papo-@VERSION@-preview-@BUILD@-x86_64.AppImage --font-check --snapshot /tmp/papo-fonts.png
Use --appimage-extract-and-run before these arguments when FUSE is unavailable.

If fonts still look blurry, report the --font-check output, desktop version,
monitor scaling, graphics driver and whether the session is Wayland or X11.
Please also test login, messages, embeds, images, clipboard uploads, voice,
cameras and screen sharing using your own accounts.

The frontend is GPL-3.0-only. Matching source is in
papo-gtk-@VERSION@-preview-@BUILD@-source.tar.gz. Runtime licenses, exact Fedora/RPM
Fusion package versions and build information are inside usr/share/licenses and
usr/share/papo-gtk. Source links and the build recipe are in APPIMAGE.md.
