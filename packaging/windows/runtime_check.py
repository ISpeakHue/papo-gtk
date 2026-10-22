"""EN-US: Offline acceptance of the shipped Python/GI, icons, codecs and DLL origins.
PT-BR: Aceitação offline de Python/GI, ícones, codecs e origens das DLLs distribuídos.
"""
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


def trusted_codec_roots(packages, program_files):
    """EN-US: Recognize only registered, Microsoft Store-signed OS codec extensions.
    PT-BR: Reconhece apenas extensões de codecs registradas e assinadas pela Microsoft Store.
    """
    names = {'Microsoft.HEVCVideoExtension', 'Microsoft.VP9VideoExtensions'}
    base = program_files / 'WindowsApps'
    roots = []
    for package in packages:
        name = package.get('Name')
        location = Path(package.get('InstallLocation') or '.')
        if (name in names and package.get('PublisherId') == '8wekyb3d8bbwe'
                and package.get('PackageFamilyName') == name + '_8wekyb3d8bbwe'
                and package.get('SignatureKind') in {'Store', 'System'}
                and package.get('Status') == 'Ok' and location.is_absolute()
                and location.is_relative_to(base) and len(location.relative_to(base).parts) == 1
                and location.name.startswith(name + '_') and location.name.endswith('__8wekyb3d8bbwe')):
            roots.append(location)
    return roots


def os_codec_roots():
    shell = Path(os.environ['SystemRoot']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
    # EN-US: Query registered packages offline; a path/name alone does not establish OS ownership.
    # PT-BR: Consulta pacotes registrados offline; só o caminho/nome não comprova origem do sistema.
    command = """[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$ErrorActionPreference = 'Stop'
$packages = @(Get-AppxPackage -PackageTypeFilter Main | Select-Object Name,PackageFamilyName,PublisherId,InstallLocation,@{N='SignatureKind';E={$_.SignatureKind.ToString()}},@{N='Status';E={$_.Status.ToString()}})
ConvertTo-Json -InputObject $packages -Compress
"""
    result = subprocess.run([str(shell), '-NoProfile', '-Command', command], check=True,
                            capture_output=True, encoding='utf-8-sig', timeout=20,
                            creationflags=subprocess.CREATE_NO_WINDOW)
    packages = json.loads(result.stdout)
    assert isinstance(packages, list), 'Invalid registered-package inventory'
    return trusted_codec_roots(packages, Path(os.environ['ProgramFiles']).resolve())


def loaded_modules():
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    psapi = ctypes.WinDLL('psapi', use_last_error=True)
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    process = kernel.GetCurrentProcess()
    modules = (wintypes.HMODULE * 2048)()
    needed = wintypes.DWORD()
    psapi.EnumProcessModulesEx.argtypes = (wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), wintypes.DWORD)
    if not psapi.EnumProcessModulesEx(process, modules, ctypes.sizeof(modules), ctypes.byref(needed), 3):
        raise ctypes.WinError(ctypes.get_last_error())
    assert needed.value <= ctypes.sizeof(modules), 'Loaded-module audit buffer exhausted'
    psapi.GetModuleFileNameExW.argtypes = (wintypes.HANDLE, wintypes.HMODULE, wintypes.LPWSTR, wintypes.DWORD)
    names = []
    for module in modules[:needed.value // ctypes.sizeof(wintypes.HMODULE)]:
        buffer = ctypes.create_unicode_buffer(32768)
        if not psapi.GetModuleFileNameExW(process, module, buffer, len(buffer)):
            raise ctypes.WinError(ctypes.get_last_error())
        names.append(Path(buffer.value).resolve())
    return names


def main():
    binary = Path(sys.argv[1]).resolve()
    root = binary.parent.parent
    stage = root.parent
    manifest = json.loads((root / 'RUNTIME-MANIFEST.json').read_text(encoding='utf-8'))
    build = json.loads((stage / 'BUILD-INFO.json').read_text(encoding='utf-8'))
    digest = hashlib.sha256((root / 'RUNTIME-MANIFEST.json').read_bytes()).hexdigest()
    assert digest == build['runtime']['manifest_sha256'], 'Runtime manifest changed'
    for entry in manifest['files']:
        path = (stage / entry['path']).resolve()
        assert path.is_relative_to(stage), 'Manifest path escapes package'
        assert path.stat().st_size == entry['size'], f'Runtime file size mismatch: {entry["path"]}'
        with path.open('rb') as stream:
            assert hashlib.file_digest(stream, 'sha256').hexdigest() == entry['sha256'], f'Runtime checksum mismatch: {entry["path"]}'
    assert Path(sys.executable).resolve().is_relative_to(root), 'System Python was selected'
    assert Path(sys.prefix).resolve() == root, ('Wrong Python prefix', sys.prefix)
    for path in sys.path:
        assert path and Path(path).resolve().is_relative_to(root), ('External Python import path', path)
    import gi
    import cairo
    for namespace, version in [('Gst', '1.0'), ('GstVideo', '1.0'), ('GstApp', '1.0'),
                               ('GstPbutils', '1.0'), ('GstWebRTC', '1.0'), ('GstSdp', '1.0'),
                               ('Gtk', '4.0'), ('Adw', '1'), ('GdkPixbuf', '2.0')]:
        gi.require_version(namespace, version)
    from gi.repository import Gst, GstVideo, GstApp, GstPbutils, GstWebRTC, GstSdp, Gtk, Adw, GdkPixbuf, Gio
    Gst.init(None)
    factories = ('opusenc', 'opusdec', 'vp8enc', 'vp8dec', 'webrtcbin', 'nicesrc', 'nicesink', 'dtlsenc', 'dtlsdec', 'wasapisrc', 'wasapisink')
    for name in factories:
        assert Gst.ElementFactory.find(name), f'Missing packaged media factory: {name}'
    webrtc = Gst.ElementFactory.make('webrtcbin')
    assert webrtc is not None
    for plugin in Gst.Registry.get().get_plugin_list():
        filename = plugin.get_filename()
        if filename:
            assert Path(filename).resolve().is_relative_to(root), ('External GStreamer plugin', filename)
    fixtures = root / 'share/papo-gtk/check-fixtures'

    def decode(description):
        pipeline = Gst.parse_launch(description)
        try:
            pipeline.set_state(Gst.State.PLAYING)
            message = pipeline.get_bus().timed_pop_filtered(12 * Gst.SECOND, Gst.MessageType.ERROR | Gst.MessageType.EOS)
            assert message is not None, 'Packaged media decode timed out'
            if message.type == Gst.MessageType.ERROR:
                raise RuntimeError(str(message.parse_error()))
        finally:
            pipeline.set_state(Gst.State.NULL)

    for name in ('chat-h264.mp4', 'chat-video.mov', 'chat-video.webm'):
        uri = Gst.filename_to_uri(str(fixtures / name))
        decode(f'uridecodebin uri="{uri}" ! videoconvert ! fakesink sync=false')
    uri = Gst.filename_to_uri(str(fixtures / 'voice.wav'))
    decode(f'uridecodebin uri="{uri}" ! audioconvert ! audioresample ! opusenc ! opusdec ! fakesink sync=false')
    import importlib.util
    spec = importlib.util.spec_from_file_location('runtime_icons', Path(__file__).with_name('runtime_icons.py'))
    runtime_icons = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runtime_icons)
    previous = sys.argv
    try:
        sys.argv = ['runtime_icons', '--binary', str(binary)]
        runtime_icons.main()
    finally:
        sys.argv = previous
    schema = Gio.SettingsSchemaSource.new_from_directory(str(root / 'share/glib-2.0/schemas'), None, False)
    assert schema is not None
    loaded = loaded_modules()
    windows = Path(os.environ['SystemRoot']).resolve()
    external = [path for path in loaded if not path.is_relative_to(root) and not path.is_relative_to(windows)]
    codecs = os_codec_roots() if external else []
    system_codecs = [path for path in external if any(path.is_relative_to(codec) for codec in codecs)]
    external = [path for path in external if path not in system_codecs]
    assert not external, ('External runtime DLLs loaded', external)
    print(json.dumps({'status': 'ok', 'python': sys.version.split()[0], 'prefix': str(root),
                      'gtk': f'{Gtk.get_major_version()}.{Gtk.get_minor_version()}.{Gtk.get_micro_version()}',
                      'gstreamer': Gst.version_string(), 'checked_files': len(manifest['files']),
                      'loaded_modules': len(loaded), 'media_fixtures': 4,
                      'os_codec_modules': [str(path) for path in system_codecs],
                      'runtime_cache': os.environ.get('GST_REGISTRY_1_0')}, ensure_ascii=False))


if __name__ == '__main__':
    main()
