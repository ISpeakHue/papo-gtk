#!/usr/bin/env python3
"""EN-US: Retain an opt-in instrumented UI test executable for the exact packaged source.
PT-BR: Preserva um executável instrumentado e opcional de testes da interface para o mesmo código do pacote.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

from build import source_state, require_source


def test_executable(lines):
    executables = []
    for line in lines:
        event = json.loads(line)
        if (event.get('reason') == 'compiler-artifact'
                and event.get('target', {}).get('name') == 'papo-gtk'
                and event.get('profile', {}).get('test') and event.get('executable')):
            executables.append(Path(event['executable']))
    if len(executables) != 1:
        raise RuntimeError('Expected one native Papo test executable')
    return executables[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifacts', required=True, type=Path)
    parser.add_argument('--directory', required=True, type=Path)
    args = parser.parse_args()
    source = source_state()
    require_source(source)
    executable = test_executable(args.artifacts.read_text(encoding='utf-8').splitlines())
    args.directory.mkdir(parents=True, exist_ok=True)
    target = args.directory / 'papo-media-tests.exe'
    shutil.copy2(executable, target)
    subprocess.run(['strip', '--strip-debug', str(target)], check=True)
    with target.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    metadata = {'schema': 1, 'source_commit': source['commit'], 'source_dirty': False,
                'executable': target.name, 'sha256': digest,
                'scope': 'Instrumented shared UI tests; uses the separately verified portable ZIP runtime. Not a release executable.'}
    (args.directory / 'MEDIA-TESTS.json').write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    for name in ('check-media-ui.ps1',):
        script = Path(__file__).with_name(name)
        (args.directory / name).write_text(script.read_text(encoding='utf-8-sig'), encoding='utf-8-sig')
    require_source(source, source_state())


if __name__ == '__main__':
    main()
