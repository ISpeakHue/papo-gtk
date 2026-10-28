"""EN-US: Native ABI/ownership probes without opening capture or a system picker.
PT-BR: Testes nativos de ABI/propriedade sem iniciar captura nem seletor do sistema.
"""
import ctypes
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'packaging/windows'))
from capture import build_bridge
from pe import imports, is_system


@unittest.skipUnless(os.name == 'nt' and shutil.which('g++'), 'Native Windows/UCRT64 compiler required')
class WindowsNativeCapture(unittest.TestCase):
    def test_native_abi_owner_and_frame_bounds_without_capture(self):
        with tempfile.TemporaryDirectory(prefix='Papo-capture-á ') as directory:
            path = Path(directory)
            dll_path = path / 'papo-capture.dll'
            build_bridge(dll_path)
            self.assertIn('d3d11.dll', imports(dll_path))
            self.assertTrue(all(is_system(name) for name in imports(dll_path)))
            dll = ctypes.WinDLL(str(dll_path), winmode=0x900)
            try:
                self.assertEqual(dll.papo_capture_abi(), 1)
                self.assertIn(dll.papo_capture_supported(), (0, 1))
            finally:
                # EN-US: Windows locks loaded DLLs; release the test-only mapping before cleanup.
                # PT-BR: Windows bloqueia DLLs carregadas; libera o mapeamento de teste antes da limpeza.
                kernel = ctypes.WinDLL('kernel32', use_last_error=True)
                kernel.FreeLibrary.argtypes = (ctypes.c_void_p,)
                kernel.FreeLibrary.restype = ctypes.c_int
                self.assertTrue(kernel.FreeLibrary(dll._handle))
            binary = path / 'capture-test.exe'
            subprocess.run(['g++', '-std=c++17', '-O2', '-Wall', '-Wextra', '-Werror',
                            str(ROOT / 'tests/windows_capture_native.cpp'), '-static',
                            '-lole32', '-lruntimeobject', '-ld3d11', '-ldxgi', '-luuid', '-o', str(binary)], check=True)
            result = subprocess.run([str(binary)], timeout=15, capture_output=True, text=True, check=True)
            self.assertIn('no capture performed', result.stdout)


if __name__ == '__main__':
    unittest.main()
