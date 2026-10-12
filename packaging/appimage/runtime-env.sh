#!/usr/bin/env bash
# Shared by AppRun and the bundle's validation commands.
export LD_LIBRARY_PATH="$PAPO_APPDIR/usr/lib"
export PAPO_PYTHON="$PAPO_APPDIR/usr/bin/python3"
export PYTHONHOME="$PAPO_APPDIR/usr"
export PYTHONPATH="$PAPO_APPDIR/usr/lib/python3/dist-packages"
export PYTHONNOUSERSITE=1
export GI_TYPELIB_PATH="$PAPO_APPDIR/usr/lib/girepository-1.0"
export GIO_MODULE_DIR="$PAPO_APPDIR/usr/lib/gio/modules"
export OPENSSL_MODULES="$PAPO_APPDIR/usr/lib/ossl-modules"
export SPA_PLUGIN_DIR="$PAPO_APPDIR/usr/lib/spa-0.2"
export PIPEWIRE_MODULE_DIR="$PAPO_APPDIR/usr/lib/pipewire-0.3"
export PIPEWIRE_CONFIG_DIR="$PAPO_APPDIR/usr/share/pipewire"
export GTK_DATA_PREFIX="$PAPO_APPDIR/usr"
export GTK_EXE_PREFIX="$PAPO_APPDIR/usr"
export GTK_PATH="$PAPO_APPDIR/usr/lib/gtk-4.0"
export GSETTINGS_SCHEMA_DIR="$PAPO_APPDIR/usr/share/glib-2.0/schemas"
export XDG_DATA_DIRS="$PAPO_APPDIR/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export GST_PLUGIN_SYSTEM_PATH_1_0="$PAPO_APPDIR/usr/lib/gstreamer-1.0"
export GST_PLUGIN_PATH_1_0="$PAPO_APPDIR/usr/lib/gstreamer-1.0"
export GST_PLUGIN_SCANNER_1_0="$PAPO_APPDIR/usr/libexec/gstreamer-1.0/gst-plugin-scanner"
mkdir -p "${XDG_CACHE_HOME:-$HOME/.cache}/papo-gtk"
export GST_REGISTRY_1_0="${XDG_CACHE_HOME:-$HOME/.cache}/papo-gtk/appimage-gstreamer-1.24.bin"
export GDK_PIXBUF_MODULEDIR="$PAPO_APPDIR/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders"
export GDK_PIXBUF_MODULE_FILE="$PAPO_APPDIR/usr/lib/gdk-pixbuf-2.0/loaders.cache"
# Loader cache paths must refer to this mount, never to the build machine.
PAPO_MOUNT_KEY=$(printf '%s' "$PAPO_APPDIR" | sha256sum | cut -c1-16)
PAPO_PIXBUF_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/papo-gtk/appimage-pixbuf-$PAPO_MOUNT_KEY.cache"
PAPO_PIXBUF_TEMP=$(mktemp "$PAPO_PIXBUF_CACHE.XXXXXX")
"$PAPO_APPDIR/usr/bin/gdk-pixbuf-query-loaders" "$GDK_PIXBUF_MODULEDIR"/*.so > "$PAPO_PIXBUF_TEMP"
mv "$PAPO_PIXBUF_TEMP" "$PAPO_PIXBUF_CACHE"
export GDK_PIXBUF_MODULE_FILE="$PAPO_PIXBUF_CACHE"
export SSL_CERT_FILE="$PAPO_APPDIR/usr/share/papo-gtk/ca-certificates.crt"
export FONTCONFIG_PATH="$PAPO_APPDIR/usr/etc/fonts"
