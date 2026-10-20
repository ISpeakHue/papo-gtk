# Backend integration and webhook identities

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

## Backend video metadata update (2026-10-20, `3252cfc`)

The adjacent checkout matches GitHub's `main` at
`3252cfc2b0563d31d6c6eedd535c94fe5f15aceb`. Reviewed the changes since
`e9cbcd1`: container dimension extraction, attachment storage/serialization,
MIME sniffing and repair, download/range headers, and the OpenAPI additions.

- Message attachments now include nullable original video `width` and `height`
  on history, creation, edit and pinned responses, and full live messages.
  The frontend retains these fields and reserves the video's original aspect
  ratio before downloading a poster. Original geometry takes precedence over
  thumbnail geometry; absent or invalid originals fall back to thumbnail,
  learned poster geometry or the established default. Existing inline caps,
  resizing and viewport anchoring apply. Small decoded posters cannot shrink a
  box already reserved from server dimensions.
- Both dimensions must be present, within 1–16384, at most 100 million pixels,
  and at most 10:1 in either direction, matching the backend's parser/migration.
  These are layout hints, not permission to allocate an original-size texture.
  Older servers can omit both fields; earlier uploads can legitimately be null.
- New uploads persist detected container MIME. Deduplication repairs existing
  generic media metadata; downloads also sniff legacy generic blobs and retain
  the corrected Content-Type on 200/206 responses with `nosniff`. Existing
  authenticated range handling is compatible and keeps its byte budgets.
  Old list records may still be generic, so supported MOV/MP4/M4V/WebM suffixes
  can select the video UI only when MIME is absent or generic. Specific image,
  audio and text MIME remains authoritative, including AVIF/HEIC and M4A.
- Routes and event types are unchanged. The database migration runs on the
  backend. Decoder/container validation remains in force. Newly detected AVI
  and Matroska attachments keep ordinary download tiles because the native
  validator does not support those containers.

The extended `backend-contracts` GUI case checks portrait/landscape placeholders
before bytes arrive, thumbnail/default fallback, original-versus-poster stability
and narrow resizing. New automated contracts retain dimensions through REST
history/send/edit/pins and live message parsing, including nullable compatibility
and the backend's invalid-dimension bounds. All fixtures are synthetic/local.

Validation: all-target compilation was warning-free; 282 automated tests passed
(four desktop/device tests remain opt-in). The `backend-contracts`, `mov-media`
and `attachment-media` private-display GUI cases passed with GTK warnings treated
as fatal. The attachment case also verifies that AVI/Matroska files show download
tiles without scheduling unusable media previews.

## Backend contract review (2026-10-19, `e9cbcd1`)

The local backend checkout matches GitHub's `main` at
`e9cbcd1bfa31953c8482e48b2d7bec0b90aab3d7`. Reviewed model/OpenAPI additions,
message services, search/notification queries, media maintenance and session
revocation changes since the preceding GIF integration.

- Attachments now expose nullable `thumbnail_width` and `thumbnail_height`.
  Image/GIF placeholders and video posters reserve that aspect ratio before
  downloading media. The server geometry stays authoritative when a GIF uses
  original bytes. Missing, partial or invalid dimensions use the established
  learned-size/fallback path. Existing inline size limits and adaptive resizing
  still apply, including pinned chat views.
- Search results and REST notifications now carry optional `webhook_id` and
  `external_author`. Sender names, avatars and APP badges render independently
  of observed chat history. Bound search rows and expanded notification groups
  request pictures through the chat's bounded, deduplicated avatar queue/cache;
  public transport isolation and permission filtering remain in force.
- WebSocket notification events keep their earlier shape; older servers can omit
  all new fields. Native users keep their existing attribution and role colors.
- Webhook-avatar garbage collection is a server repair with no new client route.
  Same-IP ban/session revocation closes sockets and invalidates sessions through
  the existing authentication errors; the frontend already handles those errors
  and clears session state. GIF resolution remains backend-provided. External
  role colors were not added to the contract.

```sh
cargo check --locked --offline --all-targets
cargo test --locked --offline -- --test-threads=2
GTK_A11Y=none G_DEBUG=fatal-warnings PAPO_UI_CASE=backend-contracts dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The new private-display case checks media geometry before/after bytes arrive,
responsive resizing, metadata-only webhook search/notices, one shared avatar
request across views, cache reuse and revoked-channel concealment. It uses a
mock server and generated images, without reading a real session.

Validation: all-target compilation was warning-free and 280 automated tests
passed (four desktop/device tests remain opt-in). The focused `backend-contracts`,
`search-recycling`, `notification-window` and `webhook-chat` GUI cases passed with
GTK warnings treated as fatal. The existing `image-geometry` resize case exposed
a horizontal GtkBox measurement inconsistency; using a two-column grid for the
message avatar/body preserves the layout and passes that strict resize case.
The `embeds` and `media-cache` cases also passed with fatal GTK warnings after
the layout correction. The new backend case is included in CI.

## Updated backend contract (2026-10-13, `b015f18`)

The adjacent `papo-backend` update adds explicit channel read acknowledgements,
configurable message page sizes, a public avatar URL for bridge consumers and
quoted Twitter/X rich embed fields. It also restricts another user's password
reset to the server owner. Profile and avatar batch response shapes, authenticated
attachment/media routes, and WebSocket event shapes remain compatible.

The frontend now uses `PUT /channels/{id}/read` with `message_id`, accepting its
empty 204 response. Server channels and DMs are acknowledged only while their
window is active and the history has settled at the newest message. Initial
unread navigation, older pages, cached-page revalidation and background windows
do not mark history read. Repeated ticks coalesce; a failure preserves unread
state and waits at least 30 seconds before another attempt for that channel.
Responses apply the exact submitted cursor, even after channel switching or a
new incoming message. Known notices through that cursor are cleared; notices
outside cached history are confirmed by a coalesced inbox refresh. Late events
for already acknowledged, cached messages do not restore their unread status.

Opening fetches 30 messages; older pages and retained-history validation request
100. Both paths retain exact timestamp/UUID cursors and the existing bounded
history/media caches. The frontend keeps batched profile/avatar loading instead
of creating one public avatar request per member. Rich cards already render the
quote author's name/text and the backend-selected quoted media; regression
fixtures cover both `/embeds` and message-list responses.

Moderators keep ban and audit controls. Only owners see recovery-link controls,
and dispatched/stale recovery actions check ownership again. Ownership changes
close pending moderation dialogs; self-service password changes remain in
Security.

```sh
cargo test --locked --offline
PAPO_UI_CASE=backend-update dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=embeds dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The focused backend workflow uses an isolated X11 display and the existing
`tests/helpers/input_keys.py` helper to give keyboard focus to its named test
window without requiring a window manager. It checks unread navigation,
unmapped-window handling, single read writes without message refetches, failed
writes/cooldowns, successful retry, owner recovery and denied moderator actions.
These checks and the rich-card check run in CI against local mock servers;
they do not use a saved session or contact a real server.

Validated locally: 205 unit/integration tests passed, along with the focused
`backend-update`, `embeds`, `account-notices`, and `emoji-notifications` GTK
workflows. The application also builds with `cargo build --locked --offline`.


## Dialogs, emoji browsing, voice gain and Markdown

`PAPO_UI_CASE=report8` covers the away group,
native formatting/mentions, hidden and revealed animated spoiler images, pin
nicknames/avatars, incremental emoji tabs, right-click channel read with retry,
search Escape and the leaner notification/call dialogs. It runs in CI with
private HTTP fixtures. Existing animated emoji, rich embed, notification and
scroll/cache workflows remain regression checks.

`native_voice_worker_drop_releases_process` now also checks the configured
microphone gain and live, bounded updates on a generated audio source. It never
opens a physical microphone. Windowed viewer checks run without a window
manager; the private Wayland run with `PAPO_NATIVE_FULLSCREEN_TEST=1` additionally
checks control visibility on entry/exit, fitted reset, zoom/pan and retained
original/animated-image quality and cleanup.

The `report8` workflow also covers uploaded images waiting for moderation on
backends without clean-result events: both empty and omitted history status
fields update the current chat and fetch its thumbnail without navigation;
late send echoes cannot restore the stale pending state,
sensitive images stay concealed, and leaving the channel cancels pending checks
and rejects old responses. Unit tests cover the read-only inclusive page cursor,
fallback cancellation and moderation races with live edits, status changes and
deletions. The suite currently passes 218 headless tests.

The moderation event checks count HTTP requests: an acknowledged own upload and
normal completion events perform no message-history GETs; each image thumbnail
is fetched once. Replaying 25 identical events must preserve the texture and row
without another render pass or download. An early completion must survive the
message/POST race. Three unchanged pending messages from an older backend share
one fallback read and cannot schedule further checks for the same batch. Explicit
sensitive/blocked events still conceal or delete media. Fallback work is cancelled
on navigation without resetting its per-message budget during the current visit.

The `embeds` workflow also checks rendered bot sentence geometry. A formatted
sentence with a resolved mention must fit on one line in a wide chat, and must
wrap in a narrow window while staying inside the card. This catches the former
8-pixel text allocation, which checking buffer contents alone would miss. Native
author-avatar geometry checks wait for GTK allocation before comparing sizes.

The `embeds` workflow also checks the bridge author avatar cache across three
channel round trips while avatar decoding is blocked. Resolved avatar-only cards
must render their existing texture without another embed GET or decode job;
missing icons must also avoid repeat requests. The test fills the 256-entry LRU,
checks its 72px pixel budget and verifies release on eviction/session reset and
rejection of old worker results. `retention` checks that large message media still
unloads. The headless suite currently contains 219 passing tests.

## GIF presentation and editing integration

Run with the native GTK dependencies and a private X11 display:

```sh
cargo test --locked
PAPO_UI_CASE=report9 PAPO_NATIVE_INPUT_TEST=1 GTK_IM_MODULE=simple \
  dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The `report9` case verifies arrow/Tab completion for mentions and emojis,
post-navigation type-to-focus including dead-key accents, preserved input in
another editable field, single-line toolbar alignment, compact message editing,
Enter-save/failure/retry, timed download feedback, and inline Klipy/Giphy video
renditions. It also checks small images, absence of competing still-thumbnail
requests/background GTK players, release of animation paintables and reuse of
compressed GIF renditions after a channel switch. All HTTP endpoints are local
mock servers; no real account is used. CI runs the same case.

Related native cases are `image-geometry`, `media-cache`, `smooth-updates`,
`retention`, `multiline` and `full-app-input`. The last two retain real dead-key
keyboard checks when `PAPO_NATIVE_INPUT_TEST=1` is set.

## Webhooks (local fixtures)

`cargo test --locked` covers management routes and payloads, independent
`manage_webhooks` permissions, optional external identities, live message/event
decoding, one-time token redaction, and public avatar transport. Avatar tests
check cookie isolation, API base paths, redirects and oversized responses.

The default native smoke suite also runs a webhook workflow. To run it alone:

```sh
PAPO_UI_CASE=webhooks dbus-run-session -- xvfb-run -a \
  cargo test --locked ui_smoke -- --ignored --test-threads=1
```

This uses a local mock backend and synthetic tokens. It checks on-demand loading,
management errors/retry, duplicate-submit protection, creation, nickname/avatar
updates, sender labels and decoded avatars, live edits, cache reuse across
messages/channel changes, revocation cancellation/confirmation, and permission
loss. It neither reads a saved account nor contacts a real webhook integration.

## Webhook chat identities and avatars

```sh
PAPO_UI_CASE=webhook-chat dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
cargo test --locked --offline webhook_grouping
cargo test --locked --offline discord_avatar_variants
cargo test --locked --offline custom_avatar_budget
```

The local GTK fixture sends pairs of messages from eight external senders through
one webhook. Each pair must share a header while distinct senders stay separate.
All eight pictures must load through the bounded queue, with one HTTP fetch per
resource, normal contrast and no fake interactive profile buttons. Scrolling,
revisiting the channel and switching while decoding is held must reuse images
and pending work. Failed refreshes preserve displayed pictures; loss of channel
access clears the session cache. Unit fixtures check external identity boundaries,
Discord URL cache keys, conservative request pacing and `Retry-After` handling.
No real account or Discord CDN connection is needed.
