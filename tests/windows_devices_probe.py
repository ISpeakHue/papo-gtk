#!/usr/bin/env python3
"""EN-US: Explicit Windows device checks; captured samples are decoded and discarded locally.
PT-BR: Verificações explícitas de dispositivos Windows; amostras são decodificadas e descartadas localmente.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def worker():
    import importlib.util
    spec = importlib.util.spec_from_file_location('voice_engine', ROOT / 'src/voice/engine.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def inspect_devices(voice):
    return {kind: [{'id': identifier, 'name': name} for identifier, name, _ in voice.devices(kind)]
            for kind in ('Audio/Source', 'Audio/Sink', 'Video/Source')}


def microphone(voice, identifier, cycles):
    Gst = voice.Gst
    results = []
    for cycle in range(cycles):
        if identifier == 'default':
            factory = next((name for name in ('wasapi2src', 'wasapisrc') if Gst.ElementFactory.find(name)), None)
            source = Gst.ElementFactory.make(factory) if factory else None
        else:
            entries = voice.devices('Audio/Source')
            selected = voice.select_device(identifier, entries)
            source = selected.create_element(None) if selected else None
        if not source:
            raise RuntimeError('Selected microphone unavailable: ' + repr(identifier))
        pipeline = Gst.Pipeline.new(None)
        chain = Gst.parse_bin_from_description(
            'audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! '
            'volume name=microphone mute=true ! level name=meter interval=100000000 ! '
            'opusenc bitrate=32000 ! opusdec ! fakesink name=discard signal-handoffs=true sync=false', True)
        pipeline.add(source)
        pipeline.add(chain)
        if not source.link(chain):
            raise RuntimeError('Microphone could not link')
        count = [0]
        def decoded(*_):
            count[0] += 1
        chain.get_by_name('discard').connect('handoff', decoded)
        volume = chain.get_by_name('microphone')
        engine = voice.Engine()
        engine.microphone = volume
        bus = pipeline.get_bus()
        phases = []
        try:
            if pipeline.set_state(Gst.State.PLAYING) == Gst.StateChangeReturn.FAILURE:
                raise RuntimeError('Microphone start failed; check Windows privacy settings and device availability')
            for gain, muted in ((1.5, True), (0.0, False), (1.5, False), (3.0, False), (3.0, True)):
                previous_mute = volume.get_property('mute')
                engine.set_microphone_gain(gain)
                if volume.get_property('mute') != previous_mute:
                    raise AssertionError('Changing gain changed mute')
                volume.set_property('mute', muted)
                began = time.monotonic()
                before = count[0]
                levels = []
                while time.monotonic() - began < 0.7:
                    message = bus.timed_pop_filtered(100 * Gst.MSECOND, Gst.MessageType.ERROR | Gst.MessageType.ELEMENT)
                    if message and message.type == Gst.MessageType.ERROR:
                        error, _ = message.parse_error()
                        raise RuntimeError(f'Microphone error domain={error.domain} code={error.code}; check Windows privacy and device settings')
                    if message and message.type == Gst.MessageType.ELEMENT and time.monotonic() - began > 0.25:
                        structure = message.get_structure()
                        if structure and structure.get_name() == 'level':
                            levels.extend(float(value) for value in structure.get_value('rms'))
                if not levels or count[0] <= before:
                    raise AssertionError('Microphone produced no decoded audio buffers')
                if (muted or gain == 0) and max(levels) > -100:
                    raise AssertionError('Mute/zero gain did not silence the captured signal')
                phases.append({'gain': gain, 'muted': muted, 'decoded_buffers': count[0] - before,
                               'peak_rms_db': round(max(levels), 2)})
        finally:
            pipeline.set_state(Gst.State.NULL)
            _, state, _ = pipeline.get_state(3 * Gst.SECOND)
            if state != Gst.State.NULL:
                raise RuntimeError('Microphone did not release its native pipeline')
        results.append({'cycle': cycle + 1, 'phases': phases, 'released': True})
    return {'microphone': identifier, 'cycles': results, 'recording_saved': False, 'network_used': False}


def cues():
    results = {}
    for name in ('voice_join', 'voice_leave'):
        completed = subprocess.run([sys.executable, str(ROOT / 'src/media/sound.py')],
                                   input=(ROOT / f'src/media/{name}.wav').read_bytes(), capture_output=True,
                                   timeout=8, creationflags=subprocess.CREATE_NO_WINDOW, check=True)
        report = json.loads(completed.stdout)
        if not set(report['output_factories']) & {'wasapi2sink', 'wasapisink'}:
            raise RuntimeError('Cue did not select a native Windows playback sink')
        results[name] = report
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_mutually_exclusive_group(required=True)
    actions.add_argument('--list', action='store_true')
    actions.add_argument('--microphone', help='Endpoint ID from --list, or default; opens that microphone')
    actions.add_argument('--cues', action='store_true', help='Play the real join/leave cues on the current default output')
    parser.add_argument('--cycles', type=int, choices=range(1, 21), default=1)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/windows-dev/step8/devices.json')
    parser.add_argument('--child', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if sys.platform != 'win32':
        parser.error('Use the native Windows Python/GI runtime')
    if args.child:
        # EN-US: The parent owns the process tree before any native plugin/device initialization.
        # PT-BR: O pai controla a árvore de processos antes de inicializar plugins/dispositivos nativos.
        if sys.stdin.buffer.read(1) != b'1':
            raise RuntimeError('Missing process ownership handshake')
        if args.cues:
            result = cues()
        else:
            voice = worker()
            result = inspect_devices(voice) if args.list else microphone(voice, args.microphone, args.cycles)
        print(json.dumps({'status': 'pass', 'result': result}), flush=True)
        return
    sys.path.insert(0, str(ROOT / 'packaging/windows'))
    from devcheck import probe_job
    command = [sys.executable, '-B', str(Path(__file__).resolve()), *sys.argv[1:], '--child']
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=output, stderr=errors,
                                 creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            with probe_job(child):
                child.stdin.write(b'1')
                child.stdin.close()
                child.wait(timeout=20 + args.cycles * 10)
            output.seek(0)
            data = output.read(65537)
            if child.returncode or len(data) > 65536:
                errors.seek(0)
                raise RuntimeError('Device check failed: ' + errors.read(4096).decode('utf-8', errors='replace'))
            report = json.loads(data)
        except Exception as error:
            report = {'status': 'fail', 'error': str(error)}
        finally:
            if child.stdin and not child.stdin.closed:
                child.stdin.close()
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report), flush=True)
    if report['status'] != 'pass':
        raise SystemExit(1)


if __name__ == '__main__':
    main()
