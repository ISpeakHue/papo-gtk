# Windows screen sharing — step 10

Implementation dated 2026-10-28. **Native Windows desktop acceptance has not run.**
The user has no Windows machine available until release testing. This implements
the port and its tests without treating Linux/synthetic evidence as Windows
capture evidence. The step 10 checkbox and `screen_capture` capability remain
open/false until the attended completion checks below pass.

## Source and ownership contract

`src/platform/capture/windows.cpp` is a narrow native DLL built with the existing
UCRT64 compiler and Windows system APIs. It needs neither Windows App SDK nor
another Python/WinRT binding. Its ABI prefixes follow
[Microsoft's Windows metadata](https://github.com/microsoft/windows-rs/tree/master/crates/libs/windows/src/Windows/Graphics/Capture).
The bundled GStreamer WGC plugin is optional: this implementation uses `appsrc`,
avoiding a plugin's default primary-monitor selection or unavailable WGC build.

On an explicit share action, the voice worker creates an STA thread and a small
native picker-owner window. `IInitializeWithWindow` receives that thread's own
HWND; the bridge checks both its process and thread identity. The temporary owner
is excluded from capture. The system picker chooses the screen/window; its
`GraphicsCaptureItem` remains in the same native thread/process for the entire
session. GTK HWNDs and COM pointers are never transported as JSON. Cancelling,
closing the owner, unsupported APIs or any setup failure starts no substitute
desktop capture. Each restart requires fresh selection.

The bridge follows Microsoft's [screen capture flow](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)
and uses [CreateFreeThreaded](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.direct3d11captureframepool.createfreethreaded)
to acquire frames without a UI dispatcher dependency. It keeps the system's
capture border. It does not enumerate/select a monitor automatically, use GDI
desktop capture, or persist capture consent.

## Frame delivery and teardown

- The native frame pool holds two BGRA textures. One latest CPU frame is retained;
  the consumer copies it through a process-local C ABI. No BGRA frames enter the
  Rust signaling pipe. Native frame/texture references are released before the
  next acquisition or pool recreation.
- Frames must have positive dimensions, at most 8192 pixels per side and at most
  64 MiB of packed BGRA data. Oversized sources fail recoverably. Actual content
  size changes discard old pending data and recreate the pool/staging texture.
  Gst caps change with the new pixel dimensions; no global DPI workaround is used.
- Python retains at most one previous frame to keep static windows producing
  VP8 keyframes for newly subscribed peers. It stops repeating on detecting source
  closure or when sharing stops. `appsrc` is live, nonblocking, timestamped,
  limited to one buffer/64 MiB, and drops old buffers under backpressure.
  The shared encoder queue remains limited to two frames.
- Shared encoding/signaling is unchanged: VP8 at 640×360/15 fps, negotiated send
  gate and backend screen-share intent. Received streams remain independent of
  local capture capability/permission. Selection hands off a Gst source only
  after the first validated frame. A generation guard discards consent callbacks
  queued before stop/leave/disconnect.
- Picker selection expires after 90 seconds, leaving room under the shared
  135-second UI negotiation bound. A source with no initial frame fails after
  15 seconds. Closing a source, device loss or failed downstream delivery removes
  that capture and leaves the microphone/remote audio running where recovery is
  possible. Normal stop closes the session, frame pool, closed-event subscription,
  picker operation and owner window before releasing the native context.
- Native close waits at most two seconds. If a driver or WinRT teardown hangs,
  the Python worker exits rather than detaching capture. The existing Rust voice
  worker ownership reaps the process and its Windows Job Object; leave, disconnect
  and application exit retain the graceful-stop/forced-kill bound from step 8.

This is SDR BGRA capture with CPU readback and the shared software VP8 encoder.
HDR tone mapping, GPU encoding and audio capture from the shared application are
not implemented here. Peak frame storage is bounded, but several native/Python/
GStreamer copies can coexist; the 64 MiB frame bound is not a total-process RAM
limit. Qualify performance with 4K and multiple GPU/monitor configurations.

## Packaging and preview

`packaging/windows/capture.py` builds `runtime/bin/papo-capture.dll`, statically
linking its GNU support runtime. The normal package import closure, native input
archives, licenses and hash manifest cover it. Python loads only the absolute
runtime DLL, restricting dependency search to its directory and Windows system
DLLs. The packaged runtime check validates ABI 1 and bounded `appsrc` support
without opening a picker or capturing anything. A device reporting WGC unsupported
does not fail the general runtime check; attempting capture fails closed.

For later attended testing, in PowerShell with the newly built package:

```powershell
$env:PAPO_WINDOWS_CAPTURE_PREVIEW = '1'
& 'C:\Papo preview\Papo.exe'
```

The share button is available only with that explicit opt-in and the bundled DLL.
The worker checks WGC support again before selection. `--platform-info` continues
to report the conservative, release-qualified capability as false. Remove the
environment variable to leave the preview. macOS retains its unavailable capture
adapter; Linux retains its consent-based portal/PipeWire adapter.

## Automated checks

These commands do not access real desktop content, accounts or production servers:

```sh
python3 -m unittest discover -s tests -p platform_capture_test.py -v
python3 -m unittest discover -s tests -p voice_worker_test.py -v
python3 -m unittest discover -s tests -p windows_capture_gst_test.py -v
dbus-run-session -- python3 tests/portal_probe.py
python3 tests/video_probe.py
python3 tests/video_probe.py --relay
```

The SFU fixture build instructions are in [voice testing](voice.md). On Windows,
use the UCRT64 interpreter; portable probe launchers discover `voice-sfu.exe`.
CI also runs `windows_capture_test.py`, which builds the native DLL and a synthetic
C++ ownership/frame/lifetime executable. That check creates only a hidden test
window, never a picker/capture session; it explicitly skips on Linux. The package
check loads the shipped DLL from the standalone runtime.

Local results on 2026-10-28:

- GNU AMD64 cross-compilation of both native DLL and test executable passed with
  `-Wall -Wextra -Werror`; the DLL import audit contains Windows system DLLs only.
  Cross-compilation does not prove WinRT runtime behavior.
- 15 adapter tests and 14 voice worker tests passed: no pre-consent frames,
  cancellation, repeated/late close, recoverable errors, bounds, resize, static
  repeats and stale consent suppression. The real GStreamer test passed two
  BGRA→VP8→RGB resize/restart cycles without a desktop or network.
- Four private mock-portal tests passed. Direct ICE and forced local TURN tests
  passed the actual three-peer SFU path with synthetic video, including repeated
  camera/screen publication, switching, backpressure, concurrent audio and leave.
- `cargo check --locked --offline --all-targets` passed without warnings;
  307 regular Rust tests passed, six ignored. The sandbox initially blocked local
  fixture sockets; rerunning with local socket access passed. No ignored GUI suite
  or previously tracked call-controls geometry failure is counted as a pass.
- Native Windows compilation/execution, system picker, GPU capture, packaged
  source/peer checks and Windows CI for this change are **NOT RUN** locally.

## Attended Windows completion checklist

**2026-10-29 automatic Windows result:** bridge compilation, native ABI/owner/
frame bounds and actual BGRA/appsrc/VP8 resize/restart checks pass. The packaged
runtime loads the bridge without opening capture. The user requested automatic
checks only, so picker cancellation/selection, real source/peer and physical
cleanup acceptance below remain pending. `screen_capture` remains false. See the
[current local record](windows-local-2026-10-29.md).

Use disposable content with a standard-user Windows 11 account and the standalone
runtime. Preserve the source revision, native inputs/package hash, OS build, GPU/
driver versions and results. Do not run these tests unattended: they intentionally
open Windows selection UI and may transmit the explicitly selected test window
to local test peers. No saved Papo session is used.

From the checkout, point the probes at the extracted runtime and its interpreter:

```powershell
$env:PAPO_RUNTIME_DIR = 'C:\Papo preview\runtime'
$env:PAPO_WINDOWS_CAPTURE_PREVIEW = '1'
$env:PATH = (Join-Path $env:PAPO_RUNTIME_DIR 'bin') + ';' + $env:PATH
$env:PYTHONHOME = $env:PAPO_RUNTIME_DIR
$env:PYTHONNOUSERSITE = '1'
$env:GI_TYPELIB_PATH = Join-Path $env:PAPO_RUNTIME_DIR 'lib/girepository-1.0'
$env:GST_PLUGIN_SYSTEM_PATH_1_0 = Join-Path $env:PAPO_RUNTIME_DIR 'lib/gstreamer-1.0'
$env:GST_PLUGIN_SCANNER_1_0 = Join-Path $env:PAPO_RUNTIME_DIR 'libexec/gstreamer-1.0/gst-plugin-scanner.exe'
$python = Join-Path $env:PAPO_RUNTIME_DIR 'bin\python.exe'
& $python tests/windows_screen_picker_probe.py --expect-cancel
& $python tests/windows_screen_picker_probe.py --frames 120
& $python tests/video_probe.py --windows-capture --sfu target/voice-sfu.exe
& $python tests/video_probe.py --windows-capture --relay --sfu target/voice-sfu.exe
```

The source revision must match the tested package. The first two probes count
and discard frames without recording/transmission. The SFU probe first requires
picker cancellation, then two selected-window sessions, resize/DPI movement and
window closure; all other sources are synthetic. Decoded frames arrive at two
other disposable peers. The selected content must also be checked visually in the
application; a valid decoded frame alone cannot establish correct picker choice.

1. Run the native non-capture test and standalone runtime integrity/relocation
   checks. Verify a missing DLL or unsupported WGC fails without default capture.
2. Cancel/X the picker and its owner window, including immediately after clicking
   share. Verify zero source/publication/frames and that audio continues.
3. Select a test window and a monitor independently. A second application client
   must show only the selected content, including obscured-window behavior and
   static content when subscribing late. Keep the system capture indicator visible.
4. Resize, minimize/restore, change resolution and move between 100/125/150/200%
   monitors. Verify current content without uninitialized margins or stale-source
   substitution. Disconnect the selected monitor and unplug/reset the GPU where
   practical; errors must release capture without selecting another source.
5. Close the selected window. Verify the native session/border ends and the share
   state resets while the call remains usable. Restart with a different source;
   consent must be requested again and no frames from the old source may appear.
6. Repeat start/stop ten times, then leave/disconnect/close the app while picking
   and while sharing. Verify no capture indicator, worker/thread/device handle or
   increasing working set remains. Check the two-second native-close/three-second
   worker fallback using a controlled failure harness, without a real stuck driver.
7. Record direct and forced-TURN peer results from the extracted/relocated package.
   Only after these checks pass, enable the Windows `screen_capture` capability,
   remove the preview-only policy and mark step 10 complete.
