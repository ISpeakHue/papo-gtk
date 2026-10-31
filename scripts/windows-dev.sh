#!/usr/bin/env bash
# EN-US: Local development uses the same UCRT64 dependencies and GTK fix as Windows CI.
# PT-BR: Desenvolvimento local usa as mesmas dependências UCRT64 e correção GTK da CI Windows.
set -euo pipefail
[[ ${MSYSTEM:-} == UCRT64 ]] || { echo 'Run in MSYS2 UCRT64, or use scripts/windows-dev.ps1 from PowerShell.' >&2; exit 1; }
PAPO_PROJECT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$PAPO_PROJECT"
action=${1:-check}
case "$action" in check|setup|build|test|run|debug) ;; *) echo 'Expected check, setup, build, test, run or debug.' >&2; exit 1 ;; esac

if [[ $action == setup ]]; then
    # EN-US: Update the complete installation; partial MSYS2 upgrades are unsupported.
    # PT-BR: Atualiza a instalação completa; atualizações parciais MSYS2 não têm suporte.
    pacman -Syu --noconfirm --noprogressbar
    # EN-US: libadwaita supplies GTK; explicitly installing repository GTK would downgrade the backport.
    # PT-BR: libadwaita fornece GTK; instalar explicitamente GTK do repositório rebaixaria o backport.
    pacman -S --needed --noconfirm --noprogressbar base-devel git \
        mingw-w64-ucrt-x86_64-{rust,gcc,gdb,nsis,pkgconf,libadwaita,librsvg,python-gobject,gst-python,gst-plugins-base,gst-plugins-good,gst-plugins-bad,gst-libav,libnice,meson,gi-docgen,gobject-introspection,python-docutils,sassc,shaderc,vulkan-headers}
    expected=$(sed -n 's/^pkgver=//p' packaging/windows/gtk/PKGBUILD)-$(sed -n 's/^pkgrel=//p' packaging/windows/gtk/PKGBUILD)
    installed=$(pacman -Q mingw-w64-ucrt-x86_64-gtk4 | cut -d ' ' -f 2)
    if [[ $installed != "$expected" ]]; then
        [[ $installed == "${expected%-*}-1" ]] || {
            echo "GTK $installed differs from the reviewed recipe $expected. Review the upstream IME fix and recipe before changing versions." >&2
            exit 1
        }
        mkdir -p target/windows-dev/gtk
        cp packaging/windows/gtk/* target/windows-dev/gtk/
        (cd target/windows-dev/gtk && MINGW_ARCH=ucrt64 makepkg-mingw --cleanbuild --force --syncdeps --noconfirm --nocheck)
        pacman -U --noconfirm "target/windows-dev/gtk/mingw-w64-ucrt-x86_64-gtk4-$expected-any.pkg.tar.zst"
    fi
    # EN-US: Keep scanner startup outside the application even under non-ASCII installation paths.
    # PT-BR: Preserva o scanner externo mesmo com caminhos de instalação não ASCII.
    gst_recipe=packaging/windows/gstreamer/GSTREAMER-PKGBUILD
    expected=$(sed -n 's/^pkgver=//p' "$gst_recipe")-$(sed -n 's/^pkgrel=//p' "$gst_recipe")
    installed=$(pacman -Q mingw-w64-ucrt-x86_64-gstreamer | cut -d ' ' -f 2)
    if [[ $installed != "$expected" ]]; then
        [[ $installed == "${expected%-*}-1" ]] || {
            echo "GStreamer $installed differs from the reviewed Unicode-path recipe $expected." >&2
            exit 1
        }
        mkdir -p target/windows-dev/gstreamer
        cp "$gst_recipe" target/windows-dev/gstreamer/PKGBUILD
        cp packaging/windows/gstreamer/*.patch target/windows-dev/gstreamer/
        (cd target/windows-dev/gstreamer && MINGW_ARCH=ucrt64 makepkg-mingw --force --syncdeps --noconfirm --nocheck)
        pacman -U --noconfirm "target/windows-dev/gstreamer/mingw-w64-ucrt-x86_64-gstreamer-$expected-any.pkg.tar.zst"
    fi
fi

# EN-US: Override only this child shell; neither user nor machine PATH/settings are modified.
# PT-BR: Altera só este shell filho; PATH/configurações do usuário e sistema não são modificados.
unset PYTHONHOME PYTHONPATH PKG_CONFIG_LIBDIR
export PKG_CONFIG_PATH="$MINGW_PREFIX/lib/pkgconfig:$MINGW_PREFIX/share/pkgconfig"
export PAPO_RUNTIME_DIR="$(cygpath -w "$MINGW_PREFIX")"
export PAPO_PYTHON="$(cygpath -w "$MINGW_PREFIX/bin/python.exe")"
export GI_TYPELIB_PATH="$(cygpath -w "$MINGW_PREFIX/lib/girepository-1.0")"
export GST_PLUGIN_SYSTEM_PATH_1_0="$(cygpath -w "$MINGW_PREFIX/lib/gstreamer-1.0")"
export GST_PLUGIN_SCANNER_1_0="$(cygpath -w "$MINGW_PREFIX/libexec/gstreamer-1.0/gst-plugin-scanner.exe")"
mkdir -p target/windows-dev
registry=gstreamer-registry.bin
if [[ ${PAPO_DEV_EXCLUDE_HIP:-} == 1 ]]; then
    plugin_allowlist=$(python packaging/windows/devcheck.py --hip-allowlist "$(cygpath -w "$MINGW_PREFIX/lib/gstreamer-1.0")")
    export GST_PLUGIN_LOADING_WHITELIST="$plugin_allowlist"
    registry=gstreamer-registry-without-hip.bin
    echo 'Developer override: HIP plugin excluded; other plugins remain available.'
else
    unset GST_PLUGIN_LOADING_WHITELIST
fi
export GST_REGISTRY_1_0="$(cygpath -w "$PAPO_PROJECT/target/windows-dev/$registry")"
unset GST_PLUGIN_PATH GST_PLUGIN_PATH_1_0
export PYTHONNOUSERSITE=1 PYTHONDONTWRITEBYTECODE=1
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
command -v python >/dev/null || { echo 'UCRT64 Python is missing. Run the setup action first.' >&2; exit 1; }
python packaging/windows/devcheck.py --output target/windows-dev/preflight.json
case "$action" in
    check|setup) ;;
    build) cargo build --locked ;;
    test)
        python packaging/ci/python_checks.py \
            --pattern 'windows_*test.py' --pattern platform_capture_test.py \
            --pattern voice_worker_test.py --pattern probe_runtime_test.py \
            --pattern call_sound_test.py \
            --allow-skip call_sound_test.CallSoundTests.test_auto_output_children_get_papos_identity_before_playback \
            --pattern desktop_input_test.py --output target/windows-dev/python-tests.json
        cargo test --locked -- --test-threads=1 2>&1 | tee target/windows-dev/rust-tests.log
        cargo test --locked native_voice -- --ignored --test-threads=1 2>&1 | tee target/windows-dev/native-voice.log
        cargo test --locked native_call_ -- --ignored --test-threads=1 2>&1 | tee target/windows-dev/native-call.log
        ;;
    run) cargo run --locked ;;
    debug) cargo build --locked; gdb --args target/debug/papo-gtk.exe ;;
esac
