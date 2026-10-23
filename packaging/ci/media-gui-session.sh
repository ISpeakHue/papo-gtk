#!/usr/bin/env bash
# EN-US: Fullscreen/media geometry requires a private window manager, not only a framebuffer.
# PT-BR: Geometria de mídia/tela cheia exige um gerenciador de janelas privado, não só um framebuffer.
set -euo pipefail
exec dbus-run-session -- xvfb-run -a -s '-screen 0 1280x900x24' bash -c '
    openbox --sm-disable >/tmp/papo-media-openbox.log 2>&1 &
    papo_wm=$!
    trap '\''kill "$papo_wm" 2>/dev/null || true'\'' EXIT
    papo_ready=false
    for ((papo_attempt=0; papo_attempt<100; papo_attempt++)); do
        kill -0 "$papo_wm" 2>/dev/null || exit 1
        case "$(xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null)" in
            *"window id"*) papo_ready=true; break ;;
        esac
        sleep 0.1
    done
    "$papo_ready" || exit 1
    export PAPO_NATIVE_FULLSCREEN_TEST=1
    "$@"
' bash "$@"
