Papo Windows portable preview / Previa portatil para Windows

Extract the entire ZIP and open Papo.exe without a console. Use Papo.com for CLI diagnostics.
Extraia o ZIP inteiro e abra Papo.exe sem console. Use Papo.com para diagnosticos CLI.

A separate per-user Windows installer preview is also built from these exact files.
It installs under LocalAppData/Programs/Papo, creates the owned Start menu and
notification identity, and adds an Installed Apps entry. It needs no terminal or
external runtime. Upgrades retain a previous copy for recovery; uninstall keeps
settings, caches and saved sessions. Native installer/beta acceptance is pending.
Um instalador separado por usuario tambem e gerado com estes mesmos arquivos.
Instala em LocalAppData/Programs/Papo, cria o atalho e a identidade de avisos e
aparece em Aplicativos instalados. Nao requer terminal nem runtime externo.
Atualizacoes preservam a copia anterior para recuperacao; a desinstalacao mantem
configuracoes, caches e sessoes salvas. A aceitacao nativa/beta esta pendente.

The adjacent runtime directory includes GTK/libadwaita, Python/GI, GStreamer and
codecs. No external MSYS2, Rust, Python, GStreamer or PATH installation is required.
This is still an experimental preview; Windows desktop/device acceptance is pending.

Papo.exe configures only its child process. It does not change the machine PATH.
Runtime caches are private to this package location/version under LocalAppData.
For an offline check without credentials or real media devices:
    Papo.com --check-packaged-runtime
    Papo.com --check-notifications

SOURCE.txt and BUILD-INFO.json record source identity, build options and inputs.
runtime/RUNTIME-MANIFEST.json records package versions, file hashes and DLL imports.
THIRD-PARTY-NOTICES.txt points to upstream notices and included license texts.
Byte-identical rebuilds have not been demonstrated.

Native desktop alerts use the optional per-user desktop registration described below.
Local screen sharing has an opt-in Windows Graphics Capture preview; native
desktop acceptance is pending. In PowerShell, before opening Papo:
    $env:PAPO_WINDOWS_CAPTURE_PREVIEW = '1'
Sharing requires an explicit selection in the Windows picker on every start.
Avisos nativos usam o registro opcional do desktop por usuario descrito abaixo.
Compartilhamento local de tela tem previa opcional; a aceitacao nativa esta
pendente. Use a variavel acima no PowerShell antes de abrir o Papo. Cada inicio
exige selecionar explicitamente uma janela ou tela no seletor do Windows.
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
The test-toast commands use synthetic identity alerts. Registered packages also
send message alerts, respecting account notification/preview/sound settings.
Notification clicks use opaque, expiring, account/session-bound tickets. Normal
close keeps alerts for remembered-session cold activation; logout/revocation removes
the affected alerts. Windows desktop acceptance remains pending.
Os comandos test-toast usam avisos sinteticos. Pacotes registrados tambem enviam
avisos de mensagens, respeitando preferencias de notificacao, previa e som.
Cliques usam tickets opacos vinculados a conta/sessao. Encerrar preserva avisos para
retomar sessoes lembradas; logout/revogacao remove os avisos afetados.
--check-notifications tests WinRT construction/DPAPI/cold routing offline, without
visible notifications, registry registration, credentials or production requests.
--check-notifications testa WinRT/DPAPI/ativacao inicial sem avisos visiveis,
registro do desktop, credenciais ou requisicoes de producao.
Os comandos toast sao um prototipo explicito de identidade, nao alertas de mensagens.
