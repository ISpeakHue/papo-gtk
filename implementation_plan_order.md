Implementation status (2026-11-05):

Continuation handoff (2026-11-05): the remaining implementation/regression fixes
and final evidence are complete. The user explicitly allowed the final Windows
CI dependency-download error to be ignored and requested no further retry work.
It is recorded as incomplete Windows validation, not a pass. Full release/device
qualification remains pending in the checklist below.

- [x] Step 1: repeatable release baseline runner and three recorded Linux runs; [measurements](docs/testing/evidence/performance-2026-11-04/README.md).
- [x] Step 2: shared account identity, account-owned connection/settings, scoped drafts/caches and notification ownership; [architecture](docs/account-state.md).
- [x] Step 3: adjacent backend source reviewed, current/missing contracts documented and source-change checker added; [backend contracts](docs/backend/ux-contracts.md).
- [x] Step 4: embedded PT-BR/EN-US catalogs, system/default language preference, plurals, dates and translated accessible labels; [translations](docs/translations.md).
- [x] Step 5: account/conversation draft snapshots with debounce, restart recovery, draft markers and missing-file/deleted-reply handling; [draft recovery](docs/drafts-and-outbox.md).
- [x] Step 6: bounded sequential outbox, independent composer, pending/upload/failed rows, cancel/edit/manual retry and recovered-send confirmation. Automatic uncertain retries remain disabled because ordinary backend sends lack idempotency.
- [x] Step 7: local channel/DM switcher, unread/recent ranking, Ctrl+K navigation, Ctrl+F message search, keyboard selection and focus restoration.
- [x] Step 8: item-local download/processing/moderation/failure states, targeted image/video/file/original-viewer retries and stable loading frames.
- [x] Step 9: device-local automatic/on-click loading, independent low-memory retention, accounted cache usage and clearing without canceling active viewers/uploads/saves; [navigation and media controls](docs/navigation-and-media.md).
- [x] Step 10: bounded private disk thumbnails with account/revision separation, expiry, LRU limits, invalidation, clearing/logout and fresh-client request-reuse measurement.
- [x] Step 11: paged image/GIF, video and file browser, native viewers, previous/next, downloads and jump-to-message using existing backend searches and exact-hit lookups. Historical embeds without searchable text links remain outside the current API's coverage.
- [x] Step 12: local microphone meter/record-and-replay, output selection/tone test, token-scoped cleanup, transactional hardware changes preserving transport/mute, and safe failure/device-removal handling; [media and call setup](docs/media-browser-and-call-setup.md).

- [x] Step 13: device/account-local timed channel/DM mutes, account snooze, independent message/call sounds and explicit Busy/mention/DM precedence, preserving grouped inbox/server modes.
- [x] Step 14 implementation: shortcut reference, focus restoration, named controls, keyboard message actions, high-contrast text/role colors and reduced-motion wheel scrolling. Native Windows reader acceptance and real Orca/NVDA/Narrator speech checks remain pending.
- [x] Step 15: installed version, explicit anonymous bounded GitHub checks, stable/experimental selection, release notes, exact package/CPU matching and Flatpak guidance; [feature guide](docs/notifications-accessibility-and-updates.md).
- [x] Step 16: shared saved-account rail, independent live sessions and unread/call indicators, automatic remembered-session resume, restored conversation/drafts/viewport, and scoped disconnect/removal; [account guide](docs/multi-server.md).
- [x] Step 17 automation: source-bound baseline comparisons, structural/cache/request budgets, integrated account/reconnect/outbox/permission workflows, and artifact-bound acceptance tooling; [regression guide](docs/testing/release-acceptance.md).
- [ ] Step 17 full automated acceptance: the continuation passes 347 Rust tests, 185 Python tests (seven skips), nine final focused GTK cases and all three-run performance budgets. The aggregate/recent-media native playback SIGSEGV remains unresolved; the final evidence records the actual cross-platform CI result.
- [ ] Step 17 physical Linux device, native screen-reader and installed AppImage/Flatpak upgrade acceptance: receipts/checklists are ready; synthetic tests do not establish these results. The user waived manual Windows acceptance in favor of native Windows CI for this implementation. Windows remains experimental.

Initial steps 1–3 validation: 322 regular Rust tests and six Python tool checks passed; focused GTK
checks passed, including the notification rerun with a private window manager.
Linux/Windows CI now includes the new ownership check and one baseline run.
Native Windows execution of these changes remains pending; no remote CI run,
commit, push or release was requested for this implementation.

Steps 4–6 validation: 328 Rust tests and nine Python checks passed; 855 translation
entries passed validation. The new language, draft-recovery and outgoing-queue
GTK workflows passed, alongside focused compatibility checks. Linux/Windows CI
include the new cases. Broader GTK validation and its native GStreamer limitation
are recorded in [validation evidence](docs/testing/evidence/steps456-2026-11-04/README.md).
Native Windows execution remains pending; these changes are local.

Steps 7–9 validation: 333 regular Rust tests, nine Python tool checks and 13
focused GTK workflows passed; compilation is warning-free and 882 matching
translation entries passed validation. Linux/Windows CI include the new cases.
The broader native fullscreen playback stress test reproduced the previously
observed GStreamer crash and is recorded separately in the
[validation evidence](docs/testing/evidence/steps789-2026-11-05/README.md).
Native Windows and packaged desktop testing remain pending. These changes are
local. Those steps did not yet include a persistent disk thumbnail cache.

Steps 10–12 validation: 339 regular Rust tests, 27 native synthetic audio tests,
nine Python tool checks and 16 focused GTK workflows passed. The native voice
worker cleanup check passed; direct ICE/TURN checks verified live synthetic
microphone changes and subsequent decoded audio, mute and leave. Compilation is
warning-free, and 935 matching translation entries passed validation. A final
post-change performance baseline and isolated fresh-client disk comparison are
recorded in the [validation evidence](docs/testing/evidence/steps101112-2026-11-05/README.md).
The media browser uses the current backend API by user preference; no backend
endpoint or scraper was added. Native Windows, physical devices and packaged
acceptance remain pending. These changes are local.


Steps 16–17 validation is recorded in the [final evidence](docs/testing/evidence/steps1617-2026-11-05/README.md):
347 regular Rust tests, 185 Python tests (seven platform-dependent skips), 27
synthetic audio tests, three native worker cleanup checks, direct ICE/TURN calls
and three optimized performance runs. The interrupted remote run failed; the continuation recovered the empty evidence
bundle, repaired the identified GTK/test regressions, and triggered a fresh
source-matched Windows/Linux/macOS run. Its actual result and remaining native
playback/physical acceptance limitations are recorded there. These results do not
certify a stable release.

---

I’d implement these in the following order, with Linux and Windows checks included in each feature.


Steps 13–15 validation: 345 regular Rust tests, nine Python tool checks and 16
focused GTK/native accessibility cases passed; compilation is warning-free and
994 matching translation entries passed validation. Native Linux AT-SPI exposed
the composer, sender rows and Reply action. Windows execution and real
Orca/NVDA/Narrator speech acceptance remain pending. See the
[validation evidence](docs/testing/evidence/steps131415-2026-11-05/README.md).


1. **Establish performance baselines.**\
   Record startup time, channel-opening time, warm channel revisits, scrolling responsiveness, media requests and memory use during long sessions. Use repeatable scenarios to assess later changes.

2. **Prepare shared account and server state.**\
   Give settings, drafts, notifications and caches a consistent identity based on server and account. Separate connection state from the main window so future server switching can reuse the same architecture.

3. **Define the required backend changes.**\
   Confirm support for message idempotency, media-history queries and notification preferences. Document any missing endpoints before implementing features that depend on them.

4. **Introduce translation resources.**\
   Move interface strings into translation files, provide PT-BR and EN-US, and add a language preference that defaults to the system language. Include plural forms, dates and accessible labels.

5. **Persist unfinished drafts.**\
   Save text and reply references per conversation with a short debounce. Restore them after restarting, indicate channels containing drafts, and handle deleted reply targets or unavailable attachments.

6. **Add pending messages and an outgoing queue.**\
   Release the composer immediately after submitting a message. Display pending, uploading, sent and failed states on the message itself, with cancel and retry actions. Use backend idempotency before enabling automatic retries where delivery is uncertain.

7. **Implement the quick conversation switcher.**\
   Search channels and DMs by name, prioritizing recent and unread conversations. Assign `Ctrl+K` to switching and retain `Ctrl+F` for message search. Support keyboard navigation and restore focus after closing.

8. **Unify media loading and failure states.**\
   Distinguish downloading, processing, moderation and failure. Keep feedback beside the affected media, provide targeted retries, and preserve layout dimensions while loading.

9. **Add media and storage preferences.**\
   Provide automatic-loading, load-on-click and low-memory options. Show accounted cache usage and allow clearing cached media. Keep animation preferences separate from download preferences.

10. **Implement a bounded disk thumbnail cache.**\
    Reuse thumbnails across application launches, with size limits, expiry and account separation. Respect deletions and permission changes. Measure improvements against the baseline before choosing default limits.

11. **Build the conversation media browser.**\
    Add image, video and file views with incremental loading. Support previous/next navigation in the viewer and “Jump to message.” Use backend media-history queries to avoid downloading entire message histories.

12. **Improve call setup and device testing.**\
    Add a microphone-level meter, explicit microphone test, output-device selection and playback test. Then implement device switching during calls, with clear fallback behavior if switching fails. Test device removal and permission denial.

13. **Expand notification controls.**\
    Add timed channel muting, temporary notification snoozing and separate message/call sound preferences. Define how these interact with Busy status, mentions and DMs. Preserve the existing grouped notification window.

14. **Complete the accessibility and keyboard pass.**\
    Add a shortcuts window and audit focus order, focus restoration, accessible names and keyboard access to message actions. Test high contrast, large text, reduced motion and screen readers on Linux and Windows. Apply the same checks throughout earlier steps.

15. **Add release discovery and update guidance.**\
    Show the installed version and available updates, with release notes and the correct download for the installation format. Offer stable/experimental channel choices where relevant, and provide appropriate Flatpak instructions.

16. **Implement full multi-server switching.**\
    Add saved servers and accounts to the navigation rail, independent sessions and per-server unread indicators. Restore each server’s selected conversation, drafts and scroll position. Ensure a disconnected server does not interrupt others.

17. **Add regression budgets and complete release acceptance.**\
    Turn the initial performance scenarios into automated comparisons with tolerances for normal variation. Run integrated tests for reconnecting, switching servers, pending sends, cache eviction and permission changes. Finish with real desktop, audio-device and upgrade testing on Linux and Windows.

The first delivery milestone would be **steps 1–8**: measurable performance, translation groundwork, draft recovery, uninterrupted sending, faster navigation and clearer media feedback.
