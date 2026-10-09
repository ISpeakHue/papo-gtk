# Pelican icon concept

The application uses the approved head-and-neck artwork. Its 128px and 256px
PNG exports are in `hicolor/`, named `br.com.papo.gtk.png` to match the app ID.
They are embedded through `assets/papo.gresource.xml` and used for login branding,
window identity and desktop notifications. `scripts/install-desktop.py` installs
the same exports and launcher metadata for desktop shells. The PNG exports are
resized copies; the original generated artwork and earlier concepts remain intact.

`papo-pelican-head-neck.png` is the latest concept, adding a short curved
neck to the approved head design while keeping its palette and simple style.
It was generated with the built-in image generation tool; its final edit prompt
is preserved in `papo-pelican-head-neck.prompt.txt`.

`papo-pelican-head.png` is the newer, simpler head-only concept, generated
with the built-in image generation tool. It preserves the white/orange palette,
uses a small plain eye and removes the body, wing and feet. Its final prompt is
preserved in `papo-pelican-head.prompt.txt`.

`papo-pelican.png` is a transparent PNG concept generated with the built-in
image generation tool. The final prompt is preserved in `papo-pelican.prompt.txt`.

The design uses a white and grey pelican with a golden-orange bill, simplified
geometry and restrained depth, inspired by the
[GNOME app icon guidelines](https://developer.gnome.org/hig/guidelines/app-icons.html).

This is a raster concept, not a finished GNOME SVG icon set. A production icon
should be redrawn and checked on GNOME's 128px template at 64px and 32px; a
symbolic SVG companion should be prepared for contexts that require one.
