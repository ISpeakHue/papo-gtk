# Implementation-plan steps 7–9 validation

Local validation on Linux of the working tree implementing the
[conversation switcher and media controls](../../../navigation-and-media.md).
GUI fixtures use disposable configuration, synthetic loopback backends and
private Xvfb/Openbox displays in the cached Fedora CI container with Cairo
rendering. They do not use a saved session, contact a live Papo server or modify
desktop preferences.

## Feature coverage

- `conversation-switcher`: local ranking and Unicode name filtering; keyboard
  selection/activation while keeping query focus; DM navigation; retained
  conversation drafts; restored focus; no-result handling; revoked-channel
  rejection; independent message-search action.
- `media-policies`: no initial image requests in on-click mode; separate
  downloading/processing states; unchanged allocated frame dimensions; pending
  and blocked moderation; targeted retry without reloading unrelated images;
  file download failure/success beside the file; failed original-video retry
  while retaining its poster; cache clear without immediate refilling; explicit
  loading after clear; reduced active-preview limits.
- `media-preferences`: saved device choices independent of motion; settings API
  payload excludes media preferences; cache usage refresh and clearing.
- Regular tests: switcher ranking; cancellation/admission policy; hard compressed
  cache bounds and oversized entries; backward-compatible configuration defaults;
  retention limits survive account reset; cache clearing preserves uploads.

Compatibility checks cover attachment presentation/animated GIF downloads,
original-image retry and viewer lifetime, backend GIFs, progressive loading,
cache reuse, channel caches, pinned media, settings, translations and the outbox.
The exact result list, source fingerprint and test-binary hash are in
[checks.json](checks.json).

Linux and Windows CI include the three new workflows. Native Windows execution,
remote CI and packaged desktop acceptance were not run for this implementation.
No commit, push, release or backend change is included.

## Native playback limitation

The broader `recent-media` fixture passed its initial recent-preview assertions
and two native fullscreen/latest/history-eviction cycles, then exited with
SIGSEGV. It is **not counted as a passing workflow**. Core metadata from the
synthetic process identifies GStreamer 1.28.8 and the same native stack seen in
the earlier aggregate validation:

```text
g_type_check_instance_is_fundamentally_a  (libgobject-2.0.so.0)
g_object_unref                          (libgobject-2.0.so.0)
gst_play_main                           (libgstplay-1.0.so.0)
```

This identifies the failing thread, not a complete root cause. No native
playback suppression or workaround was added. Independent feature/cache checks
continued; the aggregate GUI suite was not rerun. Logs and the extracted native
stack remain locally under `target/steps789-*`, excluded from Git.

Development checks also caught a mock endpoint that returned a video instead of
the intended download failure, and an assertion that inspected a replacement
widget before GTK allocated its frame. The fixture now injects a real HTTP
failure and waits for allocation before comparing dimensions. Its video case
also exposed and now guards against hiding original-download errors behind a
successfully loaded poster.
