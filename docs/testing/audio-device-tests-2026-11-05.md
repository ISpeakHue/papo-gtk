# Local audio-device test repair

The earlier GTK workflow emitted button signals directly and used synthetic sources/sinks. A completed worker state did not establish that a user's click produced real output samples. Test progress/errors were also appended below the device groups inside the scrolling settings body.

The audio test workflow now activates the buttons with native X11 pointer events inside its own private regression window. A production-worker mode additionally requires explicitly named temporary virtual devices before capture can start. It chooses those devices through the same settings models as the application, checks the microphone meter, and verifies that status remains visible above the scrollable controls.

The application keeps local test feedback in the settings toolbar. A pending device change has explicit feedback, and an all-zero microphone recording reports silence. Linux local tests use explicit PulseAudio input/output elements when available. Their stream properties identify Papo, following the existing call-cue policy, instead of inheriting Python's saved mixer routing/mute/volume. The output signal has a fixed 48 kHz source rate and two-second duration.

The production-worker check used separate virtual input and output sinks, plus a selectable remapped virtual microphone. An 880 Hz generated signal fed only that microphone. The output sink was recorded solely to verify generated PCM and the raw data was deleted afterward. Actual button clicks produced:

- 626,400 captured mono samples at 48 kHz.
- Peak normalized amplitude: 0.3323974609375.
- 3.1 seconds of 880 Hz microphone replay.
- 2.3 seconds of 660 Hz output-test audio.

The GTK workflow, microphone-level assertion and spectral checks passed. No physical microphone or speaker was used. Local raw logs and the PCM summary are retained under `target/audio-functional-20261105/`.

The [PCM receipt](evidence/audio-device-tests-2026-11-05/pcm-verification.json) records the executable/log hashes and measured signals. Repeat with the current release-test executable using `python3 scripts/check-local-audio.py --binary target/release/deps/papo_gtk-<current-hash>`. This Linux check requires the cached `localhost/papo-ci:fedora44` image, a running local PulseAudio/PipeWire server, Podman and GStreamer command-line tools. It creates and removes its own virtual devices, confines pointer input to its private GTK fixture, and keeps only generated-signal statistics under `target/audio-device-tests/`.

Python voice-worker coverage includes Papo mixer identity, explicit Linux audio-server routing, silent-input feedback, clocked replay, stale device events, cancellation and call transport invariants. The audio setup workflow is already included in the normal Linux and Windows GUI case lists; native pointer injection is Linux-specific.
