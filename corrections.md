# Code review corrections

Reviewed: 2026-10-08. Base commit: `8269001`, including the current uncommitted chat-rendering fixes.

This review covers the frontend's history/state handling, media and avatar loading, request scheduling, WebSocket lifecycle, session persistence, member lists and related tests. Relevant local backend contracts were checked for token lifetime and avatar limits. The recent fixes for batched media redraws, offscreen GIF playback, delayed layout and the empty-chat flash are already present and are not listed as outstanding work.

The twelve corrections below are now implemented in the working tree. The finding descriptions and original locations document the reviewed behavior before correction. Automated validation is recorded below; live-server profiling remains a separate manual check.

## Priority and implementation order

P1 means high-impact correctness or responsiveness work to address first. P2 means a concrete reliability or scalability issue to address next. These are implementation priorities, not security severity ratings.

| ID | Priority | Finding | Evidence |
| --- | --- | --- | --- |
| C01 | P1 | Background history snapshots can overwrite newer live state | Reproduced with actual history code |
| C02 | P1 | A transient refresh failure can cause avoidable session expiration | Source and backend contract |
| C03 | P1 | Avatar decoding blocks GTK and retains oversized member textures | Source-confirmed; timing unmeasured |
| C04 | P1 | Presence changes cause redundant requests and rebuild member lists | Source-confirmed; load-dependent impact |
| C05 | P2 | Evicted or abandoned media loads remain marked as requested | Source-confirmed |
| C06 | P2 | DM refreshes overlap and discard useful responses | Source-confirmed |
| C07 | P2 | Older member responses can overwrite newer profile data | Source-confirmed |
| C08 | P2 | Successful HTTP deletion bypasses media cleanup | Source-confirmed |
| C09 | P2 | Reconnect discards older loaded history and the reading position | History replacement reproduced |
| C10 | P2 | Chat rendering still scales with all loaded messages and emojis | Source-confirmed complexity; timing unmeasured |
| C11 | P2 | WebSocket heartbeats have no application-level liveness deadline | Source-confirmed |
| C12 | P2 | Delayed session cleanup can delete a newer remembered login | Source-confirmed race |

Suggested sequence: C01–C02; C03–C04 with C07; C05 and C08; C06 and C11; C09–C10; C12. Keep each correction reviewable and preserve the existing request budget, permission checks and channel/session generation guards.

## C01 — Preserve live changes when a background read completes

**Locations:** [main_window/mod.rs:311](src/ui/main_window/mod.rs#L311), [main_window/mod.rs:445](src/ui/main_window/mod.rs#L445), [chat/state.rs:72](src/ui/chat/state.rs#L72), [chat/state.rs:158](src/ui/chat/state.rs#L158).

`TickCleanup` fetches the latest messages to update read state. `LiveRead` then emits an `AddMessage` for each returned row. Unlike the normal history-loading path, this request does not establish a history journal/version boundary. `merge_message` preserves newer edits, but replaces non-null reactions, previews and attachments with the incoming snapshot.

**Trigger:** the GET captures reaction count 1; a WebSocket event changes it to 2; the delayed GET arrives with count 1. The UI regresses to 1. A delayed snapshot can similarly restore a removed preview. Existing tests cover a bare send echo with omitted reactions, which is a different case.

**Suggested correction:** attach a state version to background reads and reconcile their results with changes received after that version. Share this logic with normal history reconciliation. Avoid simply treating every non-null HTTP field as newer, and do not solve this by ignoring all updates to existing messages: reconnect still needs to repair missed events.

**Regression check:** delay a read response, apply a reaction change and preview removal, then deliver the old response. Both live changes must survive. Also verify that an uncontested fresh snapshot repairs missed changes.

**Reproduced:** a temporary Rust harness compiled the repository's `Message` and `History` modules. A live count of 2 became 1 after applying the old snapshot through `upsert`.

## C02 — Retry transient session-refresh failures before expiry

**Locations:** [main_window/mod.rs:279](src/ui/main_window/mod.rs#L279), [models/auth.rs:69](src/models/auth.rs#L69). Backend contract: `../papo-backend/backend/internal/utils/jwt.go:14` defines a 24-hour token lifetime.

The refresh loop sleeps for 12 hours before every attempt, including the attempt after a network error or HTTP 5xx. If the first refresh fails around hour 12, the next attempt occurs at or after hour 24, when the original token has expired. A short outage can therefore force a later logout even after connectivity returns. The refresh response's connection expiration is available but is not used by the loop.

**Suggested correction:** schedule from the known expiration with a safety margin. Retry transient failures with bounded backoff and jitter while the existing session is valid. Keep refresh serial, stop on terminal authentication errors, and respect the shared server cooldown. Distinguish a definite rejected request from an ambiguous refresh response loss before deciding whether another refresh is safe.

**Regression check:** use controllable time and a mock refresh endpoint. Fail the hour-12 attempt, restore connectivity, and assert successful recovery well before hour 24. Verify terminal 401 handling and cancellation on logout.

## C03 — Decode avatars outside GTK and bound their retained size

**Locations:** [main_window/mod.rs:388](src/ui/main_window/mod.rs#L388), [main_window/mod.rs:650](src/ui/main_window/mod.rs#L650), [media/avatars.rs:19](src/media/avatars.rs#L19), [media/avatars.rs:40](src/media/avatars.rs#L40), [media/mod.rs:63](src/media/mod.rs#L63), [api/mod.rs:519](src/api/mod.rs#L519).

`user_profiles` collects all profile batches before returning. The GTK update handler then calls `AvatarCache::finish`, which synchronously decodes every base64 image and creates textures through `seed`. These calls use `texture_from_bytes`, not the bounded thumbnail preparation path. The cache retains textures for every loaded member, with no overall memory budget.

The local backend limits avatars to 512 × 512 and 2 MiB encoded image bytes, so arbitrary dimensions are not assumed here. Even valid 512 × 512 RGBA avatars require roughly 1 MiB each: 1,000 distinct avatars can approach 1 GiB of pixel storage before other overhead. Most member/chat avatars are displayed at only 24–36 logical pixels. Actual memory depends on image sizes and texture representation.

**Suggested correction:** prepare suitably sized avatar pixels on bounded background workers, then create GTK textures on the main thread in small batches. Publish profile batches progressively. Add a cache budget and prioritize visible members/message authors; retain the current per-user generation protection. Load a larger image separately for an open profile when needed.

**Regression/performance check:** load hundreds of valid avatars while an idle/frame callback measures main-loop responsiveness. Check decoded dimensions and aggregate cache bytes, and deliver an older avatar result after a newer update to ensure it cannot replace the new picture.

## C04 — Make presence updates local and incremental

**Locations:** [main_window/mod.rs:512](src/ui/main_window/mod.rs#L512), [main_window/mod.rs:639](src/ui/main_window/mod.rs#L639), [user_list/mod.rs:119](src/ui/user_list/mod.rs#L119), [user_list/mod.rs:128](src/ui/user_list/mod.rs#L128), [sidebar/mod.rs:277](src/ui/sidebar/mod.rs#L277).

Every `presence_update` already supplies presence information, but also starts a user-summary HTTP request. The member component clears, sorts and recreates its entire list for the presence event, and does so again when the user-summary response arrives. The DM sidebar also rebuilds its rows on presence changes.

On a busy server, these requests consume the same eight-starts-per-second budget as foreground operations, while repeated widget destruction competes with chat rendering. For example, 40 extra requests require about five seconds of request-start slots even before response latency; this is a budget calculation, not a measured login delay.

**Suggested correction:** apply status changes directly to the affected rows and move a member only when its sorting/group key changes. Separate profile invalidation from presence handling. If the backend also uses presence events to signal profile changes, debounce and batch those summary refreshes by user ID instead of issuing one request per event. Reuse unchanged sidebar rows.

**Regression/performance check:** feed a burst of repeated presence events for a large member list. Assert that unchanged rows retain their widgets, summary requests are absent or bounded/coalesced as appropriate, and a foreground send does not wait behind one HTTP request per event.

## C05 — Separate media request state from cache residency

**Locations:** [chat/transfers.rs:133](src/ui/chat/transfers.rs#L133), [chat/transfers.rs:182](src/ui/chat/transfers.rs#L182), [chat/transfers.rs:232](src/ui/chat/transfers.rs#L232), `handle_transfer` in the same file.

`hydrate_media` eagerly visits the entire loaded history. Images are limited to 32 cached entries, but eviction removes the texture/animation without clearing `requested`. Consequently, once more than 32 distinct images finish, evicted rows revert to placeholders and the normal hydration path will not load them again. The “Carregar imagem” button can explicitly retry some cases; the issue is missing automatic recovery and unnecessary offscreen work.

There is a related ownership problem for shared preview IDs: the download/decode is associated with the first message that requested it. If that message is deleted before completion, handlers reject the result even when another surviving message references the same preview. Its `requested` entry remains, so the surviving reference can stay unloaded.

**Suggested correction:** track queued/in-flight, failed and cached states separately. Load visible/nearby media first, and make evicted visible entries eligible again with deduplication and bounded concurrency. Validate a shared preview result against surviving authorized references, rather than only its initiating message. Preserve epoch/token checks. Do not merely remove all request markers on eviction while retaining eager whole-history hydration: that would create a continuous eviction/refetch loop.

**Regression checks:** load more than 32 images and scroll back to an evicted one; delete the initiating message while a shared preview is decoding; simulate a failed load and retry. Surviving visible media must recover without duplicate concurrent work or unbounded requests.

## C06 — Coalesce DM list refreshes

**Locations:** [main_window/direct.rs:84](src/ui/main_window/direct.rs#L84), [main_window/direct.rs:90](src/ui/main_window/direct.rs#L90), [main_window/direct.rs:126](src/ui/main_window/direct.rs#L126), [main_window/mod.rs:490](src/ui/main_window/mod.rs#L490).

Each `refresh_direct` call creates another task and replaces `direct.request`. Previous work continues, but its eventual result is discarded because its token no longer matches. Unknown-conversation messages, reconnect, explicit refresh and the periodic timer can trigger this repeatedly. In particular, several messages from a DM absent from the local list can start several identical list requests before the first completes.

**Suggested correction:** allow one in-flight DM refresh and one queued follow-up, as already done for access/notification refreshes. Preserve the existing version-based reconciliation of live DM events. Cancel obsolete requests when their result is genuinely no longer useful.

**Regression check:** hold the first list response, trigger many refreshes and live DM updates, then release it. Expect one request in flight, at most one follow-up, and no loss of live unread counts or hidden/blocked state.

## C07 — Protect member snapshots from out-of-order completion

**Locations:** [main_window/mod.rs:375](src/ui/main_window/mod.rs#L375), [main_window/mod.rs:380](src/ui/main_window/mod.rs#L380), [main_window/mod.rs:639](src/ui/main_window/mod.rs#L639).

Member list and individual-summary requests have no generation/version attached to their responses. `UsersLoaded` replaces the entire member vector; `UsersUpdated` replaces entries unconditionally. A slow initial/reconnect list can overwrite a newer nickname/role update that already arrived through an individual fetch. Two individual fetches for the same user can also complete in reverse order.

**Suggested correction:** use per-user request generations and a snapshot version. Merge a full list with individual updates accepted after that snapshot began. A single global “latest request” token would be insufficient because requests for different users must remain independently valid.

**Regression check:** delay a full list, deliver a newer individual result, then finish the old list. Repeat with two reversed individual responses. Preserve the newer user data and avoid unnecessary redraws when nothing changed.

## C08 — Use one deletion path for HTTP and WebSocket confirmation

**Locations:** [chat/actions.rs:242](src/ui/chat/actions.rs#L242), [chat/mod.rs:621](src/ui/chat/mod.rs#L621), [chat/transfers.rs:104](src/ui/chat/transfers.rs#L104).

The WebSocket deletion path calls `transfers.invalidate_message`, which closes image viewers, removes playback objects and cancels pending video identities. The successful HTTP `DeleteFinished` path removes history and pins directly, skipping that cleanup.

**Trigger:** suppress or delay the WebSocket confirmation, start deleting a message, and reopen its image viewer while the HTTP request is pending. The HTTP success path leaves that viewer open. Playback objects and temporary-file ownership can also outlive the removed row; whether a particular inline stream keeps playing additionally depends on GTK's unmapping behavior.

**Suggested correction:** route successful deletion through a shared, idempotent helper that removes history/pins, closes viewers and disposes playback resources. Retain the active-channel epoch check so an old operation cannot affect another conversation.

**Regression check:** suppress the WebSocket event, open media, and return HTTP 204. Assert that viewers close, playback resources are released and temporary files are removed. A later duplicate delete event must be harmless.

## C09 — Preserve the reading window during reconnect reconciliation

**Locations:** [main_window/mod.rs:472](src/ui/main_window/mod.rs#L472), [main_window/mod.rs:660](src/ui/main_window/mod.rs#L660), [chat/state.rs:39](src/ui/chat/state.rs#L39), [chat/viewport.rs:59](src/ui/chat/viewport.rs#L59).

Reconnect calls `load_history(channel, None)`. This produces `append: false`, and `History::finish` discards every previously loaded message outside the returned latest page. A user reading older messages loses that page and its anchor. Keeping a pixel offset cannot preserve a message whose row no longer exists, and subsequent browsing must load the old pages again.

**Suggested correction:** distinguish initial channel selection from same-channel reconciliation. Refresh the latest and currently viewed ranges, retaining the reading anchor and bounded cached pages. Explicitly reconcile deletions and permissions for retained ranges; simply appending forever would leave missed deletions visible and worsen memory growth.

**Regression check:** load several pages, scroll to an older message, then reconnect while the server changes a recent message and deletes another. Keep the reader's anchor while applying authoritative changes and preserving permission revocation behavior.

**Reproduced:** the temporary state harness loaded an older and a newer message, completed a latest-only `append: false` response, and confirmed that the older message disappeared.

## C10 — Limit work per chat render and avoid rebuilding the emoji index per row

**Locations:** [chat/mod.rs:801](src/ui/chat/mod.rs#L801), [chat/mod.rs:848](src/ui/chat/mod.rs#L848), [chat/text.rs:7](src/ui/chat/text.rs#L7), [chat/state.rs:39](src/ui/chat/state.rs#L39).

The renderer reuses unchanged row widgets, but first scans every loaded message, formats large string signatures, resolves mentions and reparses inline emojis. `text::parts` builds a complete emoji-name map on every call, even for plain text. For M loaded messages and E custom emojis, that alone adds O(M × E) map construction work per full render. Every loaded message also retains a GTK row and its child widgets; pagination has no general history-window bound.

The new 16 ms media batching reduces how often this work runs, but does not change its cost per pass. Offscreen GIF pausing also does not virtualize message widgets or stop eager image hydration.

**Suggested correction:** first cache the emoji-name index when the emoji catalog changes and avoid parsing work for unchanged content. Track dirty message IDs, including grouping neighbors and reply dependents. Then introduce a bounded rendered window or a suitable GTK list model/factory, preserving selection, variable-height anchors, context menus and media teardown. Coordinate this with C05 and C09.

**Performance check:** benchmark 100, 1,000 and 5,000 loaded messages with a large emoji catalog. Record frame/main-loop latency, render duration, widget count and memory for a single reaction update, image completion and new message. Use relative/scaling checks rather than machine-specific timing assertions in normal CI.

## C11 — Detect silent WebSocket failures

**Locations:** [ws/mod.rs:123](src/ws/mod.rs#L123), [ws/mod.rs:139](src/ws/mod.rs#L139), [ws/mod.rs:312](src/ws/mod.rs#L312).

The client sends a heartbeat every 25 seconds but discards `heartbeat_ack` without updating a liveness deadline. Handshake and socket writes also lack an explicit application timeout. If a peer/proxy leaves the connection open while no longer delivering events, writes may continue to succeed or stall, and the reconnect loop is not guaranteed to run promptly. An awaited write inside a selected branch also prevents that loop from servicing its other branches until the write returns.

**Suggested correction:** bound connection establishment and writes, track heartbeat acknowledgments or another explicit activity signal, and close/reconnect after a documented missed-response window. Keep teardown cancellable and retain the existing connection identity rules for voice commands. Choose deadlines compatible with the backend heartbeat contract and normal latency.

**Regression check:** use local peers that never finish the handshake, drain heartbeats without acknowledging them, and stop reading outgoing data. Verify bounded recovery and prompt shutdown when the component is dropped.

## C12 — Make remembered-session deletion conditional on its owner

**Locations:** [session.rs:32](src/session.rs#L32), [session.rs:37](src/session.rs#L37), [root.rs:365](src/ui/root.rs#L365), [root.rs:375](src/ui/root.rs#L375).

Logout/session-expiration launches asynchronous cleanup and immediately returns to login. `forget` unconditionally deletes the key for `(base URL, username)`. If the same account logs in again and saves a new credential before the old cleanup acquires the storage lock, that delayed cleanup deletes the new saved session. The storage mutex prevents concurrent writes, but does not establish which session owns the stored value. `rotated` already performs an ownership comparison for a similar race.

**Suggested correction:** distinguish “forget this session if still current” from explicit “forget any saved session for this account.” For delayed lifecycle cleanup, compare a session generation/connection identity under the storage lock. Account for cookie rotation within the same session; a raw-token-only comparison can otherwise fail to remove a rotated credential.

**Regression check:** pause cleanup for session A, save a newer session B under the same account, then release A's cleanup. B must remain. Also verify that cleanup still removes a rotated token belonging to A.

## Implementation record

| ID | Implemented correction | Regression coverage |
| --- | --- | --- |
| C01 | Independent snapshot journals coexist with pagination, replay later live changes, and treat HTTP collection omissions as authoritative empty collections. Sparse send/WS echoes retain their existing merge behavior. | `independent_reads_replay_reactions_and_removed_previews_during_pagination`; existing history race tests |
| C02 | Renewal uses JWT expiration hints and refresh-response expiration with a five-minute margin, serial bounded backoff/jitter, terminal-auth handling and task cancellation. Ambiguous losses use the backend's 60-second grace contract, with a conservative 55-second overall recovery deadline. A received rotated cookie is saved even if its JSON body is lost. | Renewal scheduling/backoff tests; local 503→200 recovery, received-cookie/body-loss preservation, 401 termination and cancellation |
| C03 | Avatar decoding runs on at most two background workers; 16-profile batches publish progressively. Member/chat textures are at most 72×72, capped at 1,024 textures (20.25 MiB of RGBA pixels). Recent authors and online members take priority. Open profiles decode a separate 192-pixel image. Per-user generations reject stale decodes. | Cache size/dimensions, stale-avatar protection, 128-avatar worker responsiveness and existing PNG/JPEG/GIF/WebP workflows |
| C04 | Online/away/busy changes update the affected member icon in place. Unchanged member/DM rows retain their widgets; grouping/name changes reorder members. Summary invalidations batch after 250 ms only when profile metadata changes. Open-profile invalidations debounce separately after 500 ms, including description-only signals. | 500-member/200-event widget-identity checks; existing presence, profile and sidebar workflows |
| C05 | Queued/in-flight, failure/backoff, metadata-only completion and texture residency are separate. Near-viewport media loads first, with eight outstanding jobs, four transfer/decode permits and up to 24 candidate keys. Evicted visible media can load again; shared results accept any surviving authorized reference. Download/decode epochs and request tokens remain enforced. Deferred embedded payloads have an 8 MiB/32-entry budget. | Actual >32-image scrolling/automatic reload/stationary-viewport checks; shared-reference deletion, deduplication, failure eligibility and existing moderation/cancellation tests |
| C06 | One DM list request may be in flight, with one coalesced follow-up. Existing live-version, unread, hidden and blocked-state reconciliation remains in place. | 50 refresh triggers while holding a result; existing reversed/stale DM and read-state workflows |
| C07 | Full member snapshots carry a version; individual batches have independent per-user generations. Queued invalidations supersede old responses immediately. Accepted own-profile writes also protect current-user fields against older access/member snapshots. | Reversed same-user responses, unrelated-user results and delayed full snapshots; existing profile-save workflow |
| C08 | Both HTTP and WS deletion use one idempotent history/pin/viewer/playback cleanup helper, scoped to the active operation epoch. | HTTP-only image/video cleanup, temporary-file removal and later duplicate WS deletion |
| C09 | Reconnect progressively fetches through the oldest retained message and reconciles authoritative deletions/edits while replaying live changes. Existing rows remain visible during the fetch, and the viewport restores its message anchor. Revocation cancels media/reconciliation and rejects late results. | Older-page/edge-deletion/live-arrival tests; mapped 5,000-message older-anchor reconciliation and existing permission checks |
| C10 | Emoji names are indexed once per catalog update. Dirty message IDs include grouping neighbors and reply dependents. Rendering retains at most 200 message rows, plus date/spacer rows; measured-height spacers support scroll and direct navigation through cached history. Off-window inline playback is released. | 100/1,000/5,000-message synthetic measurements; bounded widgets, unchanged row identity, reaction/image/new-message dirty formatting, spacer navigation and scroll/focus regressions |
| C11 | WebSocket handshake and writes have 10-second deadlines and cancellable teardown. Heartbeats run every 25 seconds; no acknowledgment for 75 seconds disconnects and enters the existing reconnect loop. Voice commands retain connection-generation filtering. | Stalled handshake, silent heartbeat-draining peer, blocked writes/cancellation and existing ping/pong, revoked-session and voice-reconnect tests |
| C12 | Remembered credentials store a stable client-session owner across cookie rotations. Delayed logout clears only its owner; explicitly disabling Remember clears the account. Resume adopts the saved owner, with legacy-token fallback. | Old logout after new login, cleanup after rotation, account scoping, resume/revocation and rotation CAS tests |

## Validation

Tests use local fixture servers, a private GTK display and an in-memory keyring. No production credentials, messages or calls are used. Synthetic performance checks bound row counts and formatting work; timing and process RSS are observations, not hardware-independent CI thresholds.

- `cargo test --locked --offline`: **159 passed, zero failed, three ignored** environment-dependent tests; 3.51 seconds.
- Full mapped GTK workflow: **passed**, 129.30 seconds. This includes avatar preparation, presence bursts, coalesced DM refreshes, media eviction/reload, deletion cleanup, permission changes and large-history navigation/reconciliation.
- `cargo build --locked --offline`: **passed**. Compiler unused-import/dead-code warnings remain.
- `git diff --check`: **passed**.
- The GUI run exposed a delayed viewport callback sending to a dropped component. Delayed viewport notifications now use fallible sends, and the complete workflow passed with this fix.

Measurements from that GUI run, with 500 custom emojis:

| Loaded messages | Retained rows, including date/spacer rows | Rows formatted for a reaction | Reaction update including test-loop dispatch | Process RSS |
| --- | --- | --- | --- | --- |
| 100 | 102 | 1 | 16.7 ms | 418.7 MiB |
| 1,000 | 201 | 1 | 24.8 ms | 448.6 MiB |
| 5,000 | 201 | 1 | 31.5 ms | 478.5 MiB |

Image completion and new-message checks also passed the bound of at most three formatted rows per update. RSS includes the entire integration-test process and its earlier workflows; it is not a measurement of chat-only memory or avatar-cache size. These synthetic observations do not establish real-server frame latency.

The original review baseline was 144 passing headless tests, three ignored environment-dependent tests, a passing full GTK workflow and a successful build. Native voice/desktop-keyring tests are not substitutes for a real multi-user call or desktop keyring check and remain separately invoked.

## UX follow-up — 2026-10-09

The smoothness review found and corrected four further issues:

| Finding | Correction | Evidence |
| --- | --- | --- |
| Image reflow briefly moves the reading position before the next restoration tick | Apply anchors after GTK allocation and before painting; compensate ongoing wheel motion in the same layout phase | Reproduced a painted frame with a 200-pixel displacement; the new mapped regression checks every painted reading offset |
| Viewport updates repeatedly scan all cached messages and normalize unchanged embed metadata | Track media revisions, reuse normalized metadata, and discover candidates only within the bounded rendered window | The 5,000-message regression checks 100 viewport updates, reactions, plain arrivals and a fresh embed |
| Small reading movements cause avoidable window recentering and row churn during live updates | Retain the materialized window while the reader remains comfortably inside it; recenter near its edges | Window-selection regression covers retained ranges, edge movement and explicit Latest navigation |
| Every render schedules another delayed media refresh, and intermediate unread pages start irrelevant downloads | Coalesce layout refresh callbacks, avoid duplicate hydration during window shifts, and defer media until the opening unread boundary is resolved | Combined opening, scrolling, cache-recovery and large-history workflows |

The media request budget, cache limits, authorization checks and stale-response
guards remain in place. The changes reduce frontend work and correct a reproduced
visual jump; real-server frame latency still depends on the desktop and content.

Validation: 168 Rust tests passed; focused smoothness checks and the full GTK
workflow passed (149.53 seconds); the application build and `git diff --check`
passed. The mapped opening stress assertion samples the first visible painted
frame rather than a later main-loop polling result.
