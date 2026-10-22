#!/usr/bin/env python3
"""EN-US: Inspect this disposable VM or press a setup key through its local QMP socket.
PT-BR: Inspeciona a VM descartável ou envia uma tecla de instalação pelo socket QMP local.
"""
import argparse
import json
from pathlib import Path
import socket
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--socket', type=Path, default=Path('/tmp/papo-windows-clean-vm/qmp.sock'))
    parser.add_argument('--screenshot', type=Path)
    parser.add_argument('--key', choices=['spc', 'ret', 'esc', 'tab', 'up', 'down', 'alt-f4', 'shift-f10', 'meta_l-r', 'meta_l-n'])
    parser.add_argument('--click', nargs=2, type=int, metavar=('X', 'Y'), help='Send a real left click after inspecting the fixture screenshot')
    parser.add_argument('--screen-size', nargs=2, type=int, default=[1280, 800], metavar=('WIDTH', 'HEIGHT'))
    parser.add_argument('--powerdown', action='store_true')
    parser.add_argument('--attach-results', action='store_true')
    parser.add_argument('--input-disk', type=Path, help='Attach a task-owned FAT image read-only for updated test fixtures')
    parser.add_argument('--type-file', type=Path, help='Type an ASCII diagnostic command into the active guest console')
    parser.add_argument('--quit', action='store_true', help='Abort only this disposable guest; prefer normal Windows shutdown')
    args = parser.parse_args()
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(10)
        connection.connect(str(args.socket))
        stream = connection.makefile('rwb')
        stream.readline()

        def command(name, arguments=None):
            payload = {'execute': name}
            if arguments is not None:
                payload['arguments'] = arguments
            stream.write(json.dumps(payload).encode() + b'\n')
            stream.flush()
            while True:
                line = stream.readline()
                if not line:
                    raise RuntimeError('VM disconnected')
                result = json.loads(line)
                if 'error' in result:
                    raise RuntimeError(result['error'])
                if 'return' in result:
                    return result['return']

        command('qmp_capabilities')
        if args.attach_results:
            command('device_add', {'driver': 'usb-storage', 'bus': 'usb.0', 'drive': 'results', 'id': 'papo_results', 'removable': True})
        if args.input_disk:
            # EN-US: Updated fixtures remain read-only; the guest never receives a host directory.
            # PT-BR: Arquivos de teste atualizados ficam somente leitura; o guest não recebe pastas do host.
            command('blockdev-add', {'driver': 'raw', 'node-name': 'papo_fixture_update', 'read-only': True,
                                    'file': {'driver': 'file', 'filename': str(args.input_disk.resolve(strict=True))}})
            command('device_add', {'driver': 'usb-storage', 'bus': 'usb.0', 'drive': 'papo_fixture_update',
                                   'id': 'papo_fixture_update_usb', 'removable': True})
        if args.key:
            command('human-monitor-command', {'command-line': f'sendkey {args.key}'})
        if args.click:
            # EN-US: Coordinates come from the inspected guest, never the host desktop.
            # PT-BR: As coordenadas vêm do guest inspecionado, nunca do desktop do host.
            x, y = args.click
            width, height = args.screen_size
            if width < 2 or height < 2 or not (0 <= x < width and 0 <= y < height):
                raise ValueError('Click outside the specified guest display')
            command('input-send-event', {'events': [
                {'type': 'abs', 'data': {'axis': 'x', 'value': round(x * 32767 / (width - 1))}},
                {'type': 'abs', 'data': {'axis': 'y', 'value': round(y * 32767 / (height - 1))}},
            ]})
            command('input-send-event', {'events': [{'type': 'btn', 'data': {'down': True, 'button': 'left'}}]})
            command('input-send-event', {'events': [{'type': 'btn', 'data': {'down': False, 'button': 'left'}}]})
        if args.type_file:
            text = args.type_file.read_text(encoding='ascii')
            plain = {' ': 'spc', '\n': 'ret', '.': 'dot', ',': 'comma', '/': 'slash', '\\': 'backslash', '-': 'minus', '=': 'equal', ';': 'semicolon', "'": 'apostrophe', '[': 'bracket_left', ']': 'bracket_right', '`': 'grave_accent'}
            shifted = dict(zip('!@#$%^&*()_+{}:"<>?|~', ['1','2','3','4','5','6','7','8','9','0','minus','equal','bracket_left','bracket_right','semicolon','apostrophe','comma','dot','slash','backslash','grave_accent']))
            for character in text:
                shift = character.isupper() or character in shifted
                key = shifted.get(character, plain.get(character, character.lower()))
                if not (character.isascii() and (character.isalnum() or character in plain or character in shifted)):
                    raise ValueError('Unsupported console character')
                keys = ['shift', key] if shift else [key]
                events = [{'type':'key', 'data':{'down':True, 'key':{'type':'qcode', 'data':value}}} for value in keys]
                events += [{'type':'key', 'data':{'down':False, 'key':{'type':'qcode', 'data':value}}} for value in reversed(keys)]
                command('input-send-event', {'events':events})
                time.sleep(0.025)
        if args.screenshot:
            command('screendump', {'filename': str(args.screenshot.resolve()), 'format': 'png'})
        if args.quit:
            command('quit')
            return
        if args.powerdown:
            command('system_powerdown')
        print(json.dumps(command('query-status')))


if __name__ == '__main__':
    main()
