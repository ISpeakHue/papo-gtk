"""EN-US: Keep the OS-codec exception narrow while rejecting external runtimes.
PT-BR: Mantém restrita a exceção para codecs do sistema e rejeita runtimes externos.
"""
import importlib.util
from pathlib import Path
import unittest
import tempfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('windows_runtime_check', ROOT / 'packaging/windows/runtime_check.py')
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class WindowsRuntimeCheck(unittest.TestCase):
    def setUp(self):
        self.base = Path(tempfile.gettempdir()).resolve() / 'Program Files'
        self.package = {
            'Name': 'Microsoft.HEVCVideoExtension',
            'PackageFamilyName': 'Microsoft.HEVCVideoExtension_8wekyb3d8bbwe',
            'PublisherId': '8wekyb3d8bbwe', 'SignatureKind': 'Store', 'Status': 'Ok',
            'InstallLocation': str(self.base / 'WindowsApps/Microsoft.HEVCVideoExtension_2.0.61931.0_x64__8wekyb3d8bbwe'),
        }

    def test_registered_microsoft_os_codec_is_recognized(self):
        self.assertEqual(check.trusted_codec_roots([self.package], self.base), [Path(self.package['InstallLocation'])])

    def test_external_or_untrusted_packages_are_not_os_components(self):
        changes = [
            {'Name': 'PythonSoftwareFoundation.Python.3.14'},
            {'PublisherId': 'untrusted'}, {'PackageFamilyName': 'different'},
            {'SignatureKind': 'Developer'}, {'Status': 'Tampered'},
            {'InstallLocation': '/another/runtime'}, {'InstallLocation': 'relative/path'},
            {'InstallLocation': str(self.base / 'WindowsApps/Microsoft.HEVCVideoExtension_wrong__8wekyb3d8bbwe/nested')},
            {'InstallLocation': str(self.base / 'WindowsApps/different_2.0_x64__8wekyb3d8bbwe')},
        ]
        for change in changes:
            with self.subTest(change=change):
                self.assertEqual(check.trusted_codec_roots([self.package | change], self.base), [])


if __name__ == '__main__':
    unittest.main()
