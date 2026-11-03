#!/usr/bin/env python3
"""EN-US: Attended WGC selection/cancel probe. Counts buffers; nothing is stored or transmitted.
PT-BR: Teste assistido de seleção/cancelamento WGC. Conta quadros; nada é salvo ou transmitido.
"""
import argparse
import json
import importlib.util
import os
from pathlib import Path
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--expect-cancel', action='store_true')
parser.add_argument('--frames', type=int, default=60)
parser.add_argument('--test-pattern', action='store_true', help='Require the disposable four-color test window')
parser.add_argument('--resize-control', type=Path, help='Control file for the disposable test window')
args = parser.parse_args()
if sys.platform != 'win32':
    parser.error('Use Windows for this attended probe; running it opens a real system picker')
if not 1 <= args.frames <= 900:
    parser.error('--frames must be between 1 and 900')

import gi
gi.require_version('Gst', '1.0')
from gi.repository import Gst, GLib
Gst.init(None)
root = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('windows_capture', root / 'src/platform/capture/windows.py')
capture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)
loop = GLib.MainLoop()
pipeline = None
count = 0
passed = False
sizes = set()
matching = 0
handoff_connection = None
active_sink = None
timeout_source = None


def finish():
    global pipeline, handoff_connection, active_sink, timeout_source
    portal.close()
    if active_sink is not None and handoff_connection is not None:
        active_sink.disconnect(handoff_connection)
        handoff_connection = None
    if pipeline:
        pipeline.set_state(Gst.State.NULL)
        pipeline.get_state(3 * Gst.SECOND)
        pipeline = None
    active_sink = None
    if timeout_source is not None:
        GLib.source_remove(timeout_source)
        timeout_source = None
    loop.quit()
    return False


def ready(source):
    global pipeline, active_sink, handoff_connection
    if args.expect_cancel:
        raise AssertionError('A cancelled picker must never deliver a source')
    pipeline = Gst.Pipeline.new(None)
    sink = Gst.ElementFactory.make('fakesink')
    sink.set_property('signal-handoffs', True)
    sink.set_property('sync', False)
    def handoff(_sink, buffer, _pad):
        global count, passed, matching
        count += 1
        # EN-US: Hold negotiated caps alive across reads; the appsrc property can change during resize.
        # PT-BR: Mantém os caps negociados vivos; a propriedade appsrc pode mudar ao redimensionar.
        caps = _pad.get_current_caps()
        structure = caps.get_structure(0)
        width, height = structure.get_value('width'), structure.get_value('height')
        sizes.add((width, height))
        if args.test_pattern:
            expected = ((26,26,217),(51,204,26),(230,77,26),(26,204,242))
            points = ((.25,.35),(.75,.35),(.25,.75),(.75,.75))
            colors = [buffer.extract_dup((int(height*y)*width+int(width*x))*4,3) for x,y in points]
            matching += all(all(abs(value-wanted)<45 for value,wanted in zip(color,want)) for color,want in zip(colors,expected))
        if count == 40 and args.resize_control:
            args.resize_control.write_text(json.dumps({'width':width-200 if width>1000 else width+200,'height':540 if height>600 else 720}),encoding='utf-8')
        if count == args.frames:
            passed = (not args.test_pattern or matching > count//2) and (not args.resize_control or len(sizes)>1)
            GLib.idle_add(finish)
    active_sink = sink
    handoff_connection = sink.connect('handoff', handoff)
    pipeline.add(source)
    pipeline.add(sink)
    assert source.link(sink)
    pipeline.set_state(Gst.State.PLAYING)


def ended():
    global passed
    passed = args.expect_cancel and count == 0 and not portal.delivered and portal.terminal_state == 2 and portal.failure is None
    finish()


portal = capture.create_screen_capture(ready, ended)
print('Cancel the system picker.' if args.expect_cancel else 'Select a disposable test window. Resize it while buffers are counted.', flush=True)
portal.start()
timeout_source = GLib.timeout_add_seconds(140, finish)
try:
    loop.run()
finally:
    finish()
print(json.dumps({'terminal_state': portal.terminal_state, 'native': portal.terminal_diagnostics, 'failure': portal.failure, 'buffers': count, 'sizes':sorted(sizes), 'matching_pattern_frames':matching}), flush=True)
assert passed and portal.closed and portal.handle is None, 'Capture/cancellation acceptance failed'
print(f'PASS: {count} buffers; native capture joined and released; nothing stored or sent', flush=True)
