# EN-US: Verify a built executable's own icon resources using its packaged GTK runtime.
# PT-BR: Verifica os recursos de ícones do executável compilado usando seu próprio ambiente GTK.
"""Run on a private display with --binary pointing to the packaged executable."""
import argparse
import json
from pathlib import Path
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, Gtk

PREFIX = "/br/com/papo/gtk/icons"


def embedded_resource(binary, expected):
    # EN-US: Rust embeds a GVDB byte array in read-only data, without an ELF .gresource section.
    # PT-BR: O Rust incorpora bytes GVDB em dados somente leitura, sem uma seção ELF .gresource.
    data = binary.read_bytes()
    offset = 0
    while True:
        offset = data.find(b"GVariant", offset)
        if offset < 0:
            raise RuntimeError("Executable does not contain the expected Papo icon resources")
        try:
            resource = Gio.Resource.new_from_data(GLib.Bytes.new(data[offset:]))
            names = resource.enumerate_children(PREFIX + "/scalable/actions/", Gio.ResourceLookupFlags.NONE)
            if {name.removesuffix(".svg") for name in names} == expected:
                return resource
        except GLib.Error:
            pass
        offset += 8


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    catalog = binary.parents[1] / "share/licenses/papo-gtk/symbolic-icons/sources.json"
    expected = {item["name"] for item in json.loads(catalog.read_text())}
    if not expected:
        raise RuntimeError("Packaged icon attribution catalog is empty")
    resource = embedded_resource(binary, expected)
    Gio.resources_register(resource)
    Gtk.init()
    # EN-US: Use the display's actual theme, including a nonexistent desktop theme, with no fallback SVG files supplied by this probe.
    # PT-BR: Usa o tema real do display, incluindo um tema inexistente, sem fornecer arquivos SVG alternativos no teste.
    for selected in ("Adwaita", "breeze", "PapoUnavailableDesktopTheme"):
        Gtk.Settings.get_default().set_property("gtk-icon-theme-name", selected)
        theme = Gtk.IconTheme.get_for_display(Gdk.Display.get_default())
        theme.add_resource_path(PREFIX)
        for name in expected:
            for scale in (1, 2):
                if not theme.has_icon(name):
                    raise RuntimeError(f"Missing icon in {selected}: {name}")
                icon = theme.lookup_icon(name, None, 16, scale, Gtk.TextDirection.LTR, Gtk.IconLookupFlags(0))
                source = icon.get_file()
                if source is None or source.get_uri() != f"resource://{PREFIX}/scalable/actions/{name}.svg":
                    raise RuntimeError(f"Icon came from a desktop theme or missing-image fallback: {name}")
                pixels = Gdk.Texture.new_from_file(source)
                if pixels.get_width() != 16 or pixels.get_height() != 16:
                    raise RuntimeError(f"Invalid icon geometry: {name}")
    print(json.dumps({
        "status": "ok", "control_icons": len(expected), "desktop_themes": 3,
        "scales": [1, 2],
        "gtk": f"{Gtk.get_major_version()}.{Gtk.get_minor_version()}.{Gtk.get_micro_version()}",
    }))


if __name__ == "__main__":
    main()
