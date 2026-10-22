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

Windows desktop alerts, local screen sharing and isolated video conversion are
currently disabled. Synthetic checks do not establish real-device compatibility.
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

4. [ ] **Establish Windows application identity and native startup.**

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

5. [ ] **Validate credentials, configuration and Windows file operations.**

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

6. [ ] **Implement the Windows isolated media-helper backend.**

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

7. [ ] **Restore and validate all packaged media paths.**

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
