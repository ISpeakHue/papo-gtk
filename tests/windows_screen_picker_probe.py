#!/usr/bin/env python3
"""EN-US: Attended WGC selection/cancel probe. Counts buffers; nothing is stored or transmitted.
PT-BR: Teste assistido de seleção/cancelamento WGC. Conta quadros; nada é salvo ou transmitido.
"""
import argparse
import importlib.util
import os
from pathlib import Path
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--expect-cancel', action='store_true')
parser.add_argument('--frames', type=int, default=60)
args = parser.parse_args()
if sys.platform != 'win32' or os.environ.get('PAPO_WINDOWS_CAPTURE_PREVIEW') != '1':
    parser.error('Use Windows and explicitly set PAPO_WINDOWS_CAPTURE_PREVIEW=1 before running this attended probe')
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


def finish():
    global pipeline
    portal.close()
    if pipeline:
        pipeline.set_state(Gst.State.NULL)
        pipeline = None
    loop.quit()
    return False


def ready(source):
    global pipeline
    if args.expect_cancel:
        raise AssertionError('A cancelled picker must never deliver a source')
    pipeline = Gst.Pipeline.new(None)
    sink = Gst.ElementFactory.make('fakesink')
    sink.set_property('signal-handoffs', True)
    sink.set_property('sync', False)
    def handoff(*_):
        global count, passed
        count += 1
        if count == args.frames:
            passed = True
            GLib.idle_add(finish)
    sink.connect('handoff', handoff)
    pipeline.add(source)
    pipeline.add(sink)
    assert source.link(sink)
    pipeline.set_state(Gst.State.PLAYING)


def ended():
    global passed
    passed = args.expect_cancel and count == 0 and not portal.delivered
    finish()


portal = capture.create_screen_capture(ready, ended)
print('Cancel the system picker.' if args.expect_cancel else 'Select a disposable test window. Resize it while buffers are counted.', flush=True)
portal.start()
GLib.timeout_add_seconds(140, finish)
try:
    loop.run()
finally:
    finish()
assert passed and portal.closed and portal.handle is None, 'Capture/cancellation acceptance failed'
print(f'PASS: {count} buffers; native capture joined and released; nothing stored or sent', flush=True)
