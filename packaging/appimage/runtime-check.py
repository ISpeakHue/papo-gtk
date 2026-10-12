"""Check the bundled runtime using synthetic data and no physical devices."""

import ctypes
import json
import os
from pathlib import Path
import ssl
import sys

import gi

gi.require_version("Gst", "1.0")
gi.require_version("GstWebRTC", "1.0")
gi.require_version("GstSdp", "1.0")
gi.require_version("GstRtp", "1.0")
from gi.repository import Gio, Gst, GstWebRTC, GstSdp, GstRtp


def main():
    root = Path(os.environ["PAPO_APPDIR"]).resolve()
    assert Path(sys.executable).resolve().is_relative_to(root)
    assert Path(gi.__file__).resolve().is_relative_to(root)
    Gst.init(None)
    required = (
        "webrtcbin", "nicesrc", "nicesink", "dtlsenc", "srtpenc",
        "opusenc", "opusdec", "rtpopuspay", "rtpopusdepay", "level", "volume",
        "vp8enc", "vp8dec", "rtpvp8pay", "rtpvp8depay", "jpegenc", "appsink",
        "videoconvert", "videoscale", "videorate", "pipewiresrc", "v4l2src",
        "autoaudiosrc", "autoaudiosink", "wavparse", "matroskademux", "qtdemux",
        "avdec_h264", "h264parse", "decodebin", "playbin",
        "fakesink", "queue", "capsfilter", "valve", "audioconvert", "audioresample",
    )
    for name in required:
        factory = Gst.ElementFactory.find(name)
        assert factory is not None, f"Missing codec/element: {name}"
        assert Path(factory.get_plugin().get_filename()).resolve().is_relative_to(root), name
    for description in (
        "audiotestsrc num-buffers=8 ! audioconvert ! opusenc ! opusdec ! fakesink",
        "videotestsrc num-buffers=3 ! video/x-raw,width=160,height=90 ! vp8enc deadline=1 ! vp8dec ! fakesink",
    ):
        pipeline = Gst.parse_launch(description)
        try:
            pipeline.set_state(Gst.State.PLAYING)
            message = pipeline.get_bus().timed_pop_filtered(10 * Gst.SECOND, Gst.MessageType.EOS | Gst.MessageType.ERROR)
            assert message is not None, "Synthetic decode timed out"
            if message.type == Gst.MessageType.ERROR:
                raise RuntimeError(message.parse_error()[0].message)
        finally:
            pipeline.set_state(Gst.State.NULL)

    gtk = ctypes.CDLL("libgtk-4.so.1")
    adwaita = ctypes.CDLL("libadwaita-1.so.0")
    assert gtk.gtk_get_minor_version() >= 16
    assert adwaita.adw_get_minor_version() >= 6
    assert ssl.create_default_context().get_ca_certs(), "HTTPS certificates missing"
    print(json.dumps({"status": "ok", "python": sys.version.split()[0],
        "gtk": f"4.{gtk.gtk_get_minor_version()}", "libadwaita": f"1.{adwaita.adw_get_minor_version()}",
        "gstreamer": Gst.version_string(), "checked_elements": len(required)}, indent=2))


if __name__ == "__main__":
    main()
