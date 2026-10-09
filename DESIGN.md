# Papo's native chat design

The main window uses Discord's familiar navigation structure with GTK 4 and
libadwaita controls, system colors and GNOME window behavior.

- A compact rail switches between direct messages and the connected server.
- Channel navigation has a server menu, search, category headings, unread
  indicators and an account footer. Server administration, moderation and blocks
  live in the server menu; account preferences, security and logout live in the
  footer's settings menu. The rail ends above the account footer, which spans
  the full navigation width. Existing permission checks still govern the actions.
- The conversation toolbar shows the channel and topic, pins, notifications with
  an unread badge, members and an
  overflow menu for emojis and channel preferences.
- Messages show author avatars, local dates and times, replies, attachments,
  previews and reaction pills. Consecutive messages by the same author within
  five minutes share a heading. Replies, edited messages and pinned messages
  retain their own heading. Message actions appear on hover or keyboard focus;
  right-click, touch-and-hold, Menu or Shift+F10 opens reply, copy, editing,
  deletion, pinning and reaction details according to permissions. Role colors
  follow the backend's assigned role order. Inline image thumbnails preserve
  aspect ratio and display at up to 640 × 480 pixels, including small backend
  thumbnails; image embeds use up to 420 × 300. Decoded textures remain bounded
  to 680 × 460 without enlarging tiny chat source buffers. Replies use a compact,
  single-line author/excerpt reference; navigating briefly highlights the original
  message in yellow/orange, then fades back over 900 ms without replacing its
  widget. Read-only message text supports selection without an
  editing caret and renders known `:custom_name:` emojis inline. HTTP/HTTPS links
  are blue, underlined and open through the system URI handler; text selection
  and copying remain available. GIF paintables preserve animation while visible;
  clipped pictures pause frame advancement and cached/unmapped pictures stop
  their timers. Successful media completions share a batched history update;
  stale completions do not redraw it. Shared preview metadata excludes encoded
  image payloads. The viewport follows actual layout extent changes when reading
  the latest messages, including allocations after a send confirmation, while
  preserving manual scrolling through older history. Channel loading waits for
  a history snapshot before displaying the empty-chat prompt. Pointer context
  menus open at the click; keyboard menus retain their toolbar anchor. Delete is
  red and opens a compact confirmation. Both the menu and hover toolbar offer
  adding reactions; reaction tooltips fetch participant names on demand.
- The composer keeps attachment, emoji, mention and send controls together. The
  emoji button is immediately left of `@`; `:` filters Unicode names and server
  codes without taking typing focus. The native picker provides all Unicode
  emojis, and server emoji pages appear progressively after background decoding. Typing `@`
  opens a filtered panel above the composer; the `@` button lists members.
  Suggestions preserve typing focus and support Down, Enter, Tab and Escape. Sending keeps the composer
  focused, temporarily read-only until its request completes. A banner
  above it provides a jump to recent messages while reading older history. Presence
  remains visible in the member list, direct-message list and account footer.
- Older messages load automatically near the top of history, with one request
  at a time. A failed page exposes an explicit retry rather than repeatedly
  issuing requests. Discrete mouse wheels animate; touchpads retain GTK scrolling
  and libadwaita respects the desktop's reduced-motion setting.
- Voice participants appear beneath their room before joining. Compact call
  controls sit above the account footer: microphone, call settings, leave and
  screen sharing. Device selectors and camera controls live in a separate
  settings window. Watching a participant opens a separate stream window. A
  microphone beside the name and a semantic highlight mark active speakers,
  including when avatars are hidden. A short original cue plays once on accepted
  room entry, respecting the account sound preference; cancellation releases its
  helper process without blocking GTK.
- Preview cards fetch complete backend metadata even when the message summary
  contains no image fields. They display provider, author/title, text, thumbnail
  and the backend's video relay when available. Videos and video attachments
  play inline with native controls. Closing, deletion or permission/session
  changes stop playback and release temporary files. No remote embed scripts run.

The member pane becomes an overlay at widths up to 1100 logical pixels. At
760 pixels and below, navigation also becomes an overlay. Choosing a channel
or conversation dismisses the navigation overlay. Selecting a voice room keeps
the overlay open so its call controls remain accessible. Toolbar buttons and keyboard
shortcuts can reopen either pane. Theme scaling uses libadwaita's `Sp` units.

| Shortcut | Action |
| --- | --- |
| Ctrl+K or Ctrl+F | Search messages |
| Ctrl+, | Account preferences |
| Ctrl+Shift+M | Toggle members |
| Alt+1 | Server channels |
| Alt+2 | Direct messages |
| F9 | Toggle navigation |

The application follows the configured system/light/dark appearance and accent.
Colors use Adwaita's semantic theme colors rather than a fixed Discord palette.
Native header bars, window controls, dialogs, focus indicators and tooltips keep
the desktop's behavior. Custom CSS is limited to application surfaces, spacing,
selection states and compact chat controls.

Design references: GNOME HIG [sidebars](https://developer.gnome.org/hig/patterns/nav/sidebars.html),
[header bars](https://developer.gnome.org/hig/patterns/containers/header-bars.html)
and libadwaita [theme colors](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1.6/css-variables.html).

See [TESTING.md](TESTING.md#native-chat-layout) for automated layout checks and
mock-data previews. Preview generation never connects to a production server.

## Fourth media and completion pass

Mention completion uses a bounded panel above the composer, preserving native
input focus. Member rows show display names and usernames. Down navigates into
the list; Enter or Tab completes a mention; Escape closes suggestions.

Attachments use responsive aspect-ratio frames up to 640 × 480. Videos use a
640 × 360 frame with a centered play action and native playback controls. Image
embeds use up to 420 × 300. Media shrinks with the chat width, and image/video
caption rows offer an icon with the accessible tooltip “Baixar arquivo”.

Active speakers have a visible microphone beside their name and a semantic
success-color highlight. The indicator works with avatars shown or hidden and
is suppressed for muted participants. The join cue starts on accepted room
entry through a cancellable native audio helper and follows the saved sound
setting.

## Bug report 5 interactions

Read cursors are captured on selection, before history updates mark the channel
read. The first unread row is revealed without moving focus from the composer;
fully read channels open at the bottom. A bounded sequential older-page search
can locate an unread boundary outside the latest page. Explicit human navigation
cancels it. A confirmed send returns to the latest row even if WebSocket and HTTP
responses carry identical content.

Opening a channel keeps the history mapped for native text measurement, but
defers its first paint until the selected scroll position and allocated geometry
are stable. Unread-page searches do not expose intermediate positions. The
older-history banner is allocated before presentation, and channel generations
invalidate obsolete reveal callbacks. One opening callback is reused across
media/live updates, with a bounded frame budget; images download independently.
Empty results, errors and explicit reader navigation also release presentation.
Normal pagination, reconnects and live messages retain the visible history.

Video picture clicks toggle playback without claiming native controls. Started
videos explicitly enable sound, and closing still detaches their controls before
teardown. The image viewer closes on a background click, Escape or focus moving
outside it; both inline GIFs and their viewers animate within bounded caches.
Preview cards clip to their measured allocation and leave space before the next
message. Search starts at 540 × 520, with advanced filters collapsed.

Voice entry and departure have distinct short cues under the same sound
preference. Local capture reports speaking transitions with a short release
threshold; muting immediately removes the microphone indicator and highlight.
See [BUG_FIXES_5.md](BUG_FIXES_5.md) for implementation bounds and checks.

## Bounded chat and background updates

The chat keeps at most 200 message rows, plus date and measured-height spacer
rows, while retaining history data for scrolling and reply/navigation lookups.
Dirty message IDs include grouping neighbors and reply dependents. Catalog
updates rebuild the emoji-name index once. Scrolling materializes another window
and restores its message anchor; off-window inline playback releases its files.
Media hydration prioritizes the viewport and nearby rows, admits at most eight
outstanding jobs, and uses four transfer/decode permits. Texture residency,
in-flight work, failures with bounded retries, and metadata-only completions are
separate states. Deferred embedded payloads have an 8 MiB/32-entry budget.

Avatar decoding runs on two background workers. Batches publish 16 profiles at a
time, with 72-pixel member textures and a 1,024-texture limit. Profiles use a
separate 192-pixel avatar. Presence status changes update existing rows directly;
metadata invalidations batch per member and reject stale response generations.
Sparse presence events preserve cached names until a summary confirms removal.

Background read snapshots have independent live-event journals. Reconnect
revalidates retained pages and authoritative deletions without clearing the
visible chat. DM refreshes allow one in-flight request and one follow-up. Renewal
uses token expiry and bounded outage retries; remembered credentials carry a
session owner that survives rotation. Socket handshakes/writes have ten-second
deadlines, and 75 seconds without a heartbeat acknowledgment triggers reconnect.


## Report 6 interaction and media updates

The header exposes search beside notifications. Composer and reaction choosers
share a searchable Unicode/server catalog with square cells and 96-result pages;
colon completion stays inline. Notification authors use the existing member and
avatar caches. Reply context has an accent surface and the author's role color.

Chat media is cached for the session within fixed texture, animation, metadata
and temporary-video-file budgets. Selecting a channel cancels its obsolete work
and releases players while retaining reusable assets. Current server metadata,
moderation and permissions still decide whether an asset may be displayed.
Media-only updates reconcile individual media slots instead of replacing the
message's text, avatar and controls. Known geometry survives thumbnail eviction.

At startup the sidebar uses the same background avatar cache as the rest of the
application, avoiding a second full-size decode on GTK's UI thread. Server icons
decode on the bounded image workers to at most 64 × 64 pixels. An unchanged icon
is reused, and request generations reject obsolete results after replacement or
removal. Member-avatar batches change existing avatar widgets in chat, member,
DM and voice rows without formatting history or rebuilding those rows. Existing
symbolic fallbacks remain visible while images load; no startup animation is added.

Chat scrolling takes priority over automatic media work. While the wheel,
touchpad or scrollbar is moving, newly decoded pictures stay queued for display
and automatic image hydration waits. A single idle callback resumes work after
a short pause, using the current viewport so skipped images need not load first.
Existing placeholders retain their geometry during movement. Once idle, image
layout preserves the reading anchor. History reflow compensates scroll geometry
without stopping an ongoing wheel animation; explicit navigation still stops it.

Anchor corrections run after GTK layout and before painting, including geometry
compensation during wheel animation. A decoded image above the reading position
therefore does not expose an intermediate frame at the old scroll offset. The
frame-clock handler connects only while the history is mapped and disconnects
on unmap. Layout-triggered media refreshes share one pending callback.

Media metadata normalization follows the history's media revision; unchanged
viewport updates, reactions and plain live messages reuse normalized metadata.
Candidate discovery visits at most the 200 materialized message rows, with the
existing near-viewport priorities and transfer limits. Unread-boundary searches
reserve media work for their final destination. The rendered history window
retains its rows across small reading movements and recenters near its edges;
explicit navigation and following the newest messages remain immediate.

Inline video uses GTK's video surface with explicit transport and mute/volume
controls. Its native MediaControls stream is detached to avoid a competing mute
binding. Volume appears on hover or keyboard focus; mute preserves the chosen
level. Cached files do not keep a player running in an inactive channel.

Attached and embedded videos share a fullscreen button. It moves the existing
player and controls into a fullscreen window, preserving position, play/pause,
volume and mute. Escape, F11, the exit button or closing that window restores the
inline player. A placeholder preserves the message's space in the timeline.
Fullscreen playback survives scrolling its row out of view; deleting the message,
revoking access, switching channels or disposing playback closes the window.

Text and embed-only sends keep the composer's layout stable while awaiting the
server. Upload progress and cancellation appear only for sends containing files;
send failures retain the draft and display an error.

Message TextViews request the height of GTK's complete wrapped layout, including
inline paintables, after their allocated width is known. A WidgetPaintable size
observer updates this on resize without continuous timers. This prevents a newly
appended grouped message from remaining at zero height until the next input.

## Rich embeds

The message model keeps the backend's nested embed metadata. Live
`message_embeds_update` lists replace cards, including an empty-list clear;
concurrent read journals and POST completion preserve newer complete lists.
Complete text-only cards render without per-card API calls. Thumbnail dimensions
reserve geometry, and media continues through authorized, bounded session caches.

Cards use native labels, safe links, color accents, ordered inline field grids
and a left-aligned libadwaita clamp. Its tightening threshold matches the maximum
width so cards align with the message text at wide and narrow sizes. The composer
and message edit dialog share a form-based
custom-card editor. Validation uses backend character and media constraints.
Remote HTML is never loaded; a backend-pattern YouTube URL opens in the browser.
See [EMBEDS.md](EMBEDS.md) for the API and backend image/edit limitations.

Inline playback forces its initial volume through the native audio backend,
then repeats this once after preparation if the user has not changed mute or
volume. GTK's cached default properties alone do not guarantee an audio update.
There is no recurring timer or reset on later play/pause actions.
