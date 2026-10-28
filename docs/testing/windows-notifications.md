# Windows notifications and activation

Step 9 implementation, 2026-10-27. The Windows notification capability is enabled
behind a runtime readiness check. **New Windows desktop acceptance is NOT RUN**;
the user requested deferring Windows-machine testing until a release is ready.
Linux/fake-transport checks and a GNU Windows cross-build do not qualify a Windows
11 desktop or the final distributable.

## Native delivery and registration

The adapter extends step 4's UCRT-compatible `papo-shell.exe`, AppUserModelID,
shortcut, stub CLSID and protocol. It uses system WinRT APIs without the Windows
App SDK or a COM activation server. Existing packaging builds the helper statically
and checks its DLL closure. Inbox grouping, badges and notification policy remain
shared with Linux. macOS desktop alerts remain unavailable.

For a ZIP, register this package's identity once, then restart Papo:

```powershell
.\Papo.com --register-desktop
.\Papo.com --desktop-info
```

This is the same explicit, ownership-checked, current-user operation documented
in [step 4](windows-identity.md). The future installer must invoke it. Message
handling does not silently install a protocol handler or overwrite another
package's registration. Without the helper or matching identity, native delivery
stays unavailable and the inbox/badges continue working. A source-tree `cargo run`
without that packaged helper/registration cannot deliver toasts.

Alerts honor enabled, preview and sound preferences, channel settings and access.
A missing enabled value follows the shared default. Disabled previews use generic
text. Content is bounded, stripped of invalid XML characters and escaped. Preview
text travels only through private stdin, never executable arguments, metadata
storage or diagnostics. Silent toasts explicitly request silence; enabled sounds
use Windows' notification sound, respecting OS notification/Focus Assist settings.

The integration follows Microsoft's
[AppUserModelID requirements](https://learn.microsoft.com/en-us/windows/win32/shell/enable-desktop-toast-with-appusermodelid)
and [toast XML contract](https://learn.microsoft.com/en-us/uwp/schemas/tiles/toastschema/element-toast).
Protocol activation is retained; inline reply/text-input actions are not added.

## Activation and lifetime

The shell receives only `papo-notification://open/<canonical UUID>`, without account
IDs, server URLs, message/channel IDs or credentials. Existing SID/session-scoped,
same-user named-pipe forwarding presents the running window, including when
minimized. Unknown/malformed tickets grant no destination.

Routing metadata lives in LocalAppData under
`papo-gtk/notification-tickets-<Windows desktop session>.dat`, protected by
current-user DPAPI with application-specific entropy. There is no plaintext
production fallback. It contains only tickets, backend/user/stable session owner,
notification/channel/message IDs, expiry, group and revocation state. Previews
and authentication tokens/cookies are never stored. Files are bounded to 256 KiB
and 128 entries; tickets expire after 24 hours. Native tags/groups fit within 16
characters; groups separate accounts, sessions and Windows desktop sessions.

A click waits for authentication and fresh notification, channel-access and DM
snapshots. Resolution requires the same backend, user and stable session owner,
then consumes the ticket. GTK rechecks access after asynchronous resolution and
before navigation; normal message APIs still enforce server authorization.
Remembered-session restoration preserves the stable owner across cold starts.
A fresh login or revoked session cannot reuse a previous session's ticket. Login
is never bypassed; mismatched-account clicks never sign out or navigate the active
account. The Windows instance model currently has one active account/window per
desktop session. Tests cover multiple account registries without adding automatic
account switching or simultaneous account windows.

Show, read/permission reconciliation, activation and logout are serialized to
prevent delayed delivery from resurrecting removed alerts. Read, deleted, disabled
or unreadable alerts are removed through native history. Revocation is persisted
before removal; failed removals stay revoked and retry periodically/after restart.
If consumption cannot be persisted, navigation is refused. Logout, expiry and
account replacement revoke the old owner. Normal close preserves tickets/toasts
for remembered-session cold activation.

Work runs off GTK's thread. The queue is bounded to 64 requests; native helper
operations have five-second deadlines with kill/wait on timeout. A failed queue,
store or native service leaves the in-app inbox available. No alert is shown
before its routing metadata is saved.

## Automated checks

```sh
cargo test --locked platform::notifications:: -- --test-threads=1
python -m unittest discover -s tests -p windows_identity_test.py -v
PAPO_UI_CASE=windows-notifications cargo test --locked ui_smoke -- --ignored --test-threads=1
```

Use a private display/session bus for the Linux GUI case. It uses a synthetic
shell transport and backend, sending no real desktop alert. Windows CI includes
these cases, a DPAPI round-trip/tamper test, and native XML/ABI construction without
registration or visible notifications.

For a built Windows ZIP, from an unrelated directory or accented/spaced path:

```powershell
.\Papo.com --check-notifications
```

This uses real packaged WinRT construction and DPAPI with temporary synthetic
metadata. It checks cold restoration, account/session rejection and replay. It
registers nothing, shows no alert, reads no credentials and makes no backend
requests. `check-portable.ps1` invokes it before/after relocation. Its JSON reports
`desktop_delivery: false`; it is not visual desktop acceptance.

## Deferred release acceptance

Use the final artifact, disposable accounts and an authorized backend. Follow
[Windows evidence requirements](windows.md): artifact/source hashes, OS edition/
build, runtime manifest and backend revision. Check:

1. Registered-package delivery while active, inactive and minimized. Clicking
   opens the exact message. Repeat mentions/replies, server channels and DMs.
2. Disabled notifications produce no alert/sound and remove old alerts. Disable
   preview and sound separately; verify generic text and silence. Include Unicode
   and literal XML-like text. Restore preferences afterward.
3. Read/delete a message, mute its channel, revoke channel/DM access, log out and
   expire/revoke the session. Confirm history removal and rejection of stale clicks.
4. Close with a remembered session and pending alert; click to cold-start, restore
   the owner and navigate after access loads. Without a remembered session, require
   login and reject a different session owner. Reject unknown/expired/replayed URIs.
5. Click while another account/server is active: present its window without
   navigating or replacing it. Test independent Windows desktop sessions and
   ownership-checked relocation/upgrade registration.
6. Fail/recover native removal and verify revoked destinations plus retry. Repeat
   normal close/reopen and logout during delivery, and check that helpers exit.
   Distinguish OS suppression from application delivery failure.

Local results: ten actor/policy tests, the focused GTK integration and existing
notification/filter GUI checks passed;
all-target compilation passed without warnings; the full headless Rust suite
passed 307 tests (six ignored). The extended helper cross-compiled
with MinGW GNU and `-Wall -Wextra -Werror`; two identity/resource tests passed on
Linux, with native execution explicitly skipped. No new DPAPI/WinRT package check
or real toast-click result on Windows is claimed.
