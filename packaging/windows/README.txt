Papo Windows developer preview / Previa de desenvolvimento para Windows

This is an experimental build, not a standalone installer.
Esta compilacao e experimental e nao e um instalador independente.

Requires Windows x86_64 with the MSYS2 UCRT64 packages listed in MULTIPLATFORM.md:
https://github.com/ISpeakHue/papo-gtk/blob/multiplatform/MULTIPLATFORM.md

Run Papo.cmd. It uses C:\msys64\ucrt64 by default. If MSYS2 is elsewhere,
set PAPO_RUNTIME_DIR to its ucrt64 directory before launching Papo.cmd.
GTK, libadwaita, Python GI, GStreamer and codecs must use the same UCRT64 prefix.

Native notification alerts, local screen sharing and isolated video preview/GIF
conversion are not implemented on Windows yet. In-app notifications remain available.
Existing Linux releases are unchanged. Do not redistribute this preview as stable.
