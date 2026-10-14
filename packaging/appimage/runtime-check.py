"""Check the bundled runtime using synthetic data and no physical devices."""

import ctypes
import json
import os
from pathlib import Path
import ssl
import subprocess
import sys

from media_policy import check_hardware_plugins

import gi

gi.require_version("Gst", "1.0")
gi.require_version("GstWebRTC", "1.0")
gi.require_version("GstSdp", "1.0")
gi.require_version("GstRtp", "1.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import Gio, Gst, GstWebRTC, GstSdp, GstRtp, GdkPixbuf


def main():
    root = Path(os.environ["PAPO_APPDIR"]).resolve()
    assert Path(sys.executable).resolve().is_relative_to(root)
    assert Path(gi.__file__).resolve().is_relative_to(root)
    # GUI client modules must be shipped even when the desktop config selects
    # IBus/Fcitx; installing GTK alone supplies only its built-in contexts.
    input_dir=root / 'usr/lib64/gtk-4.0/4.0.0/immodules'
    for name in ('libim-ibus.so','libim-fcitx5.so'):
        module=input_dir/name
        assert module.is_file(), f"Missing GTK input client: {name}"
        ctypes.CDLL(str(module))
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
    hardware=check_hardware_plugins(root,Gst)
    fixture=root / 'usr/share/papo-gtk/chat-h264.mp4'
    assert fixture.is_file(), "Missing synthetic H264 fixture"
    for description in (
        "audiotestsrc num-buffers=8 ! audioconvert ! opusenc ! opusdec ! fakesink",
        "videotestsrc num-buffers=3 ! video/x-raw,width=160,height=90 ! vp8enc deadline=1 ! vp8dec ! fakesink",
        f'filesrc location="{fixture}" ! qtdemux name=demux demux.video_0 ! h264parse ! avdec_h264 ! fakesink sync=false',
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
    assert gtk.gtk_get_minor_version() >= 22
    assert adwaita.adw_get_minor_version() >= 9
    assert ssl.create_default_context().get_ca_certs(), "HTTPS certificates missing"
    fonts=subprocess.check_output([str(root / 'usr/bin/fc-list'), '--format', '%{family}\n'], text=True)
    assert 'Adwaita Sans' in fonts and 'Noto Color Emoji' in fonts, 'Fallback fonts missing'
    configuration=subprocess.check_output([str(root / 'usr/bin/fc-conflist')], text=True)
    assert '/etc/fonts/fonts.conf' in configuration, 'Desktop font configuration not loaded'
    icon=GdkPixbuf.Pixbuf.new_from_file(str(root / 'br.com.papo.gtk.png'))
    status=GdkPixbuf.Pixbuf.new_from_file(str(root / 'usr/share/papo-gtk/status-check.svg'))
    assert icon.get_width()==256 and status.get_width()==16, 'Portable PNG/SVG decoding failed'
    print(json.dumps({"status": "ok", "python": sys.version.split()[0],
        "gtk": f"4.{gtk.gtk_get_minor_version()}", "libadwaita": f"1.{adwaita.adw_get_minor_version()}",
        "gstreamer": Gst.version_string(), "hardware_media_plugins": hardware,
        "software_h264_decode": True, "checked_elements": len(required),
        "input_clients": ["ibus", "fcitx5"], "desktop_font_config": True, "fallback_fonts": True, "png_svg_decoding": True}, indent=2))


if __name__ == "__main__":
    main()
