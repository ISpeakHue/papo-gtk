# Performance measurements and diagnostics

Date: 2026-10-15. Source baseline: commit `79e2502` plus the existing account and
webhook changes, copied before the suggestions implementation. Measurements use
synthetic accounts and private loopback servers; no real session was read.

## Recorded samples

The bounded-history fixture used an optimized Cargo release build, private Xvnc,
X11 and Cairo, with the same runner/display size before and after. These are
single runs, with allocator and timing noise. The after sample also enabled the
new aggregate diagnostics, which were absent in the baseline. These samples do not establish percentage
improvements or predict Wayland, accelerated rendering, network or AppImage timing.

| Loaded messages | Before reaction update | Final reaction update | Mounted rows, both |
| ---: | ---: | ---: | ---: |
| 100 | 8,520 µs | 9,728 µs | 102 |
| 1,000 | 11,011 µs | 6,232 µs | 201 |
| 5,000 | 11,406 µs | 6,591 µs | 201 |

These samples retain one formatted row per reaction; the final build retains that
row's text/media widgets and updates its reaction pills in place. The larger
histories have lower measured update times in this run, while the 100-message
sample is slower. This does not establish a uniform improvement or a meaningful
growth with history size.
This is why a larger message/resource index and height-prefix-cache rewrite were
not introduced. Retain the 5,000-message coverage and profile a real workload
before adding those invalidation paths.

Process RSS was about 132/156/177 MiB before and 133/156/179 MiB after. RSS includes the runtime,
GTK, allocator, fixture and caches; it is not a measurement of thumbnail-cache
bytes or a guarantee that an allocator immediately returns freed pages to Linux.

The 100/5,000-member viewport fixture initially measured roughly 25 ms/1,161 ms
constructing the original list in a debug build. With the large-roster recycled
path, one run measured roughly 25 ms/12 ms. The small roster initially offered
22 avatars; the recycled large roster offered 19, and 20 at the opposite end.
This measures component construction, not full first paint. Factory widgets remain
bounded by GTK's viewport recycling rather than the number of server members.

The original release smooth-opening fixture failed its first-presentation timing
assertion during a continuous reaction burst. It was not counted as a pass.
Reaction updates now retain the existing message text/media widget and reconcile
only reaction pills; the smooth-update GUI fixture passes with that change.

## Aggregate diagnostics

Run an optimized build with `PAPO_PERFORMANCE=1 cargo run --release --locked` and
close the application normally to print aggregate JSON. Normal launches do not
print it. Labels are fixed in source; there are no URLs, IDs, credentials,
cookies, headers, message text or profile text in these diagnostics.

Samples expose count, total and maximum, rather than unreliable percentile
estimates from a tiny run:

- `request_us`: HTTP attempts and elapsed time; `request_queue_us`: admission wait.
  Retry attempts are counted separately. `rate_limits` counts HTTP 429 responses.
- `chat_update_us`, `inbox_update_us`, `search_update_us`: main-thread handlers.
- `notification_rows_built`, `search_widgets_built`, `member_widgets_built`:
  construction counts; distinguish retained model records from mounted widgets.
- `encoded_cache_hits`, `encoded_cache_misses`, `external_avatar_hits`:
  session-cache reuse. `encoded_cache_bytes` is sampled cache accounting, not RSS.
- `frame_paint_us`: elapsed time between GTK before/after-paint signals.
  `frame_budget_us` comes from that display's refresh interval;
  `frame_paint_over_budget` counts sampled paint work exceeding that interval.
  These are **not** GPU/compositor presentation or missed-frame measurements.
- `animation_frames`: advancing custom animation frames. Clipped animations have
  no independent polling timer; reduced motion presents the first frame.

Use the same scenario, renderer and window size for comparisons. Cover cold login,
cold channel, warm revisit, older image history, notification bursts and reconnect.
Record process RSS separately before/after returning to latest messages. GTK
fixtures print bounded-history/member samples and report aggregate diagnostics on
exit when opted in. The existing channel-cache and media-cache cases validate
request reuse and memory bounds independently of elapsed timings.

## Retention and refresh policy

- Observation cache: 512 compact records, additionally limited to 4 MiB.
- Inbox paging: after at least 1,000 loaded rows (the final page can cross the
  threshold), the older-pages action explicitly releases the current persisted
  pages and continues from the older cursor. It does not truncate older history.
  A latest-pages action restarts from the head. Head refresh at the limit resets persisted
  pages. Eviction does not acknowledge notifications or trigger desktop alerts.
  Counters describe loaded unread notices, not an authoritative server total.
- Avatars: retain existing small-picture limits and batching. Reconnect expires
  freshness without discarding pixels; viewports drive revalidation.
  Restored chat surfaces reattach cached external-author pictures as well as native
  avatars, without another download. Mutable external avatar routes expire on
  reconnect; immutable Discord hash resources keep their pixels. External
  origins share a bounded table of budgets, and never lose a live cooldown through
  eviction. Failures have separate retry deadlines and manual retry actions.
- Access/DM fallback: 30–35 seconds while active/disconnected; 90–95 seconds when
  inactive with a healthy socket. Per-session jitter avoids identical retry phases.
  Permission events and reconnect still request authoritative snapshots.
- Media: the active 64-texture/64 MiB and 48 MiB animation limits remain.
  The follow-up recent-preview policy below adds a 128 MiB/256-entry resident
  cache and raises compressed retention to 96 MiB/512 entries. No blanket
  loading delay was added.

## Conditional work

A disk thumbnail cache remains deferred. Repeated launches necessarily lose
session-only bytes, but there is no representative measurement showing enough
saved requests/time to justify persisting private media. Gather repeated-launch
cache-miss/traffic data first. If worthwhile, implement account/server namespaces,
resource revisions, asynchronous I/O, expiry, clear/logout controls and deletion,
moderation and permission-loss invalidation together.

External author colors, conditional backend snapshots and exact notification
counts require backend contract changes. The frontend does not infer them.

## Validation completed

Rechecked on **2026-10-16**: the all-target build, 257 automated tests, and the
focused `account-settings`, `notification-window`, `recovery`, `avatar-viewport`
and `search-recycling` GTK cases passed. The full GTK suite and remaining focused
cases below passed during the implementation on **2026-10-15**.

- `cargo check --locked --offline --all-targets`: passed without compilation warnings.
- `cargo test --locked --offline -- --test-threads=2`: 257 passed, five display/native
  integration cases ignored by that command.
- Full existing GTK smoke/workflow suite: passed on the private X11/Cairo display.
- Focused GTK checks passed for roster viewport demand, search recycling, settings
  drafts/close choices, profile reconciliation and emoji descriptions, status
  writes, notification filters/read batching/page windows, shared animation
  scheduling, webhook avatar restoration, reconnect ordering, channel caching,
  native composer input and smooth updates.
- Optimized bounded-history fixture: passed; samples are recorded above.
- `git diff --check` and local documentation reference checks: passed.

These local tests use synthetic backend data. They do not replace testing the
AppImage on real GNOME/KDE hardware, a live server or a real voice call.

## Media loading changes (2026-10-16)

The hydration pass previously counted already-cached attachments and inspected
GIF links against its 24-item admission budget. Enough cache hits could therefore
prevent a later uncached image from ever being requested. The budget now counts
admitted work, allowing the pass to reach uncached media while keeping the same
request pacing and download limits. The later residency policy below expands
media demand and retention to the latest opening batch.

GIF previews publish a bounded first frame before decoding all animation frames.
Only one full animation prepares at a time within the existing two decoder slots,
leaving thumbnail processing available alongside it. Cancelled blocking work
keeps its permit until it actually completes. Video poster extraction no longer
holds a download slot during local subprocess processing.

These changes remove identified scheduling/presentation delays; they do not
reduce server response time or establish a percentage speedup on a live server.
The `media-loading` fixture checks presentation while full animation decoding is
blocked, cache-hit admission and temporary texture disposal. Existing media/cache
and scrolling fixtures provide separate regression coverage.

Additional opt-in aggregates: `media_decode_wait_us`, `media_decode_us`,
`gif_decode_wait_us`, `gif_decode_us` and `media_first_frames`. They measure local
work and queue waits/counts, with the same fixed-label privacy limits described
above; they are not network-to-presentation or compositor timings.

Validation: warning-free all-target compilation, 252 automated tests passed, and
six GTK cases passed: `media-loading`, `media-cache`, `backend-gifs`, `gif-embeds`,
`scroll-media` and `smooth-updates`. The first-frame unit regression also checks
pixel equivalence and rejects malformed, oversized and excessive-dimension input.

## Recent preview residency (2026-10-16)

The previous policy discarded every decoded picture outside the viewport's nearby
scope. Returning after a short scroll or channel switch could therefore show a
placeholder and repeat decoding even when compressed bytes were cached.

The latest 30 messages now share the same opening-page constant as text loading.
Their media prefetches after visible resources using the existing paced requests
and four download slots. Only media is prefetched; offscreen member/webhook
avatars and author-only cards continue to follow viewport demand.

Recent thumbnails, posters and GIF first frames share a 128 MiB/256-entry resident
cache across up to eight channels. Existing active preview limits still apply;
common texture references are shared rather than copied. Full animations and
players are not retained across channel switches. Offscreen prefetched GIFs
prepare only a first frame; entering the viewport promotes them to animation.
Decoded older pictures have a 30-message scroll margin, then unload normally.

The compressed cache grows from 32 MiB/256 entries to 96 MiB/512 entries and
prefers recent opening batches over older scroll history. Both caches enforce
hard ceilings, so an exceptionally large batch can still cause eviction. They
allocate on demand and clear on access revocation or session reset; deletion and
moderation invalidate retained media. No private media is written to disk.

`resident_preview_hits` and `resident_preview_bytes` extend the opt-in aggregate
diagnostics. The `recent-media` fixture verifies reuse of the same texture with
decoding blocked, as well as preloading, short scrolls and invalidation. These
checks demonstrate avoided work, not a measured live-server speedup.

Validation: warning-free all-target compilation; 253 automated tests passed.
Eleven GTK regressions passed: recent-media, media-cache, media-loading,
backend-gifs, gif-embeds, retention, channel-cache, scroll-media, smooth-updates,
webhook-chat and webhooks. Recent-media additionally verifies that a prefetched
GIF resumes animation without another download; smooth-updates checks bounded
discovery over larger histories. Older retention fixtures now distinguish recent
preview residency from the full animations, players and older pages that unload.

## Web frontend comparison and media scheduling (2026-10-16)

Reviewed the public Papo-Chat/papo-frontend sources. Its `main` at
`f4ee655e482bfc0e27473895734d5e0c352e0ed0` has a placeholder channel route;
the implemented UI was inspected on `aero-webhooks` at
`42c87b5226cdd63c9cf422d0ee2a6080f5f19690`, alongside the published
`perf/channel-media-render` branch at
`3db77ac6a02231ea7217dfc4639335fe5cfdb57a`. This is a source comparison,
not a measurement of its deployed frontend or the user's live server.

| Area | Web frontend evidence | GTK finding/change |
| --- | --- | --- |
| Image demand | [Attachment.svelte](https://github.com/Papo-Chat/papo-frontend/blob/42c87b5226cdd63c9cf422d0ee2a6080f5f19690/src/lib/components/Attachment.svelte) uses native lazy image loading, thumbnails for stills and originals for GIF animation. | GTK already requests thumbnails and preserves GIF animation, but previously started no hydration while scrolling. Bounded background fetching/decoding now proceeds during motion. |
| HTTP admission | [media.ts](https://github.com/Papo-Chat/papo-frontend/blob/42c87b5226cdd63c9cf422d0ee2a6080f5f19690/src/lib/utils/media.ts) uses authenticated browser fetch without an application-level 125 ms interval. | GTK's interval delayed even the first four available download slots. A shared bucket now permits four idle credits and refills at eight starts per second. A 429 disables bursts for the session and imposes the same shared Retry-After pause as before. |
| Reuse | [Performance branch media.ts](https://github.com/Papo-Chat/papo-frontend/blob/3db77ac6a02231ea7217dfc4639335fe5cfdb57a/src/lib/utils/media.ts) reuses blob URLs and offers stable resource keys to avoid rehashing large base64 payloads. | GTK already keys compressed bytes, resident textures and in-flight work by resource UUID. The prior recent-30-message/eight-channel residency remains; cache limits were not increased for this change. |
| Video work | Attachment.svelte uses server thumbnails and starts audio/video sources on explicit interaction. | GTK additionally generates bounded posters when a server thumbnail is absent. Server thumbnails now bypass the local poster worker; pending fallback poster work is limited to two admissions so it cannot occupy all eight image request entries. Actual players remain explicit, with the existing single poster worker and range/timeout limits. |

The request policy stays below the local backend's default sustained limit of
10/sec and default burst of 20; remote configuration may differ, so Retry-After
remains authoritative. All clones, metadata and media share the same budget.
Public avatar origins retain their stricter one-request-per-500-ms pacing.
FIFO admission, cancellation without reserving future slots, and the prohibition
on replaying mutations remain covered by tests.

A deterministic Tokio-clock comparison isolates **scheduler admission delay**:

| Ready requests after idle | Previous last admission | New last admission |
| --- | ---: | ---: |
| 4 | 375 ms | 0 ms |
| 12 | 1,375 ms | 1,000 ms |

These numbers exclude server latency, connection setup, transfers, decoding and
GTK presentation. They show removed artificial waiting, not a measured browser
speed comparison or a promise that a live image appears in zero milliseconds.

During motion, obsolete preview jobs are cancelled to free transfer/decoder
slots for nearby images; recent opening-batch requests and explicit players or
image viewers are protected. Already displayed widgets and learned dimensions
are unchanged. Completions collect dirty resource IDs and mount after idle,
preserving the reading anchor and wheel animation. Unchanged scroll ticks do
not repeat full-history embed normalization; discovery stays bounded to mounted
rows and the recent opening batch.

The `media-loading` regression also holds the poster worker busy while a message
contains eight videos without thumbnails, one server-provided video thumbnail
and a still image. Both available images must arrive before the worker is freed,
and no more than two missing-thumbnail jobs may remain queued. Cached posters
bypass that admission limit because they need no extraction or transfer.

Validation: all-target compilation passed without warnings; 257 automated tests
passed, with four native/display-dependent tests ignored by the normal command.
Eight focused GTK cases passed on the isolated X11/Cairo display: scroll-media,
recent-media, media-loading, media-cache, retention, channel-cache, smooth-updates
and recovery. After the poster changes, media-loading, media-cache, recent-media,
scroll-media and smooth-updates were rerun and passed. The extended media-loading
case verifies the blocked-worker scenario above; the scheduler tests use a
virtual clock for burst/refill, late cooldown and cancelled-waiter checks.

## Member list scrolling (2026-10-16)

The member pane now shares the chat's mouse-wheel easing implementation:
160 ms with an ease-out cubic curve and accumulated destinations for consecutive
wheel steps. This applies to both the small ListBox roster and the recycled
ListView used for 1,000 or more members. The controller belongs to the containing
ScrolledWindow, so changing roster implementations does not lose it.

Touchpad/pixel scrolling stays with GTK. Clicks, scrollbar drags and navigation
keys stop a running wheel animation without consuming their normal events.
Hiding the pane or dropping the component releases the animation, and libadwaita
honors GTK's disabled-animation setting. Existing coalesced avatar viewport
demand and row recycling remain in place.

Validation: warning-free all-target compilation; the extended `avatar-viewport`
case passed for 100 and 5,000 members. The shared chat implementation also passed
the `scroll-media` and `smooth-updates` GTK regressions on the private display.
