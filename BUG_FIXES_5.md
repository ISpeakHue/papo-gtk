# Bug report 5

The changes address `/home/gabrieloliveiradecastro/Documentos/bug_fixes_5.txt`.

| Item | Behavior |
| --- | --- |
| 1 | Clicking the video picture toggles playback. Native buttons and the seek slider retain their own behavior. Existing media sizes remain. |
| 2 | Leaving a joined voice call plays a short descending cue, distinct from the entry cue. Both honor the account's sound preference. Failed joins and duplicate exits do not add cues. |
| 3 | Server emoji pages become visible as they arrive. Image decoding runs off the GTK thread, and decoded emojis remain cached across channel changes. |
| 4 | Search starts at 540 × 520 instead of 700 × 740. Advanced filters remain expandable. |
| 5 | `giphy:VxdNf4DadRSsMYAfnv`, Giphy page URLs and supported Giphy CDN URLs resolve to animated GIFs. GIF thumbnails from the backend also retain animation. |
| 6 | HTTP/HTTPS links are blue and underlined. Clicking opens the system's URI handler; selecting, dragging and copying text remain available. Code spans remain literal. |
| 7 | Clicking the background outside the displayed image closes its viewer. Escape and moving focus outside the viewer also close it. GIF viewers preserve animation. |
| 8 | Channel and DM selection snapshots the read cursor before history advances it, then positions at the first unread message. Fully read conversations open at the bottom. |
| 9 | Preview cards clip their drawing to their allocation and leave space before the next message. A mapped GitHub preview regression checks the next row's bounds. |
| 10 | The native audio worker reports local microphone activity. The local voice participant shows the existing microphone indicator and highlight, independently of server speaker updates. Muting clears it immediately. |
| 11 | An emoji button sits immediately left of `@`. It offers server emojis and the full native Unicode picker. |
| 12 | Typing `:` suggests Unicode names, common aliases and every available server emoji name, preserving custom name case. Suggestions do not take focus from typing. |
| 13 | Confirmed sends return to the latest message even when the WebSocket inserted that identical message before the HTTP response. Matching live confirmations can scroll while the send is pending. |
| 14 | User-started videos explicitly start unmuted at full volume. Native playback checks verify both audio and video tracks; closing still detaches controls and releases the temporary file. |

## Findings and bounds

The backend returns at most 25 emojis per page. The frontend previously waited
for every page before displaying any, then decoded their images on the UI thread.
Progressive loading removes that wait; later pages still depend on network latency.
Requests continue through the shared paced API scheduler.

The reported `giphy:…` syntax was not recognized by either preview pipeline, and
GTK textures displayed only the first GIF frame. The public example's 200-pixel
rendition was downloaded and decoded successfully. Public Giphy downloads use a
separate credential-free HTTPS client, accept only validated Giphy identifiers,
reject redirects, limit encoded data to 4 MiB, and share the chat's four-job media
limit. No API cookies or authorization headers reach Giphy. URLs follow the
[Giphy rendition schema](https://developers.giphy.com/docs/api/schema/).

GIF decoding caps source dimensions, frame count and decoded memory. The animated
cache has a 48 MiB budget and shares the 32-item media cache; frame timers stop
when their paintables are released. Unsupported or excessive media can be retried
manually. The original Giphy link remains available in the message.

Unread positioning uses message identity before the read timestamp. If the read
boundary is outside the latest page, older pages load sequentially, capped at 20
additional pages per switch; navigation, scrolling, sending or another selection
cancels that search. At that bound, the oldest loaded unread message is shown and
normal history pagination remains available. Conversations without a prior read
cursor use the latest page rather than downloading their entire history.

Videos can only play audio present in the source, using the system's selected
audio output. The changes do not synthesize audio for silent GIFs or silent clips.

## Regression coverage

`cargo test --locked` covers parsing, Unicode offsets, read cursor precedence,
GIF frames/timing/limits, distinct sound assets, and the existing API race and
request budget checks. The ignored `ui_smoke` workflow covers real mapped widget
interactions, emoji focus/insertion, link click versus selection, unread/latest
positioning, duplicate send confirmations, preview bounds, video picture clicks,
unmuted defaults, animated image viewers, leave cues, and local speaking/mute UI.

`tests/voice_probe.py` verifies local speaking transitions as well as bidirectional
decoded audio, server speaker activity, muting and leaving through both direct
ICE and forced TURN relay, using synthetic capture. The native worker cleanup
test checks that dropping capture reaps its process.

Verified locally on 2026-10-08:

- `cargo test --locked --offline`: 144 passed, 3 explicitly ignored.
- Mapped `ui_smoke` workflow in a private X11 display: passed (96.31 s).
- `cargo test --locked --offline native_voice -- --ignored`: passed.
- `python3 tests/voice_probe.py`: direct ICE, TURN relay, local speaking/mute,
  missing-device handling and process cleanup passed.
- `cargo build --locked --offline`: passed.
- Optional downloaded Giphy sample decoding: passed.

The virtual-display run also emitted GTK/GIO and desktop-portal diagnostics;
these did not fail the interaction assertions. Real speaker volume and the
system browser remain desktop acceptance checks; tests use synthetic audio and
intercept browser launching.
