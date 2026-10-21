"""Capture adapters receive consented sources; unsupported platforms never start capture."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


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


if __name__ == "__main__":
    unittest.main()
