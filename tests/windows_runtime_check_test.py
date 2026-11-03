"""EN-US: Keep the OS-codec exception narrow while rejecting external runtimes.
PT-BR: Mantém restrita a exceção para codecs do sistema e rejeita runtimes externos.
"""
import importlib.util
from pathlib import Path
import unittest
import tempfile
import ctypes
from types import SimpleNamespace
from unittest.mock import Mock, patch
import sys

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


class ModuleInventory(unittest.TestCase):
    def setUp(self):
        self.modules = (0x100000001, 0x200000002)
        self.snapshots = [self.modules]
        self.paths = {module: str(Path(tempfile.gettempdir()).resolve() / f'module-{module}.dll') for module in self.modules}
        self.retained = []
        self.released = []
        self.error = 0
        self.kernel = SimpleNamespace(GetCurrentProcess=Mock(return_value=-1),
                                      GetModuleHandleExW=Mock(side_effect=self.retain),
                                      GetModuleFileNameW=Mock(side_effect=self.filename),
                                      FreeLibrary=Mock(side_effect=self.release))
        self.psapi = SimpleNamespace(EnumProcessModulesEx=Mock(side_effect=self.enumerate))

    def enumerate(self, process, modules, capacity, needed, mode):
        snapshot = self.snapshots.pop(0) if len(self.snapshots) > 1 else self.snapshots[0]
        needed._obj.value = len(snapshot) * ctypes.sizeof(check.wintypes.HMODULE)
        for index, module in enumerate(snapshot):
            modules[index] = module
        return 1

    def retain(self, flags, module, result):
        self.assertEqual(flags, 4)
        result._obj.value = module
        self.retained.append(module)
        return 1

    def filename(self, module, buffer, size):
        self.assertIn(module.value, self.retained)
        buffer.value = self.paths[module.value]
        return len(buffer.value)

    def release(self, module):
        self.retained.remove(module.value)
        self.released.append(module.value)
        return 1

    def collect(self):
        def winerror(code):
            error = OSError(code, 'simulated Windows error')
            error.winerror = code
            return error
        with patch.object(check.ctypes, 'WinDLL', create=True,
                          side_effect=lambda name, **kwargs: self.kernel if name == 'kernel32' else self.psapi), \
                patch.object(check.ctypes, 'get_last_error', create=True, side_effect=lambda: self.error), \
                patch.object(check.ctypes, 'WinError', create=True, side_effect=winerror):
            return check.loaded_modules()

    def test_complete_inventory_keeps_full_width_handles_and_releases_references(self):
        self.assertEqual(self.collect(), [Path(self.paths[m]) for m in self.modules])
        self.assertEqual(self.retained, [])
        self.assertEqual(self.released, list(reversed(self.modules)))

    def test_unloaded_module_restarts_the_entire_snapshot(self):
        first, stale = self.modules
        self.snapshots = [(first, stale), (first,), (first,)]
        def retain(flags, module, result):
            if module == stale:
                self.error = 126
                return 0
            return self.retain(flags, module, result)
        self.kernel.GetModuleHandleExW.side_effect = retain
        self.assertEqual(self.collect(), [Path(self.paths[first])])
        self.assertEqual(self.retained, [])
        self.assertEqual(self.released, [first, first])

    def test_newly_loaded_modules_are_included_before_returning(self):
        first, second = self.modules
        self.snapshots = [(first,), (first, second), (first, second), (first, second)]
        self.assertEqual(self.collect(), [Path(self.paths[m]) for m in self.modules])
        self.assertEqual(self.kernel.GetModuleHandleExW.call_count, 3)
        self.assertEqual(self.retained, [])

    def test_persistent_churn_fails_instead_of_returning_an_incomplete_audit(self):
        self.error = 6
        self.kernel.GetModuleHandleExW.side_effect = None
        self.kernel.GetModuleHandleExW.return_value = 0
        with self.assertRaisesRegex(RuntimeError, 'did not stabilize'):
            self.collect()
        self.assertEqual(self.psapi.EnumProcessModulesEx.call_count, 4)

    def test_unrelated_errors_fail_immediately_and_release_acquired_references(self):
        first, second = self.modules
        def retain(flags, module, result):
            if module == second:
                self.error = 5
                return 0
            return self.retain(flags, module, result)
        self.kernel.GetModuleHandleExW.side_effect = retain
        with self.assertRaises(OSError) as error:
            self.collect()
        self.assertEqual(error.exception.winerror, 5)
        self.assertEqual(self.retained, [])
        self.assertEqual(self.released, [first])

    def test_filename_failure_is_not_silenced(self):
        self.error = 5
        self.kernel.GetModuleFileNameW.side_effect = None
        self.kernel.GetModuleFileNameW.return_value = 0
        with self.assertRaises(OSError):
            self.collect()
        self.assertEqual(self.retained, [])

    def test_truncated_paths_are_rejected_and_references_released(self):
        self.kernel.GetModuleFileNameW.side_effect = lambda module, buffer, size: size
        with self.assertRaisesRegex(AssertionError, 'truncated'):
            self.collect()
        self.assertEqual(self.retained, [])

    def test_oversized_inventory_fails_closed(self):
        def oversized(process, modules, capacity, needed, mode):
            needed._obj.value = capacity + ctypes.sizeof(check.wintypes.HMODULE)
            return 1
        self.psapi.EnumProcessModulesEx.side_effect = oversized
        with self.assertRaisesRegex(AssertionError, 'buffer exhausted'):
            self.collect()
        self.kernel.GetModuleHandleExW.assert_not_called()

    @unittest.skipUnless(sys.platform == 'win32', 'Native Windows module inventory required')
    def test_native_current_process_paths(self):
        for _ in range(10):
            paths = check.loaded_modules()
            self.assertIn(Path(sys.executable).resolve(), paths)
            self.assertTrue(any(p.name.lower() == 'kernel32.dll' for p in paths))


if __name__ == '__main__':
    unittest.main()
