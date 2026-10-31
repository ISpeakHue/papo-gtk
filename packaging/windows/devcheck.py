#!/usr/bin/env python3
"""EN-US: Check a local UCRT64 build without accounts, devices or desktop capture.
PT-BR: Verifica uma compilação UCRT64 local sem contas, dispositivos ou captura de tela.
"""
import argparse
from contextlib import contextmanager
import ctypes
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

from build import ROOT, run, toolchain, write_json

GTK_PACKAGE = 'mingw-w64-ucrt-x86_64-gtk4'
FACTORIES = ('opusenc', 'opusdec', 'vp8enc', 'vp8dec', 'webrtcbin', 'nicesrc',
             'nicesink', 'dtlsenc', 'dtlsdec', 'mfvideosrc', 'jpegdec', 'decodebin',
             'volume', 'level', 'appsrc')


def expected_gtk():
    recipe = (ROOT / 'packaging/windows/gtk/PKGBUILD').read_text(encoding='utf-8')
    version = re.search(r'^pkgver=([\d.]+)$', recipe, re.MULTILINE)
    revision = re.search(r'^pkgrel=([\d.]+)$', recipe, re.MULTILINE)
    if not version or not revision:
        raise RuntimeError('Cannot identify the reviewed GTK recipe version.')
    return f'{version[1]}-{revision[1]}'


def require_gtk(inventory):
    expected = f'{GTK_PACKAGE} {expected_gtk()}'
    if inventory.strip() != expected:
        raise RuntimeError(f'GTK must match the reviewed IME-fixed recipe: {expected}. '
                           'Run scripts/windows-dev.ps1 Setup; unreviewed GTK changes require a recipe review.')


def require_factories(gst):
    missing = [name for name in FACTORIES if not gst.ElementFactory.find(name)]
    for choices in (('wasapi2src', 'wasapisrc'), ('wasapi2sink', 'wasapisink')):
        if not any(gst.ElementFactory.find(name) for name in choices):
            missing.append(' or '.join(choices))
    if missing:
        raise RuntimeError('Missing GStreamer factories: ' + ', '.join(missing))


def hip_allowlist(directory):
    names = sorted(path.stem[6:] for path in Path(directory).glob('libgst*.dll')
                   if path.name.lower() != 'libgsthip.dll')
    if not names or any(not re.fullmatch(r'[A-Za-z0-9_-]+', name) for name in names):
        raise RuntimeError('Cannot derive the developer plugin list from UCRT64.')
    return ','.join(names)


@contextmanager
def probe_job(process):
    # EN-US: The child waits on stdin until its scanner descendants belong to this Job.
    # PT-BR: O filho aguarda stdin até que os scanners descendentes pertençam a este Job.
    class Basic(ctypes.Structure):
        _fields_ = [('times', ctypes.c_int64 * 2), ('flags', ctypes.c_uint32),
                    ('minimum', ctypes.c_size_t), ('maximum', ctypes.c_size_t),
                    ('active', ctypes.c_uint32), ('affinity', ctypes.c_size_t),
                    ('priority', ctypes.c_uint32), ('scheduling', ctypes.c_uint32)]
    class Limits(ctypes.Structure):
        _fields_ = [('basic', Basic), ('io', ctypes.c_uint64 * 6), ('memory', ctypes.c_size_t * 4)]
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.CreateJobObjectW.argtypes = (ctypes.c_void_p, ctypes.c_wchar_p)
    kernel.CreateJobObjectW.restype = ctypes.c_void_p
    kernel.SetInformationJobObject.argtypes = (ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32)
    kernel.AssignProcessToJobObject.argtypes = (ctypes.c_void_p, ctypes.c_void_p)
    kernel.CloseHandle.argtypes = (ctypes.c_void_p,)
    job = kernel.CreateJobObjectW(None, None)
    if not job:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        limits = Limits()
        limits.basic.flags = 0x2000  # EN-US: Kill on Job close. PT-BR: Encerra ao fechar o Job.
        if not kernel.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)) \
                or not kernel.AssignProcessToJobObject(job, int(process._handle)):
            raise ctypes.WinError(ctypes.get_last_error())
        yield
    finally:
        kernel.CloseHandle(job)


def native_media():
    import gi
    for namespace, version in (('Gst', '1.0'), ('GstWebRTC', '1.0'), ('GstApp', '1.0'),
                               ('GstSdp', '1.0'), ('Gtk', '4.0'), ('Adw', '1')):
        gi.require_version(namespace, version)
    from gi.repository import Gst, Gtk, Adw
    # EN-US: Factory discovery does not construct pipelines or open capture devices/windows.
    # PT-BR: Descobrir fábricas não cria pipelines nem abre dispositivos/janelas de captura.
    Gst.init(None)
    require_factories(Gst)
    return {'gstreamer': Gst.version_string(), 'gtk': [Gtk.get_major_version(), Gtk.get_minor_version(), Gtk.get_micro_version()],
            'libadwaita': [Adw.get_major_version(), Adw.get_minor_version(), Adw.get_micro_version()],
            'factories': list(FACTORIES)}


def media(timeout=45, command=None):
    command = command or [sys.executable, '-B', '-s', str(Path(__file__).resolve()), '--media-child']
    # EN-US: Files avoid blocked diagnostic pipes; reads are capped and the Job owns all descendants.
    # PT-BR: Arquivos evitam bloqueios em pipes; leituras têm limites e o Job controla os descendentes.
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=output, stderr=errors,
                                 creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            with probe_job(child):
                child.stdin.write(b'1')
                child.stdin.close()
                try:
                    child.wait(timeout=timeout)
                except subprocess.TimeoutExpired as error:
                    raise RuntimeError('GStreamer discovery timed out; its probe/scanner Job was stopped. '
                                       'Inspect driver/plugin diagnostics. Use -ExcludeHip only for a diagnosed HIP stall.') from error
            if child.returncode:
                errors.seek(0)
                raise RuntimeError('Native media probe failed: ' + errors.read(4096).decode('utf-8', errors='replace'))
            output.seek(0)
            data = output.read(65537)
            if len(data) > 65536:
                raise RuntimeError('Native media probe output exceeded its limit.')
            return json.loads(data)
        finally:
            if child.stdin and not child.stdin.closed:
                child.stdin.close()
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)


def inspect():
    report = {'schema': 1, 'status': 'fail', 'checks': {}, 'errors': []}
    if sys.platform != 'win32' or os.environ.get('MSYSTEM') != 'UCRT64':
        report['errors'].append('Use scripts/windows-dev.ps1 or an MSYS2 UCRT64 terminal.')
        return report
    try:
        if os.environ.get('CARGO_BUILD_TARGET') not in (None, '', 'x86_64-pc-windows-gnu'):
            raise RuntimeError('Remove CARGO_BUILD_TARGET or select x86_64-pc-windows-gnu; MSVC/cross targets cannot use the UCRT64 runtime.')
        # EN-US: Reuse release packaging's same-prefix and GNU Rust target validation.
        # PT-BR: Reutiliza a validação do empacotamento: mesmo prefixo e alvo Rust GNU.
        report['checks']['toolchain'] = toolchain()
        prefix = Path(sys.executable).resolve().parent
        for name in ('windres', 'gdb'):
            path = shutil.which(name)
            if not path or Path(path).resolve().parent != prefix:
                raise RuntimeError(f'{name} must come from the same UCRT64 bin directory as Python.')
        if not shutil.which('git') or not shutil.which('pacman'):
            raise RuntimeError('MSYS2 Git and pacman must be available in the UCRT64 shell.')
        for module, minimum in (('gtk4', '4.12'), ('libadwaita-1', '1.6')):
            run('pkg-config', f'--atleast-version={minimum}', module)
        gtk = run('pacman', '-Q', GTK_PACKAGE)
        report['checks']['gtk_package'] = gtk
        require_gtk(gtk)
        report['checks']['media'] = media()
        report['checks']['hip_excluded'] = os.environ.get('PAPO_DEV_EXCLUDE_HIP') == '1'
        report['status'] = 'pass'
    except Exception as error:
        report['errors'].append(str(error))
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/windows-dev/preflight.json')
    parser.add_argument('--hip-allowlist', type=Path, help=argparse.SUPPRESS)
    parser.add_argument('--media-child', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.hip_allowlist is not None:
        print(hip_allowlist(args.hip_allowlist))
        return
    if args.media_child:
        if sys.stdin.buffer.read(1) != b'1':
            raise SystemExit(2)
        print(json.dumps(native_media()))
        return
    report = inspect()
    write_json(args.output, report)
    if report['status'] != 'pass':
        raise SystemExit('Windows development preflight failed:\n' + '\n'.join(report['errors']))
    print(f'Windows development preflight passed. Report: {args.output}')


if __name__ == '__main__':
    main()
