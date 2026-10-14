"""Inspect native font/scaling settings and optionally snapshot a test window."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import gi

gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Adw, Gdk, GLib, Gtk, Pango

parser = argparse.ArgumentParser()
parser.add_argument('--snapshot', type=Path)
args = parser.parse_args()
Adw.init()
display = Gdk.Display.get_default()
if display is None:
    raise SystemExit('Font diagnostics need a Wayland or X11 display.')
settings = Gtk.Settings.get_for_display(display)
font = settings.get_property('gtk-font-name')
root = Path(os.environ['PAPO_APPDIR'])
family = Pango.FontDescription.from_string(font).get_family()
pattern = subprocess.check_output([str(root / 'usr/bin/fc-match'), '--format',
    '%{family}\t%{file}\t%{antialias}\t%{hinting}\t%{hintstyle}', family], text=True)
window = Adw.Window(default_width=640, default_height=280)
box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16,
    margin_top=24, margin_bottom=24, margin_start=24, margin_end=24)
for text in ('Papo — native font rendering', 'Olá, café, informação • @nickname',
             'Small text, crisp glyphs, emoji: 😄 🐧 🎉'):
    label = Gtk.Label(label=text, xalign=0)
    box.append(label)
box.append(Gtk.Entry(text='Typing at the desktop’s native scale'))
window.set_content(box)
loop = GLib.MainLoop()
failure = []

def capture():
    try:
        surface = window.get_surface()
        rendering = settings.get_property('gtk-font-rendering')
        result = {'display_type': type(display).__name__, 'font': font,
            'fontconfig_match': pattern.split('\t'), 'font_rendering': rendering.value_nick,
            'scale': surface.get_scale(), 'scale_factor': surface.get_scale_factor(),
            'renderer': type(window.get_renderer()).__name__,
            'gtk': f'{Gtk.get_major_version()}.{Gtk.get_minor_version()}.{Gtk.get_micro_version()}',
            'fontconfig_file': os.environ.get('FONTCONFIG_FILE'),
            'forced_backend': os.environ.get('GDK_BACKEND'),
            'forced_renderer': os.environ.get('GSK_RENDERER'),
            'forced_scale': os.environ.get('GDK_SCALE')}
        if args.snapshot:
            snapshot = Gtk.Snapshot()
            snapshot.scale(surface.get_scale(), surface.get_scale())
            paintable = Gtk.WidgetPaintable.new(window)
            paintable.snapshot(snapshot, window.get_width(), window.get_height())
            node = snapshot.to_node()
            texture = window.get_renderer().render_texture(node, None)
            args.snapshot.parent.mkdir(parents=True, exist_ok=True)
            if not texture.save_to_png(str(args.snapshot)):
                raise RuntimeError('Unable to save the font snapshot')
            result['snapshot'] = str(args.snapshot)
            result['snapshot_pixels'] = [texture.get_width(), texture.get_height()]
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except Exception as error:
        failure.append(error)
    finally:
        window.close()
        loop.quit()
    return GLib.SOURCE_REMOVE

window.present()
GLib.timeout_add(750, capture)
loop.run()
if failure:
    raise failure[0]
