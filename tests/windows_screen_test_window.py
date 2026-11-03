"""Disposable source for attended Windows capture tests; displays generated colors only."""
import argparse
import gi,time,json
from pathlib import Path
gi.require_version('Gtk','4.0')
from gi.repository import Gtk,GLib
Gtk.init()
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--control',type=Path,required=True,help='Private control file shared with the attended probe')
args=parser.parse_args()
control=args.control
control.parent.mkdir(parents=True,exist_ok=True)
control.unlink(missing_ok=True)
window=Gtk.Window(title='Papo Screen Sharing Test',default_width=800,default_height=500)
box=Gtk.Box(orientation=Gtk.Orientation.VERTICAL,spacing=8)
box.append(Gtk.Label(label='PAPO TEST 7426 — disposable local content'))
box.append(Gtk.Label(label='Select this window in the Windows sharing picker.'))
area=Gtk.DrawingArea(hexpand=True,vexpand=True)
started=time.monotonic()
def draw(_area,cr,w,h):
    for x,y,c in [(0,0,(.85,.1,.1)),(1,0,(.1,.8,.2)),(0,1,(.1,.3,.9)),(1,1,(.95,.8,.1))]:
        cr.set_source_rgb(*c);cr.rectangle(x*w/2,y*h/2,w/2,h/2);cr.fill()
    cr.set_source_rgb(1,1,1);cr.rectangle((time.monotonic()-started)*90%max(w-24,1),h/2-12,24,24);cr.fill()
area.set_draw_func(draw);box.append(area);window.set_child(box)
loop=GLib.MainLoop();window.connect('close-request',lambda *_:(loop.quit(),False)[1])
def tick():
    area.queue_draw()
    if control.exists():
        try:
            command=json.loads(control.read_text(encoding='utf-8'))
        except (FileNotFoundError,json.JSONDecodeError):
            return True
        control.unlink(missing_ok=True)
        if command.get('close'):window.close();return False
        if command.get('width'):window.set_default_size(command['width'],command['height']);window.unmaximize()
    return True
GLib.timeout_add(100,tick);window.present();print('Disposable test window ready',flush=True)
loop.run()
