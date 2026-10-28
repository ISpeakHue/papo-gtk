# Windows 1.1.0 preview

The unsigned [Windows preview 1](https://github.com/ISpeakHue/papo-gtk/releases/tag/v1.1.0-preview.1)
is available for testing the Windows-first port:

- [Per-user installer (.exe)](https://github.com/ISpeakHue/papo-gtk/releases/download/v1.1.0-preview.1/Papo-1.1.0-windows-x86_64.exe).
- [Portable ZIP](https://github.com/ISpeakHue/papo-gtk/releases/download/v1.1.0-preview.1/Papo-1.1.0-windows-x86_64-dev.zip): extract the entire archive and open `Papo.exe`.
- [Checksums](https://github.com/ISpeakHue/papo-gtk/releases/download/v1.1.0-preview.1/SHA256SUMS), application source and modified GTK source accompany the downloads.

Both packages contain the application-owned runtime. A separate Rust, MSYS2,
Python or GStreamer installation is unnecessary. Dependency notices and licenses
are included. Uninstall preserves settings, caches and saved sessions.

## Verified build

The release-profile build comes from clean commit
`474f256554f55d9279916dd5701643ca45653127`, tagged `v1.1.0-preview.1`.
[Full workflow 40756265548](https://github.com/ISpeakHue/papo-gtk/actions/runs/40756265548)
passed Windows source tests and the complete automated candidate gate:

- Fresh-runner portable startup, relocation, storage, isolation and missing-component checks.
- Packaged media checks with Cairo and automatic renderer selection, plus desktop,
  notification, voice text chat and worker lifecycle checks.
- Per-user installation, replacement, stale-uninstaller handling, locked-file
  rejection, interrupted-commit recovery, uninstall and data preservation.
- Linux regressions and macOS compile/link foundation checks.

Local verification checked all 7,862 ZIP payload files, the runtime manifest,
installer identity and hashes, matching GTK source inputs and the passing gate.
GitHub's uploaded SHA-256 digests matched all nine local release assets before
publication. `WINDOWS-CI-RESULTS.json` records the source and tested package hashes.
Byte-identical rebuilding remains unproven.

The runtime includes the [upstream GTK Windows IME disposal fix](https://github.com/GNOME/gtk/commit/2a50bd4af856d9297f0c569cfc8ad871fcd056c7).
Its corresponding GTK source, MSYS2 recipe and patches are supplied in
`gtk-4.24.1-papo-ime-source.tar.gz`. NSIS input paths use native separators and
explicit UTF-8, with a real compiler regression test for accented paths.

## Remaining acceptance

This is an unsigned experimental prerelease. Windows may show an unknown-publisher
or reputation prompt. No signing certificate is available. Windows 11 Home/Pro,
physical audio/video devices, real keyboard/IME composition, mixed-DPI displays,
accessibility and real-backend beta acceptance remain pending. Native WGC screen
capture remains opt-in. Windows Server CI does not establish Windows 11 support.

The [support policy](../../WINDOWS_SUPPORT.md), [acceptance matrix](windows.md)
and [beta checklist](windows-beta.md) retain those boundaries. The remaining
plan checkboxes are unchanged. macOS has no downloadable bundle; Linux 1.0.2
remains the latest stable release.
