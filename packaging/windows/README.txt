Papo Windows portable preview / Previa portatil para Windows

Extract the entire ZIP and open Papo.exe without a console. Use Papo.com for CLI diagnostics.
Extraia o ZIP inteiro e abra Papo.exe sem console. Use Papo.com para diagnosticos CLI.

The adjacent runtime directory includes GTK/libadwaita, Python/GI, GStreamer and
codecs. No external MSYS2, Rust, Python, GStreamer or PATH installation is required.
This is still an experimental preview; Windows desktop/device acceptance is pending.

Papo.exe configures only its child process. It does not change the machine PATH.
Runtime caches are private to this package location/version under LocalAppData.
For an offline check without credentials or real media devices:
    Papo.com --check-packaged-runtime

SOURCE.txt and BUILD-INFO.json record source identity, build options and inputs.
runtime/RUNTIME-MANIFEST.json records package versions, file hashes and DLL imports.
THIRD-PARTY-NOTICES.txt points to upstream notices and included license texts.
Byte-identical rebuilds have not been demonstrated.

Native desktop alerts, local screen sharing and isolated video poster/GIF
conversion remain unavailable on Windows. Those integrations are later port steps;
installing codecs does not enable an unsandboxed conversion fallback.

Developer overrides: PAPO_RUNTIME_DIR selects a complete alternate Papo runtime;
PAPO_PYTHON explicitly selects a helper interpreter. Normal users need neither.

Windows desktop identity / Identidade do desktop Windows:
    Papo.com --register-desktop
    Papo.com --desktop-info
    Papo.com --unregister-desktop
Registration is optional and explicit for ZIP users. It creates only this user's
Papo Start menu shortcut and protocol/toast identity. Re-register after moving the
package; unregister before removing it. It never changes machine-wide PATH.
O registro e opcional e explicito no ZIP. Cria apenas o atalho Papo e a identidade
de protocolo/notificacoes deste usuario. Registre novamente ao mover o pacote;
desregistre antes de remove-lo. Nao altera o PATH da maquina.

Papo.com --diagnostics launches with terminal diagnostics; Papo.cmd forwards there.
Papo.com --diagnostics inicia com diagnosticos de terminal; Papo.cmd encaminha para ele.
The toast commands are an opt-in identity prototype, not production message alerts.
Os comandos toast sao um prototipo explicito de identidade, nao alertas de mensagens.
