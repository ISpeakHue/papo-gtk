# Bug fixes 9

Implemented the eight requests in `bug_fixes_9.txt`.

| # | Change | Regression coverage |
|---|---|---|
| 1 | Giphy, Tenor and Klipy link cards use the backend video rendition as a silent animation when available. They no longer race a still-thumbnail request or create a video player. Provider metadata also identifies cards whose URL alone is insufficient. | Local Klipy sample URL and a Giphy metadata-only card animate through the real isolated decoder; no thumbnail GET or GTK player. |
| 2 | Public GIF downloads and image decoding have separate concurrency limits from ordinary media requests. At most two GIF jobs can wait at once, leaving room for other media; cached image decoding bypasses the HTTP slot limit. Server dimensions and encoded image dimensions survive downsampling, animation preparation and channel switches. Small compressed GIF video renditions share the existing bounded media cache. | Scheduling, first-load geometry, channel switching, scrolling, poster and weak-reference memory checks; returning to a GIF card adds no video request. |
| 3 | A short edit opens in a compact dialog with adjacent Cancel/Save buttons. Editing offers text controls only and preserves existing custom embeds. Enter saves, Shift+Enter inserts a newline, and Escape cancels. Native input-method composition gets priority over Save. | Actual dialog height, modifier handling, failed-save preservation and successful HTTP PUT. |
| 4 | Printable keys and dead keys focus the available composer after navigation. The original key event reaches GTK's normal input-method path. Other editable fields, selected text, popovers and keyboard shortcuts retain their input. | Real X11 keyboard events after sidebar focus and channel switching, accent composition, another editable field and multiline input. |
| 5 | The one-line composer shares a vertical centre with its toolbar controls; multiline growth remains bounded. | Mapped control/input geometry and existing wrapping checks. |
| 6 | Small images and GIFs retain their native size. Larger media fits the existing chat limits without allocating enlarged textures. Tiny loading controls are clipped without negative label allocations. | Small still-image/GIF fixtures and large/portrait aspect-ratio checks. |
| 7 | “Arquivo salvo.” is a separate informational notice that disappears after three seconds. Expiry belongs to its own notice/session and cannot clear newer feedback or an error. | Timed expiry and overlapping-notice/error checks. |
| 8 | Enter/Tab commits the focused mention or emoji after arrow navigation and returns the caret to the composer. Typing afterwards appends text. | Real suggestion widgets, Down navigation, Tab activation, wire-format mentions and caret checks. |

Media remains lazy and permission scoped. Compressed images, posters and small
GIF video renditions share a **32 MiB / 256-entry** cache, with a **4 MiB per-entry**
limit. Decoded textures and animation frames remain disposable. A cold attachment
without server dimensions still needs one anchored layout adjustment when its
dimensions become known; there is no fixed loading delay or wait for all media.

The original Klipy page request returned HTTP 403. Pasted links now use the
public website lookup described below; bot messages still prefer their backend
rendition. The native bot regression uses a local backend and no saved session.

See [TESTING.md](TESTING.md#bug-fixes-9) for the repeatable native regression.

Validation: `cargo build --locked --offline` and 231 Rust unit/contract tests
passed. Native checks passed for `report9`, `image-geometry`, `media-cache`,
`attachment-media`, `smooth-updates`, `multiline`, `full-app-input` and `retention`.

## Clickable emoji details

Click a custom emoji in message text, rich-card descriptions or profile text to
open a compact native popover beside it. It shows the cached preview, `:name:`
and the current server's name. Animated previews reuse the existing animation;
opening the popover makes no additional HTTP request. Standard Unicode emojis
also show their glyph and name from GTK's local emoji catalog.

Escape or clicking outside dismisses the popover. Unmapping the message/profile
also closes it and releases the preview widget. Dragging/selecting text keeps
its usual behaviour. Concealed spoilers must be revealed before their emojis
can be inspected, while emoji inside code stays literal and links keep their
normal action. Joined Unicode emoji and skin-tone modifiers use whole-glyph hit
regions rather than UTF-8 byte offsets.

The `emoji-details` native regression covers opening, origin/name, compact
dimensions, animated preview, Escape, selection/dragging, spoiler reveal and
cleanup. A separate unit check covers Unicode sequence boundaries and offsets.

Validation for this follow-up: the build and 232 Rust tests passed, along with
native `emoji-details`, `embeds`, `profile-emojis`, `report8` and `mentions`
regressions. The quoted-emoji assertion accepts the interactive text renderer;
the wrapping fixture exceeds the toolbar's minimum window width.

## Bare GIFs and Discord bridge embeds

GIF embeds use a bare, left-aligned animation surface. Provider titles, links
and descriptions appear in the opened GIF viewer; an unavailable GIF opens a
small information/retry popover. Matching source links are hidden from message
text and bridge attribution descriptions, while surrounding prose, code and the
sender's identity remain. This masking happens before media hydration, avoiding
a source URL flashing or remaining in a reused row.

Provider recognition covers custom/manual bot cards and domain-style metadata
such as `klipy.com` and `giphy.com`. A backend MP4/WebM rendition is decoded into
silent looping frames, without a video player. Non-provider videos keep their
normal playback controls. Embedded GIF bytes also retain their animation.
Static provider thumbnails cannot race an available animated rendition.
Tenor's short `.gif` page URLs follow bounded redirects to validated provider
hosts and use the final post's metadata; supported CDN video metadata is also
accepted when a GIF rendition is unavailable.

Authorized inspection confirmed the bot uses custom/manual Klipy cards with
`site_name: "klipy.com"`, a Discord-proxied MP4 and a static WebP thumbnail.
Direct Klipy pages returned a Cloudflare HTTP 403 challenge. The pasted-link
resolver below now obtains the exact item through Klipy's public website lookup.
Missing items and inaccessible provider media retain the information/retry UI.

The `gif-embeds` native case (also included in `report9`) checks the bot metadata,
bare rendering, source masking, preserved attribution, viewer metadata, shared
animation frames, failure/retry UI and compressed rendition reuse after a channel
switch. Unit checks cover domain metadata, unrelated videos, Tenor short links,
provider URL restrictions and source masking without a hydrated cache.

Failures now invalidate the media view so a failed decode replaces its spinner
with the information/retry control. Invalid temporary renditions are released;
Retry fetches fresh media. Existing video metadata avoids redundant embed GETs,
including manual cards with no fetch-method marker. A completed empty metadata
lookup is remembered so repeated layout passes do not poll the same embed.

Validation: `cargo build --locked --offline` and 236 Rust tests passed. Native
checks passed for bare GIFs, rich cards, attachment playback/download actions,
compressed media caching and the combined `report9` keyboard/media regression.

## Pasted Klipy links

Pasted `https://klipy.com/gifs/{slug}` links now resolve through the public
website's GET `/api/proxy/web/gifs/{slug}` endpoint, rather than fetching the
HTML page that returns a Cloudflare challenge. This lookup needs neither a
partner API key nor a Papo embed. The returned slug and `type: "gif"` must match
before any rendition is downloaded; unrelated search results are never used.

The resolver selects a bounded GIF rendition, with MP4/WebM animation fallback.
It validates HTTPS, exact Klipy CDN hosts, media type, dimensions and encoded
size; delivery query parameters remain intact. Metadata is capped at 256 KiB,
GIFs at 4 MiB and video renditions at 8 MiB. A successful cold load makes one
metadata GET and one media GET through the existing provider queue, without
cookies, authentication or Papo API requests. Existing session caching restores
compressed media after channel switches. Bot GIFs retain priority for their
provided backend rendition. The website endpoint is an external contract and
may change; unavailable results use the existing retry control.

Live checks loaded and decoded all three supplied examples: `halloween-scary-22`,
`happycadogt-bang` and `hop-on-google-docs-1`. The opt-in native `klipy-live` check
uses a message containing only a URL, no backend embed, and verifies animation,
bare rendering, released off-channel frames and compressed-byte reuse on return.
The ordinary tests cover exact-slug matching, media validation, one lookup plus
one media download and refusal to download unrelated/missing items.

Validation for pasted links: the build and **239 Rust tests** passed. The live
three-link decoding check and native `klipy-live` chat/cache check passed, as did
the existing `gif-embeds` bot regression. Live network tests remain opt-in.

Pre-push checks also corrected the shared image-viewer lookup to find its named
scroll area when GIF details follow it. The default smoke test now uses the
header-bar search control, waits for asynchronous DM pin controls, checks learned
source geometry, and verifies compressed-cache retention with disposable decoded
textures. The isolated `report6` geometry/cache regression passed.

The complete default `ui_smoke` suite also passed before publication, including
native media controls, direct messages, profile/search controls, adaptive layouts,
cache retention, startup and channel-switch performance checks.

## Backend GIF resolver migration (2026-10-16)

Supersedes the historical public Klipy lookup described above: provider scraping
now belongs to papo-backend (`0e80a5c`, `6d50023`). The frontend consumes animated
GIF bytes from authenticated embed responses, retains bounded animation/cache
handling and the bot video relay, and leaves unresolved links readable. The
former `klipy-live` test is replaced by the offline `backend-gifs` fixture.
Temporary composer/action errors now expire after five seconds; retryable history
failures and connection state retain their dedicated controls.

Validated on 2026-10-16: warning-free all-target compilation, 251 automated tests,
and the `backend-gifs`, `gif-embeds` and `channel-cache` GTK fixtures. The backend
GIF fixture also covers an expired timer whose dismissal is still queued behind
a newer error. Obsolete frontend scraper tests and the opt-in public lookup were
removed with their implementation.
