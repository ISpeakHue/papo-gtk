# Messages, Markdown and reactions

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

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

## Readable mention labels

Choosing a member inserts a highlighted `@display name` in the composer. The
draft and multipart request retain `@mention(<@user-id>)` so duplicate display
names cannot change the recipient. Typing an ordinary `@name` does not create a
hidden mention. Editing part of a selected label makes it plain text. Restored
drafts and failed sends preserve the display label and target; nickname updates
refresh the label without changing its recipient. Native keyboard and IME edits
are tracked through the entry's text delegate, preserving the selected member ID.
Enter, Tab and suggestion clicks return focus with an unselected caret after the
completed mention, so continued typing appends to it.

Suggestions show a cached profile picture and a nickname in the member's role
color, with the username alongside it. They respect the avatar display preference.
New avatar batches update mounted suggestions in place; role-color changes refresh
the open choices. A short list fits its rows and padding without reserving a hidden
scrollbar's minimum height; long lists retain the 240-pixel content cap and scrollbar.
The focused layout check covers single/many/empty results, filtering after scrolling,
late avatar replacement/removal, live role colors, and the avatar preference:

```sh
PAPO_UI_CASE=mention-suggestions dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

With `PAPO_DESIGN_PREVIEW=1`, it saves
`/tmp/papo-design-mention-suggestions-light.png` and
`/tmp/papo-design-mention-suggestions-dark.png`.

Received mentions have a blue highlight and open the tagged member's profile on
click. Plain `@nickname` or `@username` text from other clients also links when it
identifies one member unambiguously; ambiguous names, email addresses, URLs and
plain names in code remain ordinary text. This read-only recognition does not
turn a manually typed draft into a notification. Dragging or copying a selection
does not open a profile, and `@everyone` has no individual profile target.

```sh
PAPO_UI_CASE=mentions dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

This focused workflow verifies Unicode caret positions, highlighted spans,
duplicate-name edits, nickname changes, emoji insertion, draft switches,
failed-send retries and the exact submitted ID. It exercises the Enter/Tab
controllers, native text insertion, focus changes, continued typing and native
mention click handlers, including selection/drag protection. It also runs existing
mention completion and clipboard checks. Set `PAPO_DESIGN_PREVIEW=1` to capture
`/tmp/papo-design-composer-mention.png` and `/tmp/papo-design-clickable-mentions.png`
on the test display.

## Backgrounds for messages mentioning the current account

Received messages addressed to the current account have a persistent amber tint
and left accent, including `@everyone`. Canonical IDs work before member metadata
loads; plain usernames/nicknames resolve only when unambiguous. Self-authored
messages, other recipients, code examples, URLs and longer lookalike names do
not receive the tint. It does not depend on notification settings or read state.

Edits and member-name changes update the background. Incremental updates keep
cached message rows, and the permanent mention tint survives the separate
navigation highlight's expiry. Theme colors keep it legible in light and dark
mode; hover/focus retain the tint. Styling changes no row dimensions.

```sh
cargo test --locked --offline addressed_messages
PAPO_UI_CASE=mention-background dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The focused GTK check covers late metadata, exact IDs, names, everyone, excluded
literals, own/other messages, live arrivals, edits, nickname changes/ambiguity,
row reuse, navigation expiry and cached channel restoration. Light/dark
screenshots can be saved by setting `PAPO_DESIGN_PREVIEW=1`.

## Clickable emoji details

```sh
PAPO_UI_CASE=emoji-details PAPO_NATIVE_INPUT_TEST=1 \
  dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

This case clicks static/animated custom emojis and Unicode emojis in a real
TextView, checks the popover's preview/name/server caption and compact size,
and verifies Escape dismissal, Unicode offsets, spoiler precedence, selection,
dragging and destruction when the source unmaps. Weak references check that
closed previews are released. No server or real account is used. CI runs this
case; `embeds`, `profile-emojis` and `report8` cover the shared text renderers.

## Mentions in message editing

The editor uses the same highlighted name/ID mapping and suggestion rows as the
chat composer. Click inside an existing mention to choose a replacement, type
`@` to filter members, or use the editor's `@` button. Arrow keys navigate;
Enter/Tab accept a suggestion without saving. Shift+Enter inserts a newline.
Escape dismisses suggestions first, then closes the editor. Display names are
shown, while saves preserve canonical user IDs, including duplicate nicknames
and members whose profiles are unavailable.

```sh
PAPO_UI_CASE=edit-mentions PAPO_NATIVE_INPUT_TEST=1 GTK_IM_MODULE=simple \
  dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

This isolated workflow checks full-name replacement, Unicode offsets, punctuation,
avatars/role colors, native accented input, failure/retry, unauthorized selections,
stale dialog callbacks and text-only PUT payloads. It also exercises the existing
composer and edit shortcuts. Editing must preserve the channel draft and avoid
rebuilding chat history on each keystroke. No real account is used.

## Emoji picker resizing

```sh
PAPO_UI_CASE=emoji-grid dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The shared composer/reaction picker adds columns as its window widens and shows
more compact rows as it grows taller. Both Unicode and server tabs are checked
at narrow and wide sizes. Resizing must fill a viewport larger than the initial
96-cell page, preserve existing cells and keep the remaining catalog lazy.
Filtering to one emoji must leave free space below it rather than stretch its
row. This workflow also checks selection, incremental pages and animated emoji
cleanup using local fixtures.

## Unicode emoji categories

The Unicode tab in both composer and reaction pickers has a category strip:
faces/emotions, people/body, animals/nature, food/drink, travel/places,
activities, objects, symbols and flags. **Todos os emojis** shows the full
catalog. Category buttons have Portuguese labels/tooltips and keyboard focus.
Search spans all Unicode categories; clearing it returns to the selected group.
Selecting another category clears the query and returns its grid to the top.
Server emoji updates preserve the category, existing cells and scroll position.

```sh
cargo test --locked ui::chat::emoji::
PAPO_UI_CASE=emoji-categories dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=emoji-grid dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The category test uses GTK's bundled Unicode catalog and synthetic server emoji
metadata; it checks all categories, cross-category search, selection, empty
results, refresh/tab persistence and narrow-window layout. The grid test covers
responsive columns, lazy pages, stable spacing and animated custom emojis.

## Syntax highlighting

Fenced code blocks use the language token after the opening fence: `rust`/`rs`,
`python`/`py`, `javascript`/`js`, `json`, `bash`/`sh`, and other grammars included by
[Syntect](https://docs.rs/syntect/5.3.0/syntect/). No language is guessed for an
untagged block. Unknown languages, inline/indented code and oversized or unusually
complex blocks retain literal monospace text. Neither code nor HTML is executed.

````markdown
```rust
fn main() {
    let greeting = "Olá, mundo! 🦀";
    println!("{greeting}");
}
```
````

The bundled grammars run locally on at most two background workers. Parsing is
bounded to 32 KiB, 32 blocks, 512 lines per block, 2048 bytes per line and 4096
color spans per palette; work checks cancellation and a time budget between
lines. A single regex operation is not interruptible. Both palettes come from
one parse, so switching light/dark themes needs no new grammar work. Colors meet
a 4.5:1 contrast ratio on the code background; system high contrast uses native
foreground colors. Highlighting changes color only, preserving text, selection,
line geometry and spoiler masking. Discarded rows release their theme handlers
and cancel pending work. This adds no native library dependency to the AppImage.

```sh
cargo test --locked ui::chat::syntax::tests
cargo test --locked ui::chat::markdown::tests
PAPO_UI_CASE=syntax-highlighting dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

Regressions cover multiple grammars, aliases, Unicode character offsets,
multiline comments, separate fences, safe fallbacks, worker cancellation, light/
dark changes, spoiler reveal, exact clipboard text and disposal of queued views.

## Reaction details window

```sh
PAPO_UI_CASE=reaction-participants dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The local fixture covers two reaction filters, animated custom emoji previews,
125 participants, bounded metadata batches, progressive member rows, cached
filter switches, live nicknames/role colors/avatars, profile activation, narrow
layouts, removed/empty reactions, errors, Escape and stale-close events. It uses
synthetic accounts only and never opens a saved session.

## Replies, search, profile previews and dialogs

`PAPO_UI_CASE=report10` covers rich one-line reply previews in both the composer
and message references: Unicode/custom emoji, preservation of text beyond 160
characters, and clipping to the resized window. It also checks dated-history
jumps, returning to the recent head, disabled mention completion in DMs,
retention/invalidation of recently viewed previews, numeric channel notification
badges, and Escape closing a
transient window while leaving its root open.

`search-recycling` additionally checks substring matching for author/mention
filters, member-avatar factories, a single search after a typing burst, and
stable model objects when loading overlapping result pages. Results use rich
text and more spacious cards. `account-settings` verifies that the footer's
own-profile button opens a read-only preview, while the gear retains editing
and unsaved editor drafts survive opening/closing a preview.
`notification-filters` exercises per-channel notification counts and read state.

Reactions should sit directly below message content: messages without media
must not reserve an empty media row. Escape follows normal close handlers,
including unsaved-change prompts; focused completion/popovers and fullscreen
retain their own Escape behavior.

Author and mention dropdowns batch avatar work only for bound cells through the
shared profile loader; opening search does not fetch every server member's
portrait. Cached/fresh/in-flight profiles are reused, and late portraits update
visible cells without a new search.


## Inline draft emojis and keyboard navigation

Custom emojis in the composer and message editor use inline paintables. Drafts,
copied selections and requests retain their `:name:` codes and canonical mention
IDs. Complete codes typed or pasted become emojis after the native input method
commits; code spans, URLs and unknown names stay literal. The editor has an emoji
picker and the same `:name` suggestions, Enter/Tab completion and continued typing
as the composer. Choosing an emoji never submits the message or saves an edit.

`PAPO_UI_CASE=report11` checks inline insertion, Unicode offsets, deletion,
copy/cut/paste, rich undo/redo, draft restoration, independent editor completion, reopening the
picker, failed-save retries and exact request content. It also checks Page Up,
Page Down and End with focus in the message pane: pages move the chat with
overlap, and End returns to the recent head. The composer and other editable
fields keep native caret movement, selection and scrolling behavior.

Channel rows retain a blue unread dot and show notification counts separately
in red. Author/mention filters use a searchable, recycled list: typing filters
choices without selecting one, and clearing restores the complete list from its
first row. The `search-recycling` case exercises the actual popup and clear path,
including 100 members, matches near the end, empty results and repeated clearing.
It checks the visible scroll position after layout as well as source ordering,
while preserving the committed filter and search-field focus.

The `backend-gifs` case includes the reported Klipy page and Giphy CDN URL.
Backend `fetch_method: gif` embeds can carry both GIF data and video metadata;
the GIF bytes take priority and video is a fallback. Fixtures verify one GIF
request per embed, bare animated surfaces and cached channel returns, with no
frontend provider scraping. Public-provider availability remains server-owned.

Draft undo/redo retains emoji images and mention identities in a bounded history
(128 snapshots, 512 KiB). Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y and the context menu share
this history; it clears when the composer switches conversations or resets.

`PAPO_UI_CASE=history-navigation` exercises an old search jump across 1,250
synthetic messages with repeated timestamps. It checks forward-page retry,
contiguous bounded history, reaching the live head and rejection of stale pages.
The `report10` case additionally checks that custom reply-preview emojis fit
within the single-line text height without changing shared chat textures.

## Embed measurement warnings

The `embeds` workflow covers rich and bot cards, long inline fields, narrow/wide
resizing, delayed thumbnails and cached cards. To make GTK sizing warnings fail
the run, use a private display and enable fatal warnings for the test process:

```sh
GTK_A11Y=none G_DEBUG=fatal-warnings PAPO_UI_CASE=embeds dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The embed's width clamp aligns to the start on both axes, so GTK negotiates its
width before measuring its wrapped height. Filling vertically previously caused
width measurements constrained by a height smaller than the card's reported
minimum, emitting repeated `Trying to measure embed-…` warnings. This check keeps
normal GTK diagnostics enabled.
