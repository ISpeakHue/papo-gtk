# Multiple servers and accounts

Use **+** in the left account rail to add a server/account. Existing connections
stay alive while the login form is open; **Back to server** returns to the last
active page. Click an account to switch. Accounts on the same backend are separate
entries, identified by the canonical API URL/base path and verified user UUID.
The username below each icon distinguishes accounts on the same server. Server
logos reuse the existing account-owned icon load. The top DM button toggles the
selected account's direct-message list and shows its DM count.

The rail shows notification counts (99+ when larger), an unread dot for channel
activity without a notification count, reconnecting state, and a call indicator.
Its translated accessible name includes server, username, address and connection
state. Summaries are coalesced; an otherwise idle account updates within the
existing two-second housekeeping interval. Ordinary typing does not repeatedly
rebuild the rail.

Each live account keeps its own API client/cookie jar, WebSocket/reconnect state,
permissions, settings, inbox, outbox and chat component. Switching keeps the
selected channel/DM, unsent text/reply/files and viewport anchor; it does not
resend, transfer or cancel another account's pending sends. Network errors,
permission changes, logout and expiry affect only their source account. Old
controller messages have a separate generation guard, including when a resumed
session reuses its previous native session owner. Login form scopes also reject
late authentication completions after switching/canceling.

Hidden accounts do not acknowledge messages as read or start new media loads.
Their automatic polling slows down when their socket is healthy; reconnecting
accounts retain the existing retry cadence. Hidden chat previews use the existing
low-memory limits and discard inactive widget surfaces; returning restores the
device preference and can decode a disk/encoded-cache thumbnail again. This keeps
multiple connected accounts from retaining the full decoded cache allowance each.
Call engines remain independent and active calls remain visible in the rail.
A call is **not** ended by switching pages. Inline chat playback stops on hiding.

Account font rules are scoped to their page; only the visible account controls
the global theme. Language, motion and media preferences remain device-wide.
Media-preference changes propagate to all live accounts. Timed notifications and
sound overrides remain account/device scoped as documented in the
[notification guide](notifications-accessibility-and-updates.md).

## Persistence and boundaries

The bounded `accounts-v1.json` file lives beside the native application config.
It holds up to 32 entries: canonical address, verified UUID, username, server name,
remember-session choice and last conversation. It contains **no passwords,
cookies or tokens**. Writes are atomic, serialized and coalesced; unread counts
and connection state are session-local. A corrupt/oversized directory is preserved
and reported rather than overwritten. The file limit is 256 KiB.

Credentials continue to use the existing native keyring service. Only accounts
where session retention was chosen are resumed automatically. Startup prioritizes
the saved selected account, limits credential-resume concurrency to two and caps
live controllers at eight. Remaining saved accounts stay selectable. A missing,
expired or unavailable credential offers login for that account while others
remain usable. UUID/username checks reject a remembered credential that belongs
to a different identity.

After restarting, the last active account/conversation and persisted drafts are
restored after validating current permissions. Viewport anchors are retained
while switching live accounts; process restarts use the existing normal history/
unread opening behavior. A removed or forbidden saved conversation falls back to
an allowed conversation. No old permission snapshot authorizes a request.

The red disconnect action disconnects only the selected account and keeps its
rail entry for a later login. Right-click its rail entry → **Remove account from
list**, then confirm, to remove that saved entry and its remembered credential.
Other accounts remain connected. Existing account-scoped draft/cache retention
policies still apply; removing a rail entry does not promise to erase all local
media or drafts. Credentials shared by a renamed/recreated identity are not
blindly deleted by removing a stale directory entry.

Linux notification actions activate their owning account. Windows activation
routes a ticket to live/resuming accounts; the existing backend resolves and
consumes it only for its matching account/session and current permissions. A
wrong account cannot consume another account's ticket. No credential is placed
in activation arguments.

## Verification

`PAPO_UI_CASE=multi-server` uses two loopback servers with colliding user/channel
UUIDs. It checks drafts, scroll, pending-send routing, hidden reads/unread/media,
connection and permission loss, stale controller generations and login scopes.
`multi-account-restart` uses two identities on the **same** backend, separate
cookie/keyring slots, last-conversation/draft restart recovery and account removal.
Both use synthetic credentials, memory keyring storage and private test config.
Native Windows execution remains a separate acceptance check.
