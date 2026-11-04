#!/usr/bin/env python3
"""EN-US: Send a wheel/click only to a named private scrolling-test window.
PT-BR: Envia roda/clique somente a uma janela privada nomeada do teste de rolagem.
"""
import ctypes as c
import sys

if sys.platform != 'linux' or not sys.argv[1].startswith('Papo smooth scrolling test'):
    raise SystemExit('Private Linux/X11 scrolling fixture required')
x11 = c.CDLL('libX11.so.6')
xtst = c.CDLL('libXtst.so.6')
ptr, win = c.c_void_p, c.c_ulong
x11.XOpenDisplay.argtypes = [c.c_char_p]
x11.XOpenDisplay.restype = ptr
x11.XDefaultRootWindow.argtypes = [ptr]
x11.XDefaultRootWindow.restype = win
x11.XQueryTree.argtypes = [ptr, win, c.POINTER(win), c.POINTER(win), c.POINTER(c.POINTER(win)), c.POINTER(c.c_uint)]
x11.XFetchName.argtypes = [ptr, win, c.POINTER(ptr)]
x11.XTranslateCoordinates.argtypes = [ptr, win, win, c.c_int, c.c_int, c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(win)]
x11.XSetInputFocus.argtypes = [ptr, win, c.c_int, c.c_ulong]
x11.XStringToKeysym.argtypes = [c.c_char_p]
x11.XStringToKeysym.restype = c.c_ulong
x11.XKeysymToKeycode.argtypes = [ptr, c.c_ulong]
x11.XKeysymToKeycode.restype = c.c_uint
x11.XFree.argtypes = [ptr]
x11.XSync.argtypes = [ptr, c.c_int]
x11.XCloseDisplay.argtypes = [ptr]
xtst.XTestFakeMotionEvent.argtypes = [ptr, c.c_int, c.c_int, c.c_int, c.c_ulong]
xtst.XTestFakeButtonEvent.argtypes = [ptr, c.c_uint, c.c_int, c.c_ulong]
xtst.XTestFakeKeyEvent.argtypes = [ptr, c.c_uint, c.c_int, c.c_ulong]
display = x11.XOpenDisplay(None)
if not display:
    raise RuntimeError('Private regression display unavailable')


def find(parent):
    name = ptr()
    if x11.XFetchName(display, parent, c.byref(name)) and name.value:
        text = c.string_at(name.value).decode(errors='replace')
        x11.XFree(name)
        if text == sys.argv[1]:
            return parent
    root, owner, children, count = win(), win(), c.POINTER(win)(), c.c_uint()
    if x11.XQueryTree(display, parent, c.byref(root), c.byref(owner), c.byref(children), c.byref(count)):
        ids = list(children[:count.value])
        x11.XFree(children)
        for child in ids:
            target = find(child)
            if target:
                return target
    return None


key = 0
try:
    root = x11.XDefaultRootWindow(display)
    target = find(root)
    if not target:
        raise RuntimeError('Named scrolling fixture not found')
    x, y = int(sys.argv[2]), int(sys.argv[3])
    if not (0 <= x <= 4096 and 0 <= y <= 4096):
        raise RuntimeError('Invalid fixture coordinates')
    rx, ry, child = c.c_int(), c.c_int(), win()
    x11.XTranslateCoordinates(display, target, root, x, y, c.byref(rx), c.byref(ry), c.byref(child))
    x11.XSetInputFocus(display, target, 2, 0)
    xtst.XTestFakeMotionEvent(display, -1, rx.value, ry.value, 0)
    modifier, _, action = sys.argv[4].rpartition('+')
    if modifier:
        if modifier not in ('Shift_L', 'Control_L'):
            raise RuntimeError('Unsupported fixture modifier')
        key = x11.XKeysymToKeycode(display, x11.XStringToKeysym(modifier.encode()))
        xtst.XTestFakeKeyEvent(display, key, 1, 0)
    button = {'up': 4, 'down': 5, 'left': 6, 'right': 7, 'click': 1}[action]
    xtst.XTestFakeButtonEvent(display, button, 1, 0)
    xtst.XTestFakeButtonEvent(display, button, 0, 0)
finally:
    if key:
        xtst.XTestFakeKeyEvent(display, key, 0, 0)
    x11.XSync(display, 0)
    x11.XCloseDisplay(display)
