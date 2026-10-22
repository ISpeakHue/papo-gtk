# Windows credentials, configuration and files (step 5)

This is the implementation/acceptance contract for step 5 of
[the multiplatform plan](../../MULTIPLATFORM_PLAN.md). Windows remains a preview.

## Credential persistence

`platform::credentials` uses `keyring`'s Windows Credential Manager backend.
Production entries retain service `br.com.papo.gtk.session.v1`; the key combines
the canonical server URL and username. Windows target names use
`br.com.papo.gtk.session.v2:<SHA-256>` over a length-delimited service and the exact
UTF-8 account key. This preserves case-sensitive usernames/server paths in
Windows' case-insensitive target namespace and avoids its shorter username-field
limit. The displayed native username is simply `Papo`. Windows writes use a versioned
`PAPO-UTF8\0` prefix plus UTF-8 secret bytes. Earlier UTF-16 password entries remain
readable only when their payload's server/username match the requested account
exactly. Save/rotation writes the hashed target before removing the matching
legacy entry; logout/Remember opt-out removes matching old and new entries.
An old entry for another account differing only by case is never returned or
deleted. Malformed legacy data with unverifiable ownership is ignored and cannot
block deletion of a valid primary entry. Linux Secret Service and macOS
Keychain continue using their existing password representation.

UTF-8 leaves more of Windows' bounded blob allowance available for ASCII session
data than UTF-16 does. Oversized writes fail with a generic, secret-free error;
they never spill credentials to application files. See the pinned
[keyring 3.6.3 Windows backend](https://docs.rs/keyring/3.6.3/src/keyring/windows.rs.html)
and [Windows credential structure](https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw)
for the case-insensitive target, username limit and generic blob contract.

`session::Store` holds the storage namespace used by the existing save/resume/
refresh/logout logic. Ordinary `cargo test` builds **always** use memory, including
Windows and macOS. Only the explicit non-test Windows acceptance command creates
random `br.com.papo.gtk.session.v1.test.<UUID>` entries. There is no production
environment variable that redirects sessions into a mock store or plaintext file.

The explicit native probe starts separate Papo processes to verify:

1. Remember/save, exit, reopen, authenticated `whoami`, refresh and saved-cookie
   rotation, retaining the session owner across process boundaries.
2. Logout and another restart with no remembered session.
3. Revocation during `whoami` and during refresh, followed by another restart.
4. Transient server failure retaining the remembered session for retry.
5. Delayed logout/refresh preserving a newer login, late refresh after logout,
   and turning Remember off.
6. A scoped injected unavailable store propagating errors without persistence;
   a real native oversized-write error retaining the previous credential.
7. Legacy UTF-16 reads/migration, distinct account targets differing only by case,
   rejection of a colliding legacy lookup/deletion, long account keys, larger
   UTF-8 entries and deletion of the disposable entries.

The fixture binds only `127.0.0.1` and accepts fixed synthetic auth requests.
Child command lines contain a random probe UUID, loopback port and phase name;
they never contain credentials. Output reports assertions and counts, never
credential payloads, message text or existing saved accounts. A cleanup guard
deletes only the probe's own key. Neither the production service nor another
user's keyring is enumerated.

## Configuration and file behavior

- Windows configuration resolves through the OS known-folder API to
  `%APPDATA%\papo-gtk\config.toml`; cache lookup resolves to `%LOCALAPPDATA%`.
  Linux/macOS retain their native `dirs` locations. A missing config root fails
  saving instead of writing into the working directory.
- Configuration writes stage a private sibling file, sync/close it, then replace
  the destination. Failure leaves the previous file intact and removes the stage.
  Only nonsecret preferences and the last server/username are serialized.
- Download dialogs receive a sanitized suggested basename. Windows reserved
  devices (including superscript COM/LPT digits), invalid characters, alternate
  data-stream colons, trailing dots/spaces and component limits are handled.
  User-selected final Windows names are validated before saving. See Microsoft's
  [filename rules](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file).
- Downloads continue staging beside the selected destination before replacement.
  They preserve an existing file on a lock, error or cancellation. No HTTP,
  redirect, authorization or content-size protection is weakened.
- Temporary creation owns cleanup within the blocking open operation, covering
  cancellation before the open returns. Clipboard, download and cached-GIF
  staging share it. Clipboard files remain owned until the last draft/upload
  reference is dropped.
- All Windows manifests opt into long paths without changing machine policy.
  Native file acceptance checks Unicode paths exceeding 260 UTF-16 units. This
  does not promise every Windows shell/dialog accepts every extended path; see
  Microsoft's [long-path conditions](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation).

## Run explicitly in a disposable desktop

The acceptance command writes a random test credential, creates private temporary
files, replaces clipboard contents with a synthetic texture and briefly opens then
cancels a save dialog. Use a disposable account/desktop, never the developer's
active clipboard or existing Papo session:

```powershell
.\Papo.com --check-windows-storage
```

The command checks locked config/download files, Unicode/long paths, overwrite,
reserved names, cancellation during a staged copy, clipboard ownership and scratch
cleanup. It reads the native clipboard texture and verifies a real GTK save dialog
reports cancellation without creating a file. The Windows CI also runs the full
composer clipboard/upload regression against synthetic messages with isolated
configuration and memory-only credentials.

`packaging/windows/check-storage.ps1` extracts a checksum-verified clean-source ZIP
under a fresh accented/spaced path. It requires a standard account, captures OS
and package provenance, executes the command, verifies successful cleanup, removes
the extracted package and writes `storage.json` plus native stdout/stderr.

The [clean VM harness](../../packaging/windows/clean-vm/README.md) supports
`prepare.py --test storage`. It has no network adapter, development SDK, host
directory or guest agent. An already installed fixture can use updated read-only
test inputs and a fresh results disk, as documented for identity acceptance.
Read results from the host only after normal guest shutdown.

## Verification record (2026-10-22)

[Native CI run 39809350784](https://github.com/ISpeakHue/papo-gtk/actions/runs/39809350784)
passed Windows compilation with warnings denied, 24 packaging tests, 14 platform
tests, synthetic voice, icon/startup/composer-clipboard GUI checks, 10 session
tests and four upload-validation tests. The packaged native storage command and
independent ZIP-only runtime checks passed. macOS compiled all targets and linked
the test executable. The one ignored session test requires explicit opt-in; it is
not counted as a pass.

The same clean-source ZIP passed on offline Windows 11 Enterprise Evaluation
26H2 x64, build 26300.9457, under the standard `PapoTester` account. All 15 native
credential child phases, six synthetic loopback requests, 345-unit Unicode paths,
file/clipboard/dialog checks and cleanup passed. Existing user settings and
production credentials were untouched. The report records a nonfatal GTK warning
while enumerating a missing shell shortcut during the cancelled save dialog.

[Retained reports, provenance and checksums](evidence/windows-storage-2026-10-22/README.md)
identify source `23fd2b7e00b6966c299c365f2b3554c4b882012c` and the tested ZIP.
Linux all-target compilation and focused platform/session/upload/download checks
also passed. These results complete step 5's storage/files gate; they do not
qualify the Home/Pro release rows, real-backend flows or the remaining Windows
media, device and installer work.
