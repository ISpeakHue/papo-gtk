"""One silent video preroll frame; bounded output and no GTK/player/UI objects."""
import os
import struct
import sys

import gi

gi.require_version("Gst", "1.0")
gi.require_version("GstPbutils", "1.0")
gi.require_version("GstVideo", "1.0")
from gi.repository import Gst, GstPbutils, GstVideo

Gst.init(None)
uri = Gst.filename_to_uri(os.path.abspath(sys.argv[1]))
info = GstPbutils.Discoverer.new(3 * Gst.SECOND).discover_uri(uri)
streams = info.get_video_streams()
if not streams:
    raise RuntimeError("O arquivo não contém vídeo")
w, h = streams[0].get_width(), streams[0].get_height()
if not (0 < w <= 4096 and 0 < h <= 4096):
    raise RuntimeError("Vídeo fora do limite de dimensões")
scale = min(1, 320 / w, 180 / h)
width, height = max(1, round(w * scale)), max(1, round(h * scale))
pipeline = Gst.parse_launch(
    "uridecodebin name=source ! videoconvert ! videoscale "
    f"! video/x-raw,format=RGBA,width={width},height={height} "
    "! appsink name=frame sync=false max-buffers=1 drop=true"
)
pipeline.get_by_name("source").set_property("uri", uri)
try:
    pipeline.set_state(Gst.State.PAUSED)
    sample = pipeline.get_by_name("frame").emit("try-pull-preroll", 4 * Gst.SECOND)
    if sample is None:
        raise RuntimeError("Quadro inicial indisponível")
    layout = GstVideo.VideoInfo.new_from_caps(sample.get_caps())
    buffer = sample.get_buffer()
    ok, mapped = buffer.map(Gst.MapFlags.READ)
    if not ok:
        raise RuntimeError("Quadro ilegível")
    try:
        stride, offset = layout.stride[0], layout.offset[0]
        if stride < width * 4 or len(mapped.data) < offset + stride * (height - 1) + width * 4:
            raise RuntimeError("Geometria de quadro inválida")
        output = sys.stdout.buffer
        output.write(b"PAPOPOS1" + struct.pack("<II", width, height))
        for row in range(height):
            begin = offset + row * stride
            output.write(mapped.data[begin:begin + width * 4])
        output.flush()
    finally:
        buffer.unmap(mapped)
finally:
    pipeline.set_state(Gst.State.NULL)
