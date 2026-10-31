"""EN-US: Reject unsafe/mixed developer runtimes without invoking native devices.
PT-BR: Rejeita runtimes de desenvolvimento misturados sem acionar dispositivos nativos.
"""
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'packaging/windows'))
import devcheck


class WindowsDevelopment(unittest.TestCase):
    def test_hip_override_keeps_other_hardware_and_required_plugins(self):
        with tempfile.TemporaryDirectory() as directory:
            for name in ('hip', 'd3d11', 'd3d12', 'vpx', 'opus', 'nice'):
                (Path(directory) / f'libgst{name}.dll').touch()
            names = set(devcheck.hip_allowlist(directory).split(','))
            self.assertEqual(names, {'d3d11', 'd3d12', 'vpx', 'opus', 'nice'})
            self.assertTrue((Path(directory) / 'libgsthip.dll').exists())

    @unittest.skipUnless(sys.platform == 'win32', 'Windows native process Job required')
    def test_stalled_probe_is_bounded_and_reaped(self):
        command = [sys.executable, '-c', 'import sys,time;sys.stdin.buffer.read(1);time.sleep(60)']
        started = time.monotonic()
        with self.assertRaisesRegex(RuntimeError, 'timed out'):
            devcheck.media(timeout=0.1, command=command)
        self.assertLess(time.monotonic() - started, 5)

    def test_cross_target_override_cannot_pass_with_a_gnu_host_compiler(self):
        with patch.object(devcheck.sys, 'platform', 'win32'), \
                patch.dict(devcheck.os.environ, {'MSYSTEM': 'UCRT64', 'CARGO_BUILD_TARGET': 'x86_64-pc-windows-msvc'}), \
                patch.object(devcheck, 'toolchain') as tools:
            report = devcheck.inspect()
        self.assertEqual(report['status'], 'fail')
        self.assertIn('CARGO_BUILD_TARGET', report['errors'][0])
        tools.assert_not_called()

    def test_wrong_shell_cannot_pass_or_probe_native_media(self):
        with patch.object(devcheck.sys, 'platform', 'win32'), patch.dict(devcheck.os.environ, {'MSYSTEM': 'MINGW64'}), \
                patch.object(devcheck, 'toolchain') as tools, patch.object(devcheck, 'media') as media:
            report = devcheck.inspect()
        self.assertEqual(report['status'], 'fail')
        self.assertTrue(report['errors'])
        tools.assert_not_called()
        media.assert_not_called()

    def test_unpatched_or_different_gtk_is_rejected(self):
        with patch.object(devcheck, 'expected_gtk', return_value='4.24.1-1.1'):
            for inventory in ('', 'mingw-w64-ucrt-x86_64-gtk4 4.24.1-1',
                              'mingw-w64-ucrt-x86_64-gtk4 4.25.0-1',
                              'mingw-w64-x86_64-gtk4 4.24.1-1.1'):
                with self.subTest(inventory=inventory), self.assertRaises(RuntimeError):
                    devcheck.require_gtk(inventory)
            devcheck.require_gtk('mingw-w64-ucrt-x86_64-gtk4 4.24.1-1.1\n')

    def test_missing_codec_reports_factory_without_creating_devices(self):
        gst = Mock()
        gst.ElementFactory.find.side_effect = lambda name: None if name == 'opusenc' else object()
        with self.assertRaisesRegex(RuntimeError, 'opusenc'):
            devcheck.require_factories(gst)
        gst.ElementFactory.make.assert_not_called()

    def test_native_audio_input_and_output_are_required_without_opening_devices(self):
        gst = Mock()
        for missing in (('wasapi2src', 'wasapisrc'), ('wasapi2sink', 'wasapisink')):
            with self.subTest(missing=missing):
                gst.ElementFactory.find.side_effect = lambda name: None if name in missing else object()
                with self.assertRaisesRegex(RuntimeError, ' or '.join(missing)):
                    devcheck.require_factories(gst)
        gst.ElementFactory.make.assert_not_called()

    def test_toolchain_failure_is_actionable_and_stops_native_probes(self):
        with patch.object(devcheck.sys, 'platform', 'win32'), patch.dict(devcheck.os.environ, {'MSYSTEM': 'UCRT64'}), \
                patch.object(devcheck, 'toolchain', side_effect=RuntimeError('mixed Rust toolchain')), \
                patch.object(devcheck, 'media') as media:
            report = devcheck.inspect()
        self.assertEqual(report['status'], 'fail')
        self.assertIn('mixed Rust toolchain', report['errors'])
        media.assert_not_called()

    def test_success_requires_tools_patch_and_media(self):
        prefix = Path(sys.executable).resolve().parent
        with patch.object(devcheck.sys, 'platform', 'win32'), patch.dict(devcheck.os.environ, {'MSYSTEM': 'UCRT64'}), \
                patch.object(devcheck, 'toolchain', return_value={'rustc': 'GNU'}), \
                patch.object(devcheck.shutil, 'which', side_effect=lambda name: str(prefix / name)), \
                patch.object(devcheck, 'run', return_value='reviewed gtk'), \
                patch.object(devcheck, 'require_gtk') as gtk, \
                patch.object(devcheck, 'media', return_value={'factories': ['opusenc']}):
            report = devcheck.inspect()
        self.assertEqual(report['status'], 'pass')
        self.assertEqual(report['errors'], [])
        gtk.assert_called_once_with('reviewed gtk')


if __name__ == '__main__':
    unittest.main()
