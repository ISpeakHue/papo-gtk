"""EN-US: Portable native SFU probe launchers. PT-BR: Inicialização portátil dos testes nativos de SFU."""
import argparse
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def python():
    return os.environ.get("PAPO_TEST_PYTHON", sys.executable)


def sfu_binary(argv=None):
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--sfu", default=os.environ.get("PAPO_TEST_SFU"))
    args, _ = parser.parse_known_args(argv)
    path = Path(args.sfu) if args.sfu else ROOT / "target" / ("voice-sfu.exe" if sys.platform == "win32" else "voice-sfu")
    path = path.expanduser().resolve()
    if not path.is_file():
        raise FileNotFoundError(f"Build tests/voice-sfu first, or pass --sfu PATH: {path}")
    return str(path)


def spawn_options():
    return {"creationflags": subprocess.CREATE_NO_WINDOW} if sys.platform == "win32" else {}
