# Rich embeds

The frontend follows the adjacent papo-backend revision `958390d` (Custom Embeds
Support). Backend source was inspected locally; regression tests use fixtures.

## API and live updates

Messages now use `embeds`, with nested author, thumbnail, image, video, footer
and ordered field metadata. Custom cards may omit the URL and fetched timestamp.
The frontend requests authorized thumbnail bytes from `GET /embeds/:id` and
video from `GET /embeds/:id/video`. It no longer calls `/link-previews`.

`message_embeds_update` replaces a message's complete embed list. An empty or
null list clears it. Live updates are replayed over concurrent history and
reconnect snapshots. A POST's custom-only list cannot replace a complete list
already received over WebSocket. Legacy `previews` payloads/events remain readable
for older fixtures; outbound requests follow the updated contract.
A delayed PUT also cannot restore cards after a newer timestamped socket edit.

Complete text-only metadata renders directly without an additional GET. Media
uses the existing bounded transfer/decode queue and session caches. Changes
invalidate stale work; removing a card preserves cached media still referenced
by another message. Thumbnail dimensions reserve space before image decoding.

## Native cards and authoring

Cards display the provider/site, linked author/title, description, validated color,
ordered inline fields, thumbnail, video controls and footer. Text is plain GTK
text. Remote HTML is never embedded. Backend-generated YouTube embed URLs offer
an allowlisted browser action. Cards use a left-aligned native clamp for narrow
layouts, with the tightening threshold equal to its maximum width so the clamp
does not insert centered padding. Video cards retain a 640-pixel player surface
when sufficient width is available.

Use **Embeds personalizados** beside the composer to add, edit or remove draft
cards, then **Aplicar embeds**. This does not send the message; the normal send
control sends the applied cards with text or attachments. Embed-only messages
are supported. Closing the form cancels unapplied changes. Channel changes and
loss of channel read access close obsolete forms. The message edit dialog also
exposes its custom cards; automatic link cards remain managed by the backend.

POST sends an `embeds` JSON string in the multipart form. PUT sends the complete
custom-card list alongside `content` in JSON. Local validation follows backend
limits: 10 cards, 25 fields/card, 256-character titles/field names, 4,192-character
descriptions, 1,024-character field values, 2,048-character footers/URLs and 6,000
total text characters across cards. Counts use Unicode characters. Media URLs
require HTTPS; videos require an allowed MIME type. Colors use `#RRGGBB`.

## Current backend limitations

The inspected handler exposes only thumbnail bytes as `image_data`. It does not
expose full embed image, author-icon or footer-icon bytes, although their metadata
is present. The frontend retains that metadata and explains an unavailable full
image; it does not fetch private media by hash or bypass authorization with an
external image download.

For custom thumbnail/image uploads, the backend drops the original URL after
storing the media. Updating custom embeds replaces their records. Consequently,
editing an existing card with stored media requires re-entering an original or
replacement media URL, or explicitly removing the card. Validation prevents an
unrelated text edit from silently discarding those images. Preserving them without
re-entering URLs requires a backend update that accepts existing media references.

## Validation

Headless tests cover nested/nullable models, new endpoint paths, both multipart
send paths, JSON editing, Unicode/combined limits, URL/MIME validation, complete
WebSocket replacement/clearing, snapshot journals and failed embed-only drafts.
The mapped GTK workflow exercises rich text/fields, the actual composer form,
embed-only send state, POST/WebSocket ordering, shared-media cleanup, narrow
layouts, delayed-edit ordering and stored-media edit safeguards. Existing chat,
media and permission workflows run alongside these checks. See
[TESTING.md](TESTING.md) for commands.

Local validation on 2026-10-09: `cargo test --locked --offline` passed 167 tests
(three opt-in tests ignored); the full mapped GTK workflow passed in 133.15
seconds; `cargo build --locked --offline` passed. GUI tests ran on a private
virtual display and used local fixtures, without contacting a production server.

## Playback and alignment follow-up

Startup audio now forces a volume update through GTK to the native player,
including after asynchronous preparation. Repeating `muted=false` and
`volume=1.0` alone can do nothing because GTK skips unchanged property values.
The one-time default synchronization preserves mute/volume choices made through
the controls before preparation; it does not reset volume on every frame or
play/pause operation. See the official
[GtkMediaStream implementation](https://github.com/GNOME/gtk/blob/4.22.5/gtk/gtkmediastream.c).

An instrumented native MediaStream regression checks backend audio updates,
not just the cached GTK properties, including explicit early mute/volume and
repeated preparation. Rich-card tests check the actual left-edge position for
short and long inline fields at wide/narrow widths. The existing Twitter/X video
fixture checks large-player size and left alignment during decoded playback.

Follow-up validation on 2026-10-09: both new regressions failed before the fixes
and passed afterward. The complete GTK workflow passed in 135.17 seconds,
all 167 headless Rust tests passed (three opt-in tests ignored), and the
application build passed. Tests used a private display and local fixtures.
