# Windows calls: implementation and deferred acceptance

Step 8 implementation, 2026-10-27, updated on Windows on 2026-10-29.
Physical USB microphone and default-output checks have now run on this Windows
machine. Camera and deployed-backend acceptance remain incomplete: the user
confirmed no camera or live test accounts are available, and requested automatic
local checks rather than attended unplug/privacy/default-device changes.
W17/W18 in the [acceptance matrix](windows.md) therefore stay partially verified.
No supported-Windows release or macOS device-support claim follows from this work.

## Implemented behavior

- Enumeration prefers endpoint IDs/symbolic camera paths over friendly names,
  deduplicates provider entries and migrates an old friendly-name selection only
  when unambiguous. Enumeration stops its monitor even on failure. On Windows,
  reading an element's device property does not put it into a capture state.
- WASAPI2 speaker-loopback sources are excluded from microphone enumeration,
  selection and hotplug handling. A combined monitor filters selection by device
  class. The WASAPI2 default alias retains default-device recovery behavior;
  changed endpoints retire their old IDs so replugging refreshes settings.
- Default Windows input prefers WASAPI2, then WASAPI. The worker itself defaults
  to muted capture even if the caller omits `muted`. Gain stays bounded to 0–300%.
  No silence-producing `continue-on-error` override masks an unavailable device.
- A live device monitor refreshes GTK's device lists. Removing/changing the
  selected microphone ends the call and requires an explicit, initially muted
  rejoin. For a default microphone, audio-device changes conservatively end the
  call rather than switching live capture silently; adding another audio device
  can also require rejoining. A removed/changed camera stops that publication
  while retaining audio. Permission/start errors remain recoverable.
- Cameras accept raw or MJPEG output through `decodebin` before VP8 encoding.
  Camera selection and enabling remain explicit; merely opening settings does
  not start capture. Windows screen sharing remains the separate step 10.
- Voice and cue workers acquire process-tree ownership before receiving their
  embedded Python source. Windows uses a kill-on-close Job Object; Linux/macOS
  use a dedicated process group. Failure to establish ownership rejects startup.
  This is lifecycle containment, separate from the AppContainer decoder sandbox:
  calls intentionally need network/device access.
- Leaving first requests native cleanup, then kills an unresponsive worker after
  a bounded grace period. IPC lines/queues, pipe writes, startup and enumeration
  have bounds. Closing/dropping a worker also terminates inherited descendants.
  Cue decoding/cancellation waits for its child; GUI checks no longer assume
  Linux `/proc` for cue cleanup. TURN credentials still use private IPC.
- Packaged runtime checks require native audio, Media Foundation camera and JPEG
  decode factories without opening a device. Linux/Windows CI include synthetic
  device-policy, raw/MJPEG decode, cue and descendant-cleanup checks.

The endpoint and camera choices follow the upstream
[WASAPI2 source](https://gstreamer.freedesktop.org/documentation/wasapi2/wasapi2src.html),
[Media Foundation source](https://gstreamer.freedesktop.org/documentation/mediafoundation/mfvideosrc.html)
and [device-monitor](https://gstreamer.freedesktop.org/documentation/gstreamer/gstdevicemonitor.html)
contracts. Installed providers, drivers and privacy controls still need native
acceptance; metadata fixtures are not proof that every device reports these keys.

## Portable synthetic probes

Use the native Python/GI runtime matching the application (UCRT64 on Windows),
not an unrelated system Python. Install the dependencies in
[MULTIPLATFORM.md](../../MULTIPLATFORM.md). Keep the adjacent backend checkout at
`../papo-backend/backend`; the Go fixture imports that backend's actual SFU.
Run from the project root in a UCRT64 terminal, or a Linux/macOS shell with GI:

```sh
python -m unittest discover -s tests -p voice_worker_test.py -v
python -m unittest discover -s tests -p probe_runtime_test.py -v
python -m unittest discover -s tests -p call_sound_test.py -v
cargo test --locked native_voice -- --ignored --test-threads=1
cargo test --locked native_call_ -- --ignored --test-threads=1
cd tests/voice-sfu
go build -mod=readonly -o ../../target/voice-sfu.exe .
cd ../..
python tests/voice_probe.py --sfu target/voice-sfu.exe
python tests/video_probe.py --sfu target/voice-sfu.exe
python tests/video_probe.py --sfu target/voice-sfu.exe --relay
```

The explicit `.exe` filename above is also usable on Unix. With no `--sfu`, the
default is `target/voice-sfu.exe` on Windows and `target/voice-sfu` elsewhere.
`PAPO_TEST_SFU` and `PAPO_TEST_PYTHON` are optional path overrides. Paths containing
spaces are passed directly, without a shell; Windows probe workers hide console
windows. `--capture` remains a **Linux-only virtual PulseAudio source** option,
not a portable physical microphone test.

The fixture needs a non-loopback IPv4 adapter and permitted local UDP sockets
because the backend deliberately rejects loopback candidates. It uses disposable
local TURN credentials, synthetic audio/video and discard sinks, with no login,
database, production traffic, physical microphone or camera. macOS can use these
portable launchers once its native dependencies are available; no macOS device
or screen-capture acceptance has been performed.

## Deferred packaged Windows checks

Use two disposable accounts on an authorized test backend. Record the final
artifact/source hashes, backend revision, full Windows build, runtime manifest,
device IDs/names and drivers as required by the acceptance matrix. Repeat on the
release's required Windows 11 Home/Pro targets; CI's Server runner does not qualify
them. Use the distributed application for these checks, without developer tools.

1. Enumerate two same-model USB microphones/cameras. Verify distinct selections,
   unplug/replug and settings refresh. Confirm no capture on login/room selection
   or settings open. With no devices, text chat remains available.
2. Join initially muted. Explicitly unmute and exchange intelligible audio and
   actual camera frames between two clients through direct ICE, then forced TURN
   using test-backend configuration. Check 0/150/300% gain and mute after gain
   changes. Hear join/leave cues when enabled; verify disabled sounds remain quiet.
3. Deny microphone and camera access in Windows privacy settings. Verify useful
   errors and released resources, no automatic unmute/rejoin, and working text
   chat. Denied camera access must retain audio. Restore permission and explicitly
   rejoin/re-enable; ensure there is no stuck switch or pending negotiation.
4. Unplug the active microphone: call ends and capture is released. Unplug the
   active camera: camera ends and audio continues. Change the default microphone
   and refresh/switch selected devices. Rejoin explicitly and confirm the new
   input starts muted. Exercise raw and MJPEG camera modes where available.
5. Interrupt network/WebSocket connectivity and restore it. Confirm stale call
   events/SDP are discarded and reconnect never resumes capture automatically.
   Rejoin and repeat camera start/stop and viewer switching.
6. Repeat join/leave at least 20 times, including leaving during negotiation and
   camera startup. Close/logout while capturing and while a cue plays. Confirm
   microphone/camera privacy indicators clear, devices are usable by another app,
   and no owned Python/plugin-scanner descendants survive normal or abrupt exit.
7. Switch the default playback device, verify remote audio and cues follow the
   system selection or fail recoverably, and test a missing/disconnected output.
   Record observed output behavior; no separate in-app output selector is added.

## Local verification, 2026-10-27

Linux source-tree checks used the working tree based on frontend `39614bb` and
the adjacent backend `3252cfc`; these are not final release artifact results.

- All-target Rust compilation passed; 297 headless Rust tests passed, six ignored.
- Three native tests passed: cue completion/cancellation, repeated worker-tree
  shutdown with a synthetic descendant, and voice startup/gain/process release.
- Python device-policy/decoder and portable-path tests passed; both real cue WAVs
  decoded to discard sinks. No physical capture/output was opened.
- Actual local SFU: bidirectional decoded Opus, routes, active speakers, mute and
  leave passed with direct ICE and forced TURN. Three-client VP8 repeated camera/
  screen cycles, switches, missing-camera recovery, backpressure and leave passed
  with both transports. Screen sources here are synthetic, not Windows capture.
- Focused Linux GTK `call-sounds` and `voice-chat` cases passed on a private virtual
  display. `voice-controls` failed its camera-controls/Concluir geometry assertion;
  that layout assertion is not counted as a pass or Windows device evidence.

Those results describe the original Linux implementation run. Current Windows
verification and its remaining limits are recorded below.

## Local Windows verification, 2026-10-29

The development working tree is based on frontend `52df9a9`; it is modified and
has not been packaged or qualified as a release. The host is Windows 11 Home
23H2, build 22631.6199, outside the release's planned Home/Pro OS rows. Tests use
the native UCRT64 runtime and the explicit developer `-ExcludeHip` override.

- Native device metadata exposed WASAPI2 speaker-loopback entries in the old
  microphone list. Filtering now rejects those entries without opening devices.
  Default aliases and real endpoint identifiers were inspected locally; no
  physical microphone or camera was activated.
- Nineteen worker tests cover endpoint identity, loopback filtering, hotplug,
  default aliases, synthetic permission failure, raw/MJPEG decoding, muted native
  startup and gain changes that never unmute capture.
- The actual backend SFU fixture was built from the existing Git object
  `3252cfc2b0563d31d6c6eedd535c94fe5f15aceb`, exported into ignored test files.
  The adjacent backend checkout was not restored or modified. Two native clients
  exchange decoded synthetic Opus through direct ICE and forced local TURN;
  routes, speakers, mute, leave and missing-device cleanup pass.
- Three native clients exchange decoded VP8 over direct ICE and forced TURN.
  Camera/screen restarts, publisher/kind switches, missing-camera recovery,
  concurrent audio, bounded IPC and leave pass. The probe now waits for
  `video_reset` before subscribing, matching GTK's existing barrier. The original
  probe raced reset against incoming RTP and timed out on Windows. Sources and
  playback sinks are synthetic; this is transport/lifecycle evidence, not camera
  or consent-picker acceptance.
- Focused `voice-controls`, `voice-chat` and `call-sounds` GUI fixtures pass.
  The shared full-GUI voice fixture now checks disabled local capture when the
  platform preview is unavailable, while still checking remote viewing.
- The patched lockfile passes 310 regular Rust tests (six opt-in cases excluded
  from that run), followed by three explicit native voice/cue/process-tree tests.
  Twenty additional native synthetic startup/gain/drop cycles pass. The Python
  runs cover 51 distinct passing tests, with the Linux-only PulseAudio identity
  test explicitly skipped on Windows. Native Windows cue decoding is tested.
- The development application builds successfully. `recovery` verifies cancelled
  permission refresh at account closure; `recent-media` passes ten native
  fullscreen/latest/history-eviction cycles. These focused passes do not qualify
  the broad default GUI fixture, which still has a separate native failure
  tracked in [Follow-ups](../FOLLOW_UPS.md).

`https://api.cyberasilo.online/health` returned HTTP 200 during the initial
read-only check. No account login, production call or server mutation was made;
after the user selected local-only testing, no further server checks were run.
Local logs are under `target/windows-dev/step8/`; the ordinary developer wrapper
also retains Python, regular Rust and native-worker results in
`target/windows-dev/`. Synthetic passes do not close the seven physical/packaged
acceptance items above or the step 8 completion checkbox.

## Physical Windows audio verification, 2026-10-29

This continues the local run above on the actual Windows machine. Captured audio
was decoded and discarded locally; no recording was written and no production
account or server was used.

- Windows enumerates a KT USB Audio microphone/headphones, Steam virtual audio
  endpoints and the system-default aliases. No `Video/Source` is exposed; the
  user confirmed that no camera is available.
- The default physical microphone opens and decodes through Opus. The explicit
  USB endpoint passes **20 actual capture/encode/decode/release cycles**. Each
  cycle verifies initially muted capture, 0/150/300% gain, gain changes preserving
  mute, silence at zero gain/when muted, decoded buffer flow and a native NULL
  state after shutdown. These are physical pipelines, not synthetic sources.
- The application's Rust `Engine`/Windows Job Object path also passes **20
  physical-microphone startup/gain/drop cycles**. Every cycle starts muted and
  verifies termination through a retained native process handle. The separate
  physical pipeline check above verifies decoded samples and device release;
  neither count is relabeled as twenty deployed-backend calls.
- Papo's real Python voice workers use that microphone through the local backend
  SFU. Direct ICE passes decoded non-silent audio, routes, speaker activity, mute
  and leave. A strict TURN speech check times out with both peers connected and
  relay candidates present but without qualifying non-silent samples. A separate,
  explicitly selected quiet-input mode passes captured/decoded buffer transport,
  routes, mute and leave over **both direct ICE and forced TURN**. That mode
  does not claim speech/active-speaker or conversational-quality acceptance.
- The actual bundled join/leave cues select `wasapi2sink`; the user confirms
  hearing both. The cue helper reports its native output factory so a discard
  sink cannot be mistaken for physical playback evidence.

Reports: `target/windows-dev/step8/device-inventory.json`,
`default-microphone.json`, `usb-microphone.json`, and `physical-cues.json`.
`physical-worker-cycles.json` and `physical-worker-cycles.log` retain the real
application-worker repetition results.
These are development-tree results, not a clean packaged-release qualification.

Use the matching UCRT64 runtime. All physical operations are explicit:

```sh
python tests/windows_devices_probe.py --list
python tests/windows_devices_probe.py --microphone 'Microfone (KT USB Audio)' --cycles 20
python tests/windows_devices_probe.py --cues
python tests/voice_probe.py --sfu target/windows-dev/step8/voice-sfu.exe --microphone 'Microfone (KT USB Audio)'
# Transport only when no operator can supply speech; does not qualify speech quality.
python tests/voice_probe.py --sfu target/windows-dev/step8/voice-sfu.exe --microphone 'Microfone (KT USB Audio)' --allow-quiet-microphone
# Explicitly use the physical microphone in the real Rust worker lifecycle test.
PAPO_TEST_VOICE_DEVICE='Microfone (KT USB Audio)' cargo test --locked native_voice -- --ignored --test-threads=1
```

The device probe owns its process tree before native initialization and bounds
startup, test duration, output and cleanup. Ordinary tests still use synthetic
capture by default. The remaining acceptance blockers are concrete: a physical
camera, two authorized deployed-backend clients, attended permission/unplug/
default-output changes, and the packaged supported-OS checks. Windows availability
itself is no longer the reason these checks are pending.
