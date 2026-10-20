# Papo control icons

These SVGs are unmodified Adwaita artwork by the **GNOME Project**
([gnome.org](https://www.gnome.org/)), imported on 2026-10-20 from the installed
Adwaita icon theme, including its legacy icons. Upstream projects:

- https://gitlab.gnome.org/GNOME/adwaita-icon-theme
- https://gitlab.gnome.org/GNOME/adwaita-icon-theme-legacy

The original dual-license notice and license files are retained as `COPYING`,
`COPYING_CCBYSA3` and `COPYING_LGPL`. The LGPL v3 incorporates GPL v3, whose full
text is also provided in the application's root `LICENSE`.

`sources.json` records each embedded Papo icon name and its original Adwaita
path. The voice-channel chat control reuses `chat-message-new-symbolic.svg`;
`chat-bubbles-text-symbolic` was unavailable in Adwaita. This alias changes no
SVG artwork.

The application resource manifest gives these icons `papo-` names to prevent
desktop icon themes from substituting or omitting the application controls.
GTK still recolors the symbolic artwork for the active light/dark theme,
disabled controls and accent/destructive buttons.
