# Steps 13–15 validation — 2026-11-05

The [machine-readable record](checks.json) identifies the source and test binary.
Changes remain local; no repository push, publication or remote CI run was needed.

- `cargo check --locked --all-targets`: warning-free.
- Release Rust suite: **345 passed, zero failed, six ignored**.
- Python implementation-tool tests: nine passed; PT-BR/EN-US catalogs: 994 matching
  entries, checked placeholders/static references.
- Actionlint, GUI shell syntax and `git diff --check`: passed.
- Reviewed backend source still matches `eab626662bc4bfeb4ce1cd3165b01eccb4c2c5a5`.

Fifteen focused GTK cases passed on private Fedora 44/Xvfb/Openbox displays:
`notification-controls`, `release-discovery`, `accessibility`,
`accessibility-contrast`, `account-settings`, `conversation-switcher`,
`notification-filters`, `notification-window`, `voice-controls`,
`media-preferences`, `syntax-highlighting`, `webhook-chat`, `style-palette`,
`window-scrolling`, `desktop-input`. A separate native Linux **AT-SPI** case passed,
checking the named multiline editable composer, message sender rows and Reply
control through a private bus. It reads only the synthetic fixture process.

The native probe caught an older line overriding the composer name and hover
controls disappearing from the accessibility tree. The final implementation
keeps action widgets mapped, changes their visual opacity on hover/focus, and
defers focus-driven hiding until popup state settles. The shortcut fixture also
caught Adwaita rows being placed outside a list; the final shortcut list and sound
preferences use proper row containers. All four new portable GUI cases passed
again after these corrections.

The full GUI/native set ran before a final one-line change that formats mute
expiry with the existing app-language date/time pattern instead of a fixed
locale pattern. The record retains that binary/source fingerprint. The final
binary passed the regular Rust suite again and the focused `notification-controls`
workflow; the all-targets check is also from the final source.

Notifications cover timed expiration, persistence/restart, account separation,
unchanged server rules/inbox read state, independent sounds and consumed pending
alerts. Release tests cover semantic ordering, stable/prerelease/draft handling,
trusted repository URLs, exact package/CPU matching, anonymous loopback requests,
403 errors, redirects, malformed/oversized responses and stale GUI results.
The release window's GUI fixture never contacts GitHub. Public metadata was read
separately to confirm asset naming against the actual repository.

Native Windows execution is still pending; its CI list includes the new portable
cases. The GTK test accessibility backend and Linux AT-SPI probe do **not** prove
Orca/NVDA/Narrator speech behavior or full UIA integration. The cross-platform
[manual acceptance checklist](../../../notifications-accessibility-and-updates.md)
remains necessary. Physical display scaling, audio hardware and installation
upgrade acceptance were not part of this run. The aggregate media/fullscreen
suite was not repeated; its previously recorded native playback limitation
remains outside this change's focused validation.
