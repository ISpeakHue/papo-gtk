# Automated requirements checks

Run the suite with:

```sh
cargo test --locked
```

Install Rust, a C compiler, pkg-config, GLib development tools, OpenSSL development files, GTK 4.12+
and libadwaita 1.6+ first. On Fedora 44:

```sh
sudo dnf install rust cargo gcc pkgconf-pkg-config glib2-devel openssl-devel gtk4-devel libadwaita-devel
```

The default suite runs without a display, a Papo backend, credentials, or a database.
The HTTP/WebSocket integration test binds an ephemeral loopback port, so the
execution environment must permit local sockets. Dependencies must already be
cached when using `cargo test --locked --offline`.

GitHub Actions runs the same suite on pushes, pull requests, and manual dispatches
using `.github/workflows/test.yml`. Its Fedora container supplies the GTK and
libadwaita versions required by `Cargo.toml`. Compilation includes all UI components,
so errors such as the invalid application-window child assignment fail CI. A second
CI step runs the GTK smoke test in Xvfb to verify the header bar, window controls,
separate account/server password fields, history pagination/retry controls,
presence status icons, login/main view transitions, and the session-expired login message.
It edits the actual login fields, including Unicode typing, paste, deletion,
queued edits, selection/caret preservation, and credential clearing, to catch
text-change feedback loops that synthetic component messages miss.
It also exercises the message/emoji workflows against a stateful local mock
HTTP server, including failures, retries, confirmations, and permission changes:

```sh
dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

## Pelican branding

The executable bundles 128px and 256px pelican icons. The focused GTK check
resolves and decodes both sizes through an isolated resource-only icon theme,
verifies the login and window icons, and can render light/dark previews:

```sh
PAPO_UI_CASE=branding PAPO_DESIGN_PREVIEW=1 dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

Previews are saved to `/tmp/papo-design-pelican-light.png` and
`/tmp/papo-design-pelican-dark.png`. Validate launcher metadata with
`desktop-file-validate data/br.com.papo.gtk.desktop`.

## Chat rendering and scrolling

Chat rendering regressions in the mapped `ui_smoke` workflow also verify:

- A channel awaiting its first history response does not show the empty-chat
  prompt. An actual empty response displays a stable empty state.
- Newly confirmed outgoing messages are inside the viewport without scrolling,
  including after an unrelated row grows beyond the initial layout frames.
  Readers scrolling through older messages are not pulled to the bottom.
- A burst of 24 image completions reconciles history once; stale completions
  do not reconcile history or move the viewport.
- Shared preview versions keep the newest image paired with its metadata,
  without retaining encoded image payloads in other messages.
- Visible GIFs animate, clipped GIFs stop advancing frames, and scrolling them
  back into view resumes playback. Unmapped/cached GIFs have no frame timer.

These use generated media and a private display, without a remote account.

## Review corrections (C01–C12)

The default suite now exercises independent HTTP/live-history journals, complete
snapshot removals, out-of-order member generations, expiry-aware renewal,
503→200 recovery, lost refresh bodies, owner-scoped remembered-session cleanup,
WebSocket acknowledgment deadlines, stalled handshakes and cancellable writes.
These use ephemeral local servers and the test keyring, never a saved real session.

The existing single-thread GTK `ui_smoke` workflow also checks:

- Actual scrolling through more than 32 media images, automatic reload after
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

## Bug report 6

The mapped workflow covers the nine items in [BUG_FIXES_6.md](BUG_FIXES_6.md):
shared Unicode/server emoji search, square chooser cells, composer insertion,
same-thumbnail viewer dismissal, post-preparation audio, mute/hover volume,
notification authors, reply contrast/role colors, the header search action,
media reuse between channels and stable text widgets during image completion.
An instrumented MediaStream checks that startup/preparation actually invokes
the backend audio update, including when GTK already reports its default values.
Mute/volume choices made before preparation survive repeated preparation.
Run just this regression on a test display with `PAPO_UI_CASE=video-audio` and
the `ui_smoke` command above. Rich-card tests check left-edge positions for long
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

## Bug report 5

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
the voice section before running it.

The public example `giphy:VxdNf4DadRSsMYAfnv` was also decoded from its downloaded
200-pixel rendition. To repeat that optional check, download it to a local file
and run `PAPO_GIPHY_SAMPLE=/path/to/sample.gif cargo test --locked supplied_giphy_sample -- --nocapture`.
The normal suite uses generated fixtures and requires no Giphy network access.
See [BUG_FIXES_5.md](BUG_FIXES_5.md) for the full report and resource limits.

## Login request budget

All REST requests share a session-wide scheduler, including cloned clients,
avatars, emoji pages, previews and downloads. Request starts are spaced by at
least 125 ms (at most eight per second). FIFO admission prevents background
pagination and downloads from continually overtaking a foreground read.
HTTP 429 imposes a shared cooldown;
`Retry-After` seconds and HTTP dates are supported. Read-only requests can retry
twice, with one- and two-second backoff when the backend supplies no header.
Headers requesting more than five seconds return the error immediately while
preserving the cooldown for subsequent requests. Mutations, uploads and account
login are never replayed automatically. The two profile/summary batch POST
endpoints are read-only and can retry.

Startup subscribes to WebSocket before loading its access snapshot. If the socket
is unavailable, HTTP starts after two seconds. The first successful subscription
does not duplicate startup; subsequent connections still reconcile missed events.
Members, direct messages and notifications start after access loading succeeds.
Channel overrides come from `/channels`; only omitted/null overrides use the
legacy per-channel endpoint. In-flight access refreshes coalesce into one pending
follow-up so permission changes are still reconciled. Server emojis are reused
across channel switches for 60 seconds; opening the picker/manager, explicit
refresh, emoji mutations and reconnect can update them sooner.

The default `api::requests::tests` cover pacing across clones/media, foreground
progress during background downloads, bounded 429
recovery, shared cooldowns after rejected mutations, non-replayed account login,
safe profile-batch retries, HTTP-date parsing, and legacy permission fallback.
A 70-channel fixture requires one channel request instead of 71. The GTK workflow
checks one startup snapshot after the first socket connection, no redundant
override reads, opening within five seconds on the local mock server, cached
empty emoji lists across channel switches, eight refresh triggers coalescing into
two snapshots, and live permission revocation. A pending DM open must survive
concurrent permission refresh; automatic channel selection must not cancel it.
These local timing checks do not measure production-server latency.

## Native chat layout

The GTK workflow also maps the redesigned chat into a real window. It checks
wide, medium and narrow breakpoints, opening and dismissing navigation overlays,
member toggling, direct-message/server navigation actions, consecutive-author
message grouping and keyboard access to the message action popover. It renders
light and dark appearances and verifies that open call controls fit a narrow
window. Permission checks inspect each menu action's visibility even when its
popover is closed. All message and server data comes from the local mock API.

It also covers the reported bugs in [BUG_FIXES.md](BUG_FIXES.md): preserving
message widgets and scroll anchors through typing, live arrivals and pagination;
explicit notification navigation; keyboard focus scrolling; reply/copy/context
actions; role labels; the older-message banner; native text/voice row activation;
responsive image previews and viewer cleanup; search filters; and profile controls
visible on opening. Mention renames and edits to replied-to messages must still
update cached rows.

The second regression report, [BUG_FIXES_2.md](BUG_FIXES_2.md), adds mapped
checks for automatic pagination (including duplicate prevention, failure/retry
and end of history), wheel scrolling, expiring reply highlights, pointer popover
anchors, read-only text and inline custom emoji paintables. The HTTP fixture
checks that an X preview with no image fields fetches its full text, thumbnail
and video metadata, and rejects an older response after a preview update.
Voice checks cover pre-join participants, preserved row identity during speaking
updates, full-width footer geometry, separate device settings, stream viewing,
and the existing call leave/disconnect/revocation paths.

[BUG_FIXES_3.md](BUG_FIXES_3.md) covers the third pass. Its GTK checks add
compact destructive confirmations, context-menu reactions, on-demand participant
tooltips and request deduplication, typed/manual mentions and retained typing
focus, native inline WebM video/audio decoding, close/delete/file cleanup,
rejection of delayed downloads, and off-thread image result invalidation while
GTK timers continue to run. Highlight checks require the same row before, during
and after emphasis. Call-cue PCM validation runs headlessly; discard-sink cue decoding
and disabled sound behavior run in the GTK workflow. Speaking indicators preserve
participant buttons during updates. A 10,000-message headless fixture verifies
bulk merging, deduplication, chronology and realtime event replay.

[BUG_FIXES_4.md](BUG_FIXES_4.md) covers empty nickname fallbacks, native
character-by-character @ completion without moving typing focus, keyboard
selection, actual picture/player allocations and narrow media frames. It checks
small source decoding, enabled room-entry cues, duplicate cue suppression,
speaking indicators with either avatar setting and muted/silent transitions.
The cue helper must decode real PCM through a discard sink and reap its process
after completion or cancellation, including rapid repeated starts/stops.
Duplicate HTTP/WebSocket echoes must preserve reactions and history replay while
skipping unchanged renders. A mapped burst of 20 messages checks widget, text and
caret retention with a large preview payload; render signatures exclude encoded
images. The real SFU audio probe below separately checks active-speaker events.

The media fixtures are original, small and bundled; see
[tests/fixtures/README.md](tests/fixtures/README.md). Regenerating the video needs
FFmpeg, but running the tests does not. The GTK media backend needs VP8, Vorbis
and PCM decoding. Live production servers and SFU negotiations are outside this
mock workflow.

To save visual previews, run the same test with:

```sh
GSK_RENDERER=cairo PAPO_DESIGN_PREVIEW=1 cargo test --locked ui_smoke -- --ignored --test-threads=1
# In a headless environment:
GSK_RENDERER=cairo PAPO_DESIGN_PREVIEW=1 dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

Previews are written to `/tmp/papo-design-dark.png`, `papo-design-light.png`,
`papo-design-narrow.png`, `papo-design-narrow-navigation.png` and
`papo-design-narrow-call.png`. Additional previews are `papo-design-search.png`,
`papo-design-profile.png`, `papo-design-inline-image.png` and
`papo-design-history-banner.png`, `papo-design-sidebar-call.png` and
`papo-design-call-settings.png`, `papo-design-inline-image-narrow.png`,
`papo-design-inline-video.png` and `papo-design-mention-suggestions.png` in the same directory. Software rendering avoids compositor-specific
offscreen rendering artifacts. See [DESIGN.md](DESIGN.md) for navigation,
shortcuts and design references. High-contrast appearance, larger desktop text
scales and physical touch interaction remain manual checks.

## Saved sessions

“Manter sessão neste dispositivo” saves the authentication cookie in the desktop
Secret Service; application configuration contains only the server, username and
the preference. Tests use an in-memory substitute and never access real saved
credentials. The regular suite checks server/user isolation, safe transport,
credential redaction, reuse without a login request, revocation and deletion.
Resuming validates whoami, refreshes the existing session before the first
WebSocket connection and remembers the new cookie, so an almost-expired cookie
does not have to wait for the regular 12-hour refresh timer.
Refresh rotation updates an existing saved cookie without recreating a logged-out
entry or overwriting a newer login.

On a desktop with an unlocked keyring, verify the native storage adapter with:

```sh
cargo test --locked desktop_keyring_round_trip -- --ignored --test-threads=1
```

This creates and removes a disposable test entry under a separate service name.
If Secret Service is unavailable, login still works in memory and reopening
requires authentication. Explicit logout deletes the saved credential and calls
backend logout. Closing the window preserves a remembered session. Existing
valid sessions can be revoked from account security; the client does not revoke
other devices automatically.

## Requirements covered

These checks capture client behavior from the current implementation and bug
review; this repository does not include a separate product requirements document.

| Requirement | Automated evidence |
| --- | --- |
| Localhost HTTP login supports backend Secure cookies in subsequent profile and WebSocket requests. | `ui::login::tests::localhost_public_login_uses_secure_cookie_for_profile_and_websocket` uses `localhost` with an IPv4-only listener. |
| Supplying a server password authorizes the server before account login or registration, without requiring public metadata discovery. | `private_login_and_registration_authorize_server_before_account` verifies endpoint order, absence of a pre-authentication `/server` request, exact passwords, and temporary/session cookie replacement. |
| Missing or incorrect server passwords must stop before account authentication. | `private_server_without_password_requests_it_before_account_login`, `invalid_server_password_stops_before_registration` |
| A fresh backend without a server record still permits account authentication. | `bootstrap_without_server_record_still_allows_account_login` |
| Stopped backends explain the address/port; unhealthy backends retain their actual error. | `stopped_local_backend_explains_address_and_port`, `unhealthy_backend_reports_its_error_instead_of_connection_failure` |
| Website pages must not pass API health/authentication checks; invalid metadata must explain its endpoint and response type. | `website_html_health_response_stops_before_sending_credentials`, `non_json_server_metadata_has_actionable_diagnostics`, `server_password_endpoint_cannot_accept_html_as_success`, `account_login_and_profile_reject_html_with_endpoint_context` |
| Remote Secure authentication cookies require HTTPS; localhost development remains supported. | `api::tests::secure_auth_cookies_require_https_except_on_loopback` |
| Local addresses accept omitted schemes and API prefixes remain consistent for HTTP/WebSocket requests. | `api::tests::local_addresses_and_api_prefixes_are_normalized_consistently`, `invalid_server_addresses_fail_before_any_request` |
| The login window shows a header bar, window controls, a separate optional server-password field, and working transitions to/from chat. | `ui::root::tests::ui_smoke_window_has_header_controls_and_server_password`, run separately under a virtual display in CI. |
| Editing any login field keeps the UI responsive, preserves entered passwords and the caret, and clears credentials without recurring input events. | GTK `ui::login::tests::exercise_input`, called by the existing `ui_smoke` test; uses real editable widgets for typing, paste, deletion, queued edits, mode/error updates, and clearing. |
| Login cookies authenticate the initial WebSocket and subsequent connections. | `api::tests::login_cookie_authenticates_websocket_and_reconnect` exercises a real local HTTP login, cookie storage, two WebSocket handshakes, event delivery, and connection notifications. |
| WebSocket cookies obey URL scope and use the current session value. | `api::tests::websocket_cookies_respect_scope_and_rotation` checks Secure and Path restrictions, independent clients, and cookie rotation shared by cloned clients. |
| In-flight history must retain live arrivals, edits, and deletions. | `ui::chat::state::tests::history_preserves_live_arrivals_edits_and_deletions_during_fetch` |
| Old requests must not overwrite a different channel or a newer request. | `channel_switch_rejects_old_history_and_old_send_results`, `superseded_requests_cannot_replace_newer_history` in the chat state tests. |
| History must stay ordered and duplicate HTTP/WebSocket deliveries must appear once. | `pagination_and_http_ws_echoes_are_sorted_and_deduplicated` |
| A replacement snapshot must recover missed messages, reactions, and deletions. | `reconnect_snapshot_recovers_messages_reactions_and_deletions` |
| Failed sends retain text and reply context, expose an error, and allow retry. | `failed_send_preserves_text_and_reply_and_allows_retry` |
| Empty or duplicate submissions must not start another send, and old completions must not clear a new draft. | `blank_draft_is_not_sent`, `failed_send_preserves_text_and_reply_and_allows_retry`, `old_completion_cannot_clear_new_channel_draft` |
| Text messages and replies use multipart form fields accepted by the backend. | `api::contract_tests::text_messages_use_multipart_and_optional_reply_field` |
| History pagination uses descending order and both RFC3339 timestamps and message IDs, preserving nanoseconds. | `history_pagination_sends_timestamp_and_id_in_descending_order`, `pagination_orders_equal_timestamps_by_id_and_keeps_live_edits` |
| The client rejects a history page for another channel. | `wrong_channel_history_is_rejected` |
| Members beyond the first page load; compact summary refreshes decode a bare array. | `member_pagination_and_summary_batch_follow_backend_contract`, `nonadvancing_member_pagination_fails_instead_of_looping` |
| Pins use the current route; custom reactions retain emoji IDs. | `pin_route_and_custom_reaction_preserve_message_and_emoji_ids` |
| Live reactions preserve insertion order, remove zero counts, and survive history fetches and late message echoes. | `live_reactions_replay_during_fetch_preserve_order_and_remove_zero_counts`, `late_message_echo_keeps_live_reactions_and_edits` |
| Presence uses `members` and `online/offline/away/busy` statuses; partial channel and user-join events refresh snapshots. | `ws::backend_contract_tests::presence_uses_members_and_status_instead_of_online_boolean`, `partial_channel_events_and_user_join_request_snapshot_refreshes` |
| Typing stop, pin state, backend edit timestamps, asynchronous previews, and attachment moderation events decode correctly. | `reaction_counts_pin_state_and_typing_stop_match_payloads`, `asynchronous_previews_and_attachment_moderation_are_delivered`, `preview_changes_replay_during_history_fetch` |
| Member avatars load through authenticated full-profile batches (50 IDs maximum), with nullable/missing pictures accepted. | `api::contract_tests::avatar_profiles_use_authenticated_batches_of_at_most_fifty` |
| Avatars are cached per session; repeated member/presence loads do not refetch them, and older responses cannot undo avatar updates. | `media::avatars::tests` |
| PNG, JPEG, GIF and WebP member pictures render in chat and the member list; changed pictures replace the cached texture, and removed/corrupt pictures use the default icon. | GTK `ui_smoke_window_has_header_controls_and_server_password` test. GIF/WebP are decoded to a static frame when GDK cannot load them directly. |
| Deleted/blocked messages cannot reappear after delayed HTTP responses. | `delayed_send_response_cannot_resurrect_deleted_or_blocked_message` |
| Deleted authors and video-preview metadata decode without breaking history. | `messages_accept_deleted_authors_and_preview_video_metadata` |
| Expired/reused/banned sessions trigger login; ordinary permission denials retain the session. | `rest_errors_distinguish_expired_banned_and_permission_denied_sessions` and the GTK smoke test. |
| WebSocket pings receive pongs, heartbeat frames keep the connection alive, and a revoked cookie ends reconnect attempts. | `ws::backend_contract_tests::ping_is_answered_and_revoked_session_stops_reconnecting` |
| Malformed frames and heartbeat acknowledgements are ignored. | Existing `ws::tests` |
| Ownership, multiple roles, open/restricted channels and global management privileges determine enabled controls; ban follows the actual manage-server guard. | `models::permissions::tests::owner_open_restricted_and_multiple_roles_follow_backend_rules` and GTK role-revocation checks. |
| Full-role and channel-override requests share authentication and reject wrong-channel responses. | `permissions_use_full_roles_and_identity_checked_channel_overrides` |
| WebSocket errors retain unknown/optional codes and remain visible without crashing or clearing login. | `websocket_errors_preserve_unknown_codes_and_optional_messages`, plus GTK error display. |
| Editing retains text on failure, retries successfully, handles permission errors, and leaves a newer edit dialog open after an old completion. | GTK `actions::tests::exercise`, called by the existing `ui_smoke` test. |
| Edit/delete/pin/unpin and pinned lists use correct routes and validated response identities. | `edit_delete_pin_unpin_and_pinned_list_follow_backend_routes` |
| Message length is measured in Unicode characters, up to 8,192. | `message_limit_counts_unicode_characters_not_utf8_bytes` |
| Deletion requires confirmation; cancelling sends no DELETE; pins/unpins update rows and the panel, including incoming updates. | GTK `actions::tests::exercise`. |
| Reply/pin navigation requests older history, highlights the target, and explains unavailable messages; old-channel pin responses are ignored. | GTK `actions::tests::exercise`. |
| Emoji pagination retains timestamp/ID precision; creation/deletion match JSON contracts and nonadvancing pages fail. | `emoji_pagination_keeps_equal_timestamp_ids_and_create_delete_payloads`, `nonadvancing_emoji_page_fails_instead_of_looping` |
| Emoji uploads enforce name/image byte/dimension/format limits; duplicate-name failures retain the form; images render from a compact session cache. | `models::emoji::tests::emoji_upload_checks_decoded_size_dimensions_format_and_unicode_name`, plus GTK upload/retry/deletion checks. |
| Grouped reaction pagination uses individual reaction row cursors and merges groups across pages. | `grouped_reaction_pagination_uses_oldest_row_not_group_order`, `reactions_reject_wrong_message_and_nonadvancing_pages` |
| Counts and own-reaction membership survive history races; older edit events cannot undo newer edits. | `participant_snapshots_replay_own_membership_during_history_and_respect_deletes`, `older_edit_events_cannot_replace_newer_edits_or_reaction_membership` |
| Unicode/custom reactions toggle, participant lists show users, incoming count events reconcile own membership, and read-only users can remove their own reactions. | GTK `actions::tests::exercise`. |
| Configuration round-trips; unread status, display-name fallback, file sizes, and invalid image data behave consistently. | Existing `config::tests`, `models::*::tests`, and `media::tests` |
| Own password changes first enable self-reset, then submit the password; recovery uses a body token and rejects expired/used links without echoing secrets. | `password_change_uses_self_reset_then_password_and_never_issues_an_admin_link`, `recovery_sends_secrets_in_body_and_redacts_untrusted_error_details`, GTK security/recovery retries. |
| Recovery accepts the backend's supplied link or token; diagnostics hide passwords and invalid links. | `recovery_rejects_bad_links_and_debug_output_hides_credentials`, GTK cleared-secret checks. |
| Historical connection reuse warns without rejecting a fresh login; devices list/revoke with the correct JSON, and only current revocation returns to login. | `historical_connection_violation_warns_without_rejecting_a_fresh_login`, `connected_devices_and_revocation_match_auth_contract`, GTK cancel/other/current-session controls and output. |
| Search validates required filters, dates, links and attachment presence/absence; cursors include timestamps and IDs. | `search_validates_real_filters_dates_and_optional_false`, `search_encodes_every_filter_and_equal_timestamp_cursor`. |
| Search retries retain filters, pagination deduplicates IDs, stale/cancelled completions are ignored, and summaries navigate into older history. | GTK `main_window::search::exercise`. |
| Mention completion preserves Unicode and surrounding text, inserts backend tokens, renders names as plain text, and gates everyone by permissions. | `chat::mentions::tests`, GTK composer checks. |
| Notifications parse author IDs without inventing a channel; REST paging/read bodies match the backend and events merge with records once. | `notification_author_is_not_a_channel_and_empty_author_is_supported`, `notification_pages_and_read_wrapper_are_separate_from_channel_settings`, `event_rest_echo_resolves_once_and_stale_unread_cannot_undo_success`. |
| Ephemeral events remain local until a persisted record exists; unresolved/denied channels expose no preview or destination; read retries preserve unread state. | `missed_message_has_no_destination_and_ephemeral_dismissal_has_no_persisted_id`, GTK inbox/read/dismissal/revoked-access checks. |
| Desktop delivery honors enabled, preview, sound, mentions and channel mute choices; desktop actions navigate through the same permission checks. | `desktop_preferences_and_permissions_hide_content`, `older_snapshot_cannot_restore_notifications_after_a_channel_is_muted`, GTK private D-Bus `Notify`/sound-hint/escaped-body/click/`CloseNotification` check. |
| Live channel unread state survives older snapshots and read markers do not regress when paging older history; inactive history is never fetched for unread counts. | `live_unread_survives_old_snapshot_and_older_history_cannot_regress_read`, GTK inactive-channel badge and HTTP-request assertions. |
| DM opening accepts 200/201, all four routes use conversation IDs, self-DMs are refused, and block/outsider errors retain login. | `direct_routes_accept_existing_and_created_and_preserve_blocked_session` (protocol fixtures). |
| Block lists decode summaries; blocking uses POST, unblocking DELETE, and self-blocks send no request. | `blocking_contract_uses_post_and_delete_and_never_blocks_self` |
| Role CRUD and assignment/removal use all six routes and seven explicit permission flags, including false and nullable colors. | `all_role_management_routes_use_complete_permission_objects` |
| Server PATCH omits untouched fields and sends explicit icon removals; password errors and Debug output redact secrets. | `administration_preserves_omitted_fields_and_uses_flat_position_contract`, `patch_omissions_and_explicit_removals_differ_and_debug_redacts_secrets`, `server_password_errors_keep_status_without_echoing_secrets` |
| DM updates decode personal unread snapshots; confirmed reads survive delayed snapshots and equal timestamps; own and already-read messages do not increment unread counts. | `dm_updates_decode_personal_unread_snapshots_without_becoming_public_channels`, `main_window::direct::tests` |
| Profile DM entry, chat sends/pins, peer pictures/presence, separate drafts, hide/reopen with retained history, blocking/unblocking and reconnect converge. | GTK `main_window::direct::exercise` |
| Failed role/server saves retain fields; role/color/permission editing, assignments, channel types/ordering/overrides/deletion and server icons work; denied management sends no writes. | GTK `main_window::administration::exercise`, `exercise_denied` |
| An empty backend opens setup and creates a first text channel; password-change session revocation returns to login and clears the password. | GTK `main_window::administration::exercise_setup_and_password` |

## Scope and manual checks

The client contracts were checked against the adjacent `papo-backend` repository,
commit `41cb00e` (2026-10-03): REST handlers, WebSocket event definitions, authentication
middleware, and pagination storage queries. Tests use local mock responses derived
from those definitions. They do not run the Go backend or validate a deployment's
database, authorization, migrations, proxy configuration, or end-to-end session policy.

The automated GTK check verifies widget structure and the new control
interactions against a local mock HTTP server. Visual rendering and full
interaction with the Go backend still require a desktop smoke test:

1. Run `cargo run --locked` and log in to a test server.
2. Switch channels while history is loading; verify that messages stay in their channel.
3. Interrupt connectivity before sending. Verify the error and retained draft/reply;
   switch away and back, reconnect, and retry.
4. While disconnected, create, edit, delete, and react to messages from a second
   client. After reconnection, verify the active channel's refreshed recent history.
5. Verify a successful send appears once even when both its HTTP response and
   WebSocket event arrive.
6. Use a private test server. Supply its password in the separate server-password
   field, then log in or register. Verify that missing/wrong server passwords show
   an error before attempting account authentication.

For local development, the backend must be running at `http://localhost:8080`.
The client does not start the backend. A healthy endpoint returns `OK` at
`http://localhost:8080/health`. Follow the backend repository's startup instructions
if nothing is listening on port 8080. Account and server passwords stay in memory
and are not saved in the configuration file.

For remote connections, enter the backend API address, which can differ from the
browser frontend's address. HTML returned by `/health` or `/server` usually means
the frontend or a proxy page was reached. The client reports the endpoint and
response type without displaying page contents. Use HTTPS when the backend sets
Secure authentication cookies.

Initial/reconnect history loads the latest backend page (up to 100 messages).
Scrolling near the top automatically fetches older pages with timestamp and ID cursors; failed pages expose “Tentar novamente”.
Reconnect replaces the visible history with the latest page to recover missed
edits/deletions; older pages can be loaded again. Reaction events update counts
in place without discarding loaded history.

Presence activity is throttled to once per 30 seconds while the user interacts
with the window; heartbeat frames every 25 seconds do not count as activity.
Session refresh runs every 12 hours and stops when the main component is dropped.
Revoked/expired authentication, including server password changes and bans,
returns to login. Permission errors on individual operations remain visible in chat.

Additional manual checks against a running updated backend:

1. Send a message with a reply, load several history pages, and verify messages
   with equal timestamps are not skipped or duplicated.
2. Add/remove reactions from a second client and verify order/count updates
   without losing older pages. Create, rename, reorder, and delete channels;
   deleting the selected channel must select another text channel or disable chat.
3. Observe automatic away while idle and return online after real input. Verify
   member updates after registration, nickname changes, and role changes.
4. Post/edit a link and verify asynchronously added, refreshed, and removed
   previews. Check sensitive attachment updates from moderation.
5. Change the server password from an authorized admin client; existing sessions
   should return to login and require the new server password.

The desktop includes DMs, blocking, native voice/video and administration.
Password security, supplied-link recovery, search, and notifications are implemented.
Voice channels open dedicated call controls. Steps 4–6 add authenticated media
controls, full profiles, and preferences.
Video playback uses GTK media controls inline, with the limits described below.

[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) maps every backend route to
current frontend coverage and separates the remaining work into delivery steps
with dependencies and acceptance checks. Steps 1–15 are implemented.

Member pictures are fetched from `/users/profile_batch` after loading members;
compact `/users` and `/users/user_summary_batch` responses do not contain avatars.
Decoded textures are shared by message rows and the member list for the current
session. Reconnect and `avatar_update` events refresh pictures, including your
own sidebar avatar. Picture-fetch failures leave the current icon/picture in
place without interrupting chat. Check on a real server that another member's
avatar change becomes visible without restarting or changing channels.

## Using and checking steps 1–3

- Message rows show **Editar**, **Excluir**, **Fixar/Desafixar**, **Adicionar
  reação**, and **Quem reagiu**, according to your account/channel permissions.
  Exclusion asks for confirmation. Editing retains its form on failure and
  offers **Salvar edição** and **Cancelar edição**.
- **Fixadas** opens the channel's pinned messages. Clicking one opens and
  highlights it in history, loading older pages when needed. Reply references
  use the same navigation. **Cancelar busca da mensagem** stops navigation.
- **Adicionar reação** opens a common Unicode palette, an entry for another
  Unicode emoji (press Enter), and server custom emojis. Clicking an existing
  reaction toggles your own membership. **Quem reagiu** loads all participants.
- **Emojis** lists server emojis. Owners or users with `manage_server` can
  select and upload PNG/JPEG/GIF/WebP images, up to 256 KB and 512 × 512 pixels.
  An emoji's creator can also delete their own emoji. Deletion asks for
  confirmation. GIFs render as static thumbnails.
- Role updates and reconnect refresh access; denied operations retain login
  and refresh access. Opening edit/emoji management/pickers also revalidates
  access. Server errors stay visible, including unfamiliar WebSocket codes.

On a disposable Go backend, check these flows with owner, authorized-role,
ordinary-member and read-only accounts, including changes from a second client.
Verify permission revocation, pin limits, duplicate emoji names, custom emoji
removal, disconnected edit retries, and replies to deleted/older messages.
The automated mocks do not replace this real-server validation.

Latest local validation: 114 headless tests passed, the expanded GTK smoke test
and native-worker cleanup test passed separately, and the application built.
The real Go SFU probe passed direct ICE, forced TURN relay, active speakers,
mute/leave and native virtual-device capture. The
`xvfb-run` launcher is not installed locally; CI still uses its configured
Xvfb environment. On a desktop with an available display, the same GTK test
can be run directly:

```sh
cargo test --locked ui_smoke -- --ignored --test-threads=1
```


## Using and checking steps 4–6

- Use the attachment button beside the composer to select local files. Each
  selected file has a removal button. Send text with files, files alone, or
  a reply with files. Progress counts file bytes sent; completion waits for
  the server response. **Cancelar envio** and failed sends keep the draft,
  reply, and files. Confirm the history before manually retrying a timeout
  or cancellation: the backend may have committed the message already.
- Up to 10 files, 100 MiB each, 8,192 Unicode characters, and 110 MiB per
  multipart request are checked before sending; room is reserved for form
  overhead. Files stream from disk. Changed file sizes fail visibly.
- **Baixar arquivo** opens the system save dialog and streams with session
  cookies. Private temporary files are cleaned up on error/cancellation;
  destination files are replaced only after a staged save completes.
  Pending media stays hidden, sensitive media requires **revelar**, and a
  blocked attachment removes the entire message. Late responses cannot
  recreate removed rows or apply to a previous channel/session.
- Preview images hydrate for loaded history. URLs are clickable HTTP/HTTPS
  links. Image decoding is bounded and cached as at most 32 static textures
  of 680 × 460 pixels (under 40 MiB). Their presentation size reaches
  720 × 480, including small source thumbnails, and shrinks in narrow windows.
  Base64 and image decoding run on bounded background workers; late results
  cannot restore blocked or deleted media.
  Full preview metadata is fetched even without initial image fields.
  Four media requests run concurrently. **Carregar imagem/miniatura** can
  reload failed or evicted thumbnails. GIF previews use a static frame.
- **Reproduzir vídeo** downloads the authenticated backend relay to a private
  temporary file before inline GTK playback; video attachments use their
  authenticated download route and the 100 MiB attachment limit. GTK's installed
  media backend/codecs determine supported formats; errors appear beside the
  player. Closing a player pauses and clears its stream and removes its file;
  late completed downloads cannot reopen a closed player. Preview videos are
  limited to 256 MiB and two retained players. Seeking
  becomes local after the download finishes. Streaming Range playback and
  real-deployment codec interoperability remain acceptance checks.
- Click a chat author's name/avatar or a member row for a full profile.
  **Meu perfil** opens your nickname, description, custom status message,
  typing phrase, and separate automatic/away/busy selector. Profile failures
  keep the form. Avatar/banner replacement supports PNG/JPEG/GIF/WebP up
  to 2 MiB; avatars are limited to 512 pixels and banners to 2,048 pixels
  in each dimension. Both images can be removed. Image replacement retains
  unsaved profile text. Missing/corrupt pictures fall back gracefully.
- **Preferências** saves theme, chat text size/spacing, timestamps, avatars,
  and notification enabled/preview/sound/mentions. The previous applied
  configuration remains on failure; the edited form stays available to retry.
  A successful save applies immediately and is restored from whoami on login.
  Step 9 applies these choices to desktop delivery and inbox previews.
- **Preferências do canal** saves all/mentions/off notifications for the
  selected channel. It does not send a read-marker update. Profile/avatar and
  role events refresh supported live views; the backend broadcasts no banner
  update, so other clients see banner changes when reopening the profile.

New automated evidence:

| Requirement | Evidence |
| --- | --- |
| Files-only/text/reply multipart uses repeated `attachments`, streams data, reports bytes. | `attachment_messages_stream_repeated_files_and_preserve_reply` |
| Unicode/count/file/body limits and profile image dimensions/formats are checked. | `api::features::tests` |
| Profile text, manual status, avatar/banner replacement/removal, settings wrapper and camelCase fields match the backend. | `account_writes_follow_separate_profile_status_image_and_settings_contracts`, `avatar_and_banner_uploads_send_validated_base64_and_format` |
| Cookies authenticate binary downloads/video relay; byte limits, private permissions, exact saved bytes and cleanup hold. | `authenticated_media_is_bounded_streamed_saved_and_cleaned` |
| Failed/cancelled attachment drafts retain selections; blocked moderation tombstones a whole message across delayed history. | `file_only_draft_retains_selection_on_failure_and_cancel`, `blocked_moderation_tombstones_whole_message_and_rejects_late_history` |
| GTK upload failures/retries/cancel, history previews, relay/player construction, reveal/reset, stale/deleted media rejection, and bounded cache. | `transfers::exercise`, included in `ui_smoke` |
| GTK profile failure/retry, stale reads, manual status, image removal, preferences failure/retry and restored login settings, per-channel preferences without a read marker. | `account::exercise_preferences_and_profiles`, included in `ui_smoke` |

On a disposable Go backend, also verify multiple real files, native picker/save
cancellation, network loss during large transfers, pending→sensitive→blocked
moderation, avatar/banner replacement from two clients, profile limits, and
settings after logout/login. Exercise a real video relay with the desktop's GTK
media codecs. Automated tests use backend-derived mocks and do not establish
production or Go-backend acceptance. They simulate file-picker results; native
portal dialogs and real video decoding need desktop checks.


## Using and checking steps 7–9

- **Segurança** changes your password and lists connected session IDs with creation
  and expiry dates. Password changes prepare the backend's self-reset flag before
  submitting the new password. Failure retains the form for correction. Successful
  own-password changes keep existing sessions, matching backend behavior.
- **Encerrar sessão** and **Encerrar todas as sessões** ask for confirmation.
  Revoking another device preserves this session. Revoking the current device or
  all devices returns to login. The backend does not supply device names or a
  current-session marker; a session check determines the result of single revocation.
  A connection-violation warning asks you to review sessions and change your password.
- At login, **Tenho um link de recuperação** accepts an administrator-supplied
  token or `/passwordchange/TOKEN` link for the configured API. **Salvar nova senha**
  consumes it once. Invalid, expired, and used links have the same error. Success
  clears secret fields; closing clears them too. Recovery does not request email
  or automatically log in. Recovery revokes existing sessions on the backend.
- **Pesquisar** supports text, author, permitted channel, mentioned member, dates
  (`AAAA-MM-DD`, inclusive), links, attachment presence/absence, and ascending or
  descending order. Choose at least one filter. **Mais resultados** uses both
  timestamp and ID; **Cancelar pesquisa** cancels pending work. Select a result
  to load its conversation and highlight the full message. Missing/deleted or
  denied targets show an error in the chat.
- Type `@` and a name fragment, then use the composer's **@** button to choose a
  member. **@everyone** appears only with permission. Sent tokens render as names
  in messages, reply references, search, and notification previews; deleted users
  use a fallback name. Untrusted names remain plain text.
- **Notificações** opens the inbox. **Mais notificações** paginates; **Marcar lida**
  persists a read state with retries. **Dispensar** dismisses a live ephemeral event
  locally. The sidebar count reflects loaded unread notifications; `+` indicates
  more pages. Reading notifications and reading a channel are separate operations.
  Opening a resolvable notification marks it read and navigates to its message.
- Live notifications resolve channels through received messages or REST records.
  Unknown destinations have a generic label; denied channels expose no content.
  Reconnect refreshes persisted notifications and channel snapshots. Missing
  ephemeral events cannot be recovered from the backend; unresolved ephemeral
  events stay generic when their message was missed.
- While running, Linux desktop notifications use the session D-Bus notification
  service. Settings control delivery, previews, mentions, channel mute, and sound;
  sound suppression is sent as a desktop hint. Desktop clicks open resolvable
  messages. A desktop without the service still has the inbox. The private D-Bus
  mock used by the GTK test does not send notifications to your real desktop.
- Incoming messages update channel unread dots. Successful latest-history reads
  advance the local marker to match the backend; older pages preserve it. Live
  messages in the selected channel trigger a debounced latest-history read while
  the main window is active. Inactive channels are never read just to count unread.

On a disposable Go backend with two clients, additionally verify:

1. Change a password under the configured password policy, log out, and log in with
   the new password. Consume a supplied recovery token once, reject reuse/expiry,
   and verify other authenticated clients are revoked. Revoke another device, then
   the current device; confirm the distinct UI outcomes and historical reuse warning.
2. Search a dataset exceeding one page, with equal timestamps, using every filter
   and both sort orders. Cancel and replace requests during a slow connection;
   navigate to old messages and targets deleted or restricted by another client.
3. Send direct mentions, replies, and authorized everyone mentions from a second
   account. Check inbox and desktop delivery, settings, per-channel off/mentions/all,
   generic missed ephemeral events, read state after reconnect, and desktop clicks.
4. Keep one channel inactive while messages arrive. Verify its dot without history
   requests, switch to it to advance its channel read marker, and check that inbox
   notifications stay unread until explicitly opened or marked. Revoke channel
   access from another client and confirm previews/navigation are removed.

The mocks validate client contracts and interactions. These real-backend and
host-desktop acceptance checks have not been run in this implementation session.


## Using and checking steps 10–12

- Open another member's **Perfil**, then **Mensagem direta**. Conversations appear
  under **Mensagens diretas** with peer pictures, presence and unread counts. They
  use the same replies, attachments, reactions, editing, deletion and pins as chat.
  Drafts belong to each public channel or DM. The close button hides a DM without
  deleting its history; opening the profile again restores it.
- **Bloquear usuário** disables DMs in both directions. **Usuários bloqueados**
  lists your blocks and provides **Desbloquear**. Blocking preserves ordinary
  server messages. Opening a blocked DM shows an error and keeps you logged in.
- Owners and members with the relevant management permission see **Administração**.
  Its **Cargos** tab creates/edits/deletes roles, colors and all seven permission
  flags. **Membros** assigns/removes a selected role and shows current roles.
  **Atualizar administração** reloads lists without discarding entered values.
- **Servidor** edits the name, icon and public/private mode. Only changed fields
  are included in PATCH. Check **Definir / alterar senha do servidor** to submit
  a password. Private servers require a password; changes/removal revoke sessions.
  The app revalidates the actor's session and returns to login when revoked.
- **Canais** creates text, category and voice entries, edits names/topics, moves
  existing channels between positions 1 and the channel count, deletes with
  confirmation, and replaces/removes a selected role's four channel permissions.
  The list is flat; categories do not create nested groups. Voice entries can be
  managed, while joining calls remains step 14. Permission overrides are complete
  boolean objects; removing an entry restores the backend's inheritance rules.
- A backend without a server record opens **Criar servidor** after account login.
  Setup creates the server and a **geral** text channel. If channel creation fails
  after the server is created, administration can create it without recreating the
  server. REST failures preserve forms. Deleting or restricting the active channel
  moves/clears chat while retaining other drafts.
- Access and DM lists refresh on reconnect, relevant events and every 30 seconds.
  The backend does not broadcast block changes or role-definition edits. The
  polling interval is therefore the maximum normal delay for those remote changes;
  an operation denied by the backend disables stale access immediately.

On a disposable Go backend, additionally verify:

1. Use two member accounts to open existing/new DMs, send replies/files/reactions,
   hide/reopen, and reconnect. A third account must receive 404/403 for DM detail
   and message history. Protocol mocks cannot prove backend authorization.
2. Block in either account. Verify read/send/reaction errors, retained login,
   unchanged public chat, and reopening after unblock, including another device.
3. Use owner, authorized-role and ordinary accounts to create/edit/delete roles,
   assign/remove them, and check inherited channel access and disabled controls.
4. On a fresh database, register/login and complete setup. Save a name/icon only;
   verify public mode and password remain unchanged. Change/remove the password;
   verify all clients return to login and authenticate with the resulting policy.
5. Create all channel types; rename/topic, reorder, set/remove role overrides,
   delete or restrict the active channel from a second client, and verify snapshots,
   selection and unrelated drafts. Repeat failed saves and position conflicts.

The automated checks use local HTTP/WebSocket/D-Bus fixtures and actual GTK
widgets. They do not mutate production data or substitute for these multi-account
checks against a running Go backend.

## Using and checking steps 13–14

Owners and members with `manage_server` can open **Moderação e auditoria**.
Choose a member or paste a UUID to ban/unban someone absent from the list.
Each mutation has a confirmation; banning the server owner is disabled.
**Gerar recuperação** displays a one-use link and its expiry; **Copiar link**
performs the explicit clipboard operation. Changing the selected recipient, closing the window or losing
permission clears the link. Links are accepted by the existing login recovery
screen. Audit history is read-only, with action, actor UUID, entity type and
RFC3339 date filters, ascending/descending order and cursor pagination.
Invalid filters and failed operations retain input. Audit metadata values are
withheld from the viewer and Debug output to avoid exposing account secrets.

Click a voice channel to open compact audio controls above the sidebar account
footer, then click **Entrar na voz**. The gear opens microphone/camera settings
in a separate window. Calls start muted, matching the backend; use the microphone
button to speak. Participants appear beneath their rooms before joining and show
mute state and active speakers. Camera/screen buttons beside connected participants
open a separate stream viewer. Active speakers show a microphone badge, including
when avatar display is disabled. A short cue plays once on local call connection
if account sounds are enabled. The screen button next to the gear starts screen
sharing; device options stay in settings. **Sair da voz** or **Fechar voz** releases capture
and media connections. Switching devices ends the current call; join again to
use the selection. Voice channels with `connect_voice` can appear independently
of text read permission. Chat, heartbeat and reconnect continue during calls.
A dropped connection ends the call; reconnecting requires an explicit join.
Revocation, deletion, a failed device or SFU negotiation error releases audio.

Voice uses the native GStreamer `webrtcbin` engine through an embedded Python GI
worker with private JSON-line IPC. Audio is Opus at 48 kHz, with eight receive
slots matching the backend defaults and RFC6464 audio levels. The app requests
fresh authenticated ICE configuration for each call, sends SDP/ICE through the
owning WebSocket, and discards queued voice frames on reconnect. TURN credentials
never appear in process arguments, saved settings or Debug output. The worker's
raw diagnostics are suppressed; errors shown in GTK contain no credential URIs.
Runtime API reference: [GStreamer WebRTC documentation](https://gstreamer.freedesktop.org/documentation/webrtc/).
Camera and screen sharing extend this worker in step 15, described below.

Install the additional runtime on Fedora:

```sh
sudo dnf install python3-gobject python3-gstreamer1 gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free libnice-gstreamer1
```

Automated coverage includes ban/unban request bodies (including `false`), admin
versus own recovery contracts, token redaction, audit filter/date/cursor handling,
ICE authentication and credential redaction, actual inbound voice event shapes,
route snapshots, muted startup, participant/speaker rendering, stale worker
completions, dropped connections, SFU errors, deleted channels and revocation.
The expanded GTK smoke test exercises these controls alongside login and chat.
A separate native-worker test verifies process termination and reaping:

```sh
cargo test --locked native_voice -- --ignored
```

The real SFU probe imports the adjacent Go backend's `internal/webrtc` package;
it creates no database, production accounts or production requests. It uses two
independent native clients, a local authenticated TURN server, synthetic capture
and decoded-audio sinks. It checks audible bidirectional samples, Opus SDP/ICE,
active speakers, full/empty audio routes, mute, leave and missing-device cleanup.
TURN credentials contain reserved characters to exercise URI encoding. A Go
compiler matching the backend's `go.mod` and the adjacent backend checkout are
required:

```sh
cd tests/voice-sfu
go build -mod=readonly -o /tmp/papo-voice-sfu .
cd ../..
python3 tests/voice_probe.py
# Desktop capture verification: private virtual PulseAudio/PipeWire source.
# Requires pactl and gst-launch-1.0; does not open the physical microphone.
python3 tests/voice_probe.py --capture
```

The capture probe creates temporary private audio nodes, enumerates/selects the
virtual source through the same device API used by GTK, streams it through the
SFU, and removes the nodes afterward. CI includes the native cleanup test; the
optional **voice-sfu** workflow-dispatch job runs real audio when supplied a
backend repository and revision. No audio server is needed for its synthetic
capture path. These tests establish native interoperability, beyond protocol
mocks. Physical microphone/speaker quality and the deployed server's firewall,
TURN/TLS configuration and permissions still need checks with two real accounts.
On a disposable database, also check banning another connected client, unban,
one-use recovery expiry/reuse, owner protection and denied audit access. No
production moderation or password changes were made during this implementation.


## Step 15: camera and screen sharing

Join a voice room, choose a camera from the camera dropdown and click **Ligar
câmera**. **Desligar câmera** releases it. **Compartilhar tela** opens the Linux
desktop's permission picker for one window or monitor; **Parar compartilhamento**
closes its portal session and releases the PipeWire descriptor. Cancelling or
denying the picker keeps the audio call available. No capture starts on login,
room selection or device enumeration. Camera and screen can run together; wait
for each operation to finish before starting the other.

Members with active media have **Ver câmera** and **Ver tela** buttons. The
viewer switches its single subscription explicitly; **Parar de assistir** stops
forwarding. Old pictures are cleared on switches, publisher removal, stopped
publication, missing frames, disconnect, logout and permission loss. Calls do
not automatically rejoin after reconnecting. Camera selection applies to the
next start; stop the active camera before changing it.

The current media configuration targets the backend defaults: VP8, six video
receive slots and eight Opus receive slots. Two reusable publishing senders keep
the SDP within the backend's 17-media-section limit. Each capture restart uses a
fresh SSRC and sends its media intent before a serialized offer. A stopped
capture retains its negotiated sender but stops RTP and clears the backend's
active media role. This avoids the native transceiver reactivation failure
found in the real SFU prototype. The backend's unused outbound `VoiceOffer`
struct is not used as evidence for server-initiated video negotiation.

Publishing currently uses 640×360 at 15 fps; the GTK viewer displays up to 10 fps.
Only one unacknowledged video frame may cross IPC, with a 90 KB JPEG limit, so a
slow UI does not build an unbounded frame queue. The backend sends no video-slot
mapping/subscribe acknowledgement; using one viewer allows deterministic use of
`papo-video-0`, with a decoder reset before each new subscribe. A transmission
that never produces frames can be selected again after the timeout.

Additional Fedora screen-capture runtime (alongside the voice packages above):

```sh
sudo dnf install pipewire-gstreamer xdg-desktop-portal
# Install the portal backend matching the desktop, e.g. xdg-desktop-portal-gnome.
```

The [ScreenCast portal contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
uses CreateSession, SelectSources, Start and OpenPipeWireRemote. The worker keeps
permissions nonpersistent, closes requests/sessions on cancellation, handles
Session.Closed and closes the received descriptor. There is no direct desktop
capture fallback. Recent portals' stream serial is preferred when available;
older portals' node identifier is supported by the installed PipeWire plugin.

Automated checks (build the SFU fixture as described above first):

```sh
# Private D-Bus only: does not open your desktop picker or capture anything.
dbus-run-session -- python3 tests/portal_probe.py
# Real backend, three native clients, synthetic video/audio, no physical devices.
python3 tests/video_probe.py
python3 tests/video_probe.py --relay
```

The portal tests cover success/descriptor cleanup, denial, cancellation while a
request is pending, and desktop revocation. The SFU probe verifies decoded VP8
camera/screen frames over repeated starts/stops, camera-first and screen-first
intent ordering, switching a viewer between publishers, unsubscribe, removal,
missing-camera recovery, concurrent audio, bounded IPC and the SDP size limit.
The GTK smoke test clicks the new controls, preserves the audio call after a
capture error, rejects queued frames from a previous selection and clears a
removed publisher's picture. CI runs the private-portal test on pushes/PRs;
the optional backend workflow runs native video with direct ICE and TURN.

For a user-attended desktop check:

```sh
python3 tests/screen_picker_probe.py
```

Select a test window. This reads ten native PipeWire video buffers into a discard
sink, then closes the capture and portal session. It saves and transmits nothing.
This check passed locally on 2026-10-06 with the real desktop portal. The probe
explicitly requests video caps; an unconstrained discard sink can incorrectly
negotiate a media type and produce a PipeWire “target not found” error.

Local validation: 114 headless Rust tests, the GTK workflow test, native process
cleanup, four private-portal tests, actual SFU audio/video probes and the real
portal buffer probe. Physical camera quality, long calls, desktop-specific picker
behavior and the deployed server's video codec/TURN policy remain manual checks
with two or more authorized accounts. No production calls were made by the tests.

## Rich embed migration

The default Rust suite verifies the nested `/embeds` contract, custom multipart
POST and JSON PUT payloads, embed-only messages, Unicode/combined text limits,
safe URLs, supported video MIME types, complete WebSocket replacement/clearing
and concurrent history/reconnect journals. These tests use local fixtures.

The existing `ui_smoke` workflow also exercises rich cards, ordered inline fields,
plain text/unsafe-link rejection, the custom composer form, draft clearing,
POST/WebSocket ordering, shared cached thumbnail cleanup, narrow layouts and
delayed-edit/stored-media safeguards. Run the full workflow with the virtual-display
command above, or focus on embeds with:

```sh
PAPO_UI_CASE=embeds dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

For a manual backend check, send a URL and observe its automatic card. Create
a custom card with color, author, footer and inline fields, send it without text,
edit/remove its fields, and verify a second authorized client receives each
complete replacement. Exercise a thumbnail and an HTTPS video with a supported
codec. Recheck the card after switching channels and resizing the window.
Stored-media editing and unavailable full images are explained in
[EMBEDS.md](EMBEDS.md); their limitations originate in the inspected backend.

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
Updates arriving every frame must not restart presentation indefinitely. Rapid
switches reject the old response; empty history, request errors and explicit
reader input all release presentation. Downloads are not required for these
checks and no remote server requests are made.

```sh
PAPO_UI_CASE=channel-opening dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

On a desktop, log in and switch between read and unread channels with long
messages. History should first appear at its intended position without showing
an intermediate scroll location. Images can still load afterward.

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
