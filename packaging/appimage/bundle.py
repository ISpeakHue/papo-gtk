#!/usr/bin/env python3
"""Assemble a modern Fedora AppDir from an isolated builder, never personal data."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import ssl
import subprocess
import sys
import sysconfig

from media_policy import HARDWARE_PLUGINS, HOST_DRIVER

PROJECT = Path('/work')
BUILD = PROJECT / 'target/appimage-fedora44'
APPDIR = BUILD / 'AppDir'
LIB = APPDIR / 'usr/lib64'
# Use the modern desktop's core font libraries as well as graphics drivers.
# A bundled older Fontconfig cannot understand newer Arch font rules reliably.
EXCLUDE = re.compile(r'^(ld-linux.*|lib(c|m|dl|pthread|rt|resolv|util|anl)\.so.*|lib(GL|EGL|GLX|GLdispatch|GLESv2|OpenGL|vulkan|gbm|drm|glapi|fontconfig|freetype|harfbuzz|harfbuzz-subset)\.so.*)$')
native = []
copied_libraries = set()


def elf(path):
    with Path(path).open('rb') as stream:
        return stream.read(4) == b'\x7fELF'


def copy(source, target):
    source, target = Path(source), Path(target)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)
    if elf(source):
        native.append(target)


def tree(source, target, optional=False):
    source = Path(source)
    if optional and not source.exists():
        return
    shutil.copytree(source, target, dirs_exist_ok=True,
        ignore=shutil.ignore_patterns('__pycache__', '*.pyc', 'test', 'tests', 'site-packages'))


def dependencies(binary):
    result = subprocess.run(['ldd', str(binary)], text=True, capture_output=True)
    if result.returncode and 'not a dynamic executable' not in result.stderr + result.stdout:
        raise RuntimeError(f'ldd failed for {binary}: {result.stderr}')
    for line in result.stdout.splitlines():
        if '=> not found' in line:
            raise RuntimeError(f'Unresolved build dependency: {binary}: {line}')
        match = re.match(r'\s*(\S+) => (/\S+) ', line)
        if not match:
            continue
        name, location = match.groups()
        if EXCLUDE.match(name) or HOST_DRIVER.fullmatch(name) or name in copied_libraries:
            continue
        copied_libraries.add(name)
        target = LIB / name
        copy(location, target)
        dependencies(target)


def version(package):
    return subprocess.check_output(['pkg-config', '--modversion', package], text=True).strip()


def main():
    if APPDIR.exists():
        shutil.rmtree(APPDIR)
    LIB.mkdir(parents=True)
    copy(BUILD / 'release/papo-gtk', APPDIR / 'usr/bin/papo-gtk')
    copy(sys.executable, APPDIR / 'usr/bin/python3')
    python_version = sysconfig.get_python_version()
    tree(sysconfig.get_path('stdlib'), LIB / f'python{python_version}')
    for module in ('gi', 'cairo'):
        source = Path(importlib.util.find_spec(module).origin).parent
        tree(source, LIB / 'python3/site-packages' / module)
    tree('/usr/lib64/girepository-1.0', LIB / 'girepository-1.0')

    packages = ['gstreamer1', 'gstreamer1-plugins-base', 'gstreamer1-plugins-good',
        'gstreamer1-plugin-libav', 'libnice-gstreamer1', 'pipewire-gstreamer']
    files = subprocess.check_output(['rpm', '-ql', *packages], text=True).splitlines()
    plugins = {Path(file) for file in files if '/gstreamer-1.0/' in file and file.endswith('.so')}
    for name in ('dtls', 'srtp', 'webrtc', 'videoparsersbad', 'opusparse'):
        plugins.add(Path(f'/usr/lib64/gstreamer-1.0/libgst{name}.so'))
    plugins.update(Path('/usr/lib64/gstreamer-1.0') / name for name in HARDWARE_PLUGINS.values())
    for plugin in sorted(plugins):
        copy(plugin, LIB / 'gstreamer-1.0' / plugin.name)
    copy('/usr/libexec/gstreamer-1.0/gst-plugin-scanner', APPDIR / 'usr/libexec/gstreamer-1.0/gst-plugin-scanner')
    tree('/usr/lib64/gtk-4.0', LIB / 'gtk-4.0', optional=True)
    tree('/usr/lib64/gio/modules', LIB / 'gio/modules')
    tree('/usr/lib64/pipewire-0.3', LIB / 'pipewire-0.3')
    tree('/usr/share/pipewire', APPDIR / 'usr/share/pipewire')
    tree('/usr/share/mime', APPDIR / 'usr/share/mime')
    for component in ('support', 'audioconvert', 'audiomixer', 'videoconvert'):
        tree(Path('/usr/lib64/spa-0.2') / component, LIB / 'spa-0.2' / component, optional=True)
    tree('/usr/lib64/gdk-pixbuf-2.0/2.10.0/loaders', LIB / 'gdk-pixbuf-2.0/2.10.0/loaders')
    copy('/usr/bin/gdk-pixbuf-query-loaders-64', APPDIR / 'usr/bin/gdk-pixbuf-query-loaders')
    tree('/usr/share/glycin-loaders', APPDIR / 'usr/share/glycin-loaders')
    tree('/usr/libexec/glycin-loaders', APPDIR / 'usr/libexec/glycin-loaders')
    for loader in (APPDIR / 'usr/libexec/glycin-loaders').rglob('glycin-*'):
        if loader.is_file() and elf(loader):
            native.append(loader)
    copy('/usr/bin/bwrap', APPDIR / 'usr/libexec/bwrap.real')
    copy(PROJECT / 'packaging/appimage/bwrap', APPDIR / 'usr/bin/bwrap')
    (APPDIR / 'usr/bin/bwrap').chmod(0o755)
    for name in ('fc-match', 'fc-list', 'fc-conflist'):
        copy('/usr/bin/' + name, APPDIR / 'usr/bin' / name)

    tree('/usr/share/glib-2.0/schemas', APPDIR / 'usr/share/glib-2.0/schemas')
    subprocess.run(['glib-compile-schemas', str(APPDIR / 'usr/share/glib-2.0/schemas')], check=True)
    tree('/usr/share/icons/Adwaita', APPDIR / 'usr/share/icons/Adwaita')
    tree('/usr/share/icons/hicolor', APPDIR / 'usr/share/icons/hicolor')
    tree(PROJECT / 'assets/icons/hicolor', APPDIR / 'usr/share/icons/hicolor')
    for name in ('adwaita-sans-fonts', 'adwaita-mono-fonts', 'google-noto-sans-vf-fonts',
                 'google-noto-color-emoji-fonts', 'dejavu-sans-fonts'):
        files = subprocess.check_output(['rpm', '-ql', name], text=True).splitlines()
        for file in files:
            source = Path(file)
            if source.suffix.lower() in ('.ttf', '.otf', '.ttc'):
                copy(source, APPDIR / 'usr/share/fonts' / source.relative_to('/usr/share/fonts'))
    for locale in ('pt', 'pt_BR', 'en', 'en_GB'):
        tree(Path('/usr/share/locale') / locale, APPDIR / 'usr/share/locale' / locale, optional=True)
    fontconfig = APPDIR / 'usr/etc/fonts/fonts.conf'
    fontconfig.parent.mkdir(parents=True)
    fontconfig.write_text('''<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <!-- Load complete desktop configuration, including user rules and fonts. -->
  <include ignore_missing="yes">/etc/fonts/fonts.conf</include>
  <!-- Fallbacks supplement system fonts; they do not replace desktop choices. -->
  <dir prefix="relative">../../share/fonts</dir>
  <cachedir prefix="xdg">fontconfig</cachedir>
</fontconfig>
''')
    for name in ('runtime-env.sh', 'runtime-check.py', 'font-check.py', 'loader-config.py', 'media_policy.py'):
        copy(PROJECT / 'packaging/appimage' / name, APPDIR / 'usr/share/papo-gtk' / name)
    copy(PROJECT / 'tests/fixtures/chat-h264.mp4', APPDIR / 'usr/share/papo-gtk/chat-h264.mp4')
    copy(PROJECT / 'assets/status/busy.svg', APPDIR / 'usr/share/papo-gtk/status-check.svg')
    certificate_file = ssl.get_default_verify_paths().cafile or '/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem'
    copy(certificate_file, APPDIR / 'usr/share/papo-gtk/ca-certificates.crt')
    copy(PROJECT / 'LICENSE', APPDIR / 'usr/share/licenses/papo-gtk/LICENSE')
    tree('/usr/share/licenses', APPDIR / 'usr/share/licenses/fedora')
    manifest = subprocess.check_output(['rpm', '-qa', '--qf', '%{NAME}\t%{VERSION}-%{RELEASE}\t%{ARCH}\t%{SOURCERPM}\t%{LICENSE}\n'], text=True)
    (APPDIR / 'usr/share/papo-gtk/fedora-packages.tsv').write_text(''.join(sorted(manifest.splitlines(keepends=True))))

    for binary in LIB.rglob('*.so*'):
        if binary.is_file() and elf(binary):
            native.append(binary)
    for binary in list(native):
        dependencies(binary)
    # Keep libraries loaded through GI/ctypes, even if the frontend changes links.
    for source in ('/usr/lib64/libadwaita-1.so.0', '/usr/lib64/libgtk-4.so.1'):
        copy(source, LIB / Path(source).name)
        dependencies(LIB / Path(source).name)
    for module in Path('/usr/lib64/ossl-modules').glob('*.so'):
        copy(module, LIB / 'ossl-modules' / module.name)
        dependencies(LIB / 'ossl-modules' / module.name)

    copy(PROJECT / 'data/br.com.papo.gtk.desktop', APPDIR / 'br.com.papo.gtk.desktop')
    copy(PROJECT / 'assets/icons/hicolor/256x256/apps/br.com.papo.gtk.png', APPDIR / 'br.com.papo.gtk.png')
    (APPDIR / '.DirIcon').symlink_to('br.com.papo.gtk.png')
    copy(PROJECT / 'packaging/appimage/AppRun', APPDIR / 'AppRun')
    for binary in (APPDIR / 'AppRun', APPDIR / 'usr/bin/papo-gtk', APPDIR / 'usr/bin/python3'):
        binary.chmod(0o755)
    (APPDIR / 'usr/share/papo-gtk/build-info.json').write_text(json.dumps({
        'baseline': 'Fedora 44, x86_64', 'glibc_minimum': '2.43',
        'gtk': version('gtk4'), 'libadwaita': version('libadwaita-1'),
        'pango': version('pango'), 'fontconfig_build_version': version('fontconfig'),
        'font_libraries': 'native desktop Fontconfig/FreeType/HarfBuzz',
        'rust': subprocess.check_output(['rustc', '--version'], text=True).strip(),
        'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
        'source_dirty': bool(subprocess.check_output(['git', 'status', '--porcelain'], text=True)),
        'binary_sha256': hashlib.sha256((APPDIR / 'usr/bin/papo-gtk').read_bytes()).hexdigest(),
        'libraries': sorted(copied_libraries),
        'hardware_media_plugins': HARDWARE_PLUGINS,
        'hardware_media_driver_policy': 'VA-API and CUDA/NVDEC drivers supplied by host; automatic decoder selection',
        'font_policy': 'desktop/user configuration plus bundled fallback fonts; native renderer and scaling',
        'host_services': ['Wayland or X11', 'graphics drivers', 'native Fontconfig/FreeType/HarfBuzz', 'audio server',
            'desktop portal/PipeWire for screen sharing', 'Secret Service for remembered login']
    }, indent=2))
    print(f'AppDir assembled: {APPDIR} ({len(copied_libraries)} runtime libraries)')


if __name__ == '__main__':
    main()
