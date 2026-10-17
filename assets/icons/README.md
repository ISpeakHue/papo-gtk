# Pelican application icon

The application uses the approved head-and-neck artwork. Its 128px and 256px
PNG exports are in `hicolor/`, named `br.com.papo.gtk.png` to match the app ID.
They are embedded through [the resource manifest](../papo.gresource.xml) and used
for login branding, window identity and desktop notifications. The
[desktop installer](../../scripts/install-desktop.py) installs the same exports
and launcher metadata for desktop shells. The PNG exports are resized copies of
the approved original artwork retained here.

`papo-pelican-head-neck.png` is the latest concept, adding a short curved
neck to the approved head design while keeping its palette and simple style.
It was generated with the built-in image generation tool; its final edit prompt
is preserved in `papo-pelican-head-neck.prompt.txt`.

The design uses a white and grey pelican with a golden-orange bill, simplified
geometry and restrained depth, inspired by the
[GNOME app icon guidelines](https://developer.gnome.org/hig/guidelines/app-icons.html).

The current icon is raster PNG. A vector redraw and symbolic SVG companion can
be considered separately; the approved source, generation prompt and installed
PNG exports are retained. Earlier unused concepts are available in Git history.
