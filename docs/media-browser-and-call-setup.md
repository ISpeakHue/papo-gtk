# Disk thumbnails, conversation media and call setup

## Disk thumbnail cache

Decoded chat images, embed images and video posters can reuse private PNG
thumbnails after restarting. Full animations, original images and video files
remain outside this disk store. Cached pixels do not authorize access: loading
still requires a current readable conversation and an attachment/embed reference
in its message metadata. Sensitive/blocked/pending attachments retain the chat's
existing reveal and moderation gates.

The cache uses the native per-user cache location under
`papo-gtk/thumbnails-v1/<opaque account namespace>`. The namespace includes the
canonical API address/base path and verified account UUID. Resource filenames
hash the conversation, message, resource and revision. Headers contain those
IDs, a revision hash and creation time; they do not contain credentials, source
URLs, filenames or message text. Unix directories/files use modes 0700/0600;
Windows uses the native per-user cache directory and inherited profile access.

| Limit | Value |
| --- | ---: |
| Per account | 32 MiB / 512 files |
| Across accounts | 128 MiB / 2,048 files |
| Per encoded thumbnail | 2 MiB |
| Maximum age | Seven days |
| Concurrent encoders / queued preparations | Two / 32 |

Bounds count file contents, including headers, rather than filesystem block
allocation overhead. LRU eviction uses file access timestamps maintained by the
cache. Expiry also checks creation time. Work runs off the GTK thread; corrupt,
expired, oversized and inaccessible entries are disposable misses. Original
downloads never depend on successful cache writes.

Permission snapshots and deletion/moderation/preview events close the in-memory
admission gate immediately and coalesce disk cleanup. Earlier async writes
cannot repopulate invalidated entries. Entries without current message references
cannot be displayed, even if an offline deletion has not yet reached the client.
Logout schedules account-cache clearing. **Limpar cache de mídia** clears the
disk store as well as the existing memory/temporary caches, while open viewers,
uploads and chosen file saves retain their own resources. The preferences row
now reports memory, temporary files and disk thumbnails separately.

The initial default is deliberately smaller than the in-memory recent-preview
budget. Validation compares eight uncached previews with a fresh API/client tree
using the same authorized account and disk directory. The comparison records
thumbnail request counts and observed timings; the
[validation evidence](testing/evidence/steps101112-2026-11-05/README.md) distinguishes
this from the earlier in-process warm-revisit baseline. This is a synthetic
loopback measurement, not a claim about live-server latency.

## Conversation media browser

The image icon in the chat header opens **Mídia da conversa**, with image/GIF,
video and file filters. Click a tile once to open it. Viewers provide
**Anterior**, **Próxima**, **Ir à mensagem**, and attachment downloads. Previous
and next remain within the selected media type. Images reuse the existing zoom
and fullscreen controls; GIFs animate within the existing decoding limits.
Videos reuse the existing playback/fullscreen controls and teardown sequence.
Files open a download view. Original-load failures offer a local retry.

Discovery uses the **existing backend API**, as requested:

- Two descending `POST /search` streams find attachments and attachment-free
  messages containing links, with separate `(since,last_id)` cursors.
- A merge preserves timestamp/UUID order across page boundaries. Each user
  action resolves at most eight matching messages, with four requests in flight.
- Complete metadata comes from inclusive `limit=1` history lookups for those
  exact hits. The browser does not walk ordinary text-history pages.
- The window retains at most 200 media items. **Continuar com mídia mais antiga**
  releases the current window and consumes remaining items before fetching more;
  multi-attachment messages are not discarded at that boundary.
- Thumbnails use four bounded requests and small decoded surfaces. Low-memory
  mode limits preparations to 64 thumbnails in the current window; on-click
  mode leaves generic tiles until explicitly opened. Thumbnail failure leaves
  a usable media/file icon.

Filters apply to the currently discovered batch. An empty category can therefore
have more results behind **Carregar mais mídia**. The backend searches links in
message text: historical custom/bot embeds without a searchable link there are
not discoverable through this API. Dedicated media-history support remains a
backend follow-up; no invented endpoint, frontend scraper or backend patch is
used. See [backend contracts](backend/ux-contracts.md).

Deleted/changed/moderated items are removed conservatively until the browser is reopened;
permission loss closes the browser and its viewers. Closing cancels its pending
requests. Stale results cannot update a replaced viewer. Browsing does not mark
messages read; **Ir à mensagem** uses normal authorized chat navigation.

## Call setup and device testing

**Configurar chamada** adds a microphone-level meter, an explicit three-second
microphone recording/replay test, output-device selection and a short playback
tone. Recording uses at most 288,000 PCM bytes in memory (48 kHz, mono, 16 bit),
then discards them. It creates no file and sends no recording or signaling to a
Papo server. Headphones avoid replaying the test into an open microphone.

Opening settings or enumerating devices never starts capture. A microphone test
requires leaving the call first. Output testing does not open a microphone.
Stopping, closing settings or reaching the bounded timeout releases the test
worker. Microphone gain applies to the record stage, and call metering respects
mute. Device selections are session-local, like the existing microphone choice.

Live changes retain the call's WebRTC transport, encoder, SSRC and mute state.
Bounded raw-audio bridges separate hardware capture/output pipelines from that
transport, allowing a short gap while hardware releases/reopens. The previous
device remains available for rollback until the candidate delivers audio or
confirms playback readiness. Permission/startup failure restores the previous
selection; an unconfirmed transition or failed restoration ends media with an
explicit rejoin message. No device change automatically unmutes a call.

Selected-device removal ends the affected call safely and refreshes device lists;
the next selection is explicit. Missing saved-in-session selections fall back to
the system-default choice for a subsequent call, rather than repeatedly trying
a disappeared endpoint. Windows duplicate display names remain distinguished
by endpoint IDs, and WASAPI loopback devices remain excluded from microphone
selection.

The bridge follows GStreamer's [application data access and pipeline lifetime
guidance](https://gstreamer.freedesktop.org/documentation/application-development/advanced/pipeline-manipulation.html).
Synthetic tests cover record/replay, cancellation, denied devices, source/sink
changes and rollback. Local Go SFU checks cover direct ICE and TURN with live
synthetic microphone changes and subsequent decoded audio/mute/leave behavior.
Physical hardware, operating-system permission prompts and native packaged
Windows acceptance remain manual checks.
