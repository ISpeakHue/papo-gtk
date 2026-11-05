# Notification controls, keyboard access and release discovery

## Notification controls

Preferences → Notifications → **Notification sounds and pauses** and the same
button in the notification inbox open the controls. A channel's context menu
also has **Mute temporarily…**. The scope selector includes all readable channels
and known DMs; choosing “All notifications” snoozes the account on this device.
Choose 15 minutes, one hour, eight hours or 24 hours, or resume immediately.
Dates use UTC for persistence and local time for display. Expiry is evaluated at
alert delivery, including after a restart; no timer or backend write is needed.

| Rule | Effect |
| --- | --- |
| Missing channel access, notifications disabled, own/read message, permanent channel mode Off | Existing delivery checks still apply. A local override cannot enable forbidden alerts. |
| Busy status | Suppresses desktop message alerts, including mentions, replies and DMs. |
| Account snooze | Suppresses all desktop message alerts for this account on this device. |
| Timed channel/DM mute | Suppresses message alerts for that conversation, including mentions and replies. |
| Expiry or “Resume now” | Restores the underlying permanent rules. Already suppressed pending alerts are consumed rather than replayed. |
| Message sound | Device/account override of the server sound default; independent of call cues. |
| Call sound | Device/account override for join/leave cues. An explicit call action remains audible during Busy/snooze if call sounds are enabled. |

These controls leave the grouped inbox, unread counters, server channel modes,
mention preferences and message previews intact. Permanent channel modes remain
in the existing channel settings window/context menu. “Server default” sound
continues to sync through the backend; explicit local message/call overrides
have precedence. Sound changes take effect when **Save sounds** succeeds.
Applying a pause does not save unsaved sound switches, and saving sounds does
not clear a pause.

Overrides are saved atomically in `notification-overrides.json` beside the native
configuration file, keyed by the opaque canonical-server/account namespace.
The file contains expiry timestamps, conversation UUIDs and sound flags, never
message bodies or session credentials. Writes run off the GTK thread and are
serialized. A failed write leaves the previous policy active and displays an
error. Expired channel entries are pruned on successful writes. The reader and
writer enforce a 1 MiB file limit. These are device-local controls because the
reviewed backend has no timed-mute/snooze or separate call-sound contract.

## Keyboard and accessibility

Preferences → Application → **Keyboard shortcuts**, or Ctrl+F1 (Command+F1 on
future macOS builds), opens a scrollable shortcut reference. Important additions:

| Shortcut (Linux/Windows) | Action |
| --- | --- |
| Ctrl+K | Conversation switcher |
| Ctrl+F | Message search |
| Ctrl+Shift+N | Grouped notification inbox |
| Ctrl+Shift+H | Focus latest rendered message |
| Ctrl+Shift+L | Focus composer |
| Menu / Shift+F10 | Open actions for focused message |
| Tab / Shift+Tab | Native control focus order |
| Escape | Close the innermost popup/window through its normal close path |

Message actions already supported keyboard menus; message rows now announce
their sender and action instructions. The composer announces its multiline role,
name and Enter/Shift+Enter behavior. Icon controls take their accessible names
from their translated hover hints; text actions retain explicit names. Shared
transient windows restore a still-mapped, sensitive previous control, without
retaining old conversations or stealing focus from another open modal. Nested
utility dialogs opened from settings attach to that dialog. Existing completion,
IME composition, popover and unsaved-change handling receive Escape first.

The action toolbar stays mapped for assistive tools; opacity controls its visual
hover/focus hint. Focus changes defer hiding until a newly opened popover has
settled. It appears immediately rather than animating. Shared wheel
scrolling jumps directly when reduced motion or system-disabled animations are
selected. Native touchpad scrolling remains unchanged. High-contrast role names, message
links and mentions inherit system text colors and retain underline/bold meaning;
syntax highlighting already disables custom colors in high contrast. Preferences
keep the save action outside the scroller for large text and small windows.

Automated cases: `accessibility`, `accessibility-contrast` and Linux-only
`accessibility-atspi`, plus the existing desktop-input, switcher, scrolling,
account-settings and syntax-highlighting cases. The AT-SPI helper reads only a
private synthetic application process on a private accessibility bus, checking the exposed
editor name/states, message sender names and reply action. This verifies the
Linux accessibility bridge, not Orca speech output. Windows CI includes the
portable GTK semantics/keyboard fixtures; execution on Windows and NVDA/Narrator
acceptance remain pending.

Real desktop acceptance on **both Linux and Windows** is still required: traverse
login, account tabs, search/filters, notifications/groups, switcher, drafts/outbox,
media browser/viewers and call setup with keyboard only; check speech names,
selected states, text editing, modal focus/return, high contrast, 200% text/DPI,
reduced motion, and permission/offline/error states. Test Linux Orca/AT-SPI and
Windows NVDA or Narrator/UIA. Report any controls that are silent or unreachable;
a test accessibility backend does not substitute for native reader acceptance.

## Release discovery

Preferences → Application → **About and updates** shows the installed Cargo
version and installation-specific guidance. **Check for updates** makes an
explicit anonymous request to the repository's public GitHub release feed.
Stable excludes draft/prerelease versions (including prerelease tags even if the
flag is missing); Experimental includes prereleases and stable releases. The
highest semantic version in the bounded feed wins. Channel selection is saved
on this device when checking; it does not change a Flatpak remote's branch.

The query uses a fresh HTTP client with no cookie jar, backend authentication,
redirects or automatic retries; connect/total timeouts are 8/20 seconds. It reads
at most 100 releases and 2 MiB. Notes are plain selectable text (at most 32,768
characters); the GitHub link has complete notes. Failed/offline/rate-limited
checks retain the manual releases link and never leave a download visible.
Changing channels clears stale notes/downloads before another explicit check.
Closing/replacing a window invalidates results by generation.

Downloads must be uploaded assets from `ISpeakHue/papo-gtk`, with HTTPS GitHub
URLs matching the release tag, exact package name and running CPU. Dependency
sources, checksum files, other architectures and guessed URLs are excluded.
AppImage offers the matching AppImage. Packaged Windows detects installer versus
portable metadata and offers the matching EXE/MSI or portable ZIP; Windows remains
experimental independently of the release's Linux stability. Source builds get
build instructions instead of a guessed binary. A missing compatible asset links
to notes instead of presenting a broken download. Nothing is installed or run.

Flatpak gets `flatpak update br.com.papo.gtk`/software-center guidance. A build
installed from a standalone bundle without an update remote also needs the new
`.flatpak` bundle; the corresponding asset is offered when available. Follow the
same installation scope used for the current app when installing that bundle.
The app does not spawn Flatpak, replace an AppImage or elevate privileges.

Public endpoint semantics: [GitHub Releases REST API](https://docs.github.com/en/rest/releases/releases#list-releases).

Local implementation results are recorded in the [validation evidence](testing/evidence/steps131415-2026-11-05/README.md).
