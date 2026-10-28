"""EN-US: Build the narrow WGC bridge with the recorded UCRT64 toolchain.
PT-BR: Compila a ponte WGC restrita com a toolchain UCRT64 registrada.
"""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def build_bridge(output, compiler='g++'):
    subprocess.run([compiler, '-std=c++17', '-O2', '-shared', '-static',
                    '-Wall', '-Wextra', '-Werror', str(ROOT / 'src/platform/capture/windows.cpp'),
                    '-lole32', '-lruntimeobject', '-ld3d11', '-ldxgi', '-luuid', '-o', str(output)], check=True)


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--compiler', default='g++')
    args = parser.parse_args()
    build_bridge(args.output, args.compiler)
