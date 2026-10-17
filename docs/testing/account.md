# Accounts and profiles

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

## Login request budget

All REST requests share a session-wide scheduler, including cloned clients,
avatars, emoji pages, previews and downloads. An idle session has up to four
request credits, refilling one credit every 125 ms (eight per second sustained).
The four image download slots are unchanged. FIFO admission prevents background
pagination and downloads from continually overtaking a foreground read.
HTTP 429 imposes a shared cooldown and disables further bursts for that session;
`Retry-After` seconds and HTTP dates are supported. Read-only requests can retry
twice, with one- and two-second backoff when the backend supplies no header.
Headers requesting more than five seconds return the error immediately while
preserving the cooldown for subsequent requests. Mutations, uploads and account
login are never replayed automatically. The two profile/summary batch POST
endpoints are read-only and can retry.

Startup subscribes to WebSocket before loading its access snapshot. If the socket
is unavailable, HTTP starts after two seconds. The first successful subscription
does not duplicate startup; subsequent connections still reconcile missed events.
Members, direct messages and notifications start after the first message-history response,
with an 80 ms opportunity for GTK to allocate and paint the chat. Servers without
a readable text channel start those requests after access loading instead. Channel
pins and the emoji catalog also wait for the first history response; emoji loading
starts before pins.
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

## Emoji rendering in profiles

Profile descriptions, status messages and typing text use the chat's emoji renderer
and cached server emoji catalog. Unicode emoji display normally; known custom codes
such as `:ricardao:` display inline images, while unknown codes remain readable text.
The description editor shows a live preview when it contains known custom emoji
and preserves the original codes when saving. An open profile updates when the
emoji catalog arrives or changes, without another profile request.

```sh
PAPO_UI_CASE=profile-emojis dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

This mapped regression covers mixed Unicode/custom/unknown emoji, late catalog
arrival, removal, editor preview, save failure/retry, and preservation of raw text.
Set `PAPO_DESIGN_PREVIEW=1` for `/tmp/papo-design-profile-emojis.png` and
`/tmp/papo-design-profile-emoji-editor.png`.

## Quick presence selector

Run the native GTK smoke test with `PAPO_UI_CASE=presence-selector` in the
existing private-display test environment. It uses only a local mock server.
The check covers placement above the member list, colored option icons, saved
status on startup, no-op/repeated selections, a failed write followed by retry,
Online clearing the manual status, one status-only request per change, and
synchronization with the profile editor without losing unsaved profile fields.
`PAPO_DESIGN_PREVIEW=1` also saves `/tmp/papo-design-presence-selector.png`.
The `PAPO_UI_CASE=status-viewer` member-list checks cover separate Online,
Busy, Away and Offline sections, section ordering/counts, full presence syncs,
empty-section removal and preservation of existing rows during status changes.

## Tabbed account settings

The gear next to Disconnect opens one native account settings window with
“Meu perfil”, “Preferências” and “Segurança” tabs. Forms mount on first selection;
returning to a tab preserves its unsaved fields without fetching the profile or
sessions again. Profile saves reconcile accepted data into the mounted form and
preserve the selected tab and other drafts. Dirty forms offer Save/Discard/Cancel
on close; successful closure clears password fields. The own-avatar and preferences
shortcuts select the corresponding tab in this same settings window.

```sh
PAPO_UI_CASE=account-settings dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The local GTK fixture checks the gear button, lazy pages, repeated and rapid tab
switches, retained drafts, failed/successful profile and preferences writes,
password changes, session revocation, a narrow window and close/reopen cleanup.
The configuration mockup is reflected in the icon-and-label pill tabs, grouped
password fields, sessions header refresh icon, individual session cards and
trailing destructive end-all action. The fixture clicks the real tab controls
and checks that both reveal-enabled password fields fit the narrow window.
With `PAPO_DESIGN_PREVIEW=1`, it writes private synthetic previews to
`/tmp/papo-design-account-settings-security.png` and
`/tmp/papo-design-account-settings-narrow.png`.
The Preferences redesign uses native combo and switch rows in appearance,
display and notification groups. Mixed switch values are checked after saving
to catch incorrect field mappings; the same fixture verifies drafts, failed-save
recovery and row bounds in the narrow layout. Optional previews also include
`/tmp/papo-design-account-settings-preferences.png`, its `-light` variant and
`/tmp/papo-design-account-settings-preferences-narrow.png`.
The profile fixture loads synthetic avatar/banner images, checks avatar overlap
and keeps the Save action visible while scrolling and at narrow widths. Removing
the banner must collapse its space without losing a description draft. Optional
previews include `/tmp/papo-design-account-settings-profile.png` and
`/tmp/papo-design-account-settings-profile-narrow.png`.
The existing `account-notices` fixture continues to cover profile, preferences,
security and notification workflows.
