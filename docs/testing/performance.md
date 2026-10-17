# Caching, scrolling and performance

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

## Recent-channel caching and loading order

Recent conversations are cached in memory for the authenticated session: up to
8 inactive channels, 200 recent messages per channel and an estimated 16 MiB of
message metadata. Encoded image payloads are excluded. A cache hit displays
retained messages while the normal paced API refresh revalidates the retained
range, including missed edits and deletions. Transient refresh errors keep the
cached conversation readable. Denied access clears retained content; access
snapshots prune unavailable channels, live messages/edits/deletions update cached
conversations without rebuilding the active chat, and reconnect clears inactive
snapshots. Deletion tombstones are bounded; conversations exceeding that budget
are discarded. Nothing is persisted to
disk or reused by another login.

Lightweight media metadata, learned dimensions and recent first-frame previews survive
channel switches. A separate resident cache retains the latest 30-message batch
for up to eight channels, with a 128 MiB/256-entry bound. The active cache holds up to 64 image textures
with a 64 MiB RGBA-pixel budget. GIF animation has a separate budget. Known
image dimensions size placeholders. When dimensions are unknown, the first
decoded image replaces the guessed frame ratio immediately, without requiring
a channel revisit. Layout changes wait until scrolling pauses and preserve the
reading anchor or latest-message position before painting. Learned dimensions
are reused for unloaded placeholders and later visits. Server emoji textures remain shared
across channels for the session.

Downloads and off-thread decoding now continue during scrolling, using the
bounded nearby scope and recent opening batch. Only presentation waits for idle.
Obsolete preview jobs release their slots without removing displayed widgets,
changing placeholder dimensions or cancelling explicit media viewers. The
`scroll-media` case checks a real local image request and decode during an active
gesture, deferred mounting, uninterrupted scrolling and anchor preservation.
`media-loading` additionally blocks the local poster worker and verifies that
video server thumbnails and still images load anyway, even with eight preceding
videos lacking thumbnails. At most two fallback posters may queue; cached
poster decodes are exempt from that transfer/extraction admission limit.

The headless cache tests cover retention limits, eviction, oversized histories,
permission pruning and cancelled-request/live-deletion races. The mapped GTK
workflow additionally checks a 700 ms HTTP refresh: cached messages become visible
before it completes, missed edits/deletions reconcile, a failed refresh retains
content, and HTTP 403 clears it. The existing media/emoji regression also asserts unchanged painted row heights
after initial image completion and no chat rebuild for an unchanged emoji catalog.
The startup mock asserts that message requests
precede member, DM, notification and emoji requests. Run the focused cache check:

```sh
PAPO_UI_CASE=channel-cache dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

## State consistency and bounded background work

The default suite now exercises independent HTTP/live-history journals, complete
snapshot removals, out-of-order member generations, expiry-aware renewal,
503→200 recovery, lost refresh bodies, owner-scoped remembered-session cleanup,
WebSocket acknowledgment deadlines, stalled handshakes and cancellable writes.
These use ephemeral local servers and the test keyring, never a saved real session.

The existing single-thread GTK `ui_smoke` workflow also checks:

- Actual scrolling through more than 64 media images, automatic reload after
  eviction, a stationary viewport with no refetch loop, shared previews after
  deleting their initiating message, and HTTP-only viewer/video/file cleanup.
- A 500-member list retaining its rows across 200 status events, and one
  coalesced DM follow-up after 50 refresh triggers.
- Background avatar preparation while GTK dispatches callbacks, a 72-pixel
  member-avatar limit, the 1,024-texture pixel budget and stale-response rejection.
- Histories of 100, 1,000 and 5,000 messages with 500 custom emojis. Message
  widgets stay capped at 200 plus date/spacer rows. Reaction, image completion
  and new-message updates format only affected rows. Direct navigation, scrolling
  into spacers, older reading anchors and reconnect deletions remain usable.

Run the GUI workflow on a test display with:

```sh
cargo test --locked --offline ui_smoke -- --ignored --test-threads=1 --nocapture
```

Its `chat benchmark` lines report elapsed reaction-update time, formatted-row
count, retained rows and process RSS. Timing/RSS are observations; CI asserts
bounded work and widget identity instead of hardware-specific millisecond limits.
A real busy-server profile and a long, multi-user native voice/video call remain
manual checks. Reconnect revalidates retained pages sequentially through the
shared request budget while keeping their rows visible.

## Startup image performance

The mapped GTK workflow applies 512 member pictures in 32 batches with 200 chat
rows. It asserts zero message formatting/history renders and stable chat/member
row identity. It also verifies image removal and cached pictures on new messages.
The sidebar checks DM/voice row identity, a 2048-pixel server icon reduced to at
most 64 pixels, duplicate icon requests, and stale responses after replacement or
removal. Holding both decoder permits confirms that the UI can present and accept
updates while image preparation waits. No remote server requests are involved.

```sh
PAPO_UI_CASE=startup dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The synthetic benchmark reports avatar update time for observation; tests assert
work counts and widget identity instead of a machine-specific latency threshold.
Actual startup on a remote server also depends on network and server latency.

Local validation: 167 Rust tests passed, the focused startup checks passed, the
complete GTK workflow passed in 138.96 seconds, and the application build passed.
The full workflow's 512-avatar benchmark took 181 ms across all 32 batches, with
zero history renders, zero formatted message rows and zero replaced member rows.
This measures synthetic avatar dispatch, not total remote-server startup time.

## Scrolling while images load

The mapped workflow injects an image completion during a real wheel animation.
It checks advancing scroll frames, an unchanged placeholder, zero hydration
passes and no media render during movement. An unrelated live reaction preserves
the animation and deferred picture. The final animation destination accounts for
layout changes; after idle, the image appears automatically and retains the
reading anchor. Repeated adjustment changes exercise pixel/touchpad-style
scrolling. Explicit Latest navigation still takes priority, and permission
revocation prevents an idle callback from restoring an image.

```sh
PAPO_UI_CASE=scroll-media dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The full workflow also checks image-cache eviction/reload, stationary viewports,
shared images, history pagination/navigation and media permission/cancellation.
The wheel-cancellation fixture extends only its test gesture to one second so
software GPU rendering cannot finish the animation before the assertion. The
application's wheel gesture remains 160 ms; cancellation and deferred media
mounting are still checked while the test animation is playing.
On a real desktop, scroll continuously past uncached pictures with both wheel
and touchpad, then pause: placeholders may remain during movement, and images
should appear afterward without cancelling motion or pulling the reading position.

Local validation: 167 Rust tests passed, the focused scrolling checks passed, the
complete GTK workflow passed in 141.54 seconds, and the application build passed.

## Stable channel opening

The mapped workflow observes the frame clock after painting and checks every
visible opening frame, rather than only the final adjustment value. It covers
initial history, switching away from an older reading position, wrapped text
after narrowing the window, and an unread boundary that needs an older page.
Intermediate unread pages remain concealed while their rows are allocated.
An opaque overlay matching the chat background covers the history during this
transition. The scroller stays fully opaque and mapped, so GTK can prepare the
new rows without reviving a stale transparent frame. The regression renders
covered frames to textures and checks that old text, images and scrollbars
cannot show through in light and dark themes. Queued input from a previous
channel cannot reveal the new channel before it settles; input in the current
channel still cancels the gate. Each selection replaces the previous scroll
viewport's render node while retaining the scroller, controllers and list.
Selection and its permissions arrive together, so changing channel permissions
does not briefly redraw the old chat.

The reveal is committed after GTK allocation and anchor correction. A late
row-size change on the reveal frame reproduces a misplaced painted frame with
the former pre-layout reveal, and passes with the corrected order. On X11 the
test also reads actual pixels from its private test window, without forcing a
fresh GTK snapshot. Distinct channel colors must never reappear after the cover
or the next channel has reached the display, across cached and uncached visits.
A separate fixture colors the upper and lower parts of one history differently.
It checks the list's allocated transform against the adjustment and reads the
private window's pixels while a preceding row grows on the reveal frame, for
both latest and unread destinations. It also grows the row after opening while
following the bottom. This catches an adjustment that is correct while GTK
still draws an earlier portion of the same channel. The layout hook requests
viewport reallocation when these positions disagree and keeps the opening
cover until the transform matches; it does not add a timed loading delay.
This pixel check uses libX11 only in tests; Wayland runs retain the allocation
and rendered-texture checks.
Updates arriving every frame must not restart presentation indefinitely. Rapid
switches reject the old response; empty history, request errors and explicit
reader input all release presentation. Downloads are not required for these
checks and no remote server requests are made.

```sh
PAPO_UI_CASE=channel-opening dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

On a desktop, log in and switch between read and unread channels with long
messages. History should first appear at its intended position without showing
an intermediate scroll location or a flash of the previous channel. Images can
still load afterward.

The follow-up transition update passed 172 Rust tests, the application build,
and the complete mapped GTK workflow (173.23 seconds). Actual displayed-frame
checks passed with Cairo and Vulkan; the Vulkan opening case took 20.80 seconds.
Temporarily restoring the former pre-layout reveal made the new late-allocation
regression fail with a painted position of 2754 instead of 3219. The corrected
source was restored before final validation. The earlier opaque-cover checks
also passed with OpenGL, light/dark themes and stale input rejection.

Local validation: 167 Rust tests passed, the focused painted-frame checks
passed, the full GTK workflow passed in 144.57 seconds, and the application
build passed. The GTK run includes the scrolling/media, send, playback,
pagination, reconnect and permission regressions.

## Smooth live updates and bounded media work

The painted-frame regression replaces a tall image placeholder above the reader
with a shorter decoded picture. Before correction, one frame shifted the reading
row from 120 to -80 pixels relative to the viewport. Every sampled frame must
now retain the original offset. The existing wheel regression also verifies that
geometry compensation preserves the running animation and its destination.

The 5,000-message regression dispatches 100 stationary viewport updates: they
must do zero full-history metadata normalization and inspect at most 200 media
candidate rows per update. Reactions and plain live arrivals retain that metadata
cache; a fresh embed still invalidates it and displays its changed title. A
headless window-selection regression checks that small reading movements retain
the current window, edge movement recenters it and Latest reaches the final row.

```sh
PAPO_UI_CASE=smooth-updates dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

This focused case also runs the opening, active media-scrolling and large-history
checks. Assertions concern painted positions and work counts, rather than
machine-specific frame-time thresholds. It uses synthetic history and local
fixtures, without sending requests to the production server.

Local validation: 168 Rust tests passed, the focused smoothness checks passed,
the full GTK workflow passed in 149.53 seconds, and the application build passed.
The opening stress test records the update count at the first visible painted
frame, so later main-loop dispatch cannot misreport presentation latency.


## Older-history and media retention

After scrolling settles, media from distant older history is released, including
pictures held by still-mounted message rows. A 30-message margin around the
viewport avoids discarding pictures during short scrolls, while the latest
30-message batch keeps its previews resident within the byte limits.
Reserved media geometry prevents unloading from changing scroll positions.
Returning to the latest messages retains the newest 200 messages and enables
pagination to fetch older pages again. Channel switches release full animations
and players, retain recent first-frame previews, and exclude older pages from inactive row caches. Open image viewers and
fullscreen video are preserved while the user is viewing them.

```sh
PAPO_UI_CASE=retention dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The check uses weak GTK references to verify that image textures, GIF paintables
and older rows really lose their owners. It also checks late-decode rejection,
bottom positioning, older-page reload, and shared assets used by nearby messages.
The Rust suite verifies history trimming rejects stale pagination while retaining
live edits. Freed allocations may remain in allocator or renderer pools, so process
RSS need not fall immediately after each eviction.

## Performance and UX regressions

The suggestions implementation keeps local fixture scenarios separate so failures
identify the affected behavior. Run each with a private display/session, for example:

```sh
PAPO_UI_CASE=avatar-viewport PAPO_PERFORMANCE=1 dbus-run-session -- xvfb-run -a \
  cargo test --locked --offline ui_smoke -- --ignored --test-threads=1 --nocapture
```

Cases added or extended:

- `avatar-viewport`: 100/5,000 members, viewport/overscan demand, scrolling and
  hidden-pane behavior; large rosters recycle rows. Both roster sizes also check
  the shared chat/member wheel animation, intermediate positions, accumulated
  wheel steps, click/drag interruption, unmap cleanup, reduced motion and bounds.
- `search-recycling`: pagination overlap, stable model objects, bounded factory
  widgets, mention rendering, navigation and stale/cancelled responses.
- `account-settings`: mounted profile saves, shared shortcuts, retained drafts,
  Save/Discard/Cancel on close, local motion preference and password cleanup.
- `emoji-notifications`: animation timing, clipping, unmapping, returning to view,
  live reduced-motion changes, cleanup and notification tabs.
- `notification-window`: a recoverable 1,000-notice paging window, continued older-page
  loading and return to the latest pages, without constructing
  rows in collapsed groups or replaying read/desktop events.
- `notification-filters` / `account-notices`: lazy collapsed groups, read batching,
  retries, permissions, pagination, sender/channel counts and profile workflows.
- `webhook-chat`: sender grouping, concurrent avatar queues, reuse across channels,
  rejected stale completions and failed-refresh pixel/freshness preservation.
- `recovery`: disconnect/reconnect status, readable cached messages, unsent drafts,
  transient permission-refresh failures, visible-avatar revalidation and idle polling.
- `smooth-updates`: continuous reaction bursts, delayed allocation, image completion,
  scroll/reading anchors, opening cover and bounded 5,000-message history.
- `perf-baseline`: just the bounded-history measurement, suitable for repeatable
  `cargo test --release` samples without other fixture windows affecting RSS.

`PAPO_PERFORMANCE=1` prints aggregate diagnostics on normal application/test exit.
See [PERFORMANCE.md](../../PERFORMANCE.md) for metric definitions, recorded samples and
conditional work. No public server or saved account is needed for these cases.

## Media loading pipeline

```sh
PAPO_UI_CASE=media-loading dbus-run-session -- xvfb-run -a \
  cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

This synthetic case supplies thirty cached thumbnails and verifies that the next
uncached attachment is still requested, without re-downloading those thumbnails.
It then holds the full-animation worker and checks that two GIF first frames and
a still image are displayed before full animation decoding can finish. Releasing
the worker completes both animations, drops temporary first-frame textures and
makes no extra media requests. `media-cache`, `backend-gifs`, `gif-embeds`,
`scroll-media` and `smooth-updates` cover cache reuse, bot renditions, scrolling,
layout stability and bounded retention around this pipeline.

First-frame decoding and full animation preparation run off the GTK thread.
Full GIF decoding shares a single animation gate and the existing two decoder
slots; waiting animations do not reserve thumbnail workers. Blocking decoders
retain their permit until the work finishes, even when the requesting future is
cancelled. Network pacing and concurrent download limits remain unchanged; the recent
preview/cache budgets below apply. Video poster extraction releases its download slot once the
preview bytes have arrived, while its single poster worker remains bounded.

## Recent opening media batch

```sh
PAPO_UI_CASE=recent-media dbus-run-session -- xvfb-run -a \
  cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

Text and media share `INITIAL_MESSAGE_LIMIT = 30`. Visible/nearby media gets first
priority, then the rest of the latest 30 messages prefetches without scrolling.
Recent static previews and video posters remain resident for up to eight channels
(128 MiB/256 entries globally). Full GIF frames and video players remain scoped
to the active view; prefetched GIFs prepare just a first frame and resume full
animation when brought into view. The separate 96 MiB/512-entry compressed cache
evicts older scroll history before recent opening batches, within its hard bound.
These are demand-allocated ceilings, not memory reserved at startup.

The native fixture checks a complete 30-image preload, texture identity after
scrolling nineteen messages up and down, channel return with both decoders held,
offscreen GIF first-frame promotion without downloading again, byte/channel
eviction, shared-owner deletion, moderation
and revoked access. Older media keeps normal lazy loading and bounded eviction.
