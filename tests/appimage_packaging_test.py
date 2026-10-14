"""GPU-less checks for portable media-plugin and host-driver boundaries."""

import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

POLICY = Path(__file__).resolve().parents[1] / "packaging/appimage/media_policy.py"
spec = importlib.util.spec_from_file_location("papo_media_policy", POLICY)
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


def registry(factories=()):
    return SimpleNamespace(
        ElementFactory=object,
        Registry=SimpleNamespace(get=lambda: SimpleNamespace(get_feature_list=lambda _: factories)),
    )


class MediaPackaging(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.plugins = self.root / "usr/lib64/gstreamer-1.0"
        self.plugins.mkdir(parents=True)
        for filename in policy.HARDWARE_PLUGINS.values():
            (self.plugins / filename).touch()
        self.loader = patch.object(policy.ctypes, "CDLL")
        self.loader.start()
        self.addCleanup(self.loader.stop)

    def test_plugins_without_gpu_or_proprietary_driver_are_valid(self):
        result = policy.check_hardware_plugins(self.root, registry())
        self.assertEqual(set(result), {"va", "nvcodec"})
        self.assertTrue(all(item["bundled"] and not item["registered_decoders"] for item in result.values()))

    def test_missing_plugin_is_a_packaging_failure(self):
        (self.plugins / "libgstnvcodec.so").unlink()
        with self.assertRaisesRegex(AssertionError, "Missing hardware media plugin"):
            policy.check_hardware_plugins(self.root, registry())

    def test_host_drivers_cannot_be_accidentally_distributed(self):
        for name in ("libcuda.so.1", "libnvcuvid.so.1", "libnvidia-encode.so.580.1",
                     "radeonsi_drv_video.so", "iHD_drv_video.so", "iris_dri.so", "libgallium-26.2.4.so"):
            with self.subTest(name=name):
                path = self.root / "usr/lib64" / name
                path.touch()
                try:
                    with self.assertRaisesRegex(AssertionError, "GPU driver must come from host"):
                        policy.check_hardware_plugins(self.root, registry())
                finally:
                    path.unlink()
        for name in ("libgstva.so", "libgstva-1.0.so.0", "libva.so.2", "libgstcuda-1.0.so.0"):
            self.assertIsNone(policy.HOST_DRIVER.fullmatch(name), name)

    def test_reports_bundled_decoder_but_rejects_host_gstreamer_plugin(self):
        location = [self.plugins / "libgstva.so"]
        factory = SimpleNamespace(
            get_plugin_name=lambda: "va", get_name=lambda: "vah264dec",
            get_klass=lambda: "Codec/Decoder/Video/Hardware",
            get_plugin=lambda: SimpleNamespace(get_filename=lambda: str(location[0])),
        )
        result = policy.check_hardware_plugins(self.root, registry([factory]))
        self.assertEqual(result["va"]["registered_decoders"], ["vah264dec"])
        location[0] = Path("/usr/lib64/gstreamer-1.0/libgstva.so")
        with self.assertRaisesRegex(AssertionError, "Hardware decoder loaded outside AppDir"):
            policy.check_hardware_plugins(self.root, registry([factory]))


if __name__ == "__main__":
    unittest.main()
