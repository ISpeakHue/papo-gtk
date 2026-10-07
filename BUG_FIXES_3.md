# Third bug and design pass

Source: `/home/gabrieloliveiradecastro/Documentos/bug_fixes_3.txt`.

| Item | Implemented behavior | Regression coverage |
| --- | --- | --- |
| 1 | Red delete action and compact confirmation with cancel as the initial focus. | Delete appearance, mapped dialog dimensions, cancel/confirm and permissions. |
| 2 | Images and embeds display up to 720 × 480, even when their source thumbnail is smaller, and shrink in narrow layouts. | Mapped image dimensions and the existing viewer cleanup checks. |
| 3 | Add reaction appears in the message context menu. | Actual context button opens the picker; permission gating remains checked. |
| 4 | Reaction tooltips load participant names lazily and cache them; duplicate queries share one request. | Native hover signals, cached participant text, tooltip availability and HTTP request deduplication. |
| 5 | Typing @ opens filtered suggestions; the @ button lists members. Selection inserts backend mention tokens and returns typing focus. | Mapped typed/manual completion, empty results, selection and dismissal. |
| 6 | Native inline players support preview relays and video attachments. Close, deletion and session/permission reset stop playback and remove files. | Bundled VP8/Vorbis clip decodes video and audio; close/delete and delayed download rejection. |
| 7 | Composer focus remains during sends. Thumbnail decoding leaves GTK's thread; history pages merge and sort once; reply lookups use an index. | Successful/failed send focus, responsive GTK timers during queued image work, late decoding rejection and a 10,000-message merge fixture. |
| 8 | Highlight colors fade back over 900 ms. Changing emphasis keeps the same message row and children. | Widget identity before/during/after highlight and stale timeout checks; native CSS transitions. |
| 9 | Server rail and channel pane use subtly different semantic theme colors. | Inspected dark/light and narrow previews; native theme CSS. |
| 10 | A short original connection cue respects sound settings; active speakers show a microphone badge by their avatar, with a fallback when avatars are hidden. | PCM fixture validation, muted native decoding, disabled/repeated connection checks and stable speaking rows. |

Media sources remain authenticated. Existing encoded/source dimension limits,
decoded cache bounds, moderation checks and channel/session identities remain
in place. Image decoding has its own per-image identity, so permission/moderation
changes reject work already in flight. Video download identities prevent a late
file from reopening a player after Close.

Player teardown mutes and pauses the stream, detaches GTK's video controls, then
clears the source. The desktop regression exercises five consecutive native
open/decode/close cycles, as well as attachment playback and deletion, to cover
the intermittent cleanup crash found during this pass.

The composer remains focused and becomes temporarily read-only while a request
is pending; failed requests retain their draft and files. Requests are not
silently retried. Larger thumbnail presentation improves visible size without
inventing additional image detail. Videos download before playback; Range
streaming remains outside this change, and playback depends on installed codecs.

Performance changes address concrete synchronous work and repeated sorting.
The native timeline still retains all loaded rows; this pass does not claim
constant memory usage or a production benchmark for unlimited history.

See [TESTING.md](TESTING.md#native-chat-layout) for commands and requirements.

Validation on 2026-10-07:

- `cargo test --locked --offline`: 123 passed, 0 failed, 3 environment-dependent
  tests ignored by default.
- `GSK_RENDERER=cairo PAPO_DESIGN_PREVIEW=1 cargo test --locked --offline ui_smoke -- --ignored --test-threads=1`:
  passed twice with the final playback teardown and repeated-close regression.
- `cargo build --locked --offline`: passed; existing unused-code warnings remain.
- Inspected dark, light, inline-image and voice-sidebar previews. The desktop
  workflow occasionally emits a nonfatal `GtkText` focus-out warning.

The GTK workflow uses local HTTP fixtures and installed media codecs. Production
network behavior, native SFU integration and the real screen portal were not
retested in this pass. No dependencies were added.
