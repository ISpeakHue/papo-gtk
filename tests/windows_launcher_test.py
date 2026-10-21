"""EN-US: Exercise Unicode arguments through the real bootstrap and a tiny native child.
PT-BR: Testa argumentos Unicode pelo iniciador real e um pequeno processo filho nativo.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(os.name == 'nt' and shutil.which('gcc'), 'Native Windows GCC required')
class WindowsLauncher(unittest.TestCase):
    def test_unicode_empty_quotes_backslashes_and_unrelated_working_directory(self):
        with tempfile.TemporaryDirectory(prefix='Papo á é ç ') as directory:
            stage = Path(directory)
            runtime = stage / 'runtime'
            (runtime / 'bin').mkdir(parents=True)
            cache = runtime / 'lib/gdk-pixbuf-2.0/2.10.0/loaders.cache'
            cache.parent.mkdir(parents=True)
            cache.write_text('', encoding='utf-8')
            (runtime / 'runtime.id').write_text('0' * 64, encoding='ascii')
            stub = stage / 'arguments.c'
            stub.write_text('''#include <stdio.h>
#include <wchar.h>
int wmain(int argc, wchar_t **argv) {
    for (int i = 1; i < argc; i++) {
        for (const wchar_t *p = argv[i]; *p; p++) printf("%04x", (unsigned int)*p);
        printf("\\n");
    }
    return 0;
}
''', encoding='ascii')
            options = ['gcc', '-Os', '-static', '-static-libgcc', '-municode', '-Wl,--stack,4194304', '-Wall', '-Wextra', '-Werror']
            subprocess.run([*options, str(stub), '-o', str(runtime / 'bin/papo-gtk.exe')], check=True)
            subprocess.run([*options, str(ROOT / 'packaging/windows/launcher.c'), '-lshell32', '-o', str(stage / 'Papo.exe')], check=True)
            arguments = ['', 'á é ç', 'space and "quotes"', 'trailing space \\', 'two\\\\"quoted', 'line\nbreak', '😀']
            env = {key: value for key, value in os.environ.items() if key != 'PAPO_RUNTIME_DIR'}
            result = subprocess.run([str(stage / 'Papo.exe'), *arguments], cwd=ROOT, env=env,
                                    capture_output=True, text=True, encoding='ascii', timeout=30, check=True)
            expected = []
            for argument in arguments:
                encoded = argument.encode('utf-16-le')
                expected.append(''.join(f'{int.from_bytes(encoded[i:i+2], "little"):04x}' for i in range(0, len(encoded), 2)))
            self.assertEqual(result.stdout.splitlines(), expected)


if __name__ == '__main__':
    unittest.main()
