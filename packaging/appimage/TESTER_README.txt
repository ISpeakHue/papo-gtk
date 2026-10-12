Papo GTK — Linux x86_64 test build

1. Download Papo-0.1.0-preview-x86_64.AppImage.
2. Make it executable in your file manager's Properties/Permissions, or run:
   chmod +x Papo-0.1.0-preview-x86_64.AppImage
3. Double-click it, or run:
   ./Papo-0.1.0-preview-x86_64.AppImage
4. Enter the backend API URL, for example https://api.cyberasilo.online,
   and your own account/server credentials.

GTK, libadwaita, Python GI, voice/video codecs and the pelican icon are bundled.
No Rust compiler, local Papo backend or GTK development packages are needed.
This build targets x86_64 Linux with glibc 2.39 or newer. It is intended for
current Fedora KDE, Arch Linux and other compatible desktop systems.

The desktop still supplies graphics drivers, audio services and D-Bus.
Screen sharing needs PipeWire and a ScreenCast desktop portal backend;
on KDE this is normally xdg-desktop-portal-kde. Remembered login needs
a working Secret Service keyring. The package contains no account credentials.

If your system cannot mount AppImages using FUSE, run:
   ./Papo-0.1.0-preview-x86_64.AppImage --appimage-extract-and-run

For a local diagnostic check without microphone/camera access:
   ./Papo-0.1.0-preview-x86_64.AppImage --runtime-check
Use --appimage-extract-and-run before --runtime-check when FUSE is unavailable.

Please report your distribution, desktop, X11/Wayland session, and exact error
when testing login, messages, embeds, uploads, voice, cameras and screen sharing.
Physical-device and cross-desktop results still need testing with your accounts.

This frontend is GPL-3.0-only. Matching frontend source is supplied in
papo-gtk-0.1.0-source.tar.gz. Runtime license notices, package versions and build
information are included under usr/share/licenses and usr/share/papo-gtk inside
the AppImage. Ubuntu runtime sources are available from the Ubuntu package
archive; GTK/libadwaita sources and the build recipe are identified in the source
archive under packaging/appimage. The AppImage runtime is provided by AppImage.
