"""EN-US: Collect a UCRT64 runtime and its recursive PE dependency closure.
PT-BR: Reúne um runtime UCRT64 e o fechamento recursivo de dependências PE.
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import sysconfig

from build import ROOT, run, sha256, write_json
from pe import imports, is_system


class Bundle:
    def __init__(self, stage, prefix):
        self.stage = Path(stage)
        self.root = self.stage / 'runtime'
        self.prefix = Path(prefix).resolve()
        self.origins = {}
        self.dependencies = {}
        self.owners = {}
        msys_prefix = run('cygpath', '-u', str(self.prefix)).rstrip('/') + '/'
        for line in run('pacman', '-Ql').splitlines():
            owner, _, path = line.partition(' ')
            if path.startswith(msys_prefix) and not path.endswith('/'):
                self.owners[path[len(msys_prefix):].lower()] = owner
        self.binaries = {path.name.lower(): path for path in (self.prefix / 'bin').glob('*.dll')}

    def copy(self, source, relative=None):
        source = Path(source)
        relative = Path(relative) if relative else source.relative_to(self.prefix)
        owner = self.owners.get(source.relative_to(self.prefix).as_posix().lower())
        if not owner:
            raise RuntimeError(f'Unowned native input (generated cache?): {source}')
        source.resolve().relative_to(self.prefix)
        target = self.root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        self.origins[target.relative_to(self.stage).as_posix()] = owner
        return target

    def tree(self, relative, optional=False):
        source = self.prefix / relative
        if not source.exists():
            if optional:
                return
            raise RuntimeError(f'Missing runtime data: {source}')
        for path in sorted(source.rglob('*')):
            if path.is_file() and not any(part in {'__pycache__', 'tests', 'test', 'idlelib', 'ensurepip', 'tkinter'} for part in path.relative_to(source).parts):
                if path.suffix in {'.pyc', '.a', '.h', '.gir'} or path.name.endswith(('.cache', 'gschemas.compiled')):
                    continue
                self.copy(path)

    def closure(self):
        pending = [path for path in self.stage.rglob('*') if path.suffix.lower() in {'.dll', '.exe', '.pyd'}]
        seen = set()
        while pending:
            path = pending.pop()
            key = path.relative_to(self.stage).as_posix()
            if key in seen:
                continue
            seen.add(key)
            names = imports(path)
            self.dependencies[key] = names
            for name in names:
                if is_system(name):
                    continue
                destination = self.root / 'bin' / name
                if destination.is_file():
                    continue
                source = self.binaries.get(name)
                if source is None:
                    raise RuntimeError(f'Unresolved non-system DLL {name}, imported by {key}')
                pending.append(self.copy(source, Path('bin') / name))

    def prepare(self, executable, source, native_manifest):
        (self.root / 'bin').mkdir(parents=True)
        shutil.copy2(executable, self.root / 'bin/papo-gtk.exe')
        self.copy(self.prefix / 'bin/python.exe')
        stdlib = Path(sysconfig.get_path('stdlib')).relative_to(self.prefix)
        # EN-US: Retain the standard library, but only GI/Cairo from third-party site-packages.
        # PT-BR: Preserva a biblioteca padrão, mas só GI/Cairo dos pacotes de terceiros.
        for path in sorted((self.prefix / stdlib).rglob('*')):
            relative = path.relative_to(self.prefix / stdlib)
            if 'site-packages' in relative.parts:
                continue
            if any(part in {'__pycache__', 'test', 'tests', 'idlelib', 'ensurepip', 'tkinter'} for part in relative.parts):
                continue
            if path.is_file() and path.suffix not in {'.pyc', '.a', '.h'} and not path.name.startswith('_tkinter.'):
                self.copy(path)
        for name in ('gi', 'cairo'):
            self.tree(stdlib / 'site-packages' / name)
        for relative in ('lib/girepository-1.0', 'lib/gstreamer-1.0', 'lib/gio/modules',
                         'lib/gtk-4.0', 'lib/gdk-pixbuf-2.0', 'share/glib-2.0/schemas',
                         'share/icons/Adwaita', 'share/icons/hicolor', 'share/mime/packages', 'share/locale',
                         'etc/fonts', 'share/fonts', 'share/gtk-4.0', 'share/libadwaita', 'share/zoneinfo'):
            self.tree(Path(relative), optional=relative not in {'lib/girepository-1.0', 'lib/gstreamer-1.0', 'share/glib-2.0/schemas'})
        scanners = list((self.prefix / 'libexec').glob('gstreamer-1.0/gst-plugin-scanner.exe'))
        if not scanners:
            raise RuntimeError('Missing GStreamer plugin scanner')
        for scanner in scanners:
            self.copy(scanner)
        # EN-US: Typelibs dynamically load named DLLs, outside the static import graph.
        # PT-BR: Typelibs carregam DLLs pelo nome, fora do grafo de imports estáticos.
        for typelib in (self.root / 'lib/girepository-1.0').glob('*.typelib'):
            for match in re.findall(rb'[A-Za-z0-9_+.-]+\.dll', typelib.read_bytes()):
                name = match.decode().lower()
                if name in self.binaries:
                    self.copy(self.binaries[name], Path('bin') / name)
        subprocess.run(['gcc', '-Os', '-static', '-static-libgcc', '-municode', '-Wl,--stack,4194304', '-Wall', '-Wextra', '-Werror',
                        str(ROOT / 'packaging/windows/launcher.c'), '-lshell32', '-o', str(self.stage / 'Papo.exe')], check=True)
        self.closure()
        self.caches()
        self.licenses()
        data = self.root / 'share/papo-gtk'
        data.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / 'packaging/windows/runtime_check.py', data)
        shutil.copy2(ROOT / 'tests/icon_runtime_probe.py', data / 'runtime_icons.py')
        shutil.copytree(ROOT / 'assets/icons/symbolic/licenses', self.root / 'share/licenses/papo-gtk/symbolic-icons')
        fixtures = data / 'check-fixtures'
        fixtures.mkdir()
        for name in ('chat-h264.mp4', 'chat-video.mov', 'chat-video.webm'):
            shutil.copy2(ROOT / 'tests/fixtures' / name, fixtures)
        shutil.copy2(ROOT / 'src/media/voice_join.wav', fixtures / 'voice.wav')
        identifier = source['commit'] + native_manifest + sha256(self.root / 'bin/papo-gtk.exe')
        import hashlib
        (self.root / 'runtime.id').write_text(hashlib.sha256(identifier.encode()).hexdigest(), encoding='ascii')
        files = [{"path": path.relative_to(self.stage).as_posix(), "size": path.stat().st_size,
                  "sha256": sha256(path), "package": self.origins.get(path.relative_to(self.stage).as_posix(), 'papo-gtk/generated')}
                 for path in sorted(self.stage.rglob('*')) if path.is_file()]
        metadata = package_metadata()
        used = sorted(set(self.origins.values()))
        manifest = {'schema': 1, 'architecture': 'x86_64', 'source_commit': source['commit'],
                    'native_inputs_manifest_sha256': native_manifest,
                    'packages': {name: metadata[name] for name in used},
                    'system_dependencies': sorted({name for names in self.dependencies.values() for name in names if is_system(name)}),
                    'imports': self.dependencies, 'files': files,
                    'excluded': ['development headers/static libraries', 'stdlib tests, IDLE, ensurepip, Tk GUI',
                                 'unrelated Python site-packages', 'GPU vendor drivers', 'prebuilt GStreamer registry'],
                    'byte_identical_rebuild_demonstrated': False}
        write_json(self.root / 'RUNTIME-MANIFEST.json', manifest)
        return {'manifest_sha256': sha256(self.root / 'RUNTIME-MANIFEST.json'), 'files': len(files), 'packages': len(used)}

    def caches(self):
        env = {**os.environ, 'PATH': str(self.root / 'bin') + os.pathsep + os.environ['PATH']}
        schema_dir = self.root / 'share/glib-2.0/schemas'
        subprocess.run([str(self.prefix / 'bin/glib-compile-schemas.exe'), '--strict', str(schema_dir)], check=True, env=env)
        mime = self.root / 'share/mime'
        if mime.is_dir():
            subprocess.run([str(self.prefix / 'bin/update-mime-database.exe'), str(mime)], check=True, env=env)
        icon_tool = self.prefix / 'bin/gtk4-update-icon-cache.exe'
        if icon_tool.is_file():
            for theme in self.root.glob('share/icons/*/index.theme'):
                subprocess.run([str(icon_tool), '--force', str(theme.parent)], check=True, env=env)
        modules = self.root / 'lib/gio/modules'
        if modules.is_dir():
            subprocess.run([str(self.prefix / 'bin/gio-querymodules.exe'), str(modules)], check=True, env=env)
        for loaders in self.root.glob('lib/gdk-pixbuf-2.0/*/loaders'):
            result = subprocess.run([str(self.prefix / 'bin/gdk-pixbuf-query-loaders.exe'), *(str(path) for path in loaders.glob('*.dll'))],
                                    capture_output=True, text=True, encoding='utf-8', env=env, check=True)
            lines = []
            for line in result.stdout.splitlines():
                if re.fullmatch(r'".*\.dll"', line, re.I):
                    path = Path(json.loads(line))
                    if path.is_absolute():
                        path = path.relative_to(self.root)
                    line = json.dumps(path.as_posix())
                if not line.startswith('#'):
                    lines.append(line)
            (loaders.parent / 'loaders.cache').write_text('\n'.join(lines) + '\n', encoding='utf-8')
        if not list(self.root.glob('lib/gdk-pixbuf-2.0/*/loaders.cache')):
            raise RuntimeError('Missing relocatable GdkPixbuf loader cache')

    def licenses(self):
        # EN-US: Include upstream notices, including common SPDX texts used by MSYS2 packages.
        # PT-BR: Inclui avisos de origem e os textos SPDX comuns usados pelos pacotes MSYS2.
        self.tree(Path('share/licenses'))
        common = Path(run('cygpath', '-w', '/usr/share/licenses/common'))
        if common.is_dir():
            shutil.copytree(common, self.stage / 'licenses/msys2-common')
        notices = self.stage / 'THIRD-PARTY-NOTICES.txt'
        notices.write_text('Native runtime packages and file provenance: runtime/RUNTIME-MANIFEST.json\n'
                           'Upstream license texts: runtime/share/licenses and licenses/msys2-common\n'
                           'MSYS2 build recipes/source references: https://github.com/msys2/MINGW-packages\n'
                           'Native inputs are retained separately; see BUILD-INFO.json.\n', encoding='utf-8')


def package_metadata():
    output = subprocess.check_output(['pacman', '-Qi'], env={**os.environ, 'LC_ALL': 'C'}, text=True, encoding='utf-8')
    result = {}
    current = None
    for line in output.splitlines():
        match = re.match(r'^(Name|Version|Licenses|URL)\s*:\s*(.*)', line)
        if match:
            key, value = match.groups()
            if key == 'Name':
                current = result.setdefault(value, {})
            else:
                current[key.lower()] = value
    return result
