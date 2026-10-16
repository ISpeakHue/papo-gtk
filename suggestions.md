# Performance and UX suggestions

Reviewed: **2026-10-15**, against commit `79e2502` and the current working tree,
including tabbed account settings and the latest webhook avatar/grouping fixes.

Implementation updated **2026-10-15**. The original review below is retained as
rationale; the status table records the resulting changes. Measurements and their
limits are in [PERFORMANCE.md](PERFORMANCE.md), with fixture commands in
[TESTING.md](TESTING.md#performance-and-ux-regressions).

## Implementation status

| ID | Result |
| --- | --- |
| S00 | Added opt-in aggregate request, queue, cache, UI-update, animation and display-budget diagnostics; recorded release fixture samples. These are local fixtures, not distributed-AppImage/compositor benchmarks. |
| S01 | History notifications now carry compact observations; retained metadata is limited to 512 entries and 4 MiB. Media payloads are excluded. |
| S02 | Hidden inboxes update state/badges without building rows. Group headers persist; only expanded groups on the selected tab build/reconcile rows. Persisted pagination has a recoverable 1,000-row window. |
| S03 | Avatars follow visible chat/member rows and overscan. Rosters of at least 1,000 members use recycled widgets; smaller rosters retain their incremental list. |
| S04 | Search uses an ID set, one mention lookup per render and a recycled list; pagination retains unchanged records. Hidden search windows skip rendering. |
| S05 | Avatar results irrelevant to the mounted chat populate the cache without scheduling chat rendering. Reaction updates retain the message body and update pills in place. Larger history indexes/height aggregates remain conditional: current bounded-history samples do not establish a growing bottleneck. |
| S06 | Public avatar budgets are bounded per origin; typed failure/retry state is separate from successful-image freshness. Failed refreshes retain old pixels. A message action retries failed external avatars while respecting cooldowns. |
| S07 | Reconnect invalidates avatar freshness and revalidates visible authors/members, rather than forcing a full-roster image reload. Healthy inactive sessions poll at 90–95 seconds; active/disconnected sessions retain a 30–35-second fallback. Event-driven permission verification remains immediate. |
| S08 | Accepted profile changes reconcile into the mounted form field by field with a generation token and draft comparison. Text saves avoid forced avatar HTTP requests. Image updates preserve text drafts. |
| S09 | Own-profile/preferences shortcuts use the shared tabs. Dirty forms enable Save; closing offers Save/Discard/Cancel. Tab changes retain drafts; closing clears security passwords. |
| S10 | Local motion setting: system, always animate, or static first frame. Shared scheduling replaces per-animation offscreen polling, with scroll/layout wakeups and live preference changes. The local option is not sent to the backend. |
| S11 | Nonmodal connection/recovery/cooldown status preserves cached history and drafts. Transient access-refresh failures retain the last authorized view; explicit authorization/session failures retain existing revocation behavior. Recovery never replays message sends. |
| S12 | Visible scope text, tooltips, accessible badges and empty states explain loaded-page counts and filters. Exact channel/sender totals still require backend support. |
| S13 | Evaluated and deferred as specified: there is no measured repeated-launch traffic reduction that justifies disk persistence/invalidation yet. Existing in-memory byte limits remain unchanged; new diagnostics support that decision later. |

No new backend routes, role-color fields or server settings were invented. External
role colors, conditional snapshots and authoritative aggregate notification totals
remain backend-dependent.

## Keep the improvements already present

Chat already limits materialized rows to 200, prepares images off the GTK thread,
defers heavy media work during scrolling, restores reading anchors, and reuses
recent conversations and media. There are separate bounds for conversation
metadata, compressed media, textures, animations and avatars. Webhook pictures
now use a deduplicated queue, two active downloads and paced requests. Account
tabs already retain drafts while switching between mounted pages.

Do not replace these paths with a blanket loading delay, eagerly preload every
channel, increase request concurrency to hide latency, or retain all decoded
images indefinitely. The remaining opportunities are more specific.

## Priorities

P1: address first because the code shows avoidable memory or GTK work. P2:
valuable responsiveness or usability work. P3: optional work whose benefit
depends more heavily on usage and measurement. These are implementation
priorities, not measured severity ratings. Effort is relative: S/M/L.

| ID | Priority | Suggestion | Effort |
| --- | --- | --- | --- |
| S00 | First | Establish a release-build performance baseline | S |
| S01 | P1 | Keep compact message metadata in the notification cache | M |
| S02 | P1 | Update notification groups incrementally and build expanded rows lazily | M |
| S03 | P2 | Load member avatars according to the actual viewport | M/L |
| S04 | P2 | Reuse search result rows and build mention lookup data once | S/M |
| S05 | P2 | Reduce the remaining whole-history work in chat updates | M |
| S06 | P2 | Isolate external avatar cooldowns by origin and retain failure reasons | M |
| S07 | P2 | Make reconnect reconciliation and periodic refreshes more selective | M/L |
| S08 | P2 | Refresh a saved profile in place | M |
| S09 | P2 | Unify account entry points and protect unsaved settings | M |
| S10 | P2 | Add motion controls and reduce offscreen animation polling | M |
| S11 | P2 | Make connection and recovery state visible | S/M |
| S12 | P2 | Clarify notification counts while more pages remain unloaded | S |
| S13 | P3 | Consider an optional bounded disk thumbnail cache | L |

## S00 — Measure the production path before changing cache sizes

**Evidence:** [performance fixtures](src/ui/chat/performance.rs),
[release configuration](Cargo.toml), [logging protections](src/logging.rs).
The existing GTK fixtures check responsiveness and bounded work, but a private
display with the Cairo renderer is not a measurement of the distributed
AppImage on a friend's desktop. `cargo run` also defaults to a debug build.

**Proposal:** capture a repeatable release-build baseline for cold login, a cold
channel, a warm channel revisit, older image-heavy history, a notification burst,
and reconnect. Record request counts, queue wait time, cache hits, GTK update
time, missed frames and memory before/after returning to the latest messages.
Separate memory retained intentionally by bounded caches from retained widgets
or payloads. Treat cache byte estimates and process RSS as different measures.

Add opt-in aggregate counters and timings without URLs, message text, IDs,
cookies or tokens. For frame timing, compare with the actual display's refresh
deadline; 60 Hz and 120 Hz have different budgets. Use the same scenario and
renderer before and after each change. Do not claim a percentage gain from
source inspection alone.

## S01 — Store compact notification message observations

**Evidence:** [history response cloning](src/ui/chat/mod.rs),
[normal history observations](src/ui/chat/mod.rs),
[notification message storage](src/ui/main_window/notifications/state.rs),
[observation consumers](src/ui/main_window/notifications/mod.rs).

History responses clone full `Message` values for `HistoryObserved`. The inbox
then retains up to 512 full messages. That is an entry bound, not a byte bound:
messages can contain attachments, rich embeds and optional base64 image payloads.
When those payloads are present, this second copy can outlive their removal from
the chat's media metadata. This is a source-confirmed retention path, not a claim
that every backend history response includes image bytes.

**Proposal:** introduce a compact observed-message record containing the fields
needed for read cursors, reply detection, notification previews and external
sender attribution. Avoid transporting or storing media payload strings in
that record. Bound retained text/metadata bytes as well as entry count. Preserve
the parent-message information required to recognize replies to the current user.

**Validate:** observe messages containing large inline embed payloads, release
chat media, and inspect notification-cache bytes. Reply notifications, deletion,
read acknowledgments and webhook sender names must still work.

## S02 — Stop rebuilding notification contents for unrelated events

**Evidence:** [account event dispatch](src/ui/main_window/mod.rs),
[profile description events](src/ui/main_window/account.rs),
[inbox rendering](src/ui/main_window/notifications/mod.rs),
[group row construction](src/ui/main_window/notifications/filters.rs).

Every account event calls notification delivery and `render_inbox`, including
description editing and tab events. If an inbox view exists, rendering removes
and recreates both tabs' contents, including rows inside collapsed groups. There
is no visibility guard for an existing closed/hidden inbox view. Member-avatar
batches and normal incoming messages also trigger this path.

**Proposal:** separate badge/state updates from widget rendering. Mark the inbox
dirty only for relevant changes; update group headers/counts in place and create
message rows when their group expands. Reconcile only changed rows while keeping
focus, expansion and scroll position. Use indexed channel/user lookups instead
of repeated linear searches. Avoid constructing full rows in a hidden window.
Persisted notification rows also need a retention/paging policy: only ephemeral
events and observed message records currently have explicit count bounds.

**Validate:** type a long profile description after closing a large inbox;
notification row construction should remain zero. With many loaded notices,
opening a collapsed inbox should create headers rather than every message row.
Keep batch read writes, failed-write badges, permission filtering and pagination.

## S03 — Prioritize member avatars by the current viewport

**Evidence:** [full-member completion](src/ui/main_window/mod.rs),
[avatar batching](src/ui/main_window/mod.rs),
[chat author priority](src/ui/chat/mod.rs),
[member list construction](src/ui/user_list/mod.rs).

Member completion offers the entire member list for avatar loading. Requests
are usefully bounded, prioritized and chunked into 16 profiles, but up to 1,024
IDs may still be loaded on a cold cache. Chat priority comes from the newest 200
messages rather than the current older-history viewport. The member pane reuses
unchanged rows but initially materializes all users in a `GtkListBox`.

**Proposal:** request current visible chat authors, visible member rows and a
small overscan area first. Defer offline/far-away members and avoid loading their
pictures while the member pane is hidden. For large servers, evaluate a recycled
list model/factory with group headers; retain the existing incremental behavior
for small lists if a migration adds complexity without measurable benefit.

**Validate:** compare cold startup and scrolling with 100 and 5,000 synthetic
members. Request volume should follow viewed rows. Scrolling into older history
must prioritize those authors, and changing role/status must preserve grouping.

## S04 — Make search pagination incremental

**Evidence:** [search page merging](src/ui/main_window/search.rs),
[search result rendering](src/ui/main_window/search.rs).

Each page checks duplicates by scanning the existing result vector, clears every
result widget and rebuilds the whole list. Inside that loop, each result also
constructs a complete cloned user map just to render its mentions. The view can
be rebuilt while hidden, for example through a DM snapshot update.

**Proposal:** build a user lookup once per render, maintain an ID set for result
deduplication, and append/reconcile changed result rows. Reuse existing widgets
and preserve the reading position. For very long searches, recycle visible rows
or impose a recoverable page window rather than accumulating widgets forever.

**Validate:** paginate through hundreds of results. An added page should not
rebuild preceding rows or move focus. Recheck permission loss, cancellation,
stale responses and mention display.

## S05 — Index the remaining chat lookups before attempting a renderer rewrite

**Evidence:** [reply lookup construction](src/ui/chat/mod.rs),
[window selection and spacers](src/ui/chat/window.rs),
[external avatar completion](src/ui/chat/transfers.rs).

The bounded row renderer and dirty-row reuse are already implemented. Remaining
work still includes rebuilding a message-ID lookup from the entire loaded
history, scanning history to find rows affected by an external-avatar result,
and whole-history work for window selection/spacer estimates. These costs can
grow while reading far back even though at most 200 message rows are mounted.

**Proposal:** profile these passes, then maintain a history ID index and a
resource-to-message-reference index where justified. Cache estimated height
aggregates by history/layout revision. An avatar completion from a previous
channel should populate its cache without scheduling an unrelated active-chat
render when no displayed row uses that avatar.

**Validate:** retain the existing 5,000-message fixture and measure work for one
avatar completion, one reaction and a small scroll. Compare lookup visits as
well as formatted rows. Preserve deletion, late reply resolution, shared media,
reading anchors and cache invalidation; do not remove those checks for speed.

## S06 — Give avatar failures and cooldowns more precise ownership

**Evidence:** [external CDN scheduler](src/api/webhooks.rs),
[avatar result handling](src/ui/chat/transfers/external.rs),
[cache freshness](src/ui/chat/transfers/external.rs).

The new queue and cache already fix dropped requests and repeated downloads.
Two refinements remain: every external host shares one CDN scheduler/cooldown,
and download/decode errors become the same `None` result. Missing avatars are
cached for 60 seconds; a failed refresh of an existing picture preserves it but
also resets its successful-picture freshness interval.

**Proposal:** retain a small typed failure state distinguishing a missing image,
bad bytes, transient transport errors and rate limiting. Keep old pixels while
using a separate retry deadline. Consider bounded per-origin cooldown records
plus a shared overall avatar concurrency cap, so a long cooldown from one host
does not suspend an unrelated provider. Offer an explicit retry where useful;
avoid another continuous automatic polling loop.

**Validate:** two providers, one returning 429 with `Retry-After`; the other should
continue within the overall budget. Cover 404, invalid bytes, cancellation and
refresh failure without erasing the existing picture. Never evade a cooldown by
creating a fresh scheduler for a retry to the same origin.

## S07 — Make reconnect and idle refreshes more selective

**Evidence:** [periodic refresh](src/ui/main_window/mod.rs),
[reconnect work](src/ui/main_window/mod.rs),
[access snapshot requests](src/ui/main_window/mod.rs),
[DM polling](src/ui/main_window/direct.rs).

Every reconnect forces avatar loading for the full offered member set alongside
member, emoji, notification, DM, access and history reconciliation. Regular access
refreshes fetch whoami, server, roles and channels approximately every 30 seconds;
DMs also have a periodic fallback. Coalescing and pacing already limit overlap,
but cached data can still generate substantial verification traffic.

**Proposal:** prioritize current access and selected-chat reconciliation, then
revalidate visible avatars while retaining their cached pixels. Defer remaining
member work. Measure whether periodic requests can back off when the app is
inactive and the socket is healthy, with jitter to avoid synchronized retries.
Conditional snapshots or resource revision fields would need backend support.
Do not simply eliminate verification: avatar changes during disconnection and
missed permission events still need recovery.

**Validate:** repeatedly flap a mock socket with a large warm member cache.
Measure request counts and time to usable chat; test missed permission changes,
changed avatars, unread state and stale snapshots before relaxing any fallback.

## S08 — Apply accepted profile changes without replacing the form

**Evidence:** [profile save completion](src/ui/main_window/account.rs),
[profile construction](src/ui/main_window/account.rs).

Successful profile writes trigger avatar loading, a summary refresh and a new
profile page/read. The tabbed host now stays open, but replacing its child form
can still lose local focus/scroll and briefly show loading. Text-only changes
also force avatar work.

**Proposal:** update existing widgets from the accepted authoritative response,
refresh changed image resources only, and show a short inline saved state. If a
follow-up GET is required by the contract, keep the form mounted while reconciling
the result. Preserve drafts entered after the request began and the current tab.

**Validate:** save while scrolled deep in the profile page; switch tabs during
the request; edit another field before completion. Check errors, image removal,
nickname normalization, closed windows and out-of-order responses.

## S09 — Make all account shortcuts consistent and protect drafts on close

**Evidence:** [tabbed settings](src/ui/main_window/account/settings.rs),
[preferences action](src/ui/main_window/layout.rs),
[profile shortcut](src/ui/sidebar/mod.rs).

The gear opens the new tabs, while the preferences action and own-profile shortcut
still open standalone forms. Tab switches retain drafts, but closing and reopening
the settings window starts fresh forms without a dirty-state prompt.

**Proposal:** route account shortcuts to the appropriate tab of the same window;
keep other people's profile windows separate. Track unsaved profile/preferences
changes, enable Save when needed, and ask Save/Discard/Cancel only when closing
with actual unsaved edits. Do not prompt when merely switching tabs. Keep
password fields out of draft persistence and clear them on close as today.

**Validate:** open settings from each shortcut, change fields, switch tabs and
close through the window control or keyboard. Untouched forms must close directly;
Cancel must retain edits; saved forms must not show a discard prompt.

## S10 — Add animation preferences and reduce background wakeups

**Evidence:** [animation scheduling](src/media/animation.rs),
[consumer visibility](src/media/animation.rs),
[current display preferences](src/models/user.rs).

Animations already stop advancing when clipped. However, each mapped offscreen
animation can still poll visibility every 250 ms, with its own timer and ancestor
checks. The custom animation path does not consult a reduced-motion preference,
and current display preferences do not expose GIF/emoji motion controls.

**Proposal:** add a local motion preference, defaulting appropriately to desktop
animation settings, with static-first-frame or explicit-play behavior. Consider
one visibility update on scroll/layout and shared scheduling instead of separate
offscreen polling timers. Ensure GIFs can resume immediately when visible again.
Keep new local options separate from the server's existing settings schema unless
that contract is extended.

**Validate:** many animated emojis/GIFs, including clipped and unmapped consumers.
Compare idle wakeups, invalidations and CPU; test live preference changes, channel
switches and explicit playback. Preserve animation timing and existing cleanup.

## S11 — Show chat connectivity and recovery progress

**Evidence:** [socket disconnection handling](src/ui/main_window/mod.rs),
[reconnect reconciliation](src/ui/main_window/mod.rs),
[request cooldowns](src/api/requests.rs).

The main disconnect event calls voice cleanup, but there is no corresponding
general chat connection-status transition in that branch. A user can continue
looking at cached content without knowing whether it is current or reconnecting.

**Proposal:** use a compact, nonmodal status area for Connecting, Reconnecting,
Refreshing and Offline. Keep cached content readable and preserve the composer
draft. If requests are waiting on a server cooldown, show a user-readable retry
state rather than exposing technical errors alone. Offer retry after automatic
recovery is exhausted; never replay an ambiguous message send automatically.

**Validate:** disconnect while reading and while sending, restore the socket,
return a 429, and expire the session. Status must reflect successful data
reconciliation as well as the socket opening, without flashing empty history.

## S12 — Explain notification counts when history is only partly loaded

**Evidence:** [pagination state](src/ui/main_window/notifications/mod.rs),
[DM tab count](src/ui/main_window/notifications/mod.rs),
[group badges](src/ui/main_window/notifications/filters.rs).

The DM tab and channel/sender badges count unread notices in the local inbox.
When `has_more` is true, older unread notices can remain outside that inbox, so a
bare number can be mistaken for an authoritative total. The existing channel
filter also filters loaded pages rather than requesting a channel-specific page.

**Proposal:** label counts accessibly as loaded unread notifications and explain
that more may exist. Consider a subtle incomplete-count indicator while more
pages remain; do not imply a particular channel definitely has additional notices
without evidence. Keep the loaded-page scope clear for filtering and read actions.
Exact totals by channel/sender would require backend aggregate counts; fetching
every page solely to compute a badge would undermine the request-budget work.

**Validate:** place unread notices in multiple server pages, with the first page
containing none for the selected filter. Empty-state text, badges and pagination
must remain understandable and must not imply everything has been read.

## S13 — Evaluate disk thumbnails only after measuring cold-start demand

**Evidence:** [session-only conversations](src/ui/chat/cache.rs),
[compressed media cache](src/ui/chat/transfers/encoded.rs),
[original image viewer](src/ui/chat/transfers/viewer.rs).

Recent media reuse already works in memory. Those bytes are lost after process
exit, so repeated launches can download the same thumbnails again. This is an
optional product improvement, not evidence that the current cache is broken.

**Proposal:** if measurements show worthwhile savings, add an opt-in, size-bounded
disk cache for thumbnails and small avatars. Namespace it by server/account and
resource revision; define expiry, logout/clear-cache behavior and permission-loss
invalidation. Use asynchronous I/O. Keep original/fullscreen media separate and
do not persist session credentials or webhook tokens with cached assets.

**Validate:** compare cold and warm launches, then change accounts, delete or
moderate media, revoke access and evict files. Reopening content must still pass
current authorization checks. Include a clear-cache control and report cache
usage as an estimate rather than equating it with application RAM.

## Suggested implementation sequence

1. Record S00, then implement S01 and S02: these have the clearest source-level
   opportunities without changing chat layout or backend APIs.
2. Take the small S04 lookup improvement, then measure S03 and S05 before larger
   list/index changes. Keep each optimization independently reviewable.
3. Address S06–S09 with existing request, profile and settings regressions.
4. Add S10–S12 as focused UX changes with keyboard/accessibility checks.
5. Consider S13 only if measured repeated-launch traffic justifies the extra
   persistence and invalidation work.

The current external-author contract still supplies no role-color metadata;
external role colors require a backend contract change, not guessing from
nicknames or repurposing embed accent colors. Existing grouping, contrast and
avatar-queue fixes should remain covered by the webhook regressions.
