Papo Windows portable preview / Previa portatil para Windows

Extract the entire ZIP and open Papo.exe (Papo.cmd also forwards CLI arguments).
Extraia o ZIP inteiro e abra Papo.exe (Papo.cmd tambem encaminha argumentos CLI).

The adjacent runtime directory includes GTK/libadwaita, Python/GI, GStreamer and
codecs. No external MSYS2, Rust, Python, GStreamer or PATH installation is required.
This is still an experimental preview; Windows desktop/device acceptance is pending.

Papo.exe configures only its child process. It does not change the machine PATH.
Runtime caches are private to this package location/version under LocalAppData.
For an offline check without credentials or real media devices:
    Papo.exe --check-packaged-runtime

SOURCE.txt and BUILD-INFO.json record source identity, build options and inputs.
runtime/RUNTIME-MANIFEST.json records package versions, file hashes and DLL imports.
THIRD-PARTY-NOTICES.txt points to upstream notices and included license texts.
Byte-identical rebuilds have not been demonstrated.

Native desktop alerts, local screen sharing and isolated video poster/GIF
conversion remain unavailable on Windows. Those integrations are later port steps;
installing codecs does not enable an unsandboxed conversion fallback.

Developer overrides: PAPO_RUNTIME_DIR selects a complete alternate Papo runtime;
PAPO_PYTHON explicitly selects a helper interpreter. Normal users need neither.
