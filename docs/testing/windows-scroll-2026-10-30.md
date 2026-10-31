# Windows scrolling and text rendering — 2026-10-30

Local automatic checks on the existing Windows 11 Home 23H2 machine. These
results supplement the [previous local record](windows-local-2026-10-29.md).
They do not complete the supported Windows release matrix or attended device,
keyboard/IME, DPI or two-client acceptance checks.

## Changes

- Read-only rich message text selects GTK's `simple` input context on Windows.
  The Win32 IME context otherwise connects every mapped message view to the
  frame clock's layout phase and calls `ImmSetCompositionWindow` when scrolling
  changes its position, even without input focus. Editable composers, login
  fields and editors retain their native input context.
- Windows selects Pango's fontconfig/FreeType backend before GTK initializes.
  The backend selection reaches both Rust's environment and GLib's native CRT
  environment; setting Rust's copy alone did not change the native font map in
  the development executable. An explicit backend override remains available.
- Windows typography keeps the system family, uses an 11pt minimum for point
  sizes, and enables antialiasing, slight hinting and hinted metrics. Larger
  configured sizes, absolute pixel sizes, monitor DPI and message-size
  preferences are preserved. This host previously selected Segoe UI 9 with full
  hinting. GTK's renderer is unchanged.

## Scrolling measurement

`PAPO_UI_CASE=scroll-frames` maps a 1,000-message local chat, materializes the
existing bounded 200-row window, and injects one wheel step every 55ms for three
seconds. Frame work is wall time from `before-paint` to `after-paint`, including
layout. It is not a measurement of compositor presentation or physical input
latency. Timing is diagnostic; regression assertions cover intermediate motion,
direction, row reuse, bounded ownership, read-only input contexts and native
input eligibility for the composer.

| Measurement | Before | After |
| --- | ---: | ---: |
| Median frame work | 42.3ms | 7.9ms |
| 95th percentile | 46.8ms | 9.7ms |
| Frames observed | 69 | 237 |
| Frames above 16.67ms | 63 | 4 |

The after result retains Cairo and the existing wheel distance/easing. Skipping
offscreen snapshots alone did not improve the delay; that experiment was removed.
The native IME fix reduced layout phase work from roughly 38ms per moving frame
to tens of microseconds in this fixture. Font-backend assertions verify
`PangoCairoFcFontMap`; Portuguese and emoji samples have no missing glyphs, and a
larger configured font survives setup. Opaque local text samples were inspected.

Evidence is under `target/windows-dev/scroll/`: `baseline`, `phases-fontconfig`
and `readonly-ime` hold the isolated measurements. Updated regression and package
results are recorded separately below.

## Validation

- Regular Rust tests: 310 passed, six explicitly ignored display/native tests.
- ZIP-only runtime checks passed for scrolling/typography, multiline editing,
  desktop input policy and login input, with both explicit Cairo and automatic
  renderer selection. Both selected `GskCairoRenderer` on this host. These use
  synthetic GTK events; they do not claim physical keyboard or IME acceptance.
- The broad GUI replay exposed a font-dependent wrapping fixture: its sentence
  exceeded a 520px card at 11pt. The shorter fixture still verifies fitting on
  a wide card and wrapping after narrowing, and its focused replay passes.
- Windows CI now runs `PAPO_UI_CASE=scroll-frames` with the existing GUI checks.
- Existing profile-height warnings remain open in `FOLLOW_UPS.md`. The new
  font backend produced no Pango fallback warnings in the first broad replay.

The full replay still reproduces the previously tracked native `0xC0000005`
failure after the action fixture, during timeline history (76.04s). Its outcome
is in `target/windows-dev/scroll/regressions-final/ui-cases.json`. The earlier
replay reached rich-card checks; neither run qualifies as a full-suite pass.
Keep this separate from the now-passing scrolling and typography regressions;
the scrolling fix does not establish a fix for that native failure.

The portable preview is rebuilt locally; no installed Papo registration or
desktop setting is changed. Exact archive and runtime hashes are retained in
`target/windows-dev/scroll/portable/portable-runtime.json`, and ZIP-runtime GUI
outcomes in `target/windows-dev/scroll/bundled-ui-final/results.json`.
