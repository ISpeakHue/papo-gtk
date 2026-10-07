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
  aspect ratio and display at up to 720 × 480 pixels, including small backend
  thumbnails; decoded textures remain bounded to 680 × 460. Replies use a compact,
  single-line author/excerpt reference; navigating briefly highlights the original
  message in yellow/orange, then fades back over 900 ms without replacing its
  widget. Read-only message text supports selection without an
  editing caret and renders known `:custom_name:` emojis inline. Pointer context
  menus open at the click; keyboard menus retain their toolbar anchor. Delete is
  red and opens a compact confirmation. Both the menu and hover toolbar offer
  adding reactions; reaction tooltips fetch participant names on demand.
- The composer keeps attachment, mention and send controls together. Typing `@`
  opens filtered member suggestions; the `@` button lists members. Suggestions
  preserve typing focus and support Down/Escape. Sending keeps the composer
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
  microphone badge marks active speakers. A short original cue plays once when
  the local call connects, respecting the account sound preference.
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
