# EN-US: Play bundled call cues with Papo's mixer identity, preserving the user's audio settings.
# PT-BR: Reproduz os sons de chamada com a identidade do Papo no mixer, preservando as configurações de áudio do usuário.
"""Decode a short bundled cue in a disposable process, independent of GTK players."""
import json
import sys

import gi

gi.require_version("Gst", "1.0")
from gi.repository import GLib, Gst


def configure_audio_output(_pipeline, _bin, element):
    """Use Papo's audio identity, without changing saved volume or mute choices."""
    if element.find_property("stream-properties") is not None:
        properties = Gst.Structure.new_empty("props")
        properties.set_value("application.id", "br.com.papo.gtk")
        properties.set_value("application.name", "Papo")
        properties.set_value("media.role", "event")
        properties.set_value("media.name", "Papo call sound")
        element.set_property("stream-properties", properties)
    if element.find_property("client-name") is not None:
        element.set_property("client-name", "Papo")


def main():
    # These helpers run under Python, but must not inherit Python's restored
    # application mute/volume or appear as an unrelated interpreter in mixers.
    GLib.set_application_name("Papo")
    GLib.set_prgname("papo-gtk")
    Gst.init(None)
    wav = sys.stdin.buffer.read(65537)
    if len(wav) > 65536 or not wav.startswith(b"RIFF"):
        raise ValueError("invalid call cue")
    sink = "fakesink sync=true" if "--test-sink" in sys.argv else "autoaudiosink"
    pipeline = Gst.parse_launch(
        "appsrc name=source format=bytes ! wavparse ! audioconvert ! "
        "audioresample ! volume name=cue-volume volume=0.7 ! " + sink
    )
    # autoaudiosink creates its real sink during the state transition. Configure
    # it before opening its stream, including when PulseAudio is backed by PipeWire.
    pipeline.connect("deep-element-added", configure_audio_output)
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
