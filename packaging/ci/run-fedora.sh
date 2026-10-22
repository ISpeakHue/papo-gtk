#!/usr/bin/env bash
# EN-US: Rootless containers cannot acquire host-root capabilities; helpers keep their own sandboxes.
# PT-BR: Contêineres sem root não obtêm capacidades de root do host; helpers mantêm seus próprios sandboxes.
set -euo pipefail
papo_script=$(realpath "$1")
papo_project=${GITHUB_WORKSPACE:-$(pwd)}
mkdir -p "$papo_project/target/ci/home"
papo_result=0
podman run --rm --userns=keep-id --cap-drop ALL \
    --security-opt seccomp=unconfined --security-opt apparmor=unconfined \
    --security-opt 'unmask=/proc/*' \
    --user "$(id -u):$(id -g)" \
    --volume "$papo_project:/work:rw" \
    --volume "$(dirname "$papo_script"):/ci-steps:ro" \
    --volume "$papo_project/target/ci/home:/ci-home:rw" \
    --workdir /work --env HOME=/ci-home \
    --env CARGO_TERM_COLOR --env RUSTFLAGS --env GSK_RENDERER --env GTK_A11Y \
    --env G_MESSAGES_DEBUG --env GITHUB_STEP_SUMMARY=/work/target/ci/summary.md \
    localhost/papo-ci:fedora44 bash --noprofile --norc -e -o pipefail "/ci-steps/$(basename "$papo_script")" || papo_result=$?
if [[ -n ${GITHUB_STEP_SUMMARY:-} && -f target/ci/summary.md ]]; then
    cat target/ci/summary.md >> "$GITHUB_STEP_SUMMARY"
    rm target/ci/summary.md
fi
exit "$papo_result"
