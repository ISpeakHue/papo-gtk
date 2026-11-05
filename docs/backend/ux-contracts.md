# Backend contracts for the UX implementation plan

Reviewed local backend: `eab626662bc4bfeb4ce1cd3165b01eccb4c2c5a5`.
This is a source review, not evidence that a deployed server runs this revision.
No private messages, saved credentials or live server were accessed. Backend
source remains unchanged.

## Current capabilities

| Requirement | Existing contract | Conclusion |
| --- | --- | --- |
| Human-message idempotency | `POST /messages` accepts multipart `channel_id`, `content`, `reply_to`, `embeds`, repeated `attachments`. `CreateMessage` passes an empty webhook/source identity to storage. | **Missing.** There is no human submission key, deduplicated replay or lookup by client submission ID. Request tracing IDs do not provide this guarantee. |
| Webhook idempotency | `POST /webhooks/:webhook_id/messages`, optional `external_message_id`; uniqueness belongs to `(webhook_id, external_message_id)`. | **Present for webhooks only.** It cannot be reused to send human messages. |
| Message history | `GET /channels/:channel_id/messages` with `since`, `last_id`, `order`, `limit` (1–100, default 30). Returns messages with attachment and embed metadata. | **Present.** Text and voice-channel chats and DMs use channel IDs with access checks. |
| Media discovery | `POST /search` supports `channel_id`, `contains_attachment`, order and `(since,last_id)` pagination. Results are message summaries without attachments/embeds. | **Partial.** The frontend combines attachment/link searches and exact-hit lookups. Embeds without searchable URLs in message content remain undiscoverable historically. There is no dedicated media-history endpoint. |
| Media bytes | `GET /attachments/:file_id`, `/attachments/:file_id/thumbnail`, plus existing embed image/video endpoints. | **Present.** These retrieve known resources, not a paginated conversation media index. |
| Account preferences | `PUT /users/settings`; version 1 has notification enabled/preview/sound/mentions and display/theme settings. Authenticated profile returns settings. | **Present.** Message and call cues currently share `notifications.sound`. No separate call-sound preference. |
| Per-channel notifications | `POST /channels/:channel_id/user/:user_id/settings`, `notification_settings`: `off`, `only_mentions`, `all`. Channel lists expose the effective value. | **Present.** No expiration, snooze deadline or quiet-hours policy. |
| DM notification overrides | The existing channel-setting route accepts DM channel IDs; notification dispatch uses those rows and excludes `off`. DM list/detail responses omit the setting. | **Partial.** Writing/muting is supported, but reliable read-back and cross-client synchronization need an extension. |
| Notification inbox/read state | `GET /users/:user_id/notifications`, `PUT /users/:user_id/read_notification`, `PUT /channels/:channel_id/read`, WebSocket notification events. | **Present.** Preserve current read/unread semantics while adding delivery controls. |
| Feature negotiation | No versioned capability manifest for these proposed additions. | **Missing.** Do not infer support from the application version or ignore unknown fields on a successful response. |

The reviewed source file hashes and machine-readable findings are in
[ux-contracts.json](ux-contracts.json). Recheck after a backend update:

```sh
python3 scripts/check-backend-contracts.py --backend ../papo-backend \
  --report target/backend-contract-review.json
```

Exit 2 means relevant source changed and requires another review. The command
does not contact the server, write backend files or automatically declare new
capabilities supported. An unrelated backend commit with identical reviewed
files retains the same findings.

## Required additions before dependent frontend work

### Step 6: reliable pending sends and retries

Agree on a UUID `client_message_id` accepted by human `POST /messages`, returned
in the HTTP result and echoed on the WebSocket message. Store a unique key per
authenticated author and submission ID, bound to the original channel and
payload. Concurrent identical requests must return the same canonical message;
a reused ID with different content, reply, embed or attachment bytes must return
409 with a machine-readable conflict code. Authorize every replay against the
current account and channel before returning message content.

Deduplication must cover message creation, attachment linking, moderation and
notification fan-out. Losers must not create duplicate public resources or
orphan uploads without cleanup. Define key retention and behavior after message
deletion. Add authenticated lookup by submission ID so a dropped response can be
reconciled before uploading again. Define an explicit “unknown submission” result.

Acceptance: two concurrent identical sends, reconnect after a committed write
with a lost response, conflicting payload, another user/channel, permission
revocation, deleted submissions, and attachment cleanup. Until this exists,
the frontend can display pending sends and offer explicit manual retry, but must
not replay uncertain writes automatically. Existing POSTs remain single-attempt.

### Step 11: efficient media history

The frontend now offers a compatibility browser using current attachment/link
search filters and bounded inclusive one-message lookups for search hits. It
does not scan text history. Links are indexed from message content, so custom
embeds without a searchable link there remain undiscoverable historically. This
uses the existing API by user preference; the dedicated contract below remains
a follow-up for complete coverage and fewer requests.

Agree on `GET /channels/:channel_id/media` with a stable cursor, bounded limit,
and `kind=image|video|file|all`. Include attachments and backend-resolved link
media, grouped with their message ID, timestamp and author. Return MIME type,
dimensions, duration when known, thumbnail/resource references, moderation state
and `has_more`/next cursor. Specify duplicate handling and fallback for unknown
MIME types. Apply limits to a documented unit (media items or messages).

Use authenticated resource references; do not expose arbitrary fetch URLs or
embed scripts. Enforce current membership, read permissions, DM blocks and
moderation on listing and retrieval. Deletion/moderation/permission events must
allow clients to evict existing entries. “Jump to message” uses the existing
message ID, not an inferred position in loaded history.

Acceptance: tied timestamps, concurrent insertion/deletion between pages,
multi-attachment messages, link-only media, voice-channel text, DMs and lost
permissions. Do not implement a gallery by scanning all text-history pages.

### Step 13: notification delivery preferences

Version account settings to add separate message/call sound values and an
optional snooze deadline in UTC. Define a backward-compatible default: absent
sound fields inherit the existing `notifications.sound` preference. Older
clients sending version 1 must not erase new settings.

Add per-conversation overrides covering both channels and DMs: notification mode
and optional mute deadline. Define the effective precedence of account enabled,
Busy status, snooze, conversation mute, mentions/replies and sound preferences.
Prefer treating snooze as delivery suppression while retaining inbox/unread
state; explicitly agree this behavior with the backend maintainer. A local-only
snooze can be implemented earlier if clearly presented as device-specific.

Return persisted values/deadlines and publish preference changes so other clients
can reconcile. Expiration must restore the previous policy rather than silently
choosing a new one. Server UTC time governs synchronized deadlines.

Acceptance: explicit false values, expired deadlines, clock differences,
cross-device changes, Busy status, mentions/replies, DM blocks, channel access
revocation and old-client writes.

### Feature negotiation

Agree on a versioned capability response advertising each contract independently
(human-message idempotency, submission lookup, conversation media history and
notification settings version). Missing or unreachable discovery means support
is unknown; keep existing behavior and do not send invented fields/endpoints.
Each capability needs contract fixtures before enabling it in the frontend.

Steps 4, 5, 7–10 and 14–15 can proceed locally using the current API. Step 12's
local microphone/output tests need no new REST contract; seamless device changes
must preserve the existing voice negotiation contract. Independent sessions for
step 16 use the current per-server API, with the account ownership groundwork
implemented in step 2.
