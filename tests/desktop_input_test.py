"""EN-US: Validate native key batches without sending desktop input.
PT-BR: Valida lotes de teclas nativas sem enviar entrada ao desktop.
"""
import ctypes
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('windows_input', Path(__file__).with_name('helpers') / 'windows_input.py')
keys = importlib.util.module_from_spec(spec)
spec.loader.exec_module(keys)


class DesktopInput(unittest.TestCase):
    def test_native_amd64_layout_and_balanced_chords(self):
        if ctypes.sizeof(ctypes.c_void_p) == 8:
            self.assertEqual(ctypes.sizeof(keys.Input), 40)
        events = keys.events('Control_L+Shift_L+Return')
        self.assertEqual([e.payload.keyboard.vk for e in events], [17, 16, 13, 13, 16, 17])
        self.assertEqual([e.payload.keyboard.flags for e in events], [0, 0, 0, 2, 2, 2])
        self.assertEqual([e.payload.keyboard.flags for e in keys.events('End')], [1, 3])
        for invalid in ['dead_acute', 'unknown', 'Control_L+']:
            with self.assertRaises(ValueError):
                keys.events(invalid)

    def test_no_injection_without_opt_in_or_from_unrelated_process(self):
        with patch.object(keys.os, 'name', 'nt'), patch.object(keys.os, 'getppid', return_value=50), patch.dict(keys.os.environ, {'PAPO_NATIVE_INPUT_TEST': '0'}):
            with self.assertRaises(RuntimeError):
                keys.run(50, 'Papo desktop input test', ['End'])
        with patch.object(keys.os, 'name', 'nt'), patch.object(keys.os, 'getppid', return_value=50), patch.dict(keys.os.environ, {'PAPO_NATIVE_INPUT_TEST': '1'}):
            with self.assertRaises(RuntimeError):
                keys.run(51, 'Papo desktop input test', ['End'])
            with self.assertRaises(RuntimeError):
                keys.run(50, 'Unrelated window', ['End'])


if __name__ == '__main__':
    unittest.main()
