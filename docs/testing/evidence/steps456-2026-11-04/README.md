# Implementation-plan steps 4–6 validation

This records local validation of the implementation working tree on Fedora 44
x86_64. Tests use disposable configuration directories, private loopback backends
and private X11 displays. They never read a saved session or contact a live Papo
server. Windows CI has the new fixtures, but no native Windows run or remote CI
run is claimed here.

Implementation details: [translations](../../../translations.md),
[drafts and outgoing queue](../../../drafts-and-outbox.md).

## Checks

- `cargo check --locked --all-targets`: warning-free.
- `cargo test --locked --release -- --test-threads=1`: 328 passed; six explicit
  display/native checks excluded from the ordinary suite.
- Translation checker: 855 matching PT-BR/EN-US entries, valid plural forms,
  substitution names and literal source references.
- Python implementation-tool tests: nine passed, including invalid translation
  and missing-placeholder/plural rejection.
- Actionlint: Linux and Windows workflow files passed.
- Backend contract source comparison: unchanged reviewed commit
  `eab626662bc4bfeb4ce1cd3165b01eccb4c2c5a5`. Ordinary message idempotency is
  still missing; the frontend enables no automatic send retries.

## GTK workflows

The new `translations`, `draft-recovery` and `outgoing-queue` fixtures cover:

- English login/settings labels, accessible navigation tooltip, language saving,
  singular/plural/date formatting, and unchanged user content.
- Restarted drafts with accented text and mention UUIDs, available/missing
  attachments, deleted replies and conversation draft markers.
- Delayed recovery during programmatic channel switching versus genuine typing;
  live reply deletion without losing the draft text.
- Independent typing during delayed sequential sends, channel switching, retained
  failures, no automatic replay, and cancel/confirm of uncertain manual retries.

Focused compatibility cases include `multiline`, `account-settings`,
`account-scope`, `recovery`, `voice-chat`, `report10`, `media-cache`,
`channel-cache`, `pins-media`, `draft-previews`, `clipboard`, `report11` and
`send-video`. The final `timeline-controls` rerun also passed after the permission
fixture correction. The aggregate `default` case exercises the wider chat,
settings, permissions and media workflows; it is subject to the native failure
below. Neither aggregate attempt is counted as a passing test.

The final window-manager runs use the existing cached Fedora CI container with
private Xvfb/Openbox and Cairo rendering. Private-display host checks were also
used during development. A host Xvnc startup timed out once; no fixture ran in
that invocation. Earlier test failures exposed legacy assumptions that rejected
sends remain in the composer, and a permission fixture that changed account IDs
while expecting history to survive. Those tests now exercise failed-row editing
and same-account permission changes, respectively. The production ownership
boundary remains enforced.

Local logs are retained under `target/steps456-*` and excluded from Git. These
checks do not substitute for packaged/native Windows desktop acceptance, and do
not claim a live-server delivery or power-loss durability guarantee.

## Native aggregate-run limitation

Two unchanged aggregate attempts terminated with SIGSEGV during
`chat::actions::tests::exercise`, in the native media stack. Core metadata for
that synthetic process identifies GStreamer 1.28.8 and these frames:

```text
g_type_check_instance_is_fundamentally_a  (libgobject-2.0.so.0)
g_object_unref                          (libgobject-2.0.so.0)
gst_play_main                           (libgstplay-1.0.so.0)
```

The core identifies the failing native thread; it does not establish a complete
root-cause diagnosis or prove that all native playback paths are safe. The
focused feature checks passed. The failed log and core metadata remain in
`target/steps456-wm-default-native-crash.log` and
`target/steps456-native-crash.log`. No production media workaround or suppression
was added for this run.
