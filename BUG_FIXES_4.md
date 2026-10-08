# Fourth bug and design pass

Source: `/home/gabrieloliveiradecastro/Documentos/bug_fixes_4.txt`, with the supplied Discord screenshot as the media presentation reference. The reported launch command is `cargo run` from this checkout.

| Report | Changes | Regression coverage |
| --- | --- | --- |
| Empty entries in the @ menu | Empty or whitespace-only nicknames fall back to usernames. Suggestions show both the display name and account username. The fallback also applies to profiles, the account footer and presence labels. | Empty nickname models and an actual blank-nickname member in the mapped completion list. |
| Typing @ loses the character or fails to filter | Completion appears in a bounded panel above the composer without a popup or focus grab. Typing filters display names and usernames; Down enters the list, Enter/Tab completes, and Escape dismisses it. The @ button opens the same panel. | Native editable character insertion, retained @ and typing focus, manual/filter/empty results, keyboard selection and backend token insertion. Existing Unicode/caret and permission checks remain. |
| Missing call cue and speaking feedback | The cue starts after the server accepts joining the room, at increased volume, in an independent cancellable GStreamer helper. It respects the saved sound preference. Speakers have a microphone icon beside their name and a tinted name highlight, with or without avatars. Muted participants never light up. | Enabled/disabled sound, duplicate join/connection events, native PCM playback, speaking/muted/silent transitions and retained participant widgets. A separate real SFU probe covers actual active-speaker events. |
| Tiny media and prominent download buttons | Responsive frames give attachments up to 640 × 480 and videos a 640 × 360 presentation. Image embeds use up to 420 × 300. Tiny source images retain small decoded buffers and still display at a useful size. Video posters have a centered play action; visual attachments use an accessible download icon in the caption. | Actual Picture/Video allocations, aspect ratio, a 32-pixel image source, narrow-window shrinkage, download appearance, native decoding and repeated teardown. Moderation, permission and stale-download cleanup checks remain. |
| Delays during consecutive messages | Cached sidebar rows are reused before building widgets. Message events no longer revalidate unchanged voice access. Identical HTTP/WebSocket echoes skip a second history render. Encoded preview images stay out of render signatures; pinned-message lookup and row membership use sets. | Duplicate echoes with realtime reactions and history replay, bounded signatures, a mapped 20-message burst preserving old rows, text and caret, and existing large-history checks. |

Sends remain limited to one in-flight request, with the existing draft/file retention on failure and cancellation. These changes reduce synchronous rendering work; network latency and unlimited-history memory usage are not covered by a production performance guarantee.

Videos still download before playback. Physical output volume and microphone quality need a desktop check; the automated call-cue decoder uses a discard sink. Test servers use synthetic data and synthetic audio; no production call is needed.

The helper avoids GTK/GstPlay's observed teardown deadlock when a call ends while
a cue is preparing. Cancellation kills and reaps the helper instead of blocking
the GTK thread. Tests cover decoded audio, successful process reaping, early
cancellation and rapid repeated join/leave. No new runtime dependency is added:
the helper uses the Python/GStreamer installation required by native voice.

See [TESTING.md](TESTING.md#native-chat-layout) for test commands.

Validation on 2026-10-07:

- `cargo test --locked --offline`: 126 passed, 0 failed; 3 desktop/native tests ignored by default.
- Full GTK workflow on a private authenticated X11 display: passed twice, including native video decoding/teardown, settled media dimensions, mention typing/keyboard selection, speaking transitions, cue decoding and cancellation/reaping. Screenshot rendering waits for an actual drawable frame; the DM check waits for its asynchronous history response.
- `/usr/bin/python3 src/media/sound.py --test-sink < src/media/voice_join.wav`: 9,702 decoded PCM bytes.
- `python3 tests/voice_probe.py`: direct ICE and forced TURN relay passed bidirectional decoded audio, routing, active speakers, mute and leave; missing capture-device cleanup passed.
- `cargo build --locked --offline`: passed; `git diff --check`: clean.

The private desktop emits environment warnings for unavailable portal/keyring and
accessibility services and an existing GTK focus warning. The passing final runs
had no media teardown criticals or frame-measurement warnings. This does not
establish physical speaker volume or deployed-server behavior.
