#!/usr/bin/env bash
# EN-US: Run every case, retain each failure, and fail the job if any case fails.
# PT-BR: Executa todos os casos, preserva cada falha e falha a tarefa se algum caso falhar.
set -uo pipefail
mkdir -p target/ci/gui
papo_failures=0
: > target/ci/gui/results.tsv
papo_cases=(symbolic-icons security-icons syntax-highlighting reaction-participants default
    multiline full-app-input login-input backend-update embeds style-palette attachment-media
    media-cache report8 report10 report11 emoji-grid notification-filters account-notices
    account-settings webhook-chat report9 edit-mentions emoji-details backend-gifs
    backend-contracts voice-chat avatar-viewport search-recycling emoji-notifications
    notification-window recovery media-loading recent-media draft-previews clipboard
    emoji-categories pins-media)
# EN-US: Optional arguments select focused local reproductions without changing CI's full list.
# PT-BR: Argumentos opcionais selecionam reproduções locais sem mudar a lista completa da CI.
if (( $# )); then papo_cases=("$@"); fi
for papo_case in "${papo_cases[@]}"; do
    printf '\nRunning GTK case: %s\n' "$papo_case"
    unset PAPO_NATIVE_INPUT_TEST
    case "$papo_case" in
        multiline|full-app-input|login-input|report9|edit-mentions|emoji-details) export PAPO_NATIVE_INPUT_TEST=1 ;;
    esac
    PAPO_UI_CASE="$papo_case" GTK_IM_MODULE=simple \
        timeout --kill-after=10s 240s dbus-run-session -- xvfb-run -a \
        cargo test --locked ui_smoke -- --ignored --test-threads=1 \
        2>&1 | tee "target/ci/gui/$papo_case.log"
    papo_statuses=("${PIPESTATUS[@]}")
    papo_status=${papo_statuses[0]}
    if (( papo_statuses[1] != 0 )); then papo_status=${papo_statuses[1]}; fi
    printf '%s\t%s\n' "$papo_case" "$papo_status" >> target/ci/gui/results.tsv
    if (( papo_status != 0 )); then papo_failures=$((papo_failures + 1)); fi
    if [[ -n ${GITHUB_STEP_SUMMARY:-} ]]; then
        printf '%s: exit %s (see GUI log artifact)\n\n' "$papo_case" "$papo_status" >> "$GITHUB_STEP_SUMMARY"
    fi
done
printf '\nGTK cases: %s; failures: %s\n' "${#papo_cases[@]}" "$papo_failures"
(( papo_failures == 0 ))
