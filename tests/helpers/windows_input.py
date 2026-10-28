#!/usr/bin/env python3
"""EN-US: Send native keys only to a foreground test window owned by the invoking Rust test.
PT-BR: Envia teclas nativas só à janela de teste em primeiro plano pertencente ao teste Rust chamador.
"""
import ctypes as c
import os
import sys
import time

U32, U16 = c.c_uint32, c.c_uint16


class Keyboard(c.Structure):
    _fields_ = [('vk', U16), ('scan', U16), ('flags', U32), ('time', U32), ('extra', c.c_size_t)]


class Mouse(c.Structure):
    _fields_ = [('x', c.c_int32), ('y', c.c_int32), ('data', U32), ('flags', U32), ('time', U32), ('extra', c.c_size_t)]


class Payload(c.Union):
    _fields_ = [('keyboard', Keyboard), ('mouse', Mouse)]


class Input(c.Structure):
    _fields_ = [('kind', U32), ('payload', Payload)]


def events(chord):
    names = {'Control_L': 0x11, 'Shift_L': 0x10, 'Alt_L': 0x12, 'Meta_L': 0x5b,
             'Return': 0x0d, 'Tab': 0x09, 'Home': 0x24, 'End': 0x23, 'BackSpace': 0x08,
             'Delete': 0x2e, 'Escape': 0x1b, 'space': 0x20, 'Page_Up': 0x21, 'Page_Down': 0x22}
    codes = []
    for name in chord.split('+'):
        code = names.get(name)
        if code is None and len(name) == 1 and name.isascii() and name.isalnum():
            code = ord(name.upper())
        if code is None:
            raise ValueError('Unsupported native test key')
        codes.append(code)
    return [Input(1, Payload(keyboard=Keyboard(code, 0, up | (1 if code in (0x21, 0x22, 0x23, 0x24, 0x2e) else 0), 0, 0)))
            for keys, up in [(codes, 0), (list(reversed(codes)), 2)] for code in keys]


def run(pid, title, chords):
    if os.name != 'nt' or pid != os.getppid() or os.environ.get('PAPO_NATIVE_INPUT_TEST') != '1':
        raise RuntimeError('Native input requires explicit opt-in and the invoking test process')
    if title != 'Papo desktop input test':
        raise RuntimeError('Unexpected native test window title')
    api = c.WinDLL('user32', use_last_error=True)
    callback = c.WINFUNCTYPE(c.c_int, c.c_void_p, c.c_ssize_t)
    api.EnumWindows.argtypes = (callback, c.c_ssize_t)
    api.GetWindowThreadProcessId.argtypes = (c.c_void_p, c.POINTER(U32))
    api.GetWindowTextW.argtypes = (c.c_void_p, c.c_wchar_p, c.c_int)
    api.GetForegroundWindow.restype = c.c_void_p
    api.SetForegroundWindow.argtypes = (c.c_void_p,)
    api.IsWindow.argtypes = (c.c_void_p,)
    api.SendInput.argtypes = (U32, c.POINTER(Input), c.c_int)
    api.SendInput.restype = U32
    api.GetAsyncKeyState.argtypes = (c.c_int,)
    api.GetAsyncKeyState.restype = c.c_int16
    targets = []
    def visit(hwnd, _):
        owner, text = U32(), c.create_unicode_buffer(256)
        api.GetWindowThreadProcessId(hwnd, c.byref(owner))
        if owner.value != pid:
            return 1
        api.GetWindowTextW(hwnd, text, len(text))
        if text.value == title:
            targets.append(hwnd)
        return 1
    enum = callback(visit)
    api.EnumWindows(enum, 0)
    if len(targets) != 1:
        raise RuntimeError('Unique owned test window not found')
    target = targets[0]
    api.SetForegroundWindow(target)
    for chord in chords:
        owner = U32()
        api.GetWindowThreadProcessId(target, c.byref(owner))
        if not api.IsWindow(target) or owner.value != pid or api.GetForegroundWindow() != target:
            raise RuntimeError('Test lost foreground ownership; no input was sent')
        if any(api.GetAsyncKeyState(key) & 0x8000 for key in (0x10, 0x11, 0x12, 0x5b, 0x5c)):
            raise RuntimeError('Release physical modifier keys before the disposable input test')
        batch = events(chord)
        inputs = (Input * len(batch))(*batch)
        if api.SendInput(len(inputs), inputs, c.sizeof(Input)) != len(inputs):
            # EN-US: Release the test's keys after a partial batch; never leave a synthetic modifier held.
            # PT-BR: Libera as teclas do teste após um lote parcial; não deixa modificador sintético pressionado.
            releases = [event for event in batch if event.payload.keyboard.flags & 2]
            api.SendInput(len(releases), (Input * len(releases))(*releases), c.sizeof(Input))
            raise RuntimeError('Windows rejected native test input')
        time.sleep(.015)


if __name__ == '__main__':
    run(int(sys.argv[1]), sys.argv[2], sys.argv[3:])
