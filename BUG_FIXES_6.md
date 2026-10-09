# Bug report 6

Source: `bug_fixes_6.txt`. Changes preserve the existing request scheduler,
permission checks and channel/session generation guards.

| Item | Correction | Regression coverage |
| --- | --- | --- |
| 1 | A press in the main application closes open image viewers. Pressing the same thumbnail consumes that activation, so its subsequent click does not reopen the image. Letterbox clicks and Escape still close the viewer. | Mapped parent-press/thumbnail-click sequence, plus existing viewer and deletion tests. |
| 2 | Reaction selection searches both the installed GTK Unicode catalog and server emoji names in one grid. Unicode glyphs can also be searched directly. Cells are square, names are exposed through tooltips/accessibility labels, and results are paged in groups of 96. Catalog updates preserve active search text, caret and focus. | Unicode/custom search, square cells, search preservation during catalog updates and actual selection. |
| 3 | Videos start with sound after preparation. Explicit GTK play/seek/mute controls replace the native controls' conflicting mute/volume binding. Click toggles mute while preserving volume; pointer hover or keyboard focus reveals the volume slider. Unmuting zero volume restores the last audible level. | Decoded audio/video, post-preparation sound state, mute/unmute, hover slider and preserved/restored volume. |
| 4 | New message text requests its full native wrapped-line height as its allocated width changes, fixing the reproduced zero-height/clipped-text failure. Layout changes no longer mark the user as reading old messages, and the viewport redraws after applying its post-layout scroll position. | Three separate sends, including WS-before-HTTP delivery, verified through viewport bounds and rendered text pixels without additional input; complete wrapped text after narrow/wide resizing. |
| 5 | Notification rows show the author's display name and avatar. Missing authors use batched member lookup and the existing bounded avatar pipeline. No author/content is shown for inaccessible channels. | Notification name/avatar rendering and existing permission/privacy checks. |
| 6 | The composer emoji button opens the same compact searchable grid as reactions. Selection inserts at the saved caret; typing a colon continues to provide inline completion. Closing the chooser restores composer focus. | Composer custom-emoji insertion, common chooser search, close/selection focus restoration and existing typing checks. |
| 7 | Reply buttons use the same opacity as neighboring actions. The reply banner uses an accent-tinted surface and the author's role color; inline reply references also show the role color. | Actual reply action, role attributes, banner styling and unchanged row identity. |
| 8 | Channel switches retain a bounded session media cache. Image completions replace only changed media slots, retaining message rows, text and unchanged media. Eviction prefers offscreen entries; placeholders retain known image geometry. GIF animation memory remains bounded, with static first-frame fallback instead of an eviction/refetch loop. Video files can be reused while players stop on channel changes. | Media row identity, A→B→A texture reuse, actual >32-image eviction/reload, repeated video close/reopen, revocation/deletion cleanup and large-history regressions. |
| 9 | Search is visible in the chat header beside notifications and opens the existing message search workflow. Ctrl+K and Ctrl+F remain available. | Visible header action plus existing message search workflows. |

## Cache lifetime and limits

The session retains at most 32 thumbnail textures, 48 MiB of animated GIF frames,
128 preview metadata entries, 256 remembered image dimensions and four video
files totaling at most 128 MiB. Videos larger than that budget can play but are
not retained for reuse. Channel changes stop playback, close viewers, cancel
obsolete work and reset sensitive-content reveal consent. Permission revocation,
logout and component destruction clear the cache; moderation and deletion
invalidate affected media. Files are private temporary downloads, not a permanent
media library. Cache eviction still requires an eventual refetch.

## Verification

The final headless suite passed with 159 tests and three environment-dependent
tests ignored (3.46 seconds). The complete mapped GTK workflow passed in 122.60
seconds, including the report-6 regressions, notification and video controls,
cache eviction/reload and histories up to 5,000 messages. The application build
and `git diff --check` also passed. Existing compiler and desktop-service
warnings remain. Fixtures use a private GTK display and local HTTP servers;
no production messages or credentials are needed. Busy-server performance
still needs confirmation with the user's real server. To isolate the new
mapped chat controls while debugging:

```sh
PAPO_UI_CASE=report6 dbus-run-session -- xvfb-run -a \
  cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

Omit `PAPO_UI_CASE` for the complete GTK workflow, including notifications,
playback, permissions, scrolling and the earlier regression suites.
