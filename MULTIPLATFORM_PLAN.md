# Windows port implementation plan

Prepared on 2026-10-21 for the `multiplatform` branch, starting from `39614bb`.

The objective is a supported Windows application that installs and runs without
MSYS2 or development tools, with the chat and media features available in the
Linux application. Keep one shared Rust/GTK frontend and retain the platform
boundaries needed for a future macOS port. Implement Windows integrations behind
those boundaries; a macOS bundle and native macOS integrations are later work.

Use the numbered steps below as the implementation order. Check off a step only
after its completion checks pass and the evidence is recorded. Each step should
produce a reviewable change. Keep new code comments in EN-US and PT-BR.

## Existing groundwork

The current state is described in [MULTIPLATFORM.md](MULTIPLATFORM.md):

- Shared GTK UI, protocol, history, caches and image/emoji processing.
- Target-specific Secret Service, Windows Credential Store and Apple Keychain selection.
- Portable Python worker startup, private IPC and Windows helper console suppression.
- Platform capabilities, runtime paths, capture and decoder-isolation boundaries.
- Windows compilation, synthetic voice lifecycle, icon and chat/member startup checks.
- macOS compilation and test linking, without a supported macOS application bundle.
- A portable Windows developer ZIP with an application-owned UCRT64 runtime (step 3).

Windows desktop alerts are implemented with runtime readiness checks. Local screen
sharing is an explicit preview while native desktop acceptance is deferred.
Isolated video conversion was enabled after step 6's security gate passed.
Synthetic checks do not establish real-device compatibility.
Step 2 resolves the original checkout metadata and container SVG-loader failures;
remaining GUI assertions are tracked in the provenance record.

## Ordered implementation steps

1. [x] **Define the Windows support and acceptance matrix.**

   Use Windows 11 x86_64 as the initial desktop validation baseline. Record the
   exact supported OS builds and minimum versions after checking the selected
   GTK, Python, GStreamer and native API requirements. Treat Windows 10 and ARM64
   as additional targets requiring their own validation, rather than implicit
   compatibility promises. Retain the current UCRT64 toolchain for this port.

   Create a feature checklist covering authentication, remembered sessions,
   channels/DMs, voice-channel text chat, notifications, Markdown, emojis,
   attachments, GIFs, video, calls, cameras and screen sharing. Distinguish
   automated evidence, manual evidence and remaining gaps.

   **Complete when:** the Windows release checklist and test matrix are written;
   the current preview is clearly distinguished from a supported release.

   **Completed 2026-10-21:** [support policy](WINDOWS_SUPPORT.md) and
   [acceptance matrix/release checklist](docs/testing/windows.md) define the exact
   desktop targets, dependency constraints, evidence categories and outstanding
   Windows validation. Desktop acceptance itself remains pending.

2. [x] **Make build provenance and CI failures explainable.**

   Update `packaging/windows/build.py` and `.github/workflows/windows.yml` to record
   the commit, toolchain, native package versions and build options. Diagnose the
   checkout changes reported by the first Windows artifact: inspect actual Git
   status/diffs, line-ending configuration, file modes and generated files before
   choosing a fix. Do not suppress the dirty flag without identifying its cause.
   Pin or archive the native dependency inputs needed to rebuild release artifacts.

   Repair the Linux container's glycin/Bubblewrap SVG-loader failure with a
   supported container/user configuration and its required dependencies. Preserve
   decoder isolation while fixing CI. Keep the known GTK measurement/test issues
   visible instead of treating a partial GUI run as complete validation.

   **Complete when:** a clean Windows checkout produces accurate source metadata,
   unexpected source changes fail release builds, and Linux CI failures have
   reproducible causes and tracked fixes. Record whether byte-identical rebuilds
   have actually been demonstrated; matching source alone does not prove that.

   **Completed 2026-10-21:** [provenance and CI verification](docs/testing/build-provenance.md)
   records the reproduced CRLF/configuration mismatch, clean-source Windows ZIP,
   toolchain/options and 352 retained native package archives. Packaging rejects
   unexpected source changes. Rootless Fedora restores isolated image decoding;
   explicit codec dependencies restore the regular media regression. Individual
   GUI failures remain visible and tracked. Byte-identical rebuilding is unproven.

3. [x] **Bundle a complete Windows runtime.**

   Extend `packaging/windows/` to collect the executable and its runtime dependency
   closure: GTK/libadwaita DLLs, GLib resources, schemas, icon/SVG loaders, Python
   standard library and GI bindings/typelibs, GStreamer plugins/scanner and codecs.
   Generate relocatable loader/plugin caches where required. Include dependency
   licenses, notices and a package/version manifest. Keep GPU drivers supplied by
   Windows or the hardware vendor.

   Make `src/platform/runtime.rs` resolve this application-owned runtime without
   assuming `C:\msys64` or searching for a system Python installation. Arrange DLL
   discovery before Windows loads the GTK executable, through a validated native
   bootstrap or suitable DLL placement; setting paths inside `main()` is too late
   for startup imports. Preserve explicit developer overrides.

   **Complete when:** the ZIP runs on a clean Windows VM with no MSYS2, Rust,
   Python or GStreamer installed. Repeat from directories containing spaces and
   accented characters, with a different working directory and without changing
   the machine-wide `PATH`.

   **Completed 2026-10-21:** The native bootstrap, package-owned runtime
   collector, recursive PE import audit, relocatable caches, dependency/license
   manifests and offline runtime probes are implemented. See
   [runtime packaging and verification](docs/testing/windows-runtime.md).
   Native assembly and separate-runner relocation/missing-component checks passed.
   The rebuilt ZIP also passed on an offline Windows 11 Enterprise Evaluation
   26H2 x64 VM, build 26300.9457, under a standard account without external
   development runtimes. Accented/spaced paths, relocation, unrelated working
   directories, missing-component rejection, unchanged files/machine PATH and
   normal application startup/shutdown passed. The
   [report, logs, screenshot and input hashes](docs/testing/evidence/windows-runtime-2026-10-21/README.md)
   record this runtime gate; the Home/Pro release matrix and later port steps
   remain pending.

4. [x] **Establish Windows application identity and native startup.**

   Add executable icon/version resources and an application manifest, with DPI
   behavior compatible with GTK. Provide a GUI launch path without a console,
   while preserving an intentional way to collect diagnostics and use CLI checks.
   Give native notifications, shortcuts, the taskbar and the installer a stable
   application identity across upgrades.

   Choose the installer format after a small notification/activation prototype
   proves its identity requirements with UCRT64. Define how an activation reaches
   the correct running account/window, and how a cold start handles it. Keep
   Windows startup code conditional; preserve the macOS runtime layout hooks.

   **Complete when:** Explorer, Start menu and taskbar show the correct identity,
   normal startup opens no console, and activation routing has a tested design
   before notification integration is built on it.

   **Validation (2026-10-22):** native Windows/macOS CI passed, followed by
   standard-user, offline Windows VM acceptance. Explorer, Start search, taskbar,
   executable resources, system DPI mode and console-free startup were checked.
   Real notification-history clicks reused the running process and started a new
   process when closed; account/session policy probes passed. Per-user registration
   was removed successfully. The protocol-only prototype selects **NSIS per-user**
   for step 13. See the [identity contract](docs/testing/windows-identity.md) and
   [report, screenshots and hashes](docs/testing/evidence/windows-identity-2026-10-22/README.md).
   Production alerts/trusted cold-start message navigation remain step 9; the
   Home/Pro matrix, mixed-DPI desktop qualification and installer remain later gates.

5. [x] **Validate credentials, configuration and Windows file operations.**

   Exercise `src/platform/credentials.rs`, `src/session.rs`, `src/config/mod.rs`
   and attachment saving on Windows. Verify native credential save/resume,
   rotation, logout and revocation, including an unavailable credential store.
   Use disposable test entries; retain memory storage for ordinary tests.

   Test native configuration/cache locations, Unicode/long paths, reserved Windows
   filenames, overwriting a download, locked files, cancelled saves, clipboard
   uploads and temporary-file cleanup. Fix Windows-specific behavior through
   adapters while preserving the existing HTTP, redirect and secret-redaction rules.

   **Complete when:** restart/remember/logout work with the Windows store, no
   plaintext credential fallback exists, and file tests pass under a standard
   user account without touching an existing user's settings or credentials.

   **Completed 2026-10-22:** native Windows CI and offline standard-user Windows
   11 VM acceptance passed. Disposable Credential Manager entries verified 15
   process phases covering remember/restart/rotation/logout/revocation and safe
   store failures. Exact account targets prevent Windows case collisions, and
   guarded migration preserves legacy entries belonging to another account.
   Native paths, atomic configuration replacement, 345-unit Unicode paths,
   reserved names, overwrite/locks, cancellation, clipboard and temporary cleanup
   passed. No plaintext credential fallback was added; ordinary tests retain
   memory storage. macOS compilation/test linking also passed. See the
   [storage/file contract](docs/testing/windows-storage.md) and
   [reports and input hashes](docs/testing/evidence/windows-storage-2026-10-22/README.md).
   Home/Pro qualification and real-backend acceptance remain later release gates.

6. [x] **Implement the Windows isolated media-helper backend.**

   Add a Windows adapter beneath `src/platform/sandbox/`. Prototype AppContainer
   or an equivalent enforceable Windows boundary with the bundled Python/GI/
   GStreamer runtime before integrating it. Grant only the runtime and input access
   needed for conversion, with read-only input and private writable scratch space.
   Deny network access and access to private application data and credentials.

   Use Job Objects for process-tree cleanup and resource limits alongside the
   isolation boundary. A Job Object alone does not establish filesystem/network
   isolation. Bound IPC/output, handle inheritance, execution time and memory;
   terminate the helper tree on cancellation or parent exit. Keep failures closed.
   The network/device permissions of the voice worker are a separate concern.

   **Complete when:** Windows equivalents of the Linux isolation tests prove
   denied private-file access, denied input modification, zero successful network
   access and bounded cancellation, while healthy poster/GIF conversion succeeds.
   Extend `--check-media-runtime` and enable `isolated_media_helpers` only then.
   See [AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)
   and [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

   **Completed 2026-10-22:** The Windows AppContainer/Job Objects adapter now
   backs the shared poster/GIF helpers and fails closed. Protected input, private
   scratch, restricted inherited handles, clean runtime environment, process-tree
   cleanup and output/time/memory limits passed native security probes alongside
   three poster and three animation conversions (MP4/MOV/WebM). The Windows
   capability was enabled after the gate passed; macOS remains disabled.
   [Full native CI](https://github.com/ISpeakHue/papo-gtk/actions/runs/39868216963)
   also passed ZIP-only runtime acceptance and macOS compile/link. The
   [implementation contract](docs/testing/windows-isolation.md) and
   [source/results evidence](docs/testing/evidence/windows-isolation-2026-10-22/README.md)
   record the exact scope. Linux conversion/isolation regression checks passed.
   A fresh-extraction standard-user Windows 11 VM run also passed all assertions,
   runtime integrity and cleanup. Cold runtime setup now has a separate trusted
   preparation phase; Windows callers honor the unchanged native decoder limits.
   Full media UI and Home/Pro release qualification remain step 7/later work.

7. [x] **Restore and validate all packaged media paths.**

   Integrate the isolated helper with posters, video-backed GIF animations and
   staged attachment previews. Check `src/media/`, `src/ui/chat/transfers.rs`
   and `src/ui/chat/draft_preview.rs`. Continue using backend-supplied GIF media;
   avoid reintroducing provider-page scraping or repeated failed downloads.

   Test images, animated custom emojis, GIFs, MP4/QuickTime MOV/WebM, audio,
   thumbnails, fullscreen, zoom and downloads using the packaged runtime. Preserve
   container validation and playlist rejection. Recheck batch loading, cache reuse,
   channel switching and bounded memory. Exercise software and available hardware
   rendering/decoding paths without forcing a driver on every machine.

   **Complete when:** the supported fixture matrix works from the standalone
   package, converted animations remain animated, and revisiting recent media
   reuses the cache without leaking players or helper processes.

   **Completed 2026-10-23:** [Packaged media integration](docs/testing/windows-media.md)
   passed the native fixture matrix: images/custom animations, backend GIFs,
   staged previews, MP4/MOV/WebM playback, audio decoding, posters, downloads,
   zoom and fullscreen. Windows conversions share two cancellation-safe slots;
   cached revisits and temporary/player cleanup are checked. Full native CI,
   ZIP-only acceptance, ten software/four automatic UI cases and macOS compile/link
   passed. An unchanged standard-user offline Windows 11 VM replay passed all ten
   media cases. [Evidence and original failures](docs/testing/evidence/windows-media-2026-10-23/README.md)
   preserve the PowerShell 5.1 correction and one intermittent emoji-pause assertion
   for follow-up. Focused Linux media checks passed; the broader tracked GUI
   failures remain open. Physical GPU/audio, real-backend and Home/Pro release
   qualification remain later gates; macOS converters remain disabled.

8. [ ] **Finish Windows audio, camera and call lifecycle support.**

   Validate `src/voice/engine.py`, `src/voice/mod.rs` and `src/media/sound.py` with
   real Windows devices. Check device identifiers, default-device changes,
   hotplug, permission denial, gain, mute, call cues and camera selection. Preserve
   the shared signaling and the microphone's initial muted state.

   Make the SFU/probe launchers portable where they assume `/tmp`, `/usr/bin/python3`
   or Linux process inspection. Test direct ICE and TURN relay, reconnects, repeated
   join/leave, device switching and closing the application during capture. Extend
   process-tree ownership if any native helper descendants survive teardown.

   **Complete when:** two clients exchange real audio and camera video through the
   backend, denial/unplug cases recover, and repeated calls release devices and
   processes. Keep synthetic CI checks and record real-hardware results separately.

   **Implementation completed 2026-10-27; Windows audio hardware checked
   2026-10-29, remaining acceptance incomplete:** endpoint identity, device monitoring/recovery, muted defaults,
   raw/MJPEG cameras, bounded IPC and whole-process-tree ownership are implemented.
   SFU launchers now use native interpreter/executable paths; synthetic lifecycle,
   cue and device tests are wired into Linux/Windows CI. Local direct-ICE/TURN
   audio/video probes passed. The checkbox remains open for the original hardware
   completion criteria. A Windows development machine is now available, and the
   2026-10-29 local native/SFU checks pass. The physical USB microphone passes
   20 capture/Opus/mute/gain/release cycles and local direct/TURN buffer transport;
   the user confirms audible native call cues. No camera or live test accounts
   are available; attended permission/unplug/default-device tests are deferred
   at the user's request. Loopback microphone filtering,
   default aliases and probe reset ordering were corrected during this review.
   See [implementation, verification and deferred checklist](docs/testing/windows-calls.md).

9. [ ] **Implement native Windows notifications and activation.**

   Replace `desktop_pending.rs` on Windows with an adapter supporting delivery,
   activation and removal. Prototype a desktop-compatible WinRT/notification API
   with the identity selected in step 4; verify GNU/UCRT compatibility and any
   additional deployment requirements before selecting a binding or helper.
   Preserve the shared inbox and user preview/sound preferences.

   Generalize `NoticeMsg::DesktopAction`, currently Linux-gated, to accept native
   Windows activation. Cover an active/minimized application and cold startup.
   Treat activation data as untrusted: route to the owning account, recheck channel
   access and discard destinations after logout or revocation. Do not put tokens
   or passwords into notification arguments.

   **Complete when:** notifications open the correct channel/message, read or
   revoked alerts are removed appropriately, multiple windows/accounts route
   correctly, and disabled notifications remain quiet. Enable
   `desktop_notifications` after integration tests pass. Microsoft's
   [notification quickstart](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-quickstart)
   is a reference for activation/registration requirements, not a requirement to
   replace GTK with WinUI.

   **Implementation completed 2026-10-27; native desktop acceptance pending:**
   the existing WinRT/protocol helper now delivers/removes message alerts. Bounded,
   DPAPI-protected account/session tickets support cold activation; navigation
   waits for current permissions and rechecks them after resolution. Read/disabled/
   revoked alerts and logout use serialized removal with durable retry state.
   Shared policy and GTK integration passed, so the Windows capability is enabled
   behind a helper/registration readiness check. GNU cross-compilation passed;
   native DPAPI/WinRT construction, disposable storage and focused Windows routing
   checks now pass locally. Visible delivery/click/removal acceptance remains
   pending under the user's automatic-only boundary. See the
   [implementation and deferred acceptance checklist](docs/testing/windows-notifications.md).

10. [ ] **Implement consent-based Windows screen sharing.**

    Add a Windows backend beneath `src/platform/capture/`, retaining the shared
    WebRTC engine. First prove picker/window ownership and the handoff of a
    consented capture source to the worker. Select an integration compatible with
    the packaged GStreamer version, using a narrow native bridge if necessary.
    Do not send COM pointers as JSON or substitute automatic desktop capture when
    a picker integration fails.

    Use an explicit screen/window selection flow, such as Windows Graphics Capture
    with its system picker. Handle cancellation, unsupported capture, selected
    window closure, resolution/DPI changes and stopping/restarting a stream. Bound
    frame queues and release capture objects on leave, disconnect or application exit.

    **Complete when:** a second client receives the selected source, cancellation
    sends no frames, and changing/closing/stopping sources leaves no capture active.
    Enable `screen_capture` after these checks; retain runtime support checks and
    remote-stream viewing independently. See Microsoft's
    [screen-capture documentation](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture).

    **Implementation completed 2026-10-28; Windows acceptance deferred:** a
    UCRT64-compatible native WGC bridge owns the system picker and its HWND in the
    media worker. The consented item stays in that process; bounded BGRA copies
    feed GStreamer appsrc and the existing VP8/WebRTC path. Cancellation, closure,
    resize, static-source keyframes and joined cleanup have synthetic coverage;
    late consent callbacks cannot revive a stopped call. Packaging includes the
    bridge and offline ABI checks. Linux direct/TURN SFU and portal regression
    checks passed. Windows remains opt-in with `PAPO_WINDOWS_CAPTURE_PREVIEW=1`;
    the advertised capability stays disabled until the original native source/
    peer/cleanup completion checks pass. Attended probes and the deferred release
    checklist are in [Windows screen sharing](docs/testing/windows-capture.md).
    Native Windows bridge ABI/ownership and synthetic appsrc/VP8 checks now pass;
    picker/source/peer desktop acceptance has not run. See the
    [2026-10-29 local automatic record](docs/testing/windows-local-2026-10-29.md).

11. [ ] **Complete Windows desktop and input acceptance.**

    Validate login, composer, edit dialogs, search, preferences and media viewers
    with keyboard-only use, accessibility tools, focus changes and native file
    dialogs. Test dead-key accents, PT-BR layouts, IME composition, paste,
    Shift+Enter, selection and shortcuts on real Windows input methods.

    Test crisp fonts/icons, resizing, fullscreen and mixed-monitor scaling at
    100%, 125%, 150% and 200%. Let GTK manage scaling instead of applying a global
    workaround. Keep shared shortcut helpers and preserve Command-based macOS
    groundwork when correcting Windows-specific input behavior.

    **Complete when:** the acceptance matrix has no blocking input, focus,
    clipping, scaling or fullscreen defects in the supported Windows environment.

    **Implementation and acceptance harness prepared 2026-10-28; native Windows
    acceptance deferred:** native draft navigation, modifier-safe completion,
    platform-primary clipboard/undo/viewer shortcuts, search IM/popover Escape
    routing and editable accessibility labels are updated. GTK now selects its
    own Windows DPI mode instead of a manifest fixing it to system DPI. The
    preferences save action stays outside the scroller. Disposable desktop
    fixtures, an ownership-checked Windows key helper and a packaged test driver
    are wired into the test tooling. Real PT-BR/IME, assistive technology, file
    dialog and mixed-monitor tests remain the original completion gate; this
    checkbox is not a claim that Windows desktop acceptance has run. See
    [desktop/input checks and remaining matrix](docs/testing/windows-desktop.md).

    **2026-10-29 local automatic checks:** focused Windows desktop fixtures pass;
    the native-key helper safely refused lost foreground ownership. Real input,
    accessibility and physical DPI checks remain pending. A broad GUI native
    failure remains under investigation; fixture portability corrections are
    recorded separately in the [local record](docs/testing/windows-local-2026-10-29.md).

12. [ ] **Turn CI into a packaged-release gate.**

    Expand `.github/workflows/windows.yml` to run the portable regular tests and
    focused notification, media, sandbox, capture and lifecycle checks. Keep X11
    probes explicitly Linux-only and distinguish skipped checks from passing ones.
    Run integration checks against staged/installed files as well as the source tree.

    Add a clean Windows job/VM that executes the package with development prefixes
    removed from its environment. Verify runtime discovery, capabilities, helper
    isolation and safe failure when a required bundled component is missing. Keep
    Linux regression checks and macOS compile/link checks. Restrict
    `packaging_only` to rebuilding already-tested code; it cannot qualify a release
    by itself.

    **Complete when:** a clean source revision passes the full release workflow,
    the produced package passes clean-machine checks, and each advertised feature
    has recorded automated and/or manual evidence.

    **Implementation prepared 2026-10-28; native full-run qualification pending:**
    the workflow now runs every regular Windows Rust test, requires separate
    ZIP-only runtime/media/desktop checks, reuses Linux regression and retains
    macOS compile/link. A final gate rejects skipped/failed jobs and incomplete
    or mismatched source/build/runtime/ZIP evidence. Native storage, AppContainer
    isolation and missing-component failures run without developer discovery
    paths. Packaging-only rebuilds require an authenticated successful full run
    of identical source and build inputs and cannot emit a candidate gate.
    The clean preview revision now passes the complete automated candidate
    workflow and all packaged checks. Native Windows 11 desktop/device acceptance
    remains deferred; the original completion checkbox stays open. See the
    [verified preview](docs/testing/windows-preview.md) and
    [packaged CI gate and verification](docs/testing/windows-release-gate.md).

    **2026-10-29 working-tree checks:** 138 Python checks (one named Linux-only
    skip), 310 regular Rust tests and three explicit native lifecycle tests pass.
    Packaged runtime checks exposed and now guard a native GStreamer Unicode-path
    defect; developer and CI provisioning retain its source correction. These
    local modified-source results are not a new clean-source CI candidate.

13. [ ] **Build the installer and complete a Windows beta.**

    Produce the installer selected in step 4 and a self-contained ZIP, with correct
    shortcuts, identity registration, upgrade handling and uninstall behavior.
    Prefer a standard-user installation. Keep credentials/settings separate from
    program files; make any user-data removal an explicit choice.

    Run a beta across different GPUs, audio devices, keyboard layouts and network
    conditions. Cover sleep/resume, permission changes, repeated channel/media
    navigation and interrupted upgrades. Collect opt-in, redacted diagnostics and
    compare startup, media/cache behavior and sustained memory with Linux.

    **Complete when:** install, upgrade and uninstall pass on clean machines,
    beta blockers are resolved, and the final package no longer requires MSYS2 or
    a terminal. Leave unfinished features explicitly unavailable until resolved.

    **Implementation prepared 2026-10-28; native beta acceptance pending:**
    per-user NSIS packaging now consumes the exact standalone ZIP, records
    installer/compiler/source hashes, stages versioned copies and journals
    identity/Installed Apps changes for recovery. Ownership-checked shortcuts,
    upgrades, downgrade refusal and inventory-based uninstall preserve settings,
    caches, saved credentials and unknown files. A fresh-runner installer job and
    source/artifact-bound gate cover replacement, stale uninstallers, locks,
    journal recovery and data preservation. The opt-in numeric diagnostics
    exporter and attended beta/Linux-comparison matrix are ready. Local payload,
    privacy and gate tests, real NSIS recipe compilation and script/workflow
    validation pass. Native installer lifecycle checks now pass on fresh Windows CI runners;
    attended Windows 11 Home/Pro and real-device beta acceptance remain pending.
    The original completion checkbox stays open. See
    [installer contract, beta matrix and evidence](docs/testing/windows-beta.md).

    **2026-10-29 local automatic checks:** all ten installer tests, including
    real NSIS compilation with Unicode payload paths, pass on Windows. The
    existing installation is preserved; current-source installer lifecycle and
    beta qualification still require a clean candidate and isolated test account.

14. [ ] **Qualify and publish the supported Windows release.**

    Prepare code signing using owner-provided credentials, verify signatures on
    the final executable/installer, and retest the distributed package. Document
    signing/reputation limitations accurately. Signing material stays outside
    source control and normal CI logs.

    Update the version and release documentation together. Provide the Windows
    installer/ZIP, checksums, dependency notices and corresponding source from the
    tested commit. Update `README.md`, `MULTIPLATFORM.md` and testing instructions
    with the actual supported Windows matrix. Publish through the project's
    release process when authorized; keep macOS labeled as groundwork until its
    own runtime and native integrations are validated.

    **Complete when:** every preceding release blocker is closed, the signed
    downloadable artifacts match the tested source, clean-machine acceptance
    passes, and Windows is documented as supported with explicit limits.

    **Release tooling prepared 2026-10-28; supported-release qualification pending:**
    source version 1.1.0 and draft notes are prepared. Explicit Windows-store
    signing, NSIS embedded-uninstaller signing, PE content invariants, refreshed
    runtime identities and final signed-file rechecks are implemented. The
    qualifier requires all prior acceptance steps, the complete native Home/Pro
    feature/device/beta matrix and exact source/artifact/signature evidence. The
    publisher stages source/notices/checksums, requires the tested remote tag,
    verifies fresh signatures and uploaded digests, then publishes from a draft
    only with an explicit flag. No signing material is committed or exported.
    The owner confirmed no certificate is available. A clean committed revision
    now passes the full automated workflow, and an unsigned
    [Windows preview](docs/testing/windows-preview.md) is published as
    `v1.1.0-preview.1`. Native acceptance and supported release qualification
    remain pending; no Windows support claim is made. See
    [signing, qualification and publication](docs/testing/windows-publication.md).

## Dependencies and milestone boundaries

Follow the numbered order for integration. After steps 1–5, the implementation
work for isolation/media (6–7), calls (8), notifications (9) and capture (10) can
proceed independently where their prerequisites are satisfied. Step 7 depends
on the isolation tests in step 6; capture depends on the call lifecycle in step 8.
Notification implementation depends on the identity/activation decision in step 4.

The first milestone is a clean standalone chat build after steps 1–5. The second
is Windows feature parity after steps 6–11. The final milestone is a validated,
installable release after steps 12–14. A developer artifact or successful compiler
check alone does not complete the Windows port.
