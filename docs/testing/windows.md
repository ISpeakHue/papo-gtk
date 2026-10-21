# Windows acceptance matrix and release checklist

Defined on 2026-10-21 for step 1 of [the port plan](../../MULTIPLATFORM_PLAN.md).
The target policy is [WINDOWS_SUPPORT.md](../../WINDOWS_SUPPORT.md).
**No Windows desktop row has passed release acceptance yet.** This document
defines checks and records existing evidence; creating it runs none of the cases.

## How to record results

Use `PASS`, `FAIL`, `BLOCKED`, `NOT RUN` or `NOT APPLICABLE` for each execution.
`PASS-CI (historical)` below applies only to the named source revision, fixture,
runner and checks. `SOURCE ONLY` means shared code/tests exist without Windows
execution evidence. `UNAVAILABLE` identifies an implementation gap. Neither
status is a release pass. Explain every `NOT APPLICABLE`, such as a missing
physical camera; also test the no-device UI behavior.

Each result must identify the case ID, date, tester/job, source SHA, artifact
SHA-256, installer/ZIP, OS edition/version/full build and UBR, KB, native process
architecture, runtime/package manifest, GPU/driver, devices, input layout/IME,
display/scaling and backend revision. Record expected/observed behavior and a
redacted log or issue reference. Recheck changed behavior and its dependents
after fixes. Do not copy passes from a source-tree build to an installed package.

Use disposable accounts/backends and test credentials. Ordinary session tests
retain their memory store; any native credential test creates and removes only
its own disposable entries. Tests must isolate configuration on Windows as well
as Linux. Diagnostics must exclude passwords, cookies, tokens and private text.

## Existing evidence ledger

| ID | Evidence | Result and scope | Limits |
| --- | --- | --- | --- |
| E1 | [Native run for `ec8bf2b`](https://github.com/ISpeakHue/papo-gtk/actions/runs/39536236642), Windows job | PASS-CI (historical): all-target compilation, platform policy, unavailable-capture rejection, synthetic voice gain/startup/process cleanup, embedded icon rendering and chat/member startup. | `windows-2022` UCRT64 environment; not Windows 11 desktop, real-device, native-store or complete feature acceptance. Overall run later failed during packaging because Git was missing. |
| E2 | [Packaging run for `c911a15`](https://github.com/ISpeakHue/papo-gtk/actions/runs/39537643269) | PASS-CI (historical): optimized executable, platform-info CLI, developer ZIP generation and upload. Prior archive inspection checked ZIP integrity, AMD64 header, launcher, notices and source metadata. | Packaging-only run; native tests were skipped. External MSYS2 runtime required; source metadata reports checkout changes. |
| E3 | [Initial local results](../../MULTIPLATFORM.md#initial-verification-ec8bf2b) | Historical Linux evidence: 286 regular tests and focused synthetic/GUI checks. | No Windows feature pass. Linux container GUI failure and local GTK warning limitations remain recorded. |
| E4 | macOS job in the E1 run | PASS-CI (historical): compilation and test linking. | Future-port groundwork; no Windows or macOS desktop acceptance. |
| E5 | [Clean packaging run for `265457f`](https://github.com/ISpeakHue/papo-gtk/actions/runs/39615565000) | PASS-CI: 10 provenance/archive tests, clean-source optimized AMD64 build, retained 352 exact native package archives and build manifests. Local ZIP/hash inspection passed; [provenance record](build-provenance.md#verification-record-2026-10-21). | Packaging-only: native application tests skipped. Still an external-runtime preview; no Windows 11 desktop/standalone-runtime acceptance or byte-identical rebuild proof. |

No historical CI runner image revision, native package version or desktop UBR
should be invented from a moving runner label. Extract those details from the
original logs when available. Future runs retain them through the
[step 2 provenance and native-input manifests](build-provenance.md).

## Required Windows environments

All rows below are **NOT RUN**. Build numbers are target baselines from the
support policy, not observed machine records. Capture the exact servicing
revision actually used; qualify newer release-time revisions as that policy requires.

| Environment ID | Edition / native architecture | Baseline | Required coverage |
| --- | --- | --- | --- |
| W25H | Windows 11 25H2 Home / x86_64 | 26200.9711 | Clean install, package launch, core chat, notifications, input, media and upgrade/uninstall. |
| W25P | Windows 11 25H2 Pro / x86_64 | 26200.9711 | Same acceptance; full device/capture cases where hardware is present. |
| W26H | Windows 11 26H2 Home / x86_64 | 26300.9711 | Same acceptance on the newer release family. |
| W26P | Windows 11 26H2 Pro / x86_64 | 26300.9711 | Same acceptance; full device/capture cases where hardware is present. |
| HW1 / HW2 | Physical machines within the four target rows | Record exact OS and hardware | Across at least two GPU families/vendors, exercise real audio/camera, capture, acceleration/fallback and mixed-DPI displays. VM rendering alone does not qualify hardware behavior. |

Begin clean-machine runs with no MSYS2, Rust, Git, Python or GStreamer installed.
Launch as a standard user through Explorer/shortcuts and the installer/ZIP.
Include an installation path and Windows username with spaces/accents. Retest
with unrelated developer runtimes present to detect accidental dependency lookup.
Permission-denied/no-device cases must pass on every desktop row; use physical
hardware for successful capture/device evidence rather than skipping it everywhere.

## Feature acceptance matrix

Every manual Windows result is **NOT RUN**. The automated column describes
existing evidence only; guide links provide scenario details, not extra passes.
Each row is a required release case unless its device condition is explicitly
recorded. Run functional cases against the final package and a disposable real
backend as well as any available synthetic fixtures.

| Case | Feature and required behavior | Existing Windows automated evidence | Remaining gap / plan step |
| --- | --- | --- | --- |
| W01 | Launch standalone from installer and ZIP; resolve bundled DLLs, GI, schemas, plugins and helpers without development tools or global PATH edits. Missing components produce actionable errors. | E2 tests the developer build in MSYS2 only. | Self-contained runtime and clean-machine tests; 3, 4, 12, 13. |
| W02 | Server/account login, private-server password, registration, incorrect/expired credentials and network errors follow the backend contract without leaking secrets. | SOURCE ONLY; [account](account.md) and [workflow](workflows.md) fixtures. | Native packaged and real-backend execution; 5, 12. |
| W03 | Remember/restart/refresh/logout/revocation use Windows Credential Store; unavailable store fails safely without plaintext fallback or touching another session. | E1 compiles native access; ordinary tests use memory. | Disposable native-store acceptance; 5. |
| W04 | Settings and cache use native per-user paths; Unicode filenames, overwrite, locked files, cancelled saves and cleanup work under a standard user. | Platform path-policy coverage in E1, not complete filesystem acceptance. | Real Windows file operations; 5. |
| W05 | Channels and DMs support history, pagination, edits/deletes, replies, pins, reactions, drafts and unread state; denied destinations show no private content. | E1 covers synthetic chat/member startup only. | Full workflows and two-client permission checks; 11, 12. |
| W06 | Voice-channel text chat reads/sends while disconnected from audio, preserves drafts and obeys channel permissions. | SOURCE ONLY; existing `voice-chat` GUI case. | Native packaged/real-backend execution; 12. |
| W07 | Search/filter clearing restores expected order; navigating results, mentions and replies opens/highlights the correct message. | SOURCE ONLY; [chat](chat.md) and [workflow](workflows.md) checks. | Native UI and real-backend execution; 11, 12. |
| W08 | Markdown, fenced-code syntax colors, links, spoilers and multiline text render correctly with themes/text sizes. | SOURCE ONLY; syntax/Markdown GUI cases. | Native text rendering acceptance; 11, 12. |
| W09 | Unicode categories, custom/animated emojis, reactions and emoji detail popovers work; reduced-motion settings are respected. | E1 verifies control SVG icons, not emoji/animation behavior. | Native animation, selection and accessibility checks; 7, 11. |
| W10 | Attachments and clipboard previews support send, progress, failure/retry, removal and download without losing drafts. Moderation updates refresh media promptly. | SOURCE ONLY; draft/clipboard/backend-contract cases. | Packaged native dialogs, clipboard and upload acceptance; 5, 7. |
| W11 | Images and GIFs animate where appropriate, open with zoom/fullscreen and offer downloads through the intended viewer/context actions. | SOURCE ONLY; image/GIF GUI cases. | Packaged runtime, real GIF renditions and isolated video-to-GIF conversion; 6, 7. |
| W12 | Backend-supplied GIF embeds and webhook media render without scraping, duplicate players or mislabeled animations. External author avatars reuse batches/cache and role colors/grouping stay correct. | SOURCE ONLY; [backend](backend.md) and [media](media.md) cases. | Native provider-rendition/webhook checks; 7, 12. |
| W13 | MP4, supported MOV/QuickTime and WebM fixtures play, seek, enter/leave fullscreen and release players; supported audio fixtures decode. Unsupported/corrupt files fail recoverably. | E1 exercises voice audio, not inline media playback. | Packaged codecs, fullscreen and fallback testing; 7, 11. |
| W14 | Video posters, attachment previews and video-backed GIFs run in isolated helpers; denied network/private-file/input-write access and bounded output/time/memory are demonstrated. | UNAVAILABLE; E1 verifies unsupported-platform rejection. | Windows isolation backend and runtime acceptance; 6, 7. |
| W15 | In-app server/channel and DM/sender groups, counts, collapsed defaults, replies/mentions and read state remain correct across reconnects. | SOURCE ONLY; [notification](notifications.md) cases. | Full native inbox tests; 11, 12. |
| W16 | Native alerts honor enable/preview/sound/channel preferences; clicking active/minimized/cold-start alerts reaches the owning account/message, rechecks access and removes revoked destinations. | UNAVAILABLE; Windows desktop adapter is a placeholder. | Identity, activation and Windows notification backend; 4, 9. |
| W17 | Calls start muted, exchange audio through direct ICE and forced TURN relay, honor gain/mute and play join/leave cues through the selected output. | E1 passes synthetic worker/gain/cleanup, not actual call exchange or audible output. | Real SFU, audio output and hardware checks; 8. |
| W18 | Microphone/camera enumeration, default changes, hotplug, device switching, denial and no-device states recover; repeated calls and application exit release the process tree and devices. | E1 covers synthetic parent-worker termination only. | Real identifiers, device lifecycle and descendant cleanup; 8. |
| W19 | Local screen/window sharing requires explicit selection, handles cancellation/closure/resizing and stops all capture on leave/disconnect/exit. A second client receives the selected source. | UNAVAILABLE; capture is disabled and rejection is tested. | Consent-aware Windows capture backend; 8, 10. |
| W20 | Remote camera/screen viewing and repeated publisher changes work without requiring local screen-capture permission or leaking viewers. | SOURCE ONLY; shared video engine/probes. | Real peer streams through the packaged runtime; 7, 8, 10. |
| W21 | Reconnect, backend restart, access revocation, sleep/resume and offline failures retain appropriate drafts/state and terminate obsolete sessions/captures. | SOURCE ONLY; protocol/recovery fixtures. | Native network/session/device transitions; 8–12. |
| W22 | Profile, presence, preferences, moderation, roles, webhooks and administration obey actor permissions and preserve form state on failure. | SOURCE ONLY; existing [workflow](workflows.md) fixtures. | Native UI and disposable multi-account backend checks; 11, 12. |
| W23 | PT-BR dead keys produce `á é ç`; IME candidates commit before send; Shift+Enter, wrapping, copy/paste, selection and editing work in login, composer, edit and search fields. | SOURCE ONLY; Linux input tests are not Windows input evidence. | Actual Windows input methods/layouts; 11. |
| W24 | Icons/fonts remain crisp at 100/125/150/200% and across mixed-DPI monitors; focus, keyboard navigation, accessibility, resizing and fullscreen remain usable. | E1 passes resource icon lookup/rendering at two scales, not monitor/accessibility acceptance. | Physical display and native accessibility testing; 4, 11. |
| W25 | Repeated channel switches reuse media/avatar caches, opening batches remain bounded, scrolling stays responsive and closing media/calls returns helper counts to baseline. | E1 checks startup avatar/member behavior only. | Record warmed load timings, working-set and request/process counts for 10 channel round trips and 10 call cycles; investigate growth/redundant requests; 7, 8, 13. |
| W26 | User-facing package identity, shortcuts and diagnostics work; install/upgrade/uninstall preserve settings and credentials unless explicit removal is chosen. No routine action needs elevation. | E2 contains a command launcher; no installer acceptance. | Identity, installer, signing and upgrade testing; 4, 13, 14. |

Container validation, response limits, authenticated-origin restrictions and log
redaction from [the security guide](security.md) remain required in every package.
Exercise malformed media and isolation probes with synthetic fixtures. This
checklist does not claim all in-process playback codecs are sandboxed.

## Release gate

- [ ] The candidate has a fixed source SHA, accurate clean-source status,
  dependency manifest, version, checksums and corresponding source/notices.
- [ ] All four required desktop rows have full build/UBR records and packaged
  acceptance results. Required device success paths have physical-hardware evidence.
- [ ] W01–W26 pass with traceable evidence; device-specific exclusions are
  explained, no required feature is skipped everywhere, and no blocker remains.
- [ ] Windows desktop alerts, consent-based local capture and isolated converters
  are implemented and tested before their capability flags are enabled.
- [ ] Installer and ZIP run without external developer/runtime installations;
  native paths, identity, diagnostics and standard-user operation pass.
- [ ] Real-backend multi-client permissions, notifications, ICE/TURN, camera and
  screen publication/viewing pass; synthetic tests alone do not qualify them.
- [ ] The full Windows release workflow passes for the candidate code; a
  `packaging_only` run alone does not qualify it. Linux regressions and macOS
  compile/link checks have been reviewed and tracked failures remain visible.
- [ ] Install, upgrade, uninstall, signatures and final distributed artifacts are
  checked; input, DPI, device and performance blockers from beta are resolved.
- [ ] Published requirements match the qualified revisions and feature results;
  Windows 10, ARM64, 26H1 and other unqualified targets are not advertised.

## Result template

Copy this record into the release's evidence log for each executed case/environment:

```text
Case / environment ID:
Result (PASS / FAIL / BLOCKED / NOT RUN / NOT APPLICABLE):
Date / tester or CI job URL:
Source SHA / artifact name / SHA-256 / installer or ZIP:
OS edition / version / full build / UBR / installed KB / process architecture:
Runtime manifest / GPU and driver / audio-camera devices:
Display scales / keyboard layout and IME / backend revision:
Scenario and expected result:
Observed result and redacted evidence link:
Issue / retest reference / reason for any exclusion:
```
