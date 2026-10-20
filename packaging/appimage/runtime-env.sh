#!/usr/bin/env bash
# EN-US: Share relocated runtime paths without overriding the desktop's font and scale choices.
# PT-BR: Compartilha caminhos ajustados do pacote sem substituir as escolhas de fonte e escala do desktop.
# Shared by AppRun and the bundle's validation commands.
export LD_LIBRARY_PATH="$PAPO_APPDIR/usr/lib64"
export PAPO_PYTHON="$PAPO_APPDIR/usr/bin/python3"
export PATH="$PAPO_APPDIR/usr/bin:$PATH"
export PYTHONHOME="$PAPO_APPDIR/usr"
export PYTHONPATH="$PAPO_APPDIR/usr/lib64/python3/site-packages"
export PYTHONNOUSERSITE=1
export GI_TYPELIB_PATH="$PAPO_APPDIR/usr/lib64/girepository-1.0"
export GIO_MODULE_DIR="$PAPO_APPDIR/usr/lib64/gio/modules"
export OPENSSL_MODULES="$PAPO_APPDIR/usr/lib64/ossl-modules"
export SPA_PLUGIN_DIR="$PAPO_APPDIR/usr/lib64/spa-0.2"
export PIPEWIRE_MODULE_DIR="$PAPO_APPDIR/usr/lib64/pipewire-0.3"
export PIPEWIRE_CONFIG_DIR="$PAPO_APPDIR/usr/share/pipewire"
export GTK_DATA_PREFIX="$PAPO_APPDIR/usr"
export GTK_EXE_PREFIX="$PAPO_APPDIR/usr"
export GTK_PATH="$PAPO_APPDIR/usr/lib64/gtk-4.0${GTK_PATH:+:$GTK_PATH}"
export GSETTINGS_SCHEMA_DIR="$PAPO_APPDIR/usr/share/glib-2.0/schemas"
export XDG_DATA_DIRS="$PAPO_APPDIR/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export GST_PLUGIN_SYSTEM_PATH_1_0="$PAPO_APPDIR/usr/lib64/gstreamer-1.0"
export GST_PLUGIN_PATH_1_0="$PAPO_APPDIR/usr/lib64/gstreamer-1.0"
export GST_PLUGIN_SCANNER_1_0="$PAPO_APPDIR/usr/libexec/gstreamer-1.0/gst-plugin-scanner"
mkdir -p "${XDG_CACHE_HOME:-$HOME/.cache}/papo-gtk"
export GST_REGISTRY_1_0="${XDG_CACHE_HOME:-$HOME/.cache}/papo-gtk/appimage-fedora44-hw1-gstreamer.bin"
export GDK_PIXBUF_MODULEDIR="$PAPO_APPDIR/usr/lib64/gdk-pixbuf-2.0/2.10.0/loaders"
# Loader cache paths must refer to this mount, never to the build machine.
PAPO_MOUNT_KEY=$(printf '%s' "$PAPO_APPDIR" | sha256sum | cut -c1-16)
PAPO_PIXBUF_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/papo-gtk/appimage-pixbuf-$PAPO_MOUNT_KEY.cache"
PAPO_PIXBUF_TEMP=$(mktemp "$PAPO_PIXBUF_CACHE.XXXXXX")
shopt -s nullglob
PAPO_PIXBUF_MODULES=("$GDK_PIXBUF_MODULEDIR"/*.so)
"$PAPO_APPDIR/usr/bin/gdk-pixbuf-query-loaders" "${PAPO_PIXBUF_MODULES[@]}" > "$PAPO_PIXBUF_TEMP"
mv "$PAPO_PIXBUF_TEMP" "$PAPO_PIXBUF_CACHE"
export GDK_PIXBUF_MODULE_FILE="$PAPO_PIXBUF_CACHE"
export SSL_CERT_FILE="$PAPO_APPDIR/usr/share/papo-gtk/ca-certificates.crt"
# EN-US: Preserve explicit Fontconfig overrides and load the host's complete font rules before bundled fallbacks.
# PT-BR: Preserva ajustes explícitos do Fontconfig e carrega todas as regras de fonte do host antes das fontes alternativas do pacote.
if [[ -z "${FONTCONFIG_FILE:-}" && -z "${FONTCONFIG_PATH:-}" ]]; then
    export FONTCONFIG_FILE="$PAPO_APPDIR/usr/etc/fonts/fonts.conf"
fi
export GLYCIN_DATA_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/papo-gtk/appimage-glycin-$PAPO_MOUNT_KEY"
"$PAPO_PYTHON" "$PAPO_APPDIR/usr/share/papo-gtk/loader-config.py"
# GTK selects native Wayland/X11, the renderer and monitor scale itself. Do not
# force an XWayland backend, software renderer, hinting mode or integer scale.
