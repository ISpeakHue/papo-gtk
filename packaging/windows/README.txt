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

Native desktop alerts and local screen sharing remain unavailable on Windows.
Video poster/GIF conversion uses AppContainer and bounded Job Objects. Failed
isolation never falls back to an unrestricted converter. Complete media/UI and
real-device qualification remain later port steps.

Explicit synthetic decoder isolation acceptance in a disposable account:
    Papo.com --check-media-runtime
This creates/removes its own test credential, temporary files and container
profiles, checks native access denial/limits/cleanup and converts three fixtures.
It never reads existing Papo settings or saved sessions.
Aceitacao sintetica explicita do isolamento em uma conta descartavel:
    Papo.com --check-media-runtime
Cria/remove apenas credenciais, arquivos e perfis proprios de teste. Nao le
configuracoes existentes nem sessoes salvas do Papo.

Developer overrides: PAPO_RUNTIME_DIR selects a complete alternate Papo runtime;
PAPO_PYTHON selects an ordinary worker interpreter. Isolated decoders always use
the selected runtime's bin/python.exe. Normal users need neither override.

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
