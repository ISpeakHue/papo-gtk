# Frontend feature implementation plan

Reviewed: 2026-10-06. Backend baseline: adjacent `papo-backend`, commit
`41cb00e` (2026-10-03). Steps 1–15 were implemented on 2026-10-06. Implementation was validated with the Rust suite,
GTK interactions against local mock HTTP servers, and two native GStreamer
clients exchanging decoded audio/video through the actual Go SFU and local TURN,
plus user-approved screen capture through the real desktop portal.
Real-backend acceptance checks remain documented in `TESTING.md`.

## Scope and evidence

The backend exposes 73 method/path registrations in
[`routes.go`](../papo-backend/backend/internal/handlers/routes.go), plus
`GET /health` in [`main.go`](../papo-backend/backend/cmd/main.go): 74 routes,
including the WebSocket upgrade. The inventory below maps every route to
its handler and current frontend coverage. A route being called does not
mean all of its capabilities are available to the user.

Backend contracts were checked in handlers, models, services, middleware,
and the [WebSocket event definitions](../papo-backend/backend/internal/websocket/events.go)
and [voice event definitions](../papo-backend/backend/internal/webrtc/events.go).
Frontend evidence comes from [API methods](src/api/mod.rs),
[WebSocket parsing](src/ws/mod.rs), [window coordination](src/ui/main_window/mod.rs),
[chat controls](src/ui/chat/mod.rs), [sidebar](src/ui/sidebar/mod.rs),
[member list](src/ui/user_list/mod.rs), models, and [existing tests](TESTING.md).
Backend worker functions, migrations, rate limiting, storage, moderation
inference, and push delivery are server responsibilities rather than
missing frontend screens.

Already usable: account login/registration, separate server passwords,
session refresh/logout, text-channel navigation, paginated message history,
sending text and replies, Unicode/custom reactions, member avatars,
presence/typing, reconnect recovery, and several incoming update events.
Member avatars, full profiles and own profile editing are implemented.

Implemented in steps 1–3: effective role/channel access, visible WebSocket
errors, edit/delete controls with retries and confirmation, pin/unpin and the
pinned-message panel, reply/pin navigation into older history, Unicode/custom
emoji selection, reaction toggling/participants, and authorized custom emoji
creation/deletion. Custom images are cached as compact static textures per
session, including the first frame of GIFs.

Implemented in steps 4–6: streamed multipart attachments with progress and
cancellation, authenticated downloads and history thumbnails, sensitive-media
reveal, clickable previews, a GTK video relay prototype, full profiles and own
profile/image/status editing, and persisted appearance/notification preferences.
Voice audio, member moderation, recovery-link issuance and audit logs are
implemented in steps 13–14; camera/screen calls are implemented in step 15.

Implemented in steps 7–9: password changes and supplied-link recovery, connected
sessions and revocation, connection-violation warnings, filtered paginated search
with message navigation, member mentions, notification inbox/read controls,
desktop delivery, and live channel unread markers. Unresolved notification events
remain generic until a message or REST record supplies an authorized channel.

## Delivery order

Implement each numbered step as a separate reviewable change. Add its API
methods and models alongside the feature, rather than adding every unused
endpoint in one large change. Dependencies identify which steps must exist
first; independent steps can be scheduled separately.

| Step | Deliverable | Depends on |
| --- | --- | --- |
| 1 | **Implemented:** Permission resolution and actionable protocol errors | Existing authentication |
| 2 | **Implemented:** Edit/delete messages, pins, and message navigation | 1 |
| 3 | **Implemented:** Complete reactions and custom emoji management | 1 |
| 4 | **Implemented:** Uploads, thumbnails, downloads, and complete link previews | 1 |
| 5 | **Implemented:** Full member profiles and own profile editing | 1, media helpers from 4 |
| 6 | **Implemented:** Persisted appearance and notification preferences | 5 |
| 7 | **Implemented:** Password changes/recovery and connected devices | Existing authentication; 5 for account entry point |
| 8 | **Implemented:** Search with navigation to results | 2 |
| 9 | **Implemented:** Notifications, mentions, and live unread state | 2, 6 |
| 10 | **Implemented:** Direct messages and user blocking | 2, 5, 9 |
| 11 | **Implemented:** Role creation/editing and assignments | 1, 5 |
| 12 | **Implemented:** Server setup/settings and channel administration | 1, 11 |
| 13 | Member moderation, reset-link issuance, and audit logs | 7, 11 |
| 14 | Voice audio and call lifecycle | 1; media-engine prototype |
| 15 | Camera and screen sharing | 14 |
| Deferred | Push delivery when the desktop app is closed | Backend/desktop transport decision |

### Step 1 — Permission resolution and protocol errors

Status: Implemented. See the automated requirements and manual checks in
[TESTING.md](TESTING.md).

Fetch full roles with `GET /roles` and channel overrides with
`GET /channels/:channel_id/permissions`. Resolve server ownership, assigned
role permissions, and channel access using the backend's actual rules.
Embedded user role summaries contain names/colors/IDs but no permissions;
they cannot authorize controls by themselves. Gate composing, attachments,
moderation, pins, administration, and voice entry accordingly.

Add typed handling for WebSocket `error` frames and show operation failures
in the relevant screen. Refresh the current user's roles and accessible
channels after role changes and reconnect; permissions can also change
without a dedicated role-definition update event, so revalidate on entering
management views and after authorization failures. Preserve the existing
distinction between expired sessions and ordinary permission denials.

Acceptance: owner, authorized role, ordinary member, and denied channel
fixtures produce correct controls; permission revocation during an operation
shows the backend error without logging out. Unknown WebSocket errors remain
visible and do not crash the client.

### Step 2 — Complete message actions and navigation

Status: Implemented. See the automated requirements and manual checks in
[TESTING.md](TESTING.md).

Wire edit/delete controls to the existing API methods. Add edit/cancel state,
deletion confirmation, and errors that retain text. Editing is author-only;
deletion follows ownership/channel `delete_messages` rules. Add pin/unpin,
a pinned-message panel, and actual `message_pin` state updates. The pinned
list has no cursor contract; use its returned collection and handle the
backend's pin limit.

Build a reusable action to open a channel and highlight a referenced message,
for replies, pins, future search, and notifications. Use existing history
pagination and timestamp/ID cursors; there is no registered single-message
GET route. With only an ID available, searching older pages may be necessary.
Handle missing/deleted messages and denied channels explicitly. Keep newer
drafts/history protected from old asynchronous completions.

Acceptance: edits, deletes, pin/unpin, and navigation work through controls;
HTTP/WS echoes converge once; delayed history cannot undo changes; denied
actions retain the session; navigation handles targets outside the first page.

### Step 3 — Complete reactions and custom emojis

Status: Implemented. See the automated requirements and manual checks in
[TESTING.md](TESTING.md).

Add a Unicode/custom emoji picker, custom image cache, own-reaction indication,
remove/toggle action, and a list of users who reacted. Use GET/POST/DELETE
reaction routes and paginated `GET /emojis`; preserve custom emoji IDs rather
than substituting Unicode. Incoming count-only updates require reconciliation
when own-reaction membership matters.

Add authorized emoji upload/delete controls. Creation requires owner or
`manage_server`; deletion also supports the creator according to the handler.
Validate the backend's image/name/size limits and handle removed emoji IDs
without breaking old messages.

Acceptance: Unicode and custom reactions add/remove correctly, images render,
membership/counts remain correct across reconnect, emoji pagination completes,
and duplicate/oversized/unauthorized uploads show useful errors.

### Step 4 — Attachments and link media

Status: Implemented. See [TESTING.md](TESTING.md) for controls, validation,
and media prototype limits.

Extend `POST /messages` multipart submission with a file picker, file-only
messages, upload progress/cancellation, and retained text/reply/file selection
after failure. The backend limits content to 8,192 Unicode code points,
attachments to 10, each file to 100 MB, and the request body to 110 MB.
Do not automatically retry an ambiguous message creation timeout: it can
create duplicate messages.

Wire authenticated attachment downloads to a save dialog, fetch image
thumbnails, and render pending/sensitive moderation states with a deliberate
reveal interaction. A blocked attachment deletes the entire message; existing
tombstone handling must also prevent delayed media work from restoring it.
Use bounded, session-scoped media caching and streaming for large downloads.

Hydrate preview thumbnails for history as well as new WebSocket previews;
make URLs clickable. Add playback through the authenticated
`GET /link-previews/:preview_id/video` relay when video is available. Prototype
GTK media playback with the existing cookie client before choosing a media
dependency; don't assume an external player can authenticate the relay.

Acceptance: text+files, files-only and replies with files work; cancelled or
failed uploads retain drafts; download saves the expected bytes; history
thumbnails appear; sensitive/blocked transitions update correctly; media
responses for deleted messages or old sessions are discarded.

### Step 5 — Profiles, avatars, banners, and status

Status: Implemented. See [TESTING.md](TESTING.md) for controls, validation,
and media prototype limits.

Make member names/avatars open a profile view with full description, banner,
nickname, status, and role badges. Reuse existing authenticated profile
requests and the avatar cache; use `GET /media/:sha_hash` for banner media.
Keep profile batches at 50 IDs maximum, rather than the summary API's larger
batch limit.

Add own profile editing, avatar/banner upload or removal as supported by the
handlers, manual presence/status controls, and custom typing text. Keep manual
away/busy status distinct from the backend's transient presence/idle state.
Update the own sidebar and other clients through refresh/events.

Acceptance: nullable/deleted users and absent/corrupt media work; editing and
image replacement persist after login; stale profile responses cannot undo
changes; a second client observes supported live updates.

### Step 6 — Settings

Status: Implemented. See [TESTING.md](TESTING.md) for controls, validation,
and media prototype limits.

Apply `whoami.settings.config` on login and expose a preferences screen.
Persist through `PUT /users/settings` using the required `{ "config": ... }`
wrapper and complete validated configuration. Preserve unedited fields.
Implement theme, font size, density, timestamps, avatar visibility, and
notification enabled/preview/sound/mentions choices.

Add channel notification preferences through
`POST /channels/:channel_id/user/:user_id/settings` with `off`,
`only_mentions`, or `all`. That route changes notification preferences;
it does not mark a channel read. Notification delivery itself follows in 9.

Acceptance: preferences apply immediately, persist across restart/login,
serialize the backend field names correctly, and retain the previous value
if saving fails. Per-channel preferences round-trip separately from read state.

### Step 7 — Security and session management

Status: Implemented. See [TESTING.md](TESTING.md) for controls, automated
checks, desktop requirements, and remaining real-backend acceptance checks.

Add own password change, a connected-devices list, and controls to revoke
connections using the authenticated device/drop endpoints. Interpret the
`connection_violation` flag that is currently ignored. Revoke-current-session
and revoke-other-session operations need different UI outcomes.

Add a recovery form that consumes a supplied token/link through
`POST /auth/password_reset`, without logging or storing the secret. Handle
invalid, expired, and already-used tokens. A pasted-link form can ship before
optional desktop URI integration. There is no public email-based forgot-
password request route in this backend.

Support the existing self-reset flag/change-password flow deliberately:
`POST /users/self-id/reset` returns a response string and sets a legacy reset
flag, whereas resetting another user returns a one-use URL and expiry.
Do not decode both as the same response or invent an email recovery flow.

Acceptance: changed passwords work, reset tokens work once, revoked sessions
return to login, revoking another device keeps the current one connected, and
credential/token values never reach configuration or diagnostic logs.

### Step 8 — Search

Status: Implemented. See [TESTING.md](TESTING.md) for controls, automated
checks, desktop requirements, and remaining real-backend acceptance checks.

Add a search screen using `POST /search` with text, author, channel, mention,
date range, links, attachment filter, order, and cursor pagination. The
backend accepts `has: "link"`; attachment filtering uses `contains_attachment`,
not `has: "attachment"`. At least one filter is required.

Show result summaries and open/highlight their messages through step 2.
Search results are not full message objects, so load history rather than
constructing incomplete chat rows. Preserve the backend's permission filtering.

Acceptance: filters encode correctly, equal timestamps paginate without
duplicates, cancelled/old queries cannot overwrite new results, and a result
opens the correct conversation or explains deleted/denied content.

### Step 9 — Mentions, notifications, and unread state

Status: Implemented. See [TESTING.md](TESTING.md) for controls, automated
checks, desktop requirements, and remaining real-backend acceptance checks.

Add member mention completion and render backend tokens
`@mention(<@USER_UUID>)` as names. Offer `@everyone` according to the actual
role permission. Replies and mentions can generate persisted notifications.
Add the paginated notification inbox, mark-read requests, unread indicators,
and optional desktop notifications while the app is running, honoring step 6.

Parse `new_notification` and reconcile persisted records on reconnect. Its
`user_id` is the message author, not the recipient, and its payload lacks a
channel ID. Correlate it with received messages or REST notification records.
The `all` preference can generate an ephemeral event with no persisted row;
do not send that ID to the mark-read endpoint. If a message cannot be resolved,
show the notification without a fabricated navigation target. Adding
`channel_id` to this backend event would make navigation reliable even when
the corresponding message was missed; that is a separate backend improvement.

Update sidebar last-message/unread state on live messages, viewing, and
reconnect instead of relying only on the initial channel snapshot. History
listing itself advances the backend read marker to the newest returned
message; there is no separate channel mark-read route. Fetching inactive
channel history just to count unread messages can therefore mark it read.
Keep notification-read and channel-read state separate.

Acceptance: direct mentions/replies notify the correct user; event/REST echoes
deduplicate; ephemeral events are handled correctly; read state survives
reconnect; preview/sound/mute preferences work; denied-channel content stays
unavailable; desktop clicks open resolvable messages.

### Step 10 — Direct messages and blocking

Status: Implemented. See usage, automated checks, and real-backend acceptance
checks in [TESTING.md](TESTING.md).

Add a DM section, open-from-profile action, conversation list/detail, and hide
conversation action through all four `/dms` routes. A DM ID is its channel ID;
reuse message/reply/media/reaction handling with an explicit conversation
target instead of inventing a normal public channel object. Handle `dm_update`
snapshots, peer presence/avatar, unread state, and per-conversation drafts.

Add block/unblock controls and a blocked-users screen. Blocking restrictions
must follow the backend's DM rules; don't promise to erase or filter ordinary
server history. Hiding a DM hides the conversation, not its message history.

Acceptance: opening existing/new DMs handles both 200/201; self-DMs are refused;
two accounts can exchange messages; outsiders cannot read the conversation;
block errors retain login; hide/reopen, live updates, and reconnect converge.

### Step 11 — Role management

Status: Implemented. See usage, automated checks, and real-backend acceptance
checks in [TESTING.md](TESTING.md).

Build role list/create/edit/delete and assign/remove-member-role screens with
permission and color editors. Use the six role/assignment endpoints and
refresh role/member/access snapshots after saves and incoming assignment
events. Existing `role_add`/`role_remove` handling only refreshes summaries;
it does not provide management controls or complete access updates.

Acceptance: authorized users manage and assign roles, ordinary users cannot;
removing a role updates controls and accessible channels without restart;
failed saves preserve form data; inherited permissions match backend decisions.

### Step 12 — Server and channel administration

Status: Implemented. See usage, automated checks, and real-backend acceptance
checks in [TESTING.md](TESTING.md).

Add a server-creation wizard for a backend without a server record. Add server
name/icon/public/password settings and render the server icon. Prefer PATCH
for individual edits: omitted fields must stay omitted, not serialized as
null. PUT is a separate full-replacement contract with required fields; it
does not need a second user-facing editor just to exercise another verb.
Changing the server password revokes sessions, including the actor's.

Add channel/category/voice creation, rename/topic editing, position changes,
deletion, and role-permission overrides/removal. Read REST channel type from
`type`; channel-create WebSocket events use `channel_type`. Use the backend's
position contract rather than assuming the model supports nested categories.
Incoming channel changes refresh snapshots and repair selection; voice
activation is implemented in step 14.

Acceptance: setup leads into chat; server edits preserve untouched values;
password changes return to authentication; channel ordering persists; denied
changes are explained; deleting/restricting the active channel disables or
moves chat and preserves unrelated drafts.

### Step 13 — Member moderation and audit history

Status: Implemented. See [TESTING.md](TESTING.md) for controls, pagination,
recovery-link handling and validation.

Add ban/unban, generate another user's recovery link, and paginated audit-log
views with available actor/action/entity/date filters. Show the reset link
and expiry for deliberate copying; do not send it to anyone automatically.
Use the backend's actual authorization: the ban route currently requires
owner or `manage_server`, despite the existence of a `ban_members` model field.
Audit logs are read-only; there is no audit-delete endpoint.

Acceptance: restricted controls/requests follow actual guards, banned sessions
disconnect, unbanning works, issued links complete step 7, and logs paginate
and filter without exposing account secrets in client logs.

### Step 14 — Voice audio

Status: Implemented. The native GStreamer/Opus prototype passed direct ICE,
forced TURN relay, active-speaker and virtual-device capture checks against
the backend SFU before integration. Python GI is a runtime dependency; Rust
embeds and supervises the worker. See [TESTING.md](TESTING.md).

First make a small native media-engine prototype against this backend's SFU:
validate audio codecs, SDP/ICE exchange, GTK integration, device capture, and
TURN operation before committing to a dependency. Then add join/leave,
microphone selection/mute, participant state, active-speaker indicators, and
authenticated `GET /voice/ice-servers` handling.

Implement outbound `voice_join`, `voice_leave`, `voice_offer`, `voice_answer`,
`voice_ice_candidate`, and `voice_mute`; parse `voice_joined`, `voice_answer`,
`voice_ice_candidate`, `voice_state_update`, `voice_leave`,
`active_speaker_update`, and `voice_audio_routes`. Audio route snapshots map
SFU track slots to users, including an empty snapshot clearing old routes.
Preserve the connection that owns signaling; normal chat heartbeat/reconnect
must continue. Presence voice data can also populate channel participants.

Acceptance: two real clients exchange audio through direct and TURN paths;
mute/leave/device failure, room-full/forbidden errors, dropped WebSocket,
revocation, and deleted channels release capture and media connections.
Protocol mocks alone cannot prove audio interoperability.

### Step 15 — Camera and screen sharing

Status: Implemented. Native VP8 camera selection, permission-portal/PipeWire
screen capture, and a GTK viewer with explicit subscribe/unsubscribe controls.
One remote camera or screen can be viewed at a time. Publishing is 640×360 at
15 fps; bounded JPEG IPC displays up to 10 fps without accumulating UI frames.
Camera and screen can publish simultaneously; each start/stop is negotiated
serially with intent sent before the offer. Senders/MIDs are reused, fresh SSRCs
avoid replay conflicts after restarting capture, and RTP waits for the answer.
Stopped capture releases its device while keeping the sender available for reuse.

The actual SFU tests cover repeated starts/stops, screen-first negotiation,
switching slots/publishers, missing-camera recovery, concurrent audio, frame
backpressure, publisher removal, direct ICE and forced TURN. The portal tests
cover denial/cancellation, session revocation, Unix-FD transfer and cleanup.
A real desktop-picker probe received ten PipeWire buffers, then closed capture
without recording or transmitting them. GTK checks cover controls, cancellation,
stale-frame rejection and clearing removed publishers. Physical camera quality
and production network behavior remain manual checks.

Extend the validated audio engine with camera devices/video display,
`voice_camera`, screen-share start/stop, and track subscribe/unsubscribe.
Prototype Linux screen capture through the desktop's permission portal.
Implement the backend's actual video-slot negotiation and media-intent order;
verify renegotiation against running code rather than relying on the unused
`VoiceOffer` outbound struct alone.

Acceptance: camera and screen can start/stop repeatedly, subscriptions switch
publishers correctly, denial/cancellation is recoverable, removed publishers
clear tracks, and disconnect/logout releases camera and screen capture.

## Deferred capabilities and contract limitations

- Mobile push registration/removal is absent from the client, but registration
  accepts only `android` or `ios`. Linux desktop delivery while the app is
  closed needs a supported desktop transport/background design or backend
  changes. Step 9 covers notifications while the client is running.
- Notification events without `channel_id` cannot always be resolved after a
  missed message, particularly ephemeral `all` events. See step 9.
- Precise unread/read-position controls would benefit from an explicit backend
  read-marker endpoint. Existing history reads have side effects; the channel
  settings endpoint cannot substitute for it.
- Recovery-link desktop integration can follow the pasted-link form. There is
  no public recovery-email request API to implement.
- Backend transport and access rules remain authoritative. Existing structs,
  comments, or unused request models are not evidence that a workflow works.

## Verification for every step

1. Add focused API contract tests for new request bodies, pagination,
   multipart data, errors, and WebSocket payloads using the existing test
   infrastructure. Verify transitions and races where behavior depends on
   asynchronous state; don't write tests that only mirror trivial functions.
2. Extend GTK smoke checks for new controls and their enabled/visible states.
   Keep ordinary unit/contract tests headless, and run GTK under Xvfb in CI.
3. Validate integrated behavior against a disposable local backend with at
   least an owner, an authorized member, and an ordinary member. Use a second
   client for live updates, blocked DMs, sessions, and voice/video checks.
4. Run `cargo test --locked` and the GTK command documented in `TESTING.md`,
   record relevant manual checks, and update that document for the delivered
   requirements. Keep production moderation/password changes out of automated
   acceptance tests.

Steps 1–15 add API contracts, permission/state/media/security/search/notification
regressions, GTK control interactions with a stateful local mock server, and
desktop notification calls/actions against a private D-Bus service, moderation
controls and voice lifecycle checks. Native audio tests use the actual backend
SFU, a local TURN server, synthetic tones and native VP8 test patterns. CI runs headless/GTK/native-worker
tests; the optional workflow-dispatch SFU job takes a backend repository/ref.
Production server policy and physical-device checks remain manual.

Implemented in steps 10–12: participant DMs with existing message controls,
profile entry points, peer avatars/presence, personal unread snapshots and drafts;
block management; role/color/permission editing and assignments; fresh-server
setup; partial server settings and icons; flat channel/category/voice creation,
editing, ordering, deletion and permission overrides. Session revocation returns
to login. Reconnect and periodic refresh recover changes without backend events.

## Complete route inventory

Status definitions: **Working** = usable current behavior; **Partial** = route
used but material capabilities missing; **API only** = method/plumbing exists
without usable controls; **Missing** = workflow/API support absent;
**Deferred** = unsupported desktop push or an alternative contract without a
separate user workflow (server replacement uses the PATCH editor). Coverage is per route,
not an estimate of implementation effort or a percentage of features.

| Route | Backend handler | Frontend status | Step |
| --- | --- | --- | --- |
| `GET /health` | `HealthHandler` | Working | Existing |
| `POST /auth/register` | `RegisterHandler` | Working | Existing |
| `POST /auth/login` | `LoginHandler` | Working | Existing |
| `POST /auth/login_server` | `LoginServerHandler` | Working | Existing |
| `POST /auth/password_reset` | `ConsumePasswordResetHandler` | Working | 7 |
| `GET /auth/whoami` | `WhoamiHandler` | Working | 7 |
| `POST /auth/logout` | `LogoutHandler` | Working | Existing |
| `POST /auth/refresh` | `RefreshHandler` | Working | Existing |
| `GET /auth/connected_devices` | `ConnectedDevicesHandler` | Working | 7 |
| `POST /auth/drop_connection` | `DropConnectionHandler` | Working | 7 |
| `GET /users/blocks` | `ListUserBlocksHandler` | Working | 10 |
| `POST /users/:user_id/block` | `BlockUserHandler` | Working | 10 |
| `DELETE /users/:user_id/block` | `UnblockUserHandler` | Working | 10 |
| `GET /users` | `ListUsersHandler` | Working | Existing |
| `GET /users/:user_id/profile` | `ProfileHandler` | Working | 5 |
| `POST /users/profile_batch` | `ProfileBatchHandler` | Working | 5 |
| `POST /users/user_summary_batch` | `UserSummaryBatchHandler` | Working | Existing |
| `PUT /users/settings` | `UpdateSettingsHandler` | Working | 6 |
| `PUT /users/:user_id` | `UpdateUserHandler` | Working | 5 |
| `PUT /users/:user_id/status` | `UpdateStatusHandler` | Working | 5 |
| `GET /users/:user_id/notifications` | `ListUserNotificationsHandler` | Working | 9 |
| `PUT /users/:user_id/read_notification` | `ReadNotificationHandler` | Working | 9 |
| `PUT /users/:user_id/avatar` | `UpdateAvatarHandler` | Working | 5 |
| `PUT /users/:user_id/banner` | `UpdateBannerHandler` | Working | 5 |
| `PUT /users/:user_id/password` | `ChangePasswordHandler` | Working | 7 |
| `PUT /users/me/push-device` | `RegisterPushDeviceHandler` | Deferred | Deferred |
| `DELETE /users/me/push-device` | `RemovePushDeviceHandler` | Deferred | Deferred |
| `PUT /users/:user_id/ban` | `BanUserHandler` | Working | 13 |
| `POST /users/:user_id/reset` | `ResetUserHandler` | Working | 7, 13 |
| `POST /server` | `CreateServerHandler` | Working | 12 |
| `GET /server` | `GetServerHandler` | Working | 12 |
| `PUT /server` | `ReplaceServerHandler` | Deferred (PATCH editor covers partial edits) | 12 |
| `PATCH /server` | `PatchServerHandler` | Working | 12 |
| `GET /channels` | `ListChannelsHandler` | Working | 9, 12 |
| `POST /channels` | `CreateChannelHandler` | Working | 12 |
| `PUT /channels/:channel_id` | `UpdateChannelHandler` | Working | 12 |
| `PUT /channels/:channel_id/change_position` | `ChangeChannelPositionHandler` | Working | 12 |
| `DELETE /channels/:channel_id` | `DeleteChannelHandler` | Working | 12 |
| `GET /channels/:channel_id/pinned` | `ListPinnedMessagesHandler` | Working | 2 |
| `GET /channels/:channel_id/permissions` | `GetChannelPermissionsHandler` | Working | 1 |
| `PUT /channels/:channel_id/permissions/:role_id` | `UpdateChannelPermissionsHandler` | Working | 12 |
| `DELETE /channels/:channel_id/role/:role_id` | `DeleteChannelRoleHandler` | Working | 12 |
| `POST /channels/:channel_id/user/:user_id/settings` | `UpdateChannelUserSettingHandler` | Working | 6 |
| `GET /dms` | `ListDirectMessagesHandler` | Working | 10 |
| `POST /dms` | `OpenDirectMessageHandler` | Working | 10 |
| `GET /dms/:dm_id` | `GetDirectMessageHandler` | Working | 10 |
| `DELETE /dms/:dm_id` | `HideDirectMessageHandler` | Working | 10 |
| `GET /channels/:channel_id/messages` | `ListMessagesHandler` | Working | Existing |
| `POST /messages` | `CreateMessageHandler` | Working | 4 |
| `PUT /messages/:message_id` | `UpdateMessageHandler` | Working | 2 |
| `DELETE /messages/:message_id` | `DeleteMessageHandler` | Working | 2 |
| `POST /channels/:channel_id/messages/:message_id/pin` | `PinMessageHandler` | Working | 2 |
| `DELETE /channels/:channel_id/messages/:message_id/pin` | `UnpinMessageHandler` | Working | 2 |
| `POST /channels/:channel_id/messages/:message_id/reactions` | `AddReactionHandler` | Working | 3 |
| `GET /channels/:channel_id/messages/:message_id/reactions` | `ListReactionsHandler` | Working | 3 |
| `DELETE /channels/:channel_id/messages/:message_id/reactions` | `RemoveReactionHandler` | Working | 3 |
| `GET /attachments/:file_id` | `DownloadAttachmentHandler` | Working | 4 |
| `GET /attachments/:file_id/thumbnail` | `DownloadAttachmentThumbnailHandler` | Working | 4 |
| `GET /media/:sha_hash` | `GetMediaHandler` | Working | 4, 5 |
| `GET /link-previews/:preview_id` | `GetLinkPreviewHandler` | Working | 4 |
| `GET /link-previews/:preview_id/video` | `GetLinkPreviewVideoHandler` | Partial (GTK prototype) | 4 |
| `GET /emojis` | `ListEmojisHandler` | Working | 3 |
| `POST /emojis` | `CreateEmojiHandler` | Working | 3 |
| `DELETE /emojis/:emoji_id` | `DeleteEmojiHandler` | Working | 3 |
| `GET /roles` | `ListRolesHandler` | Working | 1 |
| `POST /roles` | `CreateRoleHandler` | Working | 11 |
| `PUT /roles/:role_id` | `UpdateRoleHandler` | Working | 11 |
| `DELETE /roles/:role_id` | `DeleteRoleHandler` | Working | 11 |
| `POST /users/:user_id/roles` | `AssignUserRoleHandler` | Working | 11 |
| `DELETE /users/:user_id/roles/:role_id` | `RemoveUserRoleHandler` | Working | 11 |
| `POST /search` | `SearchHandler` | Working | 8 |
| `GET /admin/audit-logs` | `ListAuditLogsHandler` | Working | 13 |
| `GET /voice/ice-servers` | `ICEServersHandler` | Working | 14 |
| `GET /ws` | `WebSocketHandler` | Working | 1, 2, 9, 10, 14, 15 |

The reset route supports the self-reset/password-change flow; issuing another
member's recovery link is implemented in step 13. Channel unread state is implemented;
channel administration is implemented. WebSocket notifications and DM updates are
implemented; native voice, camera/screen intent and track subscriptions are implemented.

Route totals: 70 working, 0 missing, 1 partial, 0 API only, 3 deferred.
