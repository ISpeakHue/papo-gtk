# Windows calls: implementation and deferred acceptance

Step 8 implementation, 2026-10-27. **Windows hardware acceptance is NOT RUN.**
The user has no Windows machine available and requested that device tests wait
until a release is ready. W17/W18 in the [acceptance matrix](windows.md) stay open.
No supported-Windows release or macOS device-support claim follows from this work.

## Implemented behavior

- Enumeration prefers endpoint IDs/symbolic camera paths over friendly names,
  deduplicates provider entries and migrates an old friendly-name selection only
  when unambiguous. Enumeration stops its monitor even on failure. On Windows,
  reading an element's device property does not put it into a capture state.
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

The new Windows Job Object path, provider identifiers, device permissions and
real audible/camera behavior have **not** been executed on Windows for this change.
The updated native CI checks are prepared for the next CI run; no run is claimed.
