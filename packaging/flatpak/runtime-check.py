#!/usr/bin/python3
# EN-US: Check runtime elements and actual isolated media conversion without an account or capture devices.
# PT-BR: Verifica os componentes de execução e a conversão isolada de mídia sem conta nem dispositivos de captura.
"""Offline package checks; no account, network requests or physical capture."""
import json
import ssl
import subprocess
import gi
for namespace, version in [('Gst', '1.0'), ('GstWebRTC', '1.0'), ('GstSdp', '1.0'), ('GstRtp', '1.0'), ('Gtk', '4.0'), ('Adw', '1')]:
    gi.require_version(namespace, version)
from gi.repository import Gst, GstWebRTC, GstSdp, GstRtp, Gtk, Adw
Gst.init(None)
required = ('webrtcbin', 'nicesrc', 'nicesink', 'dtlsenc', 'srtpenc', 'opusenc', 'opusdec', 'rtpopuspay', 'rtpopusdepay', 'level', 'volume', 'vp8enc', 'vp8dec', 'rtpvp8pay', 'rtpvp8depay', 'jpegenc', 'appsink', 'videoconvert', 'videoscale', 'videorate', 'pipewiresrc', 'v4l2src', 'autoaudiosrc', 'autoaudiosink', 'wavparse', 'matroskademux', 'qtdemux', 'avdec_h264', 'h264parse', 'decodebin', 'playbin', 'fakesink', 'queue', 'capsfilter', 'valve', 'audioconvert', 'audioresample', 'gtk4paintablesink')
missing = [name for name in required if not Gst.ElementFactory.find(name)]
assert not missing, f'Missing GStreamer elements: {missing}'
assert ssl.create_default_context().get_ca_certs(), 'Missing HTTPS certificates'
assert Gtk.get_minor_version() >= 22 and Adw.get_minor_version() >= 9
subprocess.run(['/app/bin/papo-gtk', '--check-media-runtime'], check=True, timeout=90)
print(json.dumps({'status':'ok', 'gtk':f'4.{Gtk.get_minor_version()}', 'libadwaita':f'1.{Adw.get_minor_version()}', 'gstreamer':Gst.version_string(), 'checked_elements':len(required), 'isolated_mp4_mov_webm_conversion':True}))
