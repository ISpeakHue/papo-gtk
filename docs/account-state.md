# Account and connection ownership

The groundwork for implementation-plan step 2 uses `account::AccountId` as the
stable key: the API's canonical base URL **including its base path**, plus the
verified user UUID. Display names, usernames, cookies and rotating login/session
IDs are not this key. URLs use the same HTTPS/loopback validation as `ApiClient`.
Serialization validates/canonicalizes the server again. The versioned SHA-256
namespace is suitable for future per-account storage and redacts Debug output;
it is an identifier, not encryption or an authorization credential.

| State | Owner |
| --- | --- |
| HTTP client, cookies and request pacing | One authenticated account connection; cloned clients share that connection's budget |
| WebSocket commands/listener and session refresh | `connection::Connection`, with account ID and cancellation on drop |
| Reconnect, access/history recovery, activity and polling state | `connection::State`; status calculation is independent of GTK |
| Accepted account preferences | `Connection.config`, under the same account identity |
| Active drafts, recent history and cached row surfaces | `chat::cache::AccountConversations`, with that same account identity as the outer namespace; inner keys are channel/DM UUIDs |
| Persistent drafts and pending sends | Account-namespaced local snapshot; Chat owns restoration and a sequential outbox; see [drafts and outbox](drafts-and-outbox.md) |
| Active media and compressed/resident caches | One Chat component under that account; reset with retained conversation state on owner change |
| Inbox, read journal, pending notifications and desktop jobs | `Notifications`, explicitly initialized with the connection's account identity |
| Desktop activation tickets | Stable server/account identity **plus** login-generation identity; stale tickets must not activate another login |
| Theme/motion/language defaults and last-login hints | Existing device configuration; these are intentionally not authenticated per-server data |

`MainWindowModel` coordinates widgets and permission snapshots; it no longer owns
individual connection flags, socket sender or transport timers. Closing its
connection cancels listener, refresh, cleanup and startup tasks. Closing one
account must not cancel another connection's work. Access-refresh UI tasks are
still canceled by their owning component.

Changing chat ownership clears both inactive and active drafts, history, media,
emoji catalog and author data before another account can display them, even if
both backends use the same user/channel UUIDs. A fresh login to the same account
keeps the same stable namespace, but activation tickets still carry a separate
session generation. Steps 5–6 add account-scoped draft/outbox persistence; the
server-switching UI remains a later step.

Credentials continue using the existing native keyring format: canonical server
base plus username is required **before** `/auth/whoami` verifies the user UUID.
This compatibility lookup is not reused as a draft/cache identity. Existing
remembered sessions are not migrated or read by the synthetic tests.

Checks include canonical URL equivalence, different API paths/ports/accounts,
serialized identity validation, bounded conversation ownership, independent
connection cancellation and an `account-scope` GTK regression. That regression
reuses identical user/channel IDs against two private loopback backends and
checks that drafts/history do not cross the boundary. Existing recovery,
notification and account-settings cases exercise the extracted connection.
