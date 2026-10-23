# Windows packaged media acceptance (step 7)

This is the media integration gate in [the port plan](../../MULTIPLATFORM_PLAN.md).
The [decoder isolation contract](windows-isolation.md) still applies. Voice
capture, physical device routing and complete Home/Pro release qualification
belong to later steps.

## Implementation

Draft previews share received attachments' supported video classification, so
MOV/MP4/M4V/WebM do not depend on the desktop MIME database. Classification is
only a presentation hint: actual bytes still pass the self-contained-container
validator before GTK playback or isolated conversion. AVI/MKV download tiles and
playlist/external-reference rejection retain their existing behavior.

Windows conversions share two process-lifetime scheduling slots across draft,
poster and animated-GIF queues. Cancelling an async waiter keeps its slot until
the native job tree and private profile finish cleanup. The explicit native
check starts two live trees, queues a replacement, cancels one and requires its
profile/input to be gone before the replacement becomes ready. Original source
read handles allow deletion sharing so a cancelled temporary download can be
unlinked while the reader closes; the separate staged input remains sealed.

Backend-provided GIF renditions continue through authenticated backend endpoints.
No provider-page scraper or new automatic retry loop is introduced. Tests retain
the existing cache budgets, failed-preview suppression, moderation and access
invalidation, row reuse and first-frame residency behavior.

## Standalone runtime and instrumented UI runner

The Windows workflow builds the application ZIP and a separate optional
`Windows-media-acceptance` artifact from the same clean source. The latter holds
the stripped **test harness**, `MEDIA-TESTS.json` and `check-media-ui.ps1`; it is
not included in normal user ZIPs or advertised as a release executable.

The independent `media-ui-runtime` job has no MSYS2 installation. It downloads
both artifacts, verifies their hashes and identical source revisions, extracts
to a fresh path with spaces and accents, then places the harness in the shipped
runtime's `bin` directory. The harness uses the actual chat renderer, transfer
code, GTK media widgets and isolated converters with synthetic loopback HTTP
fixtures. Its settings are disposable, credentials use the test memory store,
and all runtime/helper paths resolve inside the ZIP. Machine PATH is unchanged.
Original manifest files must retain their hashes, and the extracted tree is
removed after testing.

```powershell
.\check-media-ui.ps1 -Archive .\Papo-1.0.2-windows-x86_64-dev.zip `
  -OutputDirectory .\media-results
.\check-media-ui.ps1 -Archive .\Papo-1.0.2-windows-x86_64-dev.zip `
  -OutputDirectory .\media-auto-results -Renderer auto `
  -Cases mov-media,backend-gifs,status-viewer,video-audio
```

These explicit checks open temporary windows. Run on a disposable desktop,
close existing Papo instances and keep the checks artifact beside the script.
Each case has its own process and four-minute watchdog; failures retain separate
stdout/stderr and make the overall report fail. Renderer type and sampled UI
working-set peaks are recorded. Instrumented-process memory and timing are not
release-build performance measurements.

| Case | Required assertions |
| --- | --- |
| `media-emojis` | Animated GIF paintables and server custom emojis advance frames, pause offscreen/reduced-motion, and release timers. Picker resizing remains a separate regression. |
| `draft-previews` | Small image/GIF/video previews, no upload/live player, failed-send retention, removal and stale-result cancellation. |
| `attachment-media` | Bare images/animated GIFs, viewer animation, download context action and byte-exact saves; moderation/access guards. |
| `mov-media` | Real MP4/H.264/AAC, MOV (including generic MIME) and WebM/VP8/Vorbis posters and native playback; audio-track presence, reopen reuse, source-file deletion and access cleanup. |
| `backend-gifs` | Backend GIF and video renditions animate, fallback remains bounded and source pages are never scraped. |
| `media-cache` | Range-based posters, cached server thumbnails, no background players, three channel round trips without additional requests, released animation objects and invalid-preview suppression. |
| `recent-media` | Recent opening batches, cache priority and reuse across conversations. |
| `status-viewer` | Original image quality, zoom/pan, actual fullscreen transitions, unchanged chat textures/requests and viewer cancellation/disposal. |
| `send-video` | Send feedback and fullscreen stream identity, seek/volume/mute state, Escape/F11 and disposal. |
| `video-audio` | Backend audio-update calls and preservation of explicit pre-preparation mute/volume choices. |

Headless tests additionally check PNG/JPEG/WebP/GIF decoding, animated APNG/WebP,
frame/dimension/allocation limits, container/playlist guards and cache byte/LRU
budgets. The packaged runtime check decodes each video using automatic selection
and explicit software fallback, plus a WAV-to-Opus audio pipeline. Ordinary audio
attachments keep their existing download presentation; this step does not add a
new audio attachment UI.

## Rendering and decoder scope

The complete UI matrix requests Cairo software rendering. A smaller second run
clears `GSK_RENDERER` and records GTK's automatic choice. Runtime diagnostics
record actual decoder factory names and registered hardware-decoder factories;
software H.264/VP8 decoding is tested separately. An advertised factory or an
automatic renderer is not proof of physical GPU acceleration. No GPU driver or
hardware decoder is forced or redistributed. Record physical-device results
separately when that hardware is available.

For Linux comparison, use the private framebuffer **and** window manager:

```sh
PAPO_UI_CASE=mov-media GSK_RENDERER=cairo GTK_A11Y=none \
  bash packaging/ci/media-gui-session.sh \
  cargo test --locked ui_smoke -- --ignored --test-threads=1 --nocapture
```

The Fedora CI image includes Openbox and Xprop for these focused checks. A bare
Xvfb server cannot qualify native fullscreen behavior. Unrelated existing Linux
GUI qualification failures remain tracked in [follow-ups](../FOLLOW_UPS.md).

## Current result

Implementation and local verification are in progress. Native Windows CI and
fresh-extraction standard-user VM results must be recorded before marking step 7
complete. macOS remains compile/link groundwork; no macOS media runtime is claimed.
