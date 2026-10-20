# Voice, camera and screen sharing

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

## Text chat in voice channels

The chat icon beside a voice channel opens its message history in the main chat
area without joining a call or opening the microphone. With voice permission,
the channel name keeps opening the call controls; **Entrar na chamada** remains
explicit. Without voice permission, the name opens the allowed text chat. Text chats
can be switched while a call stays connected, including chats in other rooms.
The chat header identifies the voice channel and the selected message target
is independent of the call-control highlight.

Voice chats use the same message APIs, media rendering, replies, reactions,
typing, history cache, unread markers and read acknowledgements as text channels.
Search results and notifications can navigate directly to their messages.
`read_channel` controls chat visibility, `send_messages` controls the composer,
and `connect_voice` controls the call. Chat-only and connect-only channels remain
available for their allowed purpose. Revoking one permission does not revoke the
other; deletion and permission updates reconcile both views.

```sh
PAPO_UI_CASE=voice-chat dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
PAPO_UI_CASE=voice-controls dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The `voice-chat` fixture uses synthetic local messages and a test voice engine.
It checks history, actual composer submission, live message delivery, navigation,
read-only/connect-only access, call preservation and independent permission
revocation. It does not open a microphone or connect to a real server.
Validated on 2026-10-19: warning-free all-target compilation, 280 automated tests
passed, and both focused GUI cases passed with GTK warnings treated as fatal.

## Moderation and voice acceptance

Owners and members with `manage_server` can open **Moderação e auditoria**.
Choose a member or paste a UUID to ban/unban someone absent from the list.
Each mutation has a confirmation; banning the server owner is disabled.
**Gerar recuperação** displays a one-use link and its expiry; **Copiar link**
performs the explicit clipboard operation. Changing the selected recipient, closing the window or losing
permission clears the link. Links are accepted by the existing login recovery
screen. Audit history is read-only, with action, actor UUID, entity type and
RFC3339 date filters, ascending/descending order and cursor pagination.
Invalid filters and failed operations retain input. Audit metadata values are
withheld from the viewer and Debug output to avoid exposing account secrets.

Click a voice channel to open compact audio controls above the sidebar account
footer, then click **Entrar na chamada**. The gear opens microphone/camera settings
in a separate window. Calls start muted, matching the backend; use the microphone
button to speak. Participants appear beneath their rooms before joining and show
mute state and active speakers. Camera/screen buttons beside connected participants
open a separate stream viewer. Active speakers show a microphone badge, including
when avatar display is disabled. A short cue plays once on local call connection
if account sounds are enabled. The screen button next to the gear starts screen
sharing; device options stay in settings. **Sair da voz** or **Fechar voz** releases capture
and media connections. Switching devices ends the current call; join again to
use the selection. Voice channels with `connect_voice` can appear independently
of text read permission. Chat, heartbeat and reconnect continue during calls.
A dropped connection ends the call; reconnecting requires an explicit join.
Revocation, deletion, a failed device or SFU negotiation error releases audio.

Voice uses the native GStreamer `webrtcbin` engine through an embedded Python GI
worker with private JSON-line IPC. Audio is Opus at 48 kHz, with eight receive
slots matching the backend defaults and RFC6464 audio levels. The app requests
fresh authenticated ICE configuration for each call, sends SDP/ICE through the
owning WebSocket, and discards queued voice frames on reconnect. TURN credentials
never appear in process arguments, saved settings or Debug output. The worker's
raw diagnostics are suppressed; errors shown in GTK contain no credential URIs.
Runtime API reference: [GStreamer WebRTC documentation](https://gstreamer.freedesktop.org/documentation/webrtc/).
Camera and screen sharing extend this worker, as described below.

Install the additional runtime on Fedora:

```sh
sudo dnf install python3-gobject python3-gstreamer1 gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free libnice-gstreamer1
```

Automated coverage includes ban/unban request bodies (including `false`), admin
versus own recovery contracts, token redaction, audit filter/date/cursor handling,
ICE authentication and credential redaction, actual inbound voice event shapes,
route snapshots, muted startup, participant/speaker rendering, stale worker
completions, dropped connections, SFU errors, deleted channels and revocation.
The expanded GTK smoke test exercises these controls alongside login and chat.
A separate native-worker test verifies process termination and reaping:

```sh
cargo test --locked native_voice -- --ignored
```

The real SFU probe imports the adjacent Go backend's `internal/webrtc` package;
it creates no database, production accounts or production requests. It uses two
independent native clients, a local authenticated TURN server, synthetic capture
and decoded-audio sinks. It checks audible bidirectional samples, Opus SDP/ICE,
active speakers, full/empty audio routes, mute, leave and missing-device cleanup.
TURN credentials contain reserved characters to exercise URI encoding. A Go
compiler matching the backend's `go.mod` and the adjacent backend checkout are
required:

```sh
cd tests/voice-sfu
go build -mod=readonly -o /tmp/papo-voice-sfu .
cd ../..
python3 tests/voice_probe.py
# Desktop capture verification: private virtual PulseAudio/PipeWire source.
# Requires pactl and gst-launch-1.0; does not open the physical microphone.
python3 tests/voice_probe.py --capture
```

The capture probe creates temporary private audio nodes, enumerates/selects the
virtual source through the same device API used by GTK, streams it through the
SFU, and removes the nodes afterward. CI includes the native cleanup test; the
optional **voice-sfu** workflow-dispatch job runs real audio when supplied a
backend repository and revision. No audio server is needed for its synthetic
capture path. These tests establish native interoperability, beyond protocol
mocks. Physical microphone/speaker quality and the deployed server's firewall,
TURN/TLS configuration and permissions still need checks with two real accounts.
On a disposable database, also check banning another connected client, unban,
one-use recovery expiry/reuse, owner protection and denied audit access. No
production moderation or password changes were made during this implementation.


## Camera and screen sharing

Join a voice room, choose a camera from the camera dropdown and click **Ligar
câmera**. **Desligar câmera** releases it. **Compartilhar tela** opens the Linux
desktop's permission picker for one window or monitor; **Parar compartilhamento**
closes its portal session and releases the PipeWire descriptor. Cancelling or
denying the picker keeps the audio call available. No capture starts on login,
room selection or device enumeration. Camera and screen can run together; wait
for each operation to finish before starting the other.

Members with active media have **Ver câmera** and **Ver tela** buttons. The
viewer switches its single subscription explicitly; **Parar de assistir** stops
forwarding. Old pictures are cleared on switches, publisher removal, stopped
publication, missing frames, disconnect, logout and permission loss. Calls do
not automatically rejoin after reconnecting. Camera selection applies to the
next start; stop the active camera before changing it.

The current media configuration targets the backend defaults: VP8, six video
receive slots and eight Opus receive slots. Two reusable publishing senders keep
the SDP within the backend's 17-media-section limit. Each capture restart uses a
fresh SSRC and sends its media intent before a serialized offer. A stopped
capture retains its negotiated sender but stops RTP and clears the backend's
active media role. This avoids the native transceiver reactivation failure
found in the real SFU prototype. The backend's unused outbound `VoiceOffer`
struct is not used as evidence for server-initiated video negotiation.

Publishing currently uses 640×360 at 15 fps; the GTK viewer displays up to 10 fps.
Only one unacknowledged video frame may cross IPC, with a 90 KB JPEG limit, so a
slow UI does not build an unbounded frame queue. The backend sends no video-slot
mapping/subscribe acknowledgement; using one viewer allows deterministic use of
`papo-video-0`, with a decoder reset before each new subscribe. A transmission
that never produces frames can be selected again after the timeout.

Additional Fedora screen-capture runtime (alongside the voice packages above):

```sh
sudo dnf install pipewire-gstreamer xdg-desktop-portal
# Install the portal backend matching the desktop, e.g. xdg-desktop-portal-gnome.
```

The [ScreenCast portal contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
uses CreateSession, SelectSources, Start and OpenPipeWireRemote. The worker keeps
permissions nonpersistent, closes requests/sessions on cancellation, handles
Session.Closed and closes the received descriptor. There is no direct desktop
capture fallback. Recent portals' stream serial is preferred when available;
older portals' node identifier is supported by the installed PipeWire plugin.

Automated checks (build the SFU fixture as described above first):

```sh
# Private D-Bus only: does not open your desktop picker or capture anything.
dbus-run-session -- python3 tests/portal_probe.py
# Real backend, three native clients, synthetic video/audio, no physical devices.
python3 tests/video_probe.py
python3 tests/video_probe.py --relay
```

The portal tests cover success/descriptor cleanup, denial, cancellation while a
request is pending, and desktop revocation. The SFU probe verifies decoded VP8
camera/screen frames over repeated starts/stops, camera-first and screen-first
intent ordering, switching a viewer between publishers, unsubscribe, removal,
missing-camera recovery, concurrent audio, bounded IPC and the SDP size limit.
The GTK smoke test clicks the new controls, preserves the audio call after a
capture error, rejects queued frames from a previous selection and clears a
removed publisher's picture. CI runs the private-portal test on pushes/PRs;
the optional backend workflow runs native video with direct ICE and TURN.

For a user-attended desktop check:

```sh
python3 tests/screen_picker_probe.py
```

Select a test window. This reads ten native PipeWire video buffers into a discard
sink, then closes the capture and portal session. It saves and transmits nothing.
This check passed locally on 2026-10-06 with the real desktop portal. The probe
explicitly requests video caps; an unconstrained discard sink can incorrectly
negotiate a media type and produce a PipeWire “target not found” error.

Local validation: 114 headless Rust tests, the GTK workflow test, native process
cleanup, four private-portal tests, actual SFU audio/video probes and the real
portal buffer probe. Physical camera quality, long calls, desktop-specific picker
behavior and the deployed server's video codec/TURN policy remain manual checks
with two or more authorized accounts. No production calls were made by the tests.

## Voice room selection and connection messages

```sh
PAPO_UI_CASE=voice-controls dbus-run-session -- xvfb-run -a \
  cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The synthetic fixture activates the actual voice rows without capturing audio.
It checks the independent room highlight, room changes and renaming, closing the
controls, deletion, preserved text-channel selection, and that selecting a room
does not join it. Initial connections and idle reconnects must stay quiet; an
interrupted join must report its error even after reconnecting. Selecting a new
room clears the previous room's error. With `PAPO_DESIGN_PREVIEW=1`, the fixture
writes `/tmp/papo-design-voice-selection.png`.
The same fixture checks the redesigned call settings at normal and narrow
widths, the “Entrar na chamada” label, Concluir, microphone gain and camera
availability. Synthetic worker acknowledgements verify the camera switch's
pending state, successful start/stop, error recovery and call teardown without
opening a camera. Optional previews include `/tmp/papo-design-call-settings.png`,
its `-light` variant and `/tmp/papo-design-call-settings-narrow.png`.

## Call sound playback and mixer identity

```sh
/usr/bin/python3 -m unittest discover -s tests -p 'call_sound_test.py' -v
PAPO_UI_CASE=call-sounds dbus-run-session -- xvfb-run -a \
  cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The sound helper identifies its audio stream as Papo (`br.com.papo.gtk`, event
role), rather than Python. This prevents an interpreter's restored mute/zero
volume from silencing call cues. Existing Papo/event volume and mute choices,
and the account sound preference, remain respected. The headless checks verify
metadata on dynamically added output sinks without opening an audio device and
decode both bundled cues to a fake sink. The GTK case checks disabled sounds,
join/leave playback, cancellation, rapid restarts and process cleanup.
