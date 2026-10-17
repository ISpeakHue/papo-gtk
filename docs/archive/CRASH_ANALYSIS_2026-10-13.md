# Crash analysis — 2026-10-13

Archived evidence from the original investigation. This report does not describe
a verified current reproduction or a confirmed fix. See [Follow-ups](../FOLLOW_UPS.md)
for the remaining investigation and the current testing guides.

Papo crashed at **21:41:22 America/Sao_Paulo**, PID **198435**, with
**SIGSEGV (11)**. The fault occurred in a native GStreamer player cleanup
thread, rather than a Rust panic. An invalid object pointer is consistent with
a use-after-free; this dump does not identify the earlier operation that
invalidated it.

## Evidence from the dump

The crashing thread was **199821**. Its backtrace, checked against the native
libraries independently of the rebuilt application executable, is:

```text
g_type_check_instance_is_fundamentally_a
g_object_unref
gst_play_main
g_thread_proxy
start_thread
__clone3
```

Disassembly places the failing unref after the player's main loop has returned,
during cleanup of the object at player offset `0x108`. The pointer stored there
matches the pointer passed to `g_object_unref`. Its class pointer is invalid;
the object's first two words resemble a freed allocator block. The upstream
GstPlay layout and cleanup sequence identify this field as `media_info`, but
matching full debug symbols for this installed build were unavailable, so that
field identification remains an inference. See the
[upstream GstPlay implementation](https://github.com/GStreamer/gstreamer/blob/main/subprojects/gst-plugins-bad/gst-libs/gst/play/gstplay.c).

The recorded package versions were GTK **4.22.5-2.fc44**, GStreamer
**1.28.8-1.fc44** (including `gstreamer1-plugins-bad-free`, which supplies
`libgstplay`), and GLib **2.88.4-1.fc44**. Other thread stacks contain H.264/MP4
playback work. The main UI thread was updating the chat render window when the
background player thread faulted; it was not the thread receiving SIGSEGV.

## Reported trigger and corresponding application code

The user closed a fullscreen video and then jumped to the latest messages.
That sequence corresponds to:

1. [`Fullscreen::restore`](../../src/ui/chat/video/fullscreen.rs) returning the same
   player to its chat row and resuming it if it had been playing.
2. [`ChatMsg::Latest`](../../src/ui/chat/mod.rs) moving to the bottom and enabling
   history trimming.
3. [`release_offscreen_playback`](../../src/ui/chat/transfers.rs) removing a player
   outside the retained messages once it is no longer fullscreen.
4. [`Playback::drop`](../../src/ui/chat/transfers.rs) muting/pausing, closing fullscreen,
   detaching the video stream and clearing the media file. Clearing a media file
   reaches GTK's native player teardown; see the
   [GTK media backend](https://github.com/GNOME/gtk/blob/main/gtk/media/gtkgstmediafile.c).

This is a plausible trigger for the recorded native cleanup failure. It does
not prove that Papo releases a reference incorrectly: a GTK/GStreamer lifetime
bug, a timing-sensitive interaction with our teardown order, or earlier memory
corruption still need to be distinguished.

## Analysis limits and next correction

An isolated GTK test used a locally generated H.264/AAC clip, moved its player
into and out of fullscreen, then immediately detached and cleared it to model
message eviction. **30 cycles on private X11 and 30 on private headless Wayland
completed without this crash.** These tests used no Papo account or remote
server. They did not exercise Papo's real chat/history widgets or the original
video, and the Wayland test used software rendering, so the result does not
rule out the reported timing/driver-dependent failure. Both test processes
exited successfully; compositor-disconnection messages after the Wayland run
were from shutting down the private compositor.

The application executable was rebuilt at **21:41:44**, after the crash. GDB
reports that its build ID differs from the dumped executable. Application source
line numbers from that replacement binary must not be treated as evidence. The
analysis above instead uses systemd's originally recorded application symbols
and separately loaded, matching native-library code.

The next targeted regression should exercise Papo's actual fullscreen restore
followed immediately by the latest-message action, with a playing H.264/AAC
fixture, until that playback is evicted. Preserve the exact executable for any
new dump, and use native debug symbols or memory instrumentation to locate the
first invalid release. Synthetic native-player tests are useful for narrowing
the cause, but cannot establish that this application path is fixed.

No application behavior was changed as part of this analysis. No credentials,
message contents or media from the dump are included here, and the core was not
uploaded to any service.
