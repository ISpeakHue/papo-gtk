#!/usr/bin/env python3
"""User-approved real portal + PipeWire probe; buffers are counted, not stored."""
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src/voice"))
from screen_portal import ScreenPortal
import gi
gi.require_version("Gst", "1.0")
from gi.repository import Gst, GLib
Gst.init(None)
loop = GLib.MainLoop()
pipeline = None
count = 0
passed = False

def finish():
    global pipeline
    if pipeline:
        pipeline.set_state(Gst.State.NULL)
        pipeline = None
    portal.close()
    loop.quit()
    return False

def ready(fd, node, serial):
    global pipeline
    pipeline = Gst.Pipeline.new("portal-probe")
    source = Gst.ElementFactory.make("pipewiresrc")
    source.set_property("fd", fd)
    if serial is not None and source.find_property("target-object"):
        source.set_property("target-object", str(serial))
    else:
        source.set_property("path", str(node))
    sink = Gst.ElementFactory.make("fakesink")
    sink.set_property("signal-handoffs", True)
    sink.set_property("sync", False)
    def handoff(*args):
        global count, passed
        count += 1
        if count == 10:
            passed = True
            print("Received 10 native PipeWire video buffers; closing capture and permission session.", flush=True)
            GLib.idle_add(finish)
    sink.connect("handoff", handoff)
    pipeline.add(source)
    pipeline.add(sink)
    assert source.link_filtered(sink, Gst.Caps.from_string("video/x-raw"))
    bus = pipeline.get_bus()
    bus.add_signal_watch()
    def message(bus, event):
        if event.type == Gst.MessageType.ERROR:
            print("PipeWire capture failed:", event.parse_error(), flush=True)
            finish()
    bus.connect("message", message)
    pipeline.set_state(Gst.State.PLAYING)
    GLib.timeout_add_seconds(15, finish)

def ended():
    print("Screen picker cancelled/denied or sharing ended; resources released.", flush=True)
    finish()

portal = ScreenPortal(ready, ended)
print("Opening desktop screen picker. Select a test window; capture stops after 10 buffers. Nothing is saved or sent.", flush=True)
portal.start()
try:
    loop.run()
finally:
    finish()
sys.exit(0 if passed else 1)
