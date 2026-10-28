"""Capture adapters receive consented sources; unsupported platforms never start capture."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("windows_capture", ROOT / "src/platform/capture/windows.py")
windows = importlib.util.module_from_spec(spec)
spec.loader.exec_module(windows)


class PlatformCaptureTests(unittest.TestCase):
    def test_unavailable_backend_rejects_capture_without_running_callbacks(self):
        source = ROOT / "src/platform/capture/unavailable.py"
        spec = importlib.util.spec_from_file_location("unavailable_capture", source)
        backend = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(backend)
        callbacks = []
        with self.assertRaises(NotImplementedError):
            backend.create_screen_capture(lambda *_: callbacks.append("ready"), lambda: callbacks.append("ended"))
        self.assertEqual(callbacks, [])


class WindowsCaptureTests(unittest.TestCase):
    def setUp(self):
        self.bridge, self.gst, self.glib = Mock(), Mock(), Mock()
        self.bridge.start.return_value = 123
        self.bridge.state.return_value = 0
        self.gst.SECOND = 1_000_000_000
        self.gst.FlowReturn.OK = 0
        self.source = self.gst.ElementFactory.make.return_value
        self.source.emit.return_value = 0
        self.ready, self.ended = Mock(), Mock()
        self.capture = windows.WindowsCapture(self.ready, self.ended, self.bridge, self.gst, self.glib)
        self.addCleanup(self.capture.close)

    def frame(self, width=2, height=2):
        self.bridge.state.return_value = 1
        self.bridge.read.return_value = width, height, b'\0' * (width * height * 4)

    def test_pending_and_cancelled_picker_send_no_source_or_frames(self):
        self.capture.start()
        self.assertTrue(self.capture.poll())
        self.bridge.read.assert_not_called()
        self.bridge.state.return_value = 2
        self.assertFalse(self.capture.poll())
        self.ready.assert_not_called()
        self.source.emit.assert_not_called()
        self.bridge.close.assert_called_once_with(123)
        self.ended.assert_called_once_with()
        self.assertFalse(self.capture.poll())
        self.ended.assert_called_once_with()

    def test_explicit_selection_is_required_before_first_frame_and_bounded_push(self):
        self.capture.start()
        self.bridge.read.assert_not_called()
        for key, value in [('block', False), ('max-buffers', 1), ('max-bytes', 64 * 1024 * 1024), ('leaky-type', 2)]:
            self.source.set_property.assert_any_call(key, value)
        self.frame()
        self.assertTrue(self.capture.poll())
        self.ready.assert_called_once_with(self.source)
        self.source.emit.assert_called_once()
        self.gst.Buffer.new_allocate.assert_called_once_with(None, 16, None)

    def test_resize_renegotiates_raw_caps_without_selecting_another_source(self):
        self.capture.start()
        self.frame(2, 2)
        self.capture.poll()
        self.frame(4, 2)
        self.capture.poll()
        self.assertEqual(self.capture.size, (4, 2))
        self.assertEqual(self.gst.Caps.from_string.call_count, 2)
        self.assertIn('width=4,height=2', self.gst.Caps.from_string.call_args.args[0])
        self.bridge.start.assert_called_once_with()
        self.ready.assert_called_once()

    def test_closure_and_driver_failure_close_native_capture_once(self):
        for state in (3, 4):
            with self.subTest(state=state):
                self.setUp()
                self.capture.start()
                self.frame()
                self.capture.poll()
                pushes = self.source.emit.call_count
                self.bridge.state.return_value = state
                self.assertFalse(self.capture.poll())
                self.capture.close()
                self.assertEqual(self.source.emit.call_count, pushes)
                self.bridge.close.assert_called_once_with(123)
                self.ended.assert_called_once()

    def test_stop_pending_or_active_discards_late_callbacks_and_is_idempotent(self):
        self.capture.start()
        self.capture.close()
        self.frame()
        self.assertFalse(self.capture.poll())
        self.capture.start()
        self.capture.close()
        self.ready.assert_not_called()
        self.ended.assert_not_called()
        self.bridge.close.assert_called_once_with(123)
        self.glib.source_remove.assert_called_once()

    def test_unsupported_bounded_source_never_opens_picker(self):
        self.source.find_property.return_value = None
        with self.assertRaises(RuntimeError):
            self.capture.start()
        self.bridge.start.assert_not_called()

    def test_invalid_size_or_truncated_frame_never_publishes(self):
        self.capture.start()
        self.bridge.state.return_value = 1
        self.bridge.read.return_value = 8192, 8192, b''
        self.assertFalse(self.capture.poll())
        self.ready.assert_not_called()
        self.source.emit.assert_not_called()
        self.bridge.close.assert_called_once()

    def test_stop_inside_ready_cannot_push_first_frame(self):
        self.capture.start()
        self.ready.side_effect = lambda _: self.capture.close()
        self.frame()
        self.assertFalse(self.capture.poll())
        self.source.emit.assert_not_called()
        self.ended.assert_not_called()

    def test_downstream_failure_releases_capture_and_reports_ended(self):
        self.capture.start()
        self.frame()
        self.source.emit.return_value = -1
        self.assertFalse(self.capture.poll())
        self.bridge.close.assert_called_once()
        self.ended.assert_called_once()

    def test_static_selected_source_repeats_last_frame_until_stopped(self):
        self.capture.start()
        self.frame()
        self.capture.poll()
        self.bridge.read.return_value = None
        self.assertTrue(self.capture.poll())
        self.assertEqual(self.source.emit.call_count, 2)
        self.capture.close()
        self.assertIsNone(self.capture.last_frame)
        self.assertFalse(self.capture.poll())
        self.assertEqual(self.source.emit.call_count, 2)

    def test_no_initial_frame_has_a_deadline(self):
        self.capture.start()
        self.bridge.state.return_value = 1
        self.bridge.read.return_value = None
        with patch.object(windows.time, 'monotonic', side_effect=[0, 0, 16]):
            self.assertTrue(self.capture.poll())
            self.assertFalse(self.capture.poll())
        self.ready.assert_not_called()
        self.bridge.close.assert_called_once()

    def test_preview_requires_explicit_opt_in_and_mac_stays_unavailable(self):
        for platform, enabled in [('win32', '0'), ('darwin', '1')]:
            with patch.object(windows.sys, 'platform', platform), patch.dict(windows.os.environ, {'PAPO_WINDOWS_CAPTURE_PREVIEW': enabled}):
                with self.assertRaises(NotImplementedError):
                    windows.create_screen_capture(self.ready, self.ended)

    def test_native_close_timeout_terminates_worker_instead_of_detaching_capture(self):
        bridge = windows.WindowsCaptureBridge.__new__(windows.WindowsCaptureBridge)
        bridge.dll = Mock()
        bridge.dll.papo_capture_close.return_value = 0
        with patch.object(windows.os, '_exit', side_effect=SystemExit(1)) as terminate:
            with self.assertRaises(SystemExit):
                bridge.close(123)
        terminate.assert_called_once_with(1)

    def test_native_copy_requires_exact_bounded_geometry(self):
        bridge = windows.WindowsCaptureBridge.__new__(windows.WindowsCaptureBridge)
        bridge.dll = Mock()
        bridge.buffer = None
        def read(_handle, buffer, _capacity, width, height):
            width._obj.value, height._obj.value = 8192, 8192
            return -2
        bridge.dll.papo_capture_read.side_effect = read
        with self.assertRaises(ValueError):
            bridge.read(123)
        self.assertIsNone(bridge.buffer)


if __name__ == "__main__":
    unittest.main()
