# EN-US: Decode silent animation frames with bounded output outside the GTK player.
# PT-BR: Decodifica quadros de animação sem áudio, com saída limitada e fora do reprodutor GTK.
"""Bounded silent video-to-animation decoding outside GTK's player lifecycle."""
import math
import os
import struct
import sys

# Reject manifests before native discovery. The app also isolates this process.
with open(sys.argv[1], "rb") as media_input:
    header = media_input.read(12)
if not (header.startswith(b"\x1a\x45\xdf\xa3") or header[4:8] in (b"ftyp", b"moov", b"mdat", b"free", b"wide")):
    raise ValueError("Unsupported or externally referenced media")

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
original_width, original_height = streams[0].get_width(), streams[0].get_height()
duration = info.get_duration() / Gst.SECOND
if not (0 < original_width <= 4096 and 0 < original_height <= 4096 and 0 < duration <= 60):
    raise RuntimeError("GIF fora dos limites de tamanho ou duração")
fps = 10
expected_frames = math.ceil(duration * fps) + 2
scale = min(1, 420 / original_width, 300 / original_height)
scale = min(scale, math.sqrt((48 * 1024 * 1024) / (4 * expected_frames * original_width * original_height)))
width, height = max(1, int(original_width * scale)), max(1, int(original_height * scale))
pipeline = Gst.parse_launch(
    f"uridecodebin name=source ! videoconvert ! videoscale ! videorate "
    f"! video/x-raw,format=RGBA,width={width},height={height},framerate={fps}/1 "
    "! appsink name=frames sync=false max-buffers=2 drop=false"
)
pipeline.get_by_name("source").set_property("uri", uri)
sink = pipeline.get_by_name("frames")
bus = pipeline.get_bus()
output = sys.stdout.buffer
count = total = 0
try:
    output.write(b"PAPOGIF1" + struct.pack("<II", width, height))
    pipeline.set_state(Gst.State.PLAYING)
    while True:
        error = bus.pop_filtered(Gst.MessageType.ERROR)
        if error:
            raise RuntimeError(error.parse_error()[0].message)
        sample = sink.emit("try-pull-sample", Gst.SECOND // 10)
        if sample is None:
            if sink.get_property("eos"):
                break
            continue
        buffer = sample.get_buffer()
        ok, mapped = buffer.map(Gst.MapFlags.READ)
        if not ok:
            raise RuntimeError("Não foi possível ler um quadro")
        try:
            size = width * height * 4
            layout = GstVideo.VideoInfo.new_from_caps(sample.get_caps())
            stride, offset = layout.stride[0], layout.offset[0]
            if stride < width * 4 or len(mapped.data) < offset + stride * (height - 1) + width * 4:
                raise RuntimeError("Geometria inesperada no quadro")
            count += 1
            total += size
            if count > 602 or total > 48 * 1024 * 1024:
                raise RuntimeError("GIF maior que o limite de quadros decodificados")
            output.write(struct.pack("<I", 1000 // fps))
            for row in range(height):
                begin = offset + row * stride
                output.write(mapped.data[begin:begin + width * 4])
        finally:
            buffer.unmap(mapped)
    if count == 0:
        raise RuntimeError("Nenhum quadro decodificado")
    output.flush()
finally:
    pipeline.set_state(Gst.State.NULL)
