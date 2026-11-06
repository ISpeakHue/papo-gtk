#!/usr/bin/env python3
"""Click only a named window belonging to the private parent GTK regression process."""
import os
import ctypes as c,sys
if sys.platform != 'linux' or len(sys.argv)!=5 or int(sys.argv[1])!=os.getppid() or sys.argv[2]!='Papo local audio test':
 raise SystemExit('Private parent audio regression window required')
x=c.CDLL('libX11.so.6');t=c.CDLL('libXtst.so.6');P=c.c_void_p;W=c.c_ulong
x.XOpenDisplay.argtypes=[c.c_char_p];x.XOpenDisplay.restype=P
x.XDefaultRootWindow.argtypes=[P];x.XDefaultRootWindow.restype=W
x.XQueryTree.argtypes=[P,W,c.POINTER(W),c.POINTER(W),c.POINTER(c.POINTER(W)),c.POINTER(c.c_uint)]
x.XFetchName.argtypes=[P,W,c.POINTER(P)];x.XFree.argtypes=[P]
x.XTranslateCoordinates.argtypes=[P,W,W,c.c_int,c.c_int,c.POINTER(c.c_int),c.POINTER(c.c_int),c.POINTER(W)]
x.XSync.argtypes=[P,c.c_int];x.XCloseDisplay.argtypes=[P]
t.XTestFakeMotionEvent.argtypes=[P,c.c_int,c.c_int,c.c_int,c.c_ulong]
t.XTestFakeButtonEvent.argtypes=[P,c.c_uint,c.c_int,c.c_ulong]
d=x.XOpenDisplay(None)
if not d:raise RuntimeError('Private display missing')
def find(parent):
 name=P()
 if x.XFetchName(d,parent,c.byref(name)) and name.value:
  s=c.string_at(name.value).decode(errors='replace');x.XFree(name)
  if s==sys.argv[2]:return parent
 root=W();owner=W();children=c.POINTER(W)();count=c.c_uint()
 if x.XQueryTree(d,parent,c.byref(root),c.byref(owner),c.byref(children),c.byref(count)):
  ids=list(children[:count.value]);x.XFree(children)
  for child in ids:
   result=find(child)
   if result:return result
try:
 root=x.XDefaultRootWindow(d);target=find(root)
 if not target:raise RuntimeError('Private test window not found')
 px=c.c_int();py=c.c_int();child=W()
 if not x.XTranslateCoordinates(d,target,root,int(sys.argv[3]),int(sys.argv[4]),c.byref(px),c.byref(py),c.byref(child)):raise RuntimeError('Coordinate translation failed')
 t.XTestFakeMotionEvent(d,-1,px.value,py.value,0);t.XTestFakeButtonEvent(d,1,1,0);t.XTestFakeButtonEvent(d,1,0,0);x.XSync(d,0)
finally:x.XCloseDisplay(d)
