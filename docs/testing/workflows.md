# Feature workflows and acceptance checks

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

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
Voice channels open dedicated call controls. Authenticated media controls, full
profiles and preferences are implemented.
Video playback uses GTK media controls inline, with the limits described below.

The feature workflows below cover the implemented account, messaging, media,
administration and call controls. Remaining investigations and deferred product
work are tracked in [Follow-ups](../FOLLOW_UPS.md).

Member pictures are fetched from `/users/profile_batch` after loading members;
compact `/users` and `/users/user_summary_batch` responses do not contain avatars.
Decoded textures are shared by message rows and the member list for the current
session. Reconnect and `avatar_update` events refresh pictures, including your
own sidebar avatar. Picture-fetch failures leave the current icon/picture in
place without interrupting chat. Check on a real server that another member's
avatar change becomes visible without restarting or changing channels.

## Message actions, reactions and emoji management

- Message rows show **Editar**, **Excluir**, **Fixar/Desafixar**, **Adicionar
  reação**, and **Quem reagiu**, according to your account/channel permissions.
  Exclusion asks for confirmation. Editing retains its form on failure and
  offers **Salvar edição** and **Cancelar edição**.
- **Fixadas** opens the channel's pinned messages. Clicking one opens and
  highlights it in history, loading older pages when needed. Reply references
  use the same navigation. **Cancelar busca da mensagem** stops navigation.
- **Adicionar reação** opens a common Unicode palette, an entry for another
  Unicode emoji (press Enter), and server custom emojis. Clicking an existing
  reaction toggles your own membership. **Quem reagiu** opens a native **Reações**
  window with selectable emoji/count filters, animated custom emojis, avatars,
  role-colored nicknames and usernames. Clicking a member closes the reaction
  dialog and opens their profile. Member details use the existing cache/batch
  pipeline, with 50 rows initially and **Mostrar mais membros** for further rows.
  Filter changes reuse the fetched reactions, and avatar/name updates preserve
  rows and selection. Escape closes the window.
- **Emojis** lists server emojis. Owners or users with `manage_server` can
  select and upload PNG/JPEG/GIF/WebP images, up to 256 KB and 512 × 512 pixels.
  An emoji's creator can also delete their own emoji. Deletion asks for
  confirmation. Custom GIF/WebP/PNG emojis animate when enabled and within the
  decoded-animation budget.
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


## Attachments, profiles and preferences

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
  These choices apply to desktop delivery and inbox previews.
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


## Account security, search and notifications

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


## Direct messages and administration

- Open another member's **Perfil**, then **Mensagem direta**. Conversations appear
  under **Mensagens diretas** with peer pictures, presence and unread counts. They
  use the same replies, attachments, reactions, editing, deletion and pins as chat.
  Drafts belong to each public channel or DM. The close button hides a DM without
  deleting its history; opening the profile again restores it.
- **Bloquear usuário** disables DMs in both directions. **Usuários bloqueados**
  lists your blocks and provides **Desbloquear**. Blocking preserves ordinary
  server messages. Opening a blocked DM shows an error and keeps you logged in.
- Owners and members with the relevant management permission see **Administração**.
  Its **Cargos** tab creates/edits/deletes roles, colors and the available permission
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
  managed; joining/leaving calls is covered in the [voice guide](voice.md).
  Permission overrides are complete
  boolean objects; removing an entry restores the backend's inheritance rules.
- A backend without a server record opens **Criar servidor** after account login.
  Setup creates the server and a **geral** text channel. If channel creation fails
  after the server is created, administration can create it without recreating the
  server. REST failures preserve forms. Deleting or restricting the active channel
  moves/clears chat while retaining other drafts.
- Access and DM lists refresh on reconnect, relevant events and paced periodic
  fallbacks. Healthy inactive sessions use a slower fallback interval.
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
