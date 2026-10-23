"""EN-US: Never stage a dependency or ordinary executable as the packaged UI test harness.
PT-BR: Nunca empacota uma dependência ou executável comum como o runner de testes da interface.
"""
import importlib.util
import json
from pathlib import Path
import sys
import unittest

DIRECTORY = Path(__file__).resolve().parents[1] / 'packaging/windows'
sys.path.insert(0, str(DIRECTORY))
spec = importlib.util.spec_from_file_location('windows_media_tests', DIRECTORY / 'media_tests.py')
media = importlib.util.module_from_spec(spec)
spec.loader.exec_module(media)


class WindowsMediaAcceptance(unittest.TestCase):
    def event(self, name='papo-gtk', test=True, executable='test.exe'):
        return json.dumps({'reason': 'compiler-artifact', 'target': {'name': name},
                           'profile': {'test': test}, 'executable': executable})

    def test_selects_only_the_instrumented_application(self):
        self.assertEqual(media.test_executable([self.event(test=False, executable='papo.exe'),
                                               self.event(name='dependency'), self.event()]), Path('test.exe'))

    def test_missing_or_ambiguous_test_executables_fail_closed(self):
        for events in [[], [self.event(executable=None)], [self.event(), self.event(executable='other.exe')]]:
            with self.assertRaises(RuntimeError):
                media.test_executable(events)


if __name__ == '__main__':
    unittest.main()
