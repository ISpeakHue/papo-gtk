"""Decode a short bundled cue in a disposable process, independent of GTK players."""
import json
import sys

import gi

gi.require_version("Gst", "1.0")
from gi.repository import Gst


def main():
    Gst.init(None)
    wav = sys.stdin.buffer.read(65537)
    if len(wav) > 65536 or not wav.startswith(b"RIFF"):
        raise ValueError("invalid call cue")
    sink = "fakesink sync=true" if "--test-sink" in sys.argv else "autoaudiosink"
    pipeline = Gst.parse_launch(
        "appsrc name=source format=bytes ! wavparse ! audioconvert ! "
        "audioresample ! volume name=cue-volume volume=0.7 ! " + sink
    )
    decoded = [0]

    def buffer_probe(_pad, info):
        buffer = info.get_buffer()
        if buffer is not None:
            decoded[0] += buffer.get_size()
        return Gst.PadProbeReturn.OK

    volume = pipeline.get_by_name("cue-volume")
    volume.get_static_pad("sink").add_probe(Gst.PadProbeType.BUFFER, buffer_probe)
    source = pipeline.get_by_name("source")
    try:
        pipeline.set_state(Gst.State.PLAYING)
        buffer = Gst.Buffer.new_allocate(None, len(wav), None)
        buffer.fill(0, wav)
        source.emit("push-buffer", buffer)
        source.emit("end-of-stream")
        event = pipeline.get_bus().timed_pop_filtered(
            5 * Gst.SECOND, Gst.MessageType.ERROR | Gst.MessageType.EOS
        )
        if event is None:
            raise RuntimeError("call cue timed out")
        if event.type == Gst.MessageType.ERROR:
            error, _debug = event.parse_error()
            raise RuntimeError(error.message)
        if decoded[0] == 0:
            raise RuntimeError("call cue produced no decoded audio")
        print(json.dumps({"decoded_bytes": decoded[0]}), flush=True)
    finally:
        pipeline.set_state(Gst.State.NULL)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
