# Images, GIFs, embeds and video

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

## Emoji, viewer and audio regressions

The mapped workflow checks:
shared Unicode/server emoji search, square chooser cells, composer insertion,
same-thumbnail viewer dismissal, post-preparation audio, mute/hover volume,
notification authors, reply contrast/role colors, the header search action,
media reuse between channels and stable text widgets during image completion.
An instrumented MediaStream checks that startup/preparation actually invokes
the backend audio update, including when GTK already reports its default values.
Mute/volume choices made before preparation survive repeated preparation.
Run just this regression on a test display with `PAPO_UI_CASE=video-audio` and
the [GUI command in Testing](../../TESTING.md). Rich-card tests check left-edge positions for long
inline fields at wide/narrow sizes; the Twitter/X fixture also checks the
left-aligned, full-size native video surface.
Three separate send confirmations must produce visible text pixels without
another message or scroll event, and wrapped lines must remain fully visible
after narrow/wide resizing. Catalog updates preserve the active search caret
and focus; closing the composer chooser returns focus to the message entry.
Existing eviction, permission, video cleanup
and large-history regressions remain part of the full workflow.

For a shorter chooser/media/send check on a test display, set
`PAPO_UI_CASE=report6` on the `ui_smoke` command. Run without it for full coverage.

## Media, presence and send-confirmation regressions

The default suite checks Giphy token/page/CDN parsing and rejects arbitrary hosts
and malformed identifiers. Generated GIF fixtures check multiple frames, delays,
decoded sizes and input limits. Emoji and link tests include Unicode caret offsets,
custom name case, literal code and balanced URL punctuation. Read cursor tests
cover timestamp precedence, non-advancing pages and bounded pagination. The entry
and departure WAV assets are distinct, short PCM clips.

The `ui_smoke` workflow now also checks:

- Emoji suggestions retain typing focus, insert Unicode and `:OMEGALUL:`, and
  coexist with mention keyboard navigation.
- HTTP links activate on a click; dragging or selecting text does not open them.
  Tests intercept the URI launch callback rather than starting a browser.
- A read cursor outside the latest history page requests older history once,
  survives a newer read snapshot, and selects the first unread row. Fully read
  channels open at the bottom.
- Matching live sends and duplicate HTTP confirmations reveal the latest message.
- GitHub preview bounds end before the next message row.
- Video starts unmuted at nonzero volume, decodes audio and video, and picture
  clicks pause/resume it. GTK play/pause, seek, mute and hover-volume controls remain available.
- GIF thumbnails and viewers use changing paintables. Clicking inside the image
  keeps it open; clicking the background closes it. Releasing a paintable releases
  its frame timer.
- Local speaking events show the microphone indicator before a server speaker
  update, mute clears it, and departure cues play once with sounds enabled.

`python3 tests/voice_probe.py` additionally requires local speaking transitions
from the actual native worker and their clearing on mute, for both direct ICE and
forced TURN relay. It uses synthetic capture. Build the fixture as documented in
the [voice guide](voice.md) before running it.

The public example `giphy:VxdNf4DadRSsMYAfnv` was also decoded from its downloaded
200-pixel rendition. To repeat that optional check, download it to a local file
and run `PAPO_GIPHY_SAMPLE=/path/to/sample.gif cargo test --locked supplied_giphy_sample -- --nocapture`.
The normal suite uses generated fixtures and requires no Giphy network access.

## Rich embed migration

The default Rust suite verifies the nested `/embeds` contract, custom multipart
POST and JSON PUT payloads, embed-only messages, Unicode/combined text limits,
safe URLs, supported video MIME types, complete WebSocket replacement/clearing
and concurrent history/reconnect journals. These tests use local fixtures.

The existing `ui_smoke` workflow also exercises rich cards, ordered inline fields,
plain text/unsafe-link rejection, the custom composer form, draft clearing,
POST/WebSocket ordering, shared cached thumbnail cleanup, narrow layouts and
delayed-edit/stored-media safeguards. Run the full workflow with the virtual-display
[command in Testing](../../TESTING.md), or focus on embeds with:

```sh
PAPO_UI_CASE=embeds dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The shared embed color palette has a focused native regression:

```sh
PAPO_UI_CASE=style-palette dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

It verifies distinct card colors, light/dark themes, styling before the first
snapshot, cached channel switches, 5,000 repeated mount/disposal operations,
bounded color capacity and provider cleanup. The regular Rust suite also checks
that retained, undisplayed response/event fields still reject malformed metadata.
The main CI test job uses `RUSTFLAGS="-D warnings"`; production and test code must
both remain warning-free. Runtime observations that still need investigation
are tracked in [Follow-ups](../FOLLOW_UPS.md).

For a manual backend check, send a URL and observe its automatic card. Create
a custom card with color, author, footer and inline fields, send it without text,
edit/remove its fields, and verify a second authorized client receives each
complete replacement. Exercise a thumbnail and an HTTPS video with a supported
codec. Recheck the card after switching channels and resizing the window.
Stored-media editing and unavailable full images are explained in
[EMBEDS.md](../../EMBEDS.md); their limitations originate in the inspected backend.

## Send feedback and fullscreen video

The GTK workflow checks pending text and embed-only sends without transient
upload widgets or composer movement, failed-send draft retention, and visible,
cancelable file progress. An instrumented media stream checks fullscreen size,
stream identity, play/pause, timestamp, mute and volume preservation, Escape/F11,
the exit button and disposal. The real decoder workflow exercises both embedded
and attached videos, message reconciliation, background row eviction and deletion
while fullscreen. All requests use local fixtures.

To run the focused widget regressions:

```sh
PAPO_UI_CASE=send-video dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

For a desktop check, play an attachment and an embedded video, use the fullscreen
button, seek/change volume, then return using Escape. Check that playback continues
from the same position. Verify that switching channels closes fullscreen playback.

Local validation: 167 headless tests passed, the focused send/fullscreen widget
checks passed, and the complete GTK workflow passed in 142.69 seconds on a private
virtual display. `cargo build --locked --offline` also passed.

## Presence badges and original image viewing

Member, direct-message and user-footer badges use bundled SVGs: a green dot for
available, an amber crescent for away, a red minus for busy and a gray ring for
offline. They include status tooltips and do not depend on desktop theme icons.

Opening a static attachment shows its cached chat thumbnail immediately, then
downloads the authorized original to a temporary file and decodes it off the GTK
thread. Native detail is retained up to a 4096-pixel edge; larger sources are
resized with Lanczos filtering. Source dimensions, pixels and decoder allocation
are bounded. The high-resolution texture belongs only to the viewer, so closing
it releases the texture and cancels pending requests. Switching channels,
revoking access or moderating a picture prevents stale results from appearing.
Animated GIF paintables keep their existing playback. Link embed quality is
limited to the cached image supplied by `/embeds`; the current backend does not
provide an original-image endpoint for those embeds.

```sh
PAPO_UI_CASE=status-viewer dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The mapped test checks embedded resources, light/dark status rendering, member
row reuse, thumbnail-only requests before opening, an actual 1800×900 original,
unchanged chat rows/textures, released viewer textures, queued cancellation,
stale viewer tokens, request failure fallback and moderation. Decoder unit tests
cover native resolution, resizing and invalid/oversized sources. With
`PAPO_DESIGN_PREVIEW=1`, snapshots are saved under `/tmp/papo-design-status-*.png`
and `/tmp/papo-design-original-image.png`.

## Bridged Discord embed authors

Custom cards use a 2-pixel accent border, 4-pixel corners and compact padding.
The live backend supplies `author_image_data` separately from `image_data` on
the authorized `/embeds/{id}` endpoint. The renderer displays a 24-pixel sender
avatar beside the author name, reserving its geometry before decoding finishes.
Author images decode off the GTK thread and retain at most 72×72 pixels in a
separate, bounded cache. They never replace the main thumbnail or become a
full-image viewer source. Original author URLs are not fetched directly.

```sh
cargo test --locked --offline
PAPO_UI_CASE=embeds dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The mapped workflow checks author-only cards, main thumbnails plus author icons,
shared-card request deduplication, malformed icons, stable avatar height, encoded
payload removal, offscreen texture release and rejected late results after a
channel switch. Set `PAPO_DESIGN_PREVIEW=1` to save the bridge-card screenshot.

## First-load image geometry

A decoded image uses its actual aspect ratio on the first visit, even if its
placeholder was already painted. Guessed geometry never overrides known image
dimensions. Landscape, portrait, embedded images and animated GIFs fill their
frames without added letterboxing. The existing deferred media update and
pre-paint anchor restoration handle the height change after scrolling pauses.

```sh
PAPO_UI_CASE=image-geometry dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=smooth-updates dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The geometry check paints the unknown-size placeholder before delivering bytes
through the worker decoder. It compares allocated picture dimensions with the
paintable's aspect ratio, checks narrow layouts, and verifies that unloading,
channel restoration and a second decode retain the corrected geometry. The
smooth-update regressions check scrolling continuity and reading-anchor pixels
when media changes height. `PAPO_DESIGN_PREVIEW=1` saves a first-load screenshot.

## Mentions in bot embeds and image zoom

Bot embed descriptions, field values and footer text use the normal read-only
chat renderer for mentions and custom emoji. Canonical mention IDs become blue,
clickable `@nickname` labels. Loading member metadata or changing a nickname
refreshes the reused card without replacing its surrounding chat row. Member
names remain plain text, including names containing markup characters.

Click an image preview to open the fitted windowed view, then use its fullscreen
button to reveal zoom, fit and 100% controls. Leaving fullscreen restores fit and
hides zoom controls. `F11` toggles fullscreen;
`Escape` exits fullscreen first, then closes the viewer. `+`/`-`, Ctrl+wheel and
pinch gestures zoom; Ctrl+0 fits the image. Drag a zoomed image or use scrollbars
to pan. Fullscreen controls remain visible. A higher-quality image arriving while
zoomed preserves the displayed size. Zoom and fullscreen reuse the paintable
without downloading or decoding additional copies. Closing the viewer clears
its paintable even if the window object is still retained.

```sh
PAPO_UI_CASE=embeds dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=status-viewer dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The embed check covers delayed member metadata, nickname updates, profile clicks,
selection without navigation, literal markup characters and narrow layouts.
The viewer check covers zoom allocations, panning, keyboard controls, fitting,
unchanged chat textures/requests, animation playback and memory cleanup, as well
as the existing original-image cancellation, authorization and failure checks.
On a test compositor that implements fullscreen, set
`PAPO_NATIVE_FULLSCREEN_TEST=1` to assert native fullscreen transitions, visible
controls, Escape behavior and F11. A bare Xvfb display has no window manager and
cannot verify those transitions. `PAPO_DESIGN_PREVIEW=1` saves
`/tmp/papo-design-image-zoom-controls.png`.

## Uploaded GIFs and media presentation

`PAPO_UI_CASE=attachment-media` runs the native attachment regression with only
local mock media. It serves an animated GIF original alongside a still thumbnail
and verifies that the visible chat and image viewer advance frames. Ordinary
images still request thumbnails; small GIFs (up to 4 MiB) request their original
through the same authenticated, bounded, asynchronous pipeline. Larger GIFs
retain the backend thumbnail path. GIF decoding accepts the backend's 300-frame
limit and reduces preview resolution if needed to stay within 48 MiB of frames.
Oversized or undecodable animations can retain a still preview.

The native check also covers filename-free image/video/GIF previews, clickable
generic file tiles, right-click download actions (including tall messages),
the viewer's download control, saving the original GIF bytes, and rejection of
downloads after sensitive moderation or permission loss. Revealing sensitive
media restores its menu action. Unit tests cover 300-frame thumbnails and
resolution reduction without dropping frames or timing.

## Video posters and compressed media reuse

Run the native smoke test with `PAPO_UI_CASE=media-cache` in a private display.
It generates WebM and H.264/MP4 posters from the existing local fixtures, including
an MP4 padded to move its metadata past the preview prefix. It verifies prefix
and tail Range requests, supplied-thumbnail preference, embedded video posters,
and the absence of background players or retained original video files.
Unsupported media produces an explanatory placeholder and is not repeatedly
fetched by viewport updates.

The same check switches channels three times and verifies no additional requests
for cached media. Weak references prove that recent preview textures remain
resident while full GIF animations are released. Permission loss clears both
resident previews and reusable bytes. Unit checks cover the 96 MiB/512-entry
compressed LRU and its preference for recent batches, deletion/moderation/revocation,
strict HTTP-range limits, sparse-file offsets and cleanup, servers ignoring Range,
malformed responses and bounded poster encoding. No real account is used.

Recent resident previews reuse the same textures on a revisit. Other images use
compressed bytes to avoid network waits and repeated poster extraction. RSS may remain
above the live buffer total because the allocator or graphics driver retains
released storage for reuse; the weak-reference checks measure live ownership.

## Bare GIF embeds

```sh
PAPO_UI_CASE=gif-embeds dbus-run-session -- xvfb-run -a \
  cargo test --locked ui_smoke -- --ignored --test-threads=1
```

This is the GIF subset of `report9`. It uses custom/manual Klipy and Giphy cards
with domain metadata, static thumbnails and local video renditions; a separate
embedded GIF verifies the base64 payload path. It checks bare animations without
video controls or duplicate source text, preserved bridge attribution, metadata
only in the opened viewer, no HTTP request when opening, compressed cache reuse
on channel switches, and information/retry UI for a failed rendition. All API
requests use a local mock server.

## Backend GIF media and temporary chat errors

The backend resolver added in `0e80a5c` and hardened in `6d50023` supplies
`fetch_method: "gif"`, GIF thumbnail metadata and animated `image_data` through
`GET /embeds/{id}`. The frontend no longer scrapes public GIF pages or downloads
provider CDN renditions. Links without an embed remain readable until the
backend publishes the embed update. Existing bot MP4/WebM renditions still use
the authenticated video relay and render as bare animations.

```sh
PAPO_UI_CASE=backend-gifs dbus-run-session -- xvfb-run -a \
  cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

This offline fixture checks delayed embed arrival for Klipy/GIPHY/Tenor, one
backend GET per unresolved GIF, inline GIF payloads without another GET,
animation without video controls, channel cache reuse, and a GIF larger than
4 MiB within the backend's 5 MiB bound. It also checks that temporary errors
disappear after five seconds without changing a draft, and that an older timeout
cannot dismiss a newer error. History retry controls and connection status remain
visible while their underlying condition persists.

## Pinned-message media

The **Mensagens fixadas** window uses the chat renderer in a read-only scope:
GIFs animate, custom emojis render inline, and images, rich embeds, videos,
audio and other attachments retain their usual viewers and download actions.
Each pin has an **Abrir mensagem no histórico** button. Browsing pins does not
fetch ordinary channel history or acknowledge unread messages. Closing the
window, changing channels or losing access releases its media work and players.

Run the synthetic integration test on a private display:

```sh
PAPO_UI_CASE=pins-media dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

It covers old pins outside loaded history, decoded-cache reuse, animated GIFs,
custom emoji assets, attachment actions, edits/moderation/deletion, unpin/re-pin,
and window/access cleanup. No saved session or remote server is used.

## Attachment previews before sending

Selecting, dropping or pasting an image/video adds a compact thumbnail card
above the composer. Videos show a silent still frame; GIFs show their first
frame. The remove button and filename remain available for every attachment,
including files whose preview cannot be decoded. Local originals are never
modified or uploaded just to produce thumbnails. Preview decoding is serialized
away from GTK, uses bounded images, and rejects late results after removal or
conversation changes. Failed sends retain the staged files and decoded previews.

```sh
cargo test --locked ui::chat::draft_preview::
PAPO_UI_CASE=draft-previews dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=clipboard dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

These checks cover image dimensions, GIF/video thumbnails, non-media/corrupt
fallbacks, duplicate attachments, caption editing, removal, failed-send retries,
successful-send cleanup and stale completions during channel switches. The
clipboard workflow exercises pasted images and temporary-file cleanup.

## MOV / QuickTime attachments

Received MOV files use the normal inline video preview, play button, controls,
fullscreen view and context-menu download. QuickTime MIME types are recognized;
when the backend reports unknown binary data (`application/octet-stream`), a
case-insensitive `.mov` extension also selects the video path. Explicit image,
text and document MIME types retain their existing behavior. This classification
does not bypass moderation or downloaded-container validation.

The backend's Go MIME sniffer recognizes MP4 brands but can return generic binary
data for QuickTime's `qt  ` brand. No upload-header or backend change is required
for the frontend fallback. Posters keep the existing bounded range requests,
network-isolated helper and recent-media caching; playback remains explicit.

GStreamer's [QuickTime demuxer](https://gstreamer.freedesktop.org/documentation/isomp4/qtdemux.html)
handles MOV containers. The codecs inside must also have suitable GStreamer
decoders. The AppImage packaging already includes the Good plug-ins containing
`qtdemux` and the libav codec plug-in; a development launch uses system plug-ins.
No automatic transcoding or extra full-file background download is introduced.

```sh
GTK_A11Y=none G_DEBUG=fatal-warnings PAPO_UI_CASE=mov-media dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

This case uses a silent original H.264 MOV and a local mock server. It verifies
real posters and advancing native playback for both QuickTime and generic MIME
metadata, bounded player-file reuse, blocked-media handling and permission-loss
cleanup. FFmpeg is needed only to regenerate the committed fixture.
