#!/usr/bin/env python3
"""Assemble the AppDir from the isolated Ubuntu build, never from personal data."""

import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

PROJECT = Path("/work")
APPDIR = PROJECT / "target/appimage/AppDir"
LIB = APPDIR / "usr/lib"
EXCLUDE = re.compile(r"^(ld-linux.*|lib(c|m|dl|pthread|rt|resolv|util|anl)\.so.*|lib(GL|EGL|GLX|GLdispatch|GLESv2)\.so.*)$")
native = []
copied_libraries = set()


def copy(source, target):
    source, target = Path(source), Path(target)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)
    if source.read_bytes()[:4] == b"\x7fELF":
        native.append(target)


def tree(source, target):
    shutil.copytree(source, target, dirs_exist_ok=True,
        ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "test", "tests"))


def dependencies(binary):
    result = subprocess.run(["ldd", str(binary)], text=True, capture_output=True)
    if result.returncode and "not a dynamic executable" not in result.stderr + result.stdout:
        raise RuntimeError(f"ldd failed for {binary}: {result.stderr}")
    for line in result.stdout.splitlines():
        if "=> not found" in line:
            raise RuntimeError(f"Unresolved build dependency: {binary}: {line}")
        match = re.match(r"\s*(\S+) => (/\S+) ", line)
        if not match:
            continue
        name, location = match.groups()
        if EXCLUDE.match(name) or name in copied_libraries:
            continue
        copied_libraries.add(name)
        target = LIB / name
        copy(location, target)
        dependencies(target)


def main():
    if APPDIR.exists():
        shutil.rmtree(APPDIR)
    LIB.mkdir(parents=True)
    copy(PROJECT / "target/appimage/release/papo-gtk", APPDIR / "usr/bin/papo-gtk")
    copy("/usr/bin/python3.12", APPDIR / "usr/bin/python3")
    tree("/usr/lib/python3.12", LIB / "python3.12")
    for module in ("gi", "cairo"):
        tree(Path("/usr/lib/python3/dist-packages") / module, LIB / "python3/dist-packages" / module)
    tree("/usr/lib/x86_64-linux-gnu/girepository-1.0", LIB / "girepository-1.0")

    # Base/good codecs plus the WebRTC, parser and software H.264 components.
    packages = ["libgstreamer1.0-0", "gstreamer1.0-plugins-base", "gstreamer1.0-plugins-good", "gstreamer1.0-libav", "gstreamer1.0-nice", "gstreamer1.0-pipewire"]
    files = subprocess.check_output(["dpkg-query", "-L", *packages], text=True).splitlines()
    plugins = {Path(file) for file in files if "/gstreamer-1.0/" in file and file.endswith(".so")}
    for name in ("dtls", "srtp", "webrtc", "videoparsersbad", "opusparse", "openh264"):
        plugins.add(Path(f"/usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgst{name}.so"))
    for plugin in sorted(plugins):
        copy(plugin, LIB / "gstreamer-1.0" / plugin.name)
    copy("/usr/lib/x86_64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner", APPDIR / "usr/libexec/gstreamer-1.0/gst-plugin-scanner")
    tree("/usr/local/lib/gtk-4.0", LIB / "gtk-4.0")
    tree("/usr/lib/x86_64-linux-gnu/gio/modules", LIB / "gio/modules")
    tree("/usr/lib/x86_64-linux-gnu/pipewire-0.3", LIB / "pipewire-0.3")
    tree("/usr/share/pipewire", APPDIR / "usr/share/pipewire")
    for component in ("support", "audioconvert", "audiomixer", "videoconvert"):
        source = Path("/usr/lib/x86_64-linux-gnu/spa-0.2") / component
        if source.exists():
            tree(source, LIB / "spa-0.2" / component)
    tree("/usr/lib/x86_64-linux-gnu/gdk-pixbuf-2.0/2.10.0/loaders", LIB / "gdk-pixbuf-2.0/2.10.0/loaders")
    copy("/usr/lib/x86_64-linux-gnu/gdk-pixbuf-2.0/gdk-pixbuf-query-loaders", APPDIR / "usr/bin/gdk-pixbuf-query-loaders")

    for source in (Path("/usr/share/glib-2.0/schemas"), Path("/usr/local/share/glib-2.0/schemas")):
        tree(source, APPDIR / "usr/share/glib-2.0/schemas")
    subprocess.run(["glib-compile-schemas", str(APPDIR / "usr/share/glib-2.0/schemas")], check=True)
    tree("/usr/share/icons/Adwaita", APPDIR / "usr/share/icons/Adwaita")
    tree("/usr/share/icons/hicolor", APPDIR / "usr/share/icons/hicolor")
    tree(PROJECT / "assets/icons/hicolor", APPDIR / "usr/share/icons/hicolor")
    tree("/usr/share/fonts/truetype/dejavu", APPDIR / "usr/share/fonts/truetype/dejavu")
    fontconfig = APPDIR / "usr/etc/fonts/fonts.conf"
    fontconfig.parent.mkdir(parents=True)
    fontconfig.write_text('''<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <dir prefix="relative">../../share/fonts</dir>
  <dir>/usr/share/fonts</dir>
  <dir prefix="xdg">fonts</dir>
  <cachedir prefix="xdg">fontconfig</cachedir>
  <include ignore_missing="yes">/etc/fonts/conf.d</include>
</fontconfig>
''')
    for name in ("runtime-env.sh", "runtime-check.py"):
        copy(PROJECT / "packaging/appimage" / name, APPDIR / "usr/share/papo-gtk" / name)
    copy("/etc/ssl/certs/ca-certificates.crt", APPDIR / "usr/share/papo-gtk/ca-certificates.crt")
    copy(PROJECT / "LICENSE", APPDIR / "usr/share/licenses/papo-gtk/LICENSE")
    # Preserve the runtime packages' license notices and exact build versions.
    for notice in Path("/usr/share/doc").glob("*/copyright"):
        copy(notice, APPDIR / "usr/share/licenses/ubuntu" / notice.parent.name / "copyright")
    manifest = subprocess.check_output(["dpkg-query", "-W", "-f=${binary:Package}\t${Version}\t${source:Package}\t${source:Version}\n"], text=True)
    (APPDIR / "usr/share/papo-gtk/ubuntu-packages.tsv").write_text(manifest)

    for binary in LIB.rglob("*.so*"):
        if binary.is_file() and binary.read_bytes()[:4] == b"\x7fELF":
            native.append(binary)
    for binary in list(native):
        dependencies(binary)
    # These are loaded through ctypes / GI or by OpenSSL at runtime.
    for source in ("/usr/local/lib/libadwaita-1.so.0", "/usr/local/lib/libgtk-4.so.1"):
        copy(source, LIB / Path(source).name)
        dependencies(LIB / Path(source).name)
    for module in Path("/usr/lib/x86_64-linux-gnu/ossl-modules").glob("*.so"):
        copy(module, LIB / "ossl-modules" / module.name)
        dependencies(LIB / "ossl-modules" / module.name)

    copy(PROJECT / "data/br.com.papo.gtk.desktop", APPDIR / "br.com.papo.gtk.desktop")
    copy(PROJECT / "assets/icons/hicolor/256x256/apps/br.com.papo.gtk.png", APPDIR / "br.com.papo.gtk.png")
    (APPDIR / ".DirIcon").symlink_to("br.com.papo.gtk.png")
    copy(PROJECT / "packaging/appimage/AppRun", APPDIR / "AppRun")
    (APPDIR / "AppRun").chmod(0o755)
    (APPDIR / "usr/bin/papo-gtk").chmod(0o755)
    (APPDIR / "usr/bin/python3").chmod(0o755)
    (APPDIR / "usr/share/papo-gtk/build-info.json").write_text(json.dumps({
        "baseline": "Ubuntu 24.04, glibc 2.39, x86_64", "gtk": "4.16.13",
        "libadwaita": "1.6.10", "rust": "1.90.0", "libraries": sorted(copied_libraries),
        "host_services": ["Wayland or X11", "audio server", "desktop portal/PipeWire for screen sharing", "Secret Service for remembered login"]
    }, indent=2))
    print(f"AppDir assembled: {APPDIR} ({len(copied_libraries)} runtime libraries)")


if __name__ == "__main__":
    main()
