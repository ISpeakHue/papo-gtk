# Windows desktop and input — step 11

Prepared 2026-10-28. **Native Windows desktop acceptance remains deferred** until
a Windows machine is available for release testing, as requested. The code and
fixture tooling below do not substitute for that acceptance. Step 11 remains
unchecked until its real keyboard, accessibility, DPI and dialog checks pass.

## Changes and boundaries

- End/Page Up/Page Down retain their native editing behavior in the composer and
  other editable fields. History navigation uses those keys when the message pane
  owns focus. This avoids jumping to recent messages while positioning the caret.
- Enter/Tab completion requires no shortcut modifiers. Shift+Enter remains a
  newline, modified keys reach GTK, and preedit/candidate handling precedes send
  or completion. AltGr is not mistaken for a primary shortcut. The Windows
  composer inherits GTK's IM selection; the Wayland fallback stays Wayland-only.
- Shared primary-modifier helpers now cover image paste, rich composer undo/redo
  and viewer fit/modified-wheel zoom: Control on Windows/Linux and Command on
  macOS. Normal editable copy/paste and selection continue through GTK.
- Search Escape handling bubbles after native input methods and filter popovers,
  rather than closing the window before those controls can consume the key.
  Composer, message editor and search inputs have explicit accessible names.
- The Preferences save action is mounted outside the scrolling form, keeping it
  reachable in a narrow/short window. Call configuration has more default vertical
  space while keeping its existing scroller and fixed action area.
- `papo-gtk.exe` no longer declares a fixed Windows DPI-awareness mode. GDK chooses
  its mode before creating GTK windows; the bootstrap/shell helper retains its
  own manifest. This follows the [GDK Win32 implementation](https://github.com/GNOME/gtk/blob/main/gdk/win32/gdkdisplay-win32.c).
  No production `GDK_SCALE`, font-DPI override, per-monitor switch or driver
  workaround is introduced. The existing Windows GTK default is system DPI;
  removing the manifest does **not** prove fractional/mixed-monitor sharpness.

## Focused automatic fixtures

The `desktop-input` GTK case checks native End behavior, Shift+Enter, completion
modifiers, preedit routing, mention IDs, focus in a second editable and absence of
unintended message sends. Its data and backend are synthetic. Direct preedit
signals establish routing policy, not actual Windows IME candidate behavior.

```sh
python3 -m unittest discover -s tests -p desktop_input_test.py -v
python3 -m unittest discover -s tests -p windows_identity_test.py -v
PAPO_UI_CASE=desktop-input cargo test --locked ui_smoke -- --ignored --test-threads=1
```

On a disposable Windows desktop with the UCRT64 runtime selected:

```sh
PAPO_UI_CASE=desktop-input PAPO_NATIVE_INPUT_TEST=1 GTK_A11Y=test cargo test --locked ui_smoke -- --ignored --test-threads=1 --nocapture
```

The Windows helper uses [SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)
only for a unique, foreground window with the exact fixture title and the PID of
its invoking Rust test. It rejects missing opt-in, unrelated processes, lost
foreground ownership, held modifiers and unsupported key names. It balances key
press/release batches and attempts cleanup if Windows accepts only part of a
batch. It never changes keyboard layouts, injects Unicode strings or simulates
IME commits. Run this only in a disposable test session, with physical modifier
keys released. It is not a general desktop automation facility.

Linux uses XTest on a private X11 display. `PAPO_TEST_HELPERS_DIR` lets relocated
instrumented binaries locate the approved helpers without relying on the compile
machine's checkout path. The same fixture runs without native injection for
ordinary policy checks. macOS native key injection is not implemented.

The Windows workflow includes the new desktop case, login transition and message
editor cases. Full local/native CI results remain separate: editing the workflow
does not mean a Windows run has taken place.

## Packaged desktop test driver

During executable preparation on 2026-10-28, the Windows runner passed all 310
regular Rust tests but exposed an access violation while reopening the mention
editor with GTK 4.24.1. That GTK version leaves a native IME filter registered
when its context is disposed without a usable client surface. The corresponding
[upstream fix](https://github.com/GNOME/gtk/commit/2a50bd4af856d9297f0c569cfc8ad871fcd056c7)
is absent from the available UCRT64 4.24.1 package. Releasing focus before
widget teardown did not fix the native failure. The Windows workflow therefore
rebuilds the MSYS2 GTK recipe as `4.24.1-1.1` with that upstream patch and the
existing MSYS2 font-rendering patch. It installs the resulting package before
compiling or testing Papo. Native IME remains enabled; no private GTK ABI or
Compose-only replacement is used. The desktop CI step treats critical diagnostics
as failures. The patched runtime passed native desktop input, login, mention editing and
search in [run 40687585032](https://github.com/ISpeakHue/papo-gtk/actions/runs/40687585032),
with critical diagnostics fatal. That run then exposed an account-settings sizing
fixture failure; it did not produce an installer or pass the candidate gate.
The first resize correction (client content instead of window decoration) was
insufficient: run 40692230416 recorded a requested width of 390 but an unchanged
native width of 760. GTK Win32 preserves the last native allocation during its
queued layout. The shared resize fixture now reapplies a normal toplevel layout
through GTK’s public API on Windows, then asserts the actual allocated geometry.
The same helper covers the media/emoji/profile resize fixtures; production window
behavior and native input methods are unchanged. The subsequent
[full workflow 40756265548](https://github.com/ISpeakHue/papo-gtk/actions/runs/40756265548)
passed source and packaged desktop checks with these fixes. Attended native IME
acceptance remains pending; see the [published preview](windows-preview.md).

The recipe and patches live in `packaging/windows/gtk/`. Its upstream source tarball,
recipe, both patches and exact locally built package are retained in the
`Windows-UCRT64-native-inputs` artifact and hashed in `NATIVE-INPUTS.json` alongside
the official repository packages. The manifest identifies the locally built
package explicitly; it does not claim an MSYS2 signature for that package.
Subsequent builds may reuse the dependency from that retained run only after
verifying all archive hashes, the complete installed native package set, matching
recipe/patch hashes and the package identity. Unavailable or incompatible inputs
rebuild the recipe. Test results are never reused.

Step 7's instrumented UI artifact now includes `check-desktop-ui.ps1` and the
hashed Windows input helper. It reuses the verified portable ZIP and existing
`MEDIA-TESTS.json` source/runtime checks. Do not use an old test artifact with a
new package or substitute the release executable for the instrumented runner.

```powershell
.\check-desktop-ui.ps1 -Archive .\Papo-preview.zip -OutputDirectory .\desktop-results
.\check-desktop-ui.ps1 -Archive .\Papo-preview.zip -OutputDirectory .\desktop-native-results -NativeKeys
```

Use the actual ZIP filename and its adjacent `.sha256` file. The driver requires
matching clean source identities and hashes, uses an extracted runtime without
development paths, isolates settings, bounds execution, checks runtime immutability
and records per-case logs/results in `desktop-ui-auto.json` (or `-cairo.json`).
Native key injection is optional and limited to `desktop-input`; the other cases
use deterministic widget signals. Cases cover login transition, multiline
editing, mentions, search, preferences, embedded icons and media viewing.
Step 12 also runs inbox/notification and voice-channel text fixtures, plus native
synthetic voice/cue/process-tree checks, in a separate packaged-runtime CI job.
See the [packaged CI gate](windows-release-gate.md) for evidence requirements.

Automated desktop runs use GTK's **test accessibility backend**, which GTK
[recommends for test suites](https://docs.gtk.org/gtk4/running.html#gtk-a11y).
They do not exercise Narrator/AccessKit. For attended accessibility testing, launch
the ordinary package with its native backend, without `GTK_A11Y=test` or `none`.
If the native backend is missing or announcements are wrong, record a failure;
passing synthetic labels does not clear that release gate.

## Attended Windows acceptance matrix

Use a standard-user Windows 11 session, the extracted candidate package and
disposable accounts/content. Record OS edition/build/UBR, package and source
hashes, GTK/native package versions, GPU/driver, display resolutions/scales,
keyboard layout/IME version and assistive technology. Do not include credentials,
private text or real message screenshots in evidence.

| Area | Required checks | Status |
| --- | --- | --- |
| Login and view transitions | Keyboard-only field order; password visibility; PT-BR `á é ç ã ô à`; login → chat → dialog → chat focus; no forced Compose module on Win32 | NOT RUN on Windows |
| Composer and edit window | Dead-key accents; AltGr symbols; selection, Home/End and arrows; native copy/cut/paste/undo/redo; Shift+Enter; Tab completion; no sends/saves while committing candidates | NOT RUN on Windows |
| Real IME | Install/select an actual Windows IME, type candidates, confirm with Enter/Tab, cancel with Escape, change focus while preediting; exactly one commit and no accidental message action | NOT RUN |
| Search and popovers | Tab/Shift+Tab through filters; arrows/Enter to choose; Escape first dismisses dropdown/IME, later closes window; return focus correctly | NOT RUN on Windows |
| Preferences/profile/security | Keyboard-reachable tabs and switches, visible save/close actions, preserved drafts, unsaved-change prompt, large text/high-contrast readability | NOT RUN on Windows |
| Native file dialogs | Open/cancel attachment chooser, clipboard upload and Save As; Unicode/long paths; overwritten/locked files; modal ownership and focus return, no accidental send | NOT RUN |
| Media viewers | Keyboard open/zoom/fit/fullscreen/Escape; close while playing; focus return to chat; repeated opens/closes and resize without clipped controls | NOT RUN on Windows |
| Accessibility | Narrator and, where available, another Windows reader: field labels, password privacy, focus/selection state, tabs, completion, dialogs, errors and icon-only buttons; no keyboard trap | NOT RUN |
| Single-monitor scaling | At 100%, 125%, 150% and 200%: login/chat/dialog text and icons crisp, controls reachable at small sizes, no clipped content; both available normal renderer and software fallback | NOT RUN |
| Mixed monitors | Move all main/dialog/viewer windows between unequal scales while editing or fullscreen; correct sizing, readable sharp text, caret/candidate location and no off-screen windows | NOT RUN |

Do not treat a system-DPI bitmap-scaled window as a pass for mixed-monitor
sharpness. If GDK's current default fails this matrix, investigate the bundled
GTK/backend and its supported modes before advertising the port. Experimental
environment overrides or integer-only virtual-display results are not a release
qualification for fractional scaling.

For an isolated attended full-app accent check from the matching source/runtime:

```sh
cargo test --locked desktop_accent_check -- --ignored --test-threads=1 --nocapture
```

This opens a local mock-backed window, never a saved account. Type actual accents
in the reference entry and full chat composer, then close it. Its summary records
success indicators and module/backend metadata without typed text. Completing it
does not replace the other fields, real IME, screen-reader or DPI checks above.

## Local verification record

Local checks passed on 2026-10-28:

- Warning-free `cargo check --locked --offline --all-targets`; 308 regular Rust
  tests passed, six ignored.
- Two native-key structure/ownership-policy tests; 29 Windows packaging/resource
  tests completed with three native-only skips. The new GTK DPI manifest test
  passed; no native Windows DPI result is implied.
- Seven focused GTK cases passed at scale 1: desktop input, rich composer/editor
  behavior, actual X11 editor/login keyboard events, search, account settings and
  media viewer. Initial native-key runs failed because the host-built test binary
  referenced the host checkout inside the container; the explicit helper-directory
  override fixes that relocation issue.
- Preferences initially failed because Save was below the viewport; the fixed
  footer passes both normal/narrow layouts. Call controls initially failed their
  geometry assertion; the larger default window and settling the queued theme
  layout before measurement pass the focused case, including narrow controls.
- Desktop input and account settings passed at scale 2 on a private 2560×1920
  X11 display with GTK's test accessibility backend. This tests integer scaling
  only and does not establish physical Windows DPI or assistive-tool behavior.

Windows native
execution, packaged PowerShell execution, assistive technology, file dialogs,
physical fractional/mixed-monitor DPI and Windows dead-key/IME results are pending.
The PowerShell driver is prepared for later CI/release execution; no native pass
is claimed. The historical step 8 call-controls failure is retained in that
step's original record; the focused pass above applies to this changed layout.

## Local Windows automatic verification, 2026-10-29

Thirteen focused desktop/notification/voice-text cases now pass on the physical
Windows host; timeline, rich-width and backend-GIF regressions pass separately.
The broad GUI fixture remains an investigation, and GTK profile measurement
warnings remain visible. Correcting fixture paths and font-dependent test data
does not establish a fix for its intermittent native access violation.

An explicit native-key attempt stopped because the fixture did not own the
foreground window; no input was sent. Do not bypass this ownership check. Real
keyboard/IME, accessibility, dialogs and physical fractional/mixed-monitor DPI
remain the attended matrix above. The user requested automatic checks only.
See [commands, evidence scope and remaining release work](windows-local-2026-10-29.md).
