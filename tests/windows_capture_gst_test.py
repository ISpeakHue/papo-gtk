"""EN-US: Real Gst BGRA→VP8→RGB with synthetic consent/frame delivery; no desktop or network.
PT-BR: Gst BGRA→VP8→RGB real com consentimento/quadros sintéticos; sem desktop nem rede.
"""
import importlib.util
from pathlib import Path
import time
import unittest

import gi
gi.require_version('Gst', '1.0')
gi.require_version('GstApp', '1.0')
from gi.repository import Gst, GLib

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('capture_adapter', ROOT / 'src/platform/capture/windows.py')
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)
Gst.init(None)


class Frames:
    def __init__(self):
        self.phase = 0
        self.running = False
        self.closed = 0
    def start(self):
        self.running = True
        return 1
    def state(self, _):
        return 1 if self.running else 2
    def read(self, _):
        size = (64, 36) if self.phase == 0 else (128, 72)
        bgra = b'\x00\x00\xff\xff' if self.phase == 0 else b'\xff\x00\x00\xff'
        return *size, bgra * (size[0] * size[1])
    def close(self, _):
        self.running = False
        self.closed += 1


class WindowsCaptureGst(unittest.TestCase):
    def test_real_appsrc_bounds_caps_resize_vp8_and_restart(self):
        for cycle in range(2):
            bridge = Frames()
            pipeline = Gst.Pipeline.new(None)
            decoded = Gst.parse_bin_from_description(
                'queue max-size-buffers=2 leaky=downstream ! videoconvert ! videoscale ! '
                'video/x-raw,format=I420,width=64,height=36 ! vp8enc deadline=1 cpu-used=8 ! '
                'vp8dec ! videoconvert ! video/x-raw,format=RGB ! '
                'appsink name=frames sync=false max-buffers=1 drop=true', True)
            sink = decoded.get_by_name('frames')
            ended = []
            def ready(source):
                pipeline.add(source)
                pipeline.add(decoded)
                self.assertTrue(source.link(decoded))
                self.assertNotEqual(pipeline.set_state(Gst.State.PLAYING), Gst.StateChangeReturn.FAILURE)
            capture = adapter.WindowsCapture(ready, lambda: ended.append(True), bridge, Gst, GLib)
            try:
                capture.start()
                self.assertFalse(capture.delivered, 'No source before explicit native selection/frame')
                self.assertEqual(capture.source.get_property('max-buffers'), 1)
                self.assertEqual(capture.source.get_property('leaky-type').value_nick, 'downstream')
                for phase in range(2):
                    bridge.phase = phase
                    deadline = time.monotonic() + 4
                    found = False
                    while time.monotonic() < deadline:
                        context = GLib.MainContext.default()
                        while context.pending():
                            context.iteration(False)
                        sample = sink.emit('try-pull-sample', 50 * Gst.MSECOND)
                        if sample:
                            data = sample.get_buffer().extract_dup(0, 3)
                            if data[phase * 2] > 200 and data[1] < 40:
                                found = True
                                break
                    self.assertTrue(found, f'Cycle {cycle}: decoded color after resize {phase}')
                self.assertEqual(capture.size, (128, 72))
                capture.close()
                capture.close()
                self.assertFalse(capture.poll())
                self.assertEqual(bridge.closed, 1)
                self.assertEqual(ended, [])
            finally:
                capture.close()
                pipeline.set_state(Gst.State.NULL)


if __name__ == '__main__':
    unittest.main()
