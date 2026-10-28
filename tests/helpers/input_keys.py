#!/usr/bin/env python3
"""Send real keys only to the explicitly named private regression window."""
import ctypes as c
import sys
import time

# EN-US: Fail explicitly before loading X11 on Windows or macOS.
# PT-BR: Falha explicitamente antes de carregar X11 no Windows ou macOS.
if sys.platform != 'linux':
    raise SystemExit('Linux-only X11/XTest fixture; use the native platform input helper')

x11=c.CDLL('libX11.so.6');xtst=c.CDLL('libXtst.so.6')
ptr=c.c_void_p;window=c.c_ulong
x11.XOpenDisplay.argtypes=[c.c_char_p];x11.XOpenDisplay.restype=ptr
x11.XDefaultRootWindow.argtypes=[ptr];x11.XDefaultRootWindow.restype=window
x11.XQueryTree.argtypes=[ptr,window,c.POINTER(window),c.POINTER(window),c.POINTER(c.POINTER(window)),c.POINTER(c.c_uint)]
x11.XFetchName.argtypes=[ptr,window,c.POINTER(ptr)]
x11.XFree.argtypes=[ptr]
x11.XStringToKeysym.argtypes=[c.c_char_p];x11.XStringToKeysym.restype=c.c_ulong
x11.XKeysymToKeycode.argtypes=[ptr,c.c_ulong];x11.XKeysymToKeycode.restype=c.c_uint
x11.XSetInputFocus.argtypes=[ptr,window,c.c_int,c.c_ulong]
x11.XSync.argtypes=[ptr,c.c_int];x11.XCloseDisplay.argtypes=[ptr]
xtst.XTestFakeKeyEvent.argtypes=[ptr,c.c_uint,c.c_int,c.c_ulong]
display=x11.XOpenDisplay(None)
if not display:raise RuntimeError('Private test display unavailable')
def find(parent):
    name=ptr()
    if x11.XFetchName(display,parent,c.byref(name)) and name.value:
        text=c.string_at(name.value).decode(errors='replace');x11.XFree(name)
        if text==sys.argv[1]:return parent
    root=window();owner=window();children=c.POINTER(window)();count=c.c_uint()
    if x11.XQueryTree(display,parent,c.byref(root),c.byref(owner),c.byref(children),c.byref(count)):
        ids=list(children[:count.value]);x11.XFree(children)
        for child in ids:
            target=find(child)
            if target:return target
    return None
try:
    target=find(x11.XDefaultRootWindow(display))
    if not target:raise RuntimeError('Named regression window not found')
    x11.XSetInputFocus(display,target,2,0);x11.XSync(display,0)
    for chord in sys.argv[2:]:
        codes=[]
        for name in chord.split('+'):
            code=x11.XKeysymToKeycode(display,x11.XStringToKeysym(name.encode()))
            if not code:raise RuntimeError(f'Key {name} missing from private test layout')
            codes.append(code);xtst.XTestFakeKeyEvent(display,code,1,0)
        for code in reversed(codes):xtst.XTestFakeKeyEvent(display,code,0,0)
        x11.XSync(display,0);time.sleep(.012)
finally:x11.XCloseDisplay(display)
