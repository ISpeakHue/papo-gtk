"""Call cues use their own mixer identity; tests never open an audio device."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("call_sound", ROOT / "src/media/sound.py")
sound = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sound)
sound.Gst.init(None)


class CallSoundTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == "linux", "Linux-only PulseAudio mixer identity; native Windows cues are checked separately")
    def test_auto_output_children_get_papos_identity_before_playback(self):
        sink = sound.Gst.ElementFactory.make("pulsesink")
        if sink is None:
            self.skipTest("PulseAudio GStreamer plugin is not installed")
        # Even an intentionally muted sink keeps its mute and volume: the fix
        # isolates application identities instead of overriding mixer choices.
        sink.set_property("mute", True)
        sink.set_property("volume", 0.0)
        pipeline = sound.Gst.Pipeline.new(None)
        pipeline.connect("deep-element-added", sound.configure_audio_output)
        output = sound.Gst.Bin.new(None)
        pipeline.add(output)
        output.add(sink)
        properties = sink.get_property("stream-properties")
        self.assertEqual(properties.get_string("application.id"), "br.com.papo.gtk")
        self.assertEqual(properties.get_string("application.name"), "Papo")
        self.assertEqual(properties.get_string("media.role"), "event")
        self.assertEqual(sink.get_property("client-name"), "Papo")
        self.assertTrue(sink.get_property("mute"))
        self.assertEqual(sink.get_property("volume"), 0.0)

    def test_both_cues_decode_through_the_real_helper(self):
        for name in ("voice_join", "voice_leave"):
            with self.subTest(cue=name):
                wav = (ROOT / f"src/media/{name}.wav").read_bytes()
                result = subprocess.run(
                    [sys.executable, str(ROOT / "src/media/sound.py"), "--test-sink"],
                    input=wav, capture_output=True, check=True, timeout=10,
                )
                self.assertEqual(json.loads(result.stdout)["decoded_bytes"], len(wav) - 44)


if __name__ == "__main__":
    unittest.main()
