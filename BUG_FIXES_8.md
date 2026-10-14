# Bug fixes 8

Implemented the eleven requests in `bug_fixes_8.txt`. Earlier uncommitted fixes
remain in place.

| # | Change | Verification |
|---|---|---|
| 1 | Away members have an **AUSENTES** group and Busy members have a separate **OCUPADOS** group. Presence changes update counts and move existing rows between groups without rebuilding unaffected members; empty groups disappear. | Mapped grouping/count/row identity checks with 500 members. |
| 2 | Windowed image viewing stays fitted and shows the fullscreen action. Zoom controls and zoom gestures/shortcuts are enabled only in fullscreen. Leaving fullscreen restores fit. | Actual fullscreen, zoom, pan, image quality, animation and cleanup checks on private Wayland; windowed checks in CI. |
| 3 | Right-click a readable text channel and choose **Marcar canal como lido**. A one-message query resolves the current cursor; a successful explicit read clears its badge without switching channels. Empty/denied channels are not acknowledged. | HTTP contracts; native context menu, failure/retry and navigation checks. |
| 4 | Voice settings have a **Volume do microfone** slider, initially 150%, with a 0–300% range. It applies to an ongoing call; slider updates are coalesced. Gain never changes mute or opens capture by itself. The chosen level is retained during this app session. | Native synthetic capture checks configured/live gain, bounds and worker cleanup; mapped settings check. |
| 5 | Pinned messages show the author's nickname, role color and avatar, with safe fallbacks for missing members. Opening pins resolves missing author metadata through cached/batched requests; late identity/avatar changes update the view. | Native avatar/name, nickname refresh and permission-scoped metadata checks. |
| 6 | Removed **Atualizar notificações**. Inbox opening and incoming events continue to refresh it automatically. | Native inbox checks and existing notification regressions. |
| 7 | Removed **Atualizar mensagens fixadas**. Opening pins and live pin events continue to refresh them. | Native pin checks and existing pin workflows. |
| 8 | Composer and reaction emoji pickers have separate **Unicode** and current-server tabs. Server names are shown on the tab and long names are bounded. Custom emojis are scoped to the authenticated server session. | Mapped tab, search, selection and animated emoji checks. |
| 9 | Emoji cells append automatically near the scroll boundary, in batches of 96. The existing cells, selected tab, search/caret and scroll position survive catalog updates. There is no **Mais emojis** button. | Native incremental loading and widget identity checks. |
| 10 | Escape closes Search and cancels its pending query through the existing close handler. | Native key-controller and cancellation checks. |
| 11 | Native Markdown supports headings, nested bold/italic, underline, strikethrough, inline/fenced code, lists, tasks, quotes, tables, links, footnotes and clickable spoilers. Mentions and animated custom emojis remain supported. | Parser, Unicode offset, native tag/profile interaction, spoiler masking/reveal and rich-card regressions. |

Markdown uses the [pulldown-cmark parser](https://github.com/pulldown-cmark/pulldown-cmark),
locked to 0.13.4. It creates GTK text/tag/paintable objects rather than rendering
HTML. Raw HTML is literal text, unsafe link schemes are inactive, and Markdown
image syntax displays alt text; the authenticated backend embed/attachment
pipeline remains responsible for images. Code preserves literal mention/emoji
tokens. Spoilers conceal custom emoji images until clicked and do not expose
their contents in hover tooltips. No webview or extra remote media fetch was added.

Papo currently connects to one server per authenticated main window. Each emoji
picker therefore has a Unicode tab and that server's custom emoji tab; it does
not fetch emoji catalogs using unrelated saved server sessions.

```sh
cargo test --locked --offline
PAPO_UI_CASE=report8 dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=emoji-notifications dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
cargo test --locked --offline native_voice -- --ignored
```

Use `PAPO_UI_CASE=status-viewer` and `PAPO_NATIVE_FULLSCREEN_TEST=1` under a
private compositor with native fullscreen support to test fullscreen zoom.
A bare Xvfb display does not have a window manager and cannot verify this state.
All test servers are local mocks; audio capture uses a generated test signal.

Validated: 210 unit/integration tests passed, plus the `report8` workflow on X11
and Wayland, real fullscreen image controls, animated emoji/notification and rich
embed regressions, scrolling/channel opening with up to 5,000 messages, native
dead-key input and the synthetic audio worker. The application builds with
`cargo build --locked --offline`.

## Uploaded images stuck at “Aguardando moderação”

The local backend's moderation worker broadcasts sensitive results, but does not
broadcast clean or failed results. Its history attachment JOIN also omits the
moderation column, returning `moderation_status: ""`, even though the upload
response says `pending`. The first refresh fix incorrectly required an explicit
terminal verdict; its synthetic `clean` fixture missed the actual response.

The frontend processes moderation updates directly, without fetching history.
Both `attachment_moderation_update` and `moderation_update` are accepted with
the same channel/message/attachment/status fields. Duplicate states do not
rebuild rows or discard an existing thumbnail. Completion events arriving before
the message or its POST echo are retained in a bounded queue and replayed when
the attachment arrives.

After a successful upload acknowledgement, the sender can display the authorized
thumbnail immediately while retaining the pending moderation metadata. A later
sensitive result conceals it; a blocked result removes the message. A clean
completion keeps the thumbnail instead of downloading it again.

Older servers get a single batched fallback after a three-second event grace
period, only for nearby pending attachments without an acknowledged local send.
It requests a small 30-message page and applies the returned attachment states.
Empty or omitted fields now use the same available-media behavior as opening
history, without fabricating a `clean` verdict. Explicit pending/processing or
unknown verdicts stay pending. Known sensitive/blocked states and newer events
remain authoritative. Images load through the existing asynchronous pipeline.

The previous per-message retry loop was removed. Each retained pending message is
checked at most once, including unchanged/failed responses;
several nearby images share one page. Requests share the existing session budget
and start one at a time, outside scrolling/history loading. Leaving the channel,
hiding the chat or losing permission cancels pending work. In-flight requests
retain their event journals while scrolling. Permission failures stop fallback
checks for that visit.
Sensitive images still require an explicit reveal. Newer moderation/deletion
events override delayed checks, and late send echoes cannot restore a completed
attachment to pending.

The `report8` native workflow verifies both empty and omitted history statuses,
actual thumbnail fetching without switching channels, immediate acknowledged
uploads and completion events, 25 duplicate events without extra downloads or
row rebuilds, early events, sensitive reveal, blocked removal, and one fallback
for three unchanged pending messages. It also checks stale send echoes,
cancellation and rejection of delayed responses after leaving a channel.
Headless tests cover inclusive pagination, bounded early-event retention,
duplicate suppression, cancellation, legacy responses and state races.

## Bot embed sentences wrapping prematurely

The native rich-text view reported no useful natural width to the left-aligned
embed card. With a short author name the card could shrink to 8 pixels of text
width, wrapping a sentence even in a wide chat. A lightweight wrapper now
measures rendered text, font tags, resolved mentions and inline emoji dimensions
to supply a useful preferred width. It retains the existing card width cap and
wraps to fit narrow windows; it does not add newline characters.

The `embeds` workflow reproduces the original 8-pixel failure and verifies that a
fitting formatted sentence stays on one line, narrow windows still wrap, and
bridge avatars/profile links continue working. Avatar geometry checks wait for
GTK allocation before comparing the reserved and loaded sizes.

Validation: 218 unit/integration tests passed. The latest `report8` native run
passed request-count, event ordering, duplicate suppression and legacy fallback
checks on a private X11 display; sending/video checks also passed. The prior
`embeds` run verified the text-width fix. The application builds with
`cargo build --locked --offline`.

The complete legacy UI workflow was also attempted. Its shared notification
fixture now resets unread/failure state independently, and its mention assertion
checks the displayed nickname. That run still stops on a missing **Fixar** button
in the pin-action checks; it does not complete the full workflow. The focused
moderation/request-count checks above pass independently.

## Discord bridge author avatars reloading between channels

Author avatar textures and their loaded flags were discarded on channel changes
and when their messages left the viewport. The embed metadata cache intentionally
strips base64 bytes, so returning to those messages fetched the full embed and
decoded the avatar again.

Completed author avatars now stay in an authenticated session's bounded LRU
cache across scrolling and channel changes. It retains at most 256 entries,
including unavailable-image results, and at most about 5.1 MiB of decoded 72×72
avatar pixels. Existing temporary payload limits remain in place. Cache identity
includes the embed ID, version and author metadata; updated identities invalidate
old entries, while sparse summaries retain resolved metadata. No disk storage or
sharing by display name was added.

Channel changes cancel unfinished work and reject old completions while retaining
finished icons. Permission/session resets and user changes clear the cache.
Large chat images, videos and GIF frames continue using the existing eviction
behavior. A full embed fetched for its main image reuses an already decoded
author avatar rather than starting another avatar decode.

The native `embeds` workflow switches away and back three times with avatar
decoders held. It verifies immediate texture reuse, no new avatar-only embed
requests, cached missing-avatar results, the LRU pixel limit, eviction/reset
cleanup and rejection of stale completions. A unit test covers identity changes,
new embed versions and sparse summaries.

Validated: 219 unit/integration tests, native `embeds` cache checks and the
`retention` media-unloading workflow passed. `cargo build --locked --offline`
and `git diff --check` also passed.
