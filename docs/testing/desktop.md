# Desktop integration and layout

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

## Pelican branding

For the packaged runtime, run the AppImage's `--runtime-check` command and see
[APPIMAGE.md](../../APPIMAGE.md) for the isolated build and validation results.

The executable bundles 128px and 256px pelican icons. The focused GTK check
resolves and decodes both sizes through an isolated resource-only icon theme,
verifies the login and window icons, and can render light/dark previews:

```sh
PAPO_UI_CASE=branding PAPO_DESIGN_PREVIEW=1 dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

Previews are saved to `/tmp/papo-design-pelican-light.png` and
`/tmp/papo-design-pelican-dark.png`. Validate launcher metadata with
`desktop-file-validate data/br.com.papo.gtk.desktop`.

## Native chat layout

The GTK workflow also maps the redesigned chat into a real window. It checks
wide, medium and narrow breakpoints, opening and dismissing navigation overlays,
member toggling, direct-message/server navigation actions, consecutive-author
message grouping and keyboard access to the message action popover. It renders
light and dark appearances and verifies that open call controls fit a narrow
window. Permission checks inspect each menu action's visibility even when its
popover is closed. All message and server data comes from the local mock API.

It also covers preserving
message widgets and scroll anchors through typing, live arrivals and pagination;
explicit notification navigation; keyboard focus scrolling; reply/copy/context
actions; role labels; the older-message banner; native text/voice row activation;
responsive image previews and viewer cleanup; search filters; and profile controls
visible on opening. Mention renames and edits to replied-to messages must still
update cached rows.

Additional mapped checks cover automatic pagination (including duplicate prevention, failure/retry
and end of history), wheel scrolling, expiring reply highlights, pointer popover
anchors, read-only text and inline custom emoji paintables. The HTTP fixture
checks that an X preview with no image fields fetches its full text, thumbnail
and video metadata, and rejects an older response after a preview update.
Voice checks cover pre-join participants, preserved row identity during speaking
updates, full-width footer geometry, separate device settings, stream viewing,
and the existing call leave/disconnect/revocation paths.

Further GTK checks cover
compact destructive confirmations, context-menu reactions, on-demand participant
tooltips and request deduplication, typed/manual mentions and retained typing
focus, native inline WebM video/audio decoding, close/delete/file cleanup,
rejection of delayed downloads, and off-thread image result invalidation while
GTK timers continue to run. Highlight checks require the same row before, during
and after emphasis. Call-cue PCM validation runs headlessly; discard-sink cue decoding
and disabled sound behavior run in the GTK workflow. Speaking indicators preserve
participant buttons during updates. A 10,000-message headless fixture verifies
bulk merging, deduplication, chronology and realtime event replay.

The workflow also covers empty nickname fallbacks, native
character-by-character @ completion without moving typing focus, keyboard
selection, actual picture/player allocations and narrow media frames. It checks
small source decoding, enabled room-entry cues, duplicate cue suppression,
speaking indicators with either avatar setting and muted/silent transitions.
The cue helper must decode real PCM through a discard sink and reap its process
after completion or cancellation, including rapid repeated starts/stops.
Duplicate HTTP/WebSocket echoes must preserve reactions and history replay while
skipping unchanged renders. A mapped burst of 20 messages checks widget, text and
caret retention with a large preview payload; render signatures exclude encoded
images. The real SFU audio probe below separately checks active-speaker events.

The media fixtures are original, small and bundled; see
[tests/fixtures/README.md](../../tests/fixtures/README.md). Regenerating the video needs
FFmpeg, but running the tests does not. The GTK media backend needs VP8, Vorbis
and PCM decoding. Live production servers and SFU negotiations are outside this
mock workflow.

To save visual previews, run the same test with:

```sh
GSK_RENDERER=cairo PAPO_DESIGN_PREVIEW=1 cargo test --locked ui_smoke -- --ignored --test-threads=1
# In a headless environment:
GSK_RENDERER=cairo PAPO_DESIGN_PREVIEW=1 dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

Previews are written to `/tmp/papo-design-dark.png`, `papo-design-light.png`,
`papo-design-narrow.png`, `papo-design-narrow-navigation.png` and
`papo-design-narrow-call.png`. Additional previews are `papo-design-search.png`,
`papo-design-profile.png`, `papo-design-inline-image.png` and
`papo-design-history-banner.png`, `papo-design-sidebar-call.png` and
`papo-design-call-settings.png`, `papo-design-inline-image-narrow.png`,
`papo-design-inline-video.png` and `papo-design-mention-suggestions.png` in the same directory. Software rendering avoids compositor-specific
offscreen rendering artifacts. See [DESIGN.md](../../DESIGN.md) for navigation,
shortcuts and design references. High-contrast appearance, larger desktop text
scales and physical touch interaction remain manual checks.

## Conversation header and full-height navigation

Run the focused native layout regression with a private/mock server:

```sh
GSK_RENDERER=cairo PAPO_UI_CASE=gnome-layout dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
```

It authenticates through the real root component against the local fixture,
checks that only the conversation header is mapped after login, and verifies
that navigation starts at the top while the header spans the chat and member
pane. It covers 1280, 900, 540 and 360 pixel widths, including a long channel
topic, live title updates, geometric title centering over the chat with
absent/short/long descriptions, visible toolbar controls, compact overflow actions, sidebar breakpoints,
navigation/member toggles (docked, hidden and overlay), title alignment after
those toggles, search and notification windows, and restoration of the login
header after confirming disconnect from the icon beside the account settings
gear. Cancelling or dismissing the confirmation preserves the active session
and sends no logout request.
`PAPO_DESIGN_PREVIEW=1` saves
`/tmp/papo-design-gnome-{width}.png`. No saved session or remote server is used.

## AppImage hardware video plugins

```sh
python3 -m unittest discover -s tests -p 'appimage_packaging_test.py'
bash scripts/build-appimage.sh
```

The packaging regressions verify GPU-less operation, required plugin files,
exclusion of host/vendor drivers, and rejection of host GStreamer plugins in
the bundled decoder report. The build runs `AppRun --runtime-check` in the
isolated builder without GPU devices. That check loads both VA-API/NVIDIA
plugin libraries, permits empty hardware decoder registrations, and explicitly
decodes the original H.264 fixture with `avdec_h264` to validate software fallback.
It also retains the existing Opus/VP8, font/input-client and image-loader checks.
On a tester's machine, the same runtime check reports the registered hardware
video decoders; it does not require them to exist or claim that a particular
chat video selected one. See [APPIMAGE.md](../../APPIMAGE.md#hardware-video-decoding).

## Desktop text size and high-density displays

Chat and auxiliary rich-text views now use relative text sizes (small 0.9em,
medium 1em, large 1.3em), inheriting the desktop font instead of forcing a pixel
size. Unicode picker cells use 1.8em glyphs. GTK keeps control of monitor and
fractional scaling; the app does not force a global GDK scale.

Test a 3840×2160 display at the desktop's preferred scale, increase the desktop
text size, then move windows between monitors. Check search, profiles, reply
previews and the emoji picker as well as chat. A focused synthetic check can use
`GDK_SCALE=2 PAPO_UI_CASE=report10` with the GUI command from the testing index;
this is a test override, not a required launch option. Mixed-monitor fractional
scaling still requires real GNOME/KDE acceptance.

The GUI smoke harness uses a temporary `XDG_CONFIG_HOME` for every process,
so remembered sessions and device-local animation settings cannot leak into
fixtures or be changed by them. The previous environment is restored afterward.


## Notification activation

Desktop notifications retain their channel/message destination independently of
inbox pagination. Activation uses the desktop's Wayland activation token before
presenting the app, then navigates to the notified message. The target survives
a short close/read synchronization race; permissions are checked again on click,
and logout or access revocation discards it immediately.

The `account-notices` GUI case uses a private D-Bus notification service. It
covers activation tokens, a close signal preceding the click, inbox-row eviction,
read synchronization and immediate access revocation, without sending a real
notification. Also verify a background/minimized Papo on GNOME and KDE: click a
notification for another channel and confirm that the app opens that message.
