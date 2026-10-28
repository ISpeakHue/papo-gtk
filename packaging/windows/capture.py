"""EN-US: Build the narrow WGC bridge with the recorded UCRT64 toolchain.
PT-BR: Compila a ponte WGC restrita com a toolchain UCRT64 registrada.
"""
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def build_bridge(output, compiler='g++'):
    # EN-US: MinGW's linker can misdecode accented output paths; copy with Python's Unicode file APIs.
    # PT-BR: O linker MinGW pode interpretar caminhos acentuados incorretamente; copia com APIs Unicode do Python.
    with tempfile.TemporaryDirectory(prefix='papo-capture-build-') as directory:
        binary = Path(directory) / 'papo-capture.dll'
        subprocess.run([compiler, '-std=c++17', '-O2', '-shared', '-static',
                        '-Wall', '-Wextra', '-Werror', str(ROOT / 'src/platform/capture/windows.cpp'),
                        '-lole32', '-lruntimeobject', '-ld3d11', '-ldxgi', '-luuid', '-o', str(binary)], check=True)
        shutil.copy2(binary, output)


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--compiler', default='g++')
    args = parser.parse_args()
    build_bridge(args.output, args.compiler)
