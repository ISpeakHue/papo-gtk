# Notifications and unread state

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

## Inbox, unread markers, profiles and GIF integration

```sh
PAPO_UI_CASE=report7 dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=account-notices dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

`report7` checks reuse of 199/200 recent-channel rows, inactive edits/deletions,
unread-marker geometry, the real Up-key controller, draft protection, stale
results, automatic provider GIF animation, duplicate-preview suppression, and
isolated silent video-to-animation fallback/cleanup. `account-notices` checks the native
profile header and narrow layout, save failure/retry and image-update draft
restoration, automatic inbox/page reads, one bulk write for five notices, and
persistence of live events seen before their API records arrive. Both are also
covered by the full mapped workflow.

Set `PAPO_DESIGN_PREVIEW=1` to save profile screenshots under
`/tmp/papo-design-profile*.png`. GIF resolution belongs to the backend; the
`backend-gifs` fixture checks its authenticated media responses without scraping
provider pages or accessing a public account.

Intermittent native GStreamer teardown crashes were observed in the legacy video
stress loop; a passing rerun does not establish that the cause is fixed. See the
[archived crash analysis](../archive/CRASH_ANALYSIS_2026-10-13.md) and
[follow-up investigation](../FOLLOW_UPS.md). Automatic GIF video decoding uses
an isolated worker with bounded output and cancellation cleanup.

## Animated custom emojis and separate notification tabs

Custom GIF, animated WebP and APNG emojis retain their frames and timing at a
32-pixel thumbnail size. Decoding runs on a worker. Cached emoji IDs reuse their
frames across channels and catalog refreshes. Animation pauses when its widgets
are unmapped or outside the scroll viewport, including inline profile text.
The catalog has a 32 MiB decoded animation budget; oversized animations fall
back to a static thumbnail. Each animation is limited to 240 frames and 1 MiB.

The notification window has **Servidor** and **Mensagens diretas** tabs. Only
accessible notices in the selected tab are automatically acknowledged; switching
tabs uses the existing loaded pages and does not make another GET request. Both
tabs share backend pagination. Known hidden conversations keep their notices in
the DM tab, with unavailable message contents concealed. The top-left DM icon
shows an unread message count (capped visually at `99+`), sourced from existing
conversation snapshots and live events, and clears after reading the messages.
Reading notification previews alone does not mark the conversation history read.

```sh
cargo test --locked --offline
PAPO_UI_CASE=emoji-notifications dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=account-notices dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=profile-emojis dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The focused GTK check verifies visible frame changes, offscreen pause/resume,
picker playback, frame reuse and release, startup/live DM counts, duplicate-event
handling, badge clearing, separate rows and read batches, no GET on tab changes,
and classification after hiding a DM. Decoder tests cover GIF, APNG and WebP,
invalid data, downsampling, frame delays and excessive frame counts.

## Reply and mention backgrounds

Incoming replies to your messages use the same persistent colored background and
side accent as mentions. Your own replies and replies to other members remain
unhighlighted unless they independently mention you. Identification uses the
original message in the loaded history, including offscreen messages; an unknown
reply target is resolved when its history page arrives. No extra HTTP requests
or expanded media/history retention are introduced. The current backend sends
only the original message ID in `reply_to`, without its author's ID.

```sh
PAPO_UI_CASE=mention-background dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
cargo test --locked replies_to_the_current_user_are_delivered_without_an_explicit_mention
```

The GTK fixture covers replies, webhook senders, missing/late/deleted originals,
text edits, incremental row reuse, cached channel switches and navigation highlight
expiry, alongside existing mention cases. The notification regression verifies
that replies already count as notifications in mentions-only channels and still
respect muted channels and read acknowledgments.

## Notification channel filters and collapsed channel/sender groups

The server notification tab has an “All channels” dropdown plus each readable
server channel. Filtering uses already-loaded pages and preserves the selected
channel by ID through renames and refreshes. “More notifications” keeps the API's
existing global pagination; switching filters does not issue a new list request.
Unread badges still count all accessible notifications, including filtered rows.

Server notifications are nested under channel headers with the same compact
unread badges as DM sender groups. Channels start collapsed; opening the window
or selecting a channel filter does not mark a collapsed channel read. Expanding
one acknowledges its notifications in a batch, and the badge clears after a
successful write. Expansion state survives filters, refreshes and pagination.
Unread notifications in unavailable channels retain a generic header and never
expose channel names or message previews.

DM notifications are nested in collapsible groups keyed by sender ID, with cached
avatars, names and compact numeric unread badges. Equal nicknames remain separate.
Groups start collapsed when the notification window opens. Opening the DM tab
keeps their messages unread; expanding one sender marks only that sender's visible
notifications read. Its badge clears after a successful acknowledgment, while
new notifications update collapsed senders' badges. Expansion state survives
pagination and redraws. The Direct Messages tab title shows the total unread DM
notification count even while the server tab is selected. It updates on reads
and incoming notifications, disappears at zero, and excludes inaccessible DMs.
Server channel filters do not affect this total. Restricted/hidden conversations
retain generic labels without exposing their previews or author metadata.

```sh
PAPO_UI_CASE=notification-filters dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The fixture checks per-channel filtering/read writes, collapsed channel and DM groups,
duplicate nicknames, filter preservation during pagination and renames, permission
loss, nested message navigation and absence of notification refetches during UI
filter/tab changes. It also reruns the existing server/DM tab and unread-badge
regression. All API requests use local fixtures.
