# Build provenance and CI diagnostics

Step 2 of the [Windows plan](../../MULTIPLATFORM_PLAN.md) keeps source identity,
native inputs and test evidence separate. Windows remains a developer preview
requiring the external UCRT64 runtime. No byte-identical rebuild has been
established; matching source and retaining packages do not establish that result.

## Windows source diagnosis

The [historical-checkout diagnostic](https://github.com/ISpeakHue/papo-gtk/actions/runs/39614371031)
rechecked the original artifact commit, `c911a1519259dd14a9845cf9e13b9cb64d7aecb4`,
on `windows-2022` before running a compiler:

- Git for Windows 2.56.0 used its system `core.autocrlf=true` setting and saw a
  clean checkout. The local `core.filemode=false` setting applied to both Git implementations.
- MSYS2 Git did not inherit that Git for Windows system configuration. It reported
  305 modified text files (41,729 insertions and deletions), with LF in the index
  and CRLF in the working files. `git diff --summary` reported no mode changes;
  `git status --untracked-files=all` reported no generated/untracked source files.
- An actual `README.md` diff showed the same text replaced line-for-line.
  `git diff --ignore-space-at-eol --exit-code HEAD` succeeded. This identifies
  checkout line-ending differences, rather than a compiler-generated source edit.

The repository now declares text normalization in `.gitattributes`: LF for source,
CRLF when checking out the Windows `.cmd` launcher, and binary media excluded from
text conversion. It does not disable dirty detection or ignore source files.

The workflow records the checkout Git version, selected configuration, status,
mode summary and line endings before MSYS2 setup, then repeats the inspection with
MSYS2 Git. Packaging records the commit, status, diff statistics/summary, hashes of
tracked/staged diffs and untracked contents, and EOL information before compilation,
after compilation and after staging. Ignored `target/` and `dist/` outputs remain
outside the source identity. Unexpected source/index changes abort packaging.

## Building and retaining native inputs

From a consistent MSYS2 UCRT64 terminal with the packages from
[MULTIPLATFORM.md](../../MULTIPLATFORM.md):

```sh
python packaging/windows/native_inputs.py capture target/windows-native-inputs
python packaging/windows/build.py --preflight --native-inputs target/windows-native-inputs
python packaging/windows/build.py --native-inputs target/windows-native-inputs
```

Choose a fresh capture directory after updating packages; capture never merges
snapshots. Reuse an existing verified directory for another build of the same
environment. `--allow-dirty` explicitly permits a labeled local preview, while
still rejecting changes during compilation/staging. CI never uses that option.
`--packaging-only` records skipped native testing; it does not qualify a release.

`native_inputs.py` downloads the full installed MSYS2 package set (including base
MSYS tools and the native UCRT64 dependency closure), using Pacman's integrity and
signature validation. It queries the retained packages themselves and rejects
version drift against the installed inventory. It retains the MSYS/UCRT repository
databases and any downloaded signature files, and hashes every retained file in
`NATIVE-INPUTS.json`. The builder verifies that manifest, all hashes, file sizes
and the exact installed name/version set before building. Extra/missing files,
corruption and mismatched installed versions fail closed.

The source revision determines `Cargo.lock`; Cargo builds with `--locked`, an
explicit native GNU target and an application-owned output directory. The ZIP
contains `SOURCE.txt` and `BUILD-INFO.json`: source state, Cargo lock hash,
Rust/Cargo/GCC/GLib/pkg-config/Python versions and paths, native package versions
and hashes, command/options, selected build environment controls, runner image/OS
and job identity, executable SHA-256 and an explicit false reproducibility claim.
A separate ZIP checksum is uploaded. Environment capture uses an allowlist, not a
dump of credentials or CI variables. No Windows runtime bundling is claimed here.

CI uploads the preview, source/toolchain diagnostics and native package inputs as
separate artifacts. The dependency artifact requests 90-day retention, subject to
repository limits. A supported release must permanently retain those inputs with
its source and artifacts before CI retention expires (plan step 14); a link to an
expired Actions artifact is not a rebuild archive.

To restore a captured environment, download the entire native-input artifact,
verify it with `native_inputs.py verify <directory>`, then use a disposable MSYS2
installation and one `pacman -U` transaction over its package archives. Do not mix
individual old packages into a newer working installation. Verify `pacman -Q`
against the recorded inventory before building; the builder enforces equality.
Package hashes attest the retained inputs, not the bootstrap installation or
bit-for-bit equality of outputs. MSYS2 documents installation from package
archives in its [package-management guide](https://www.msys2.org/docs/package-management/).

## Linux decoder and GUI failures

The original [Linux failure](https://github.com/ISpeakHue/papo-gtk/actions/runs/39536236637)
ran as container root and reported a Glycin SVG loader exiting inside Bubblewrap.
The log lacked the loader's underlying stderr. Local root/capability comparisons
reproduced UID-map failure; a new hosted preflight also exposed a loopback
`RTM_NEWADDR: Operation not permitted` failure in the rootful Docker setup.
These are container/namespace setup failures, not evidence of a missing SVG asset.

CI now builds a Fedora 44 test image and executes it with rootless Podman,
`--userns=keep-id`, a nonzero user and **all host capabilities dropped**. Only the
checkout, step script and disposable test home are mounted; checkout credentials
are not persisted. The outer seccomp/AppArmor overrides permit nested namespace
setup. Proc subpaths are unmasked **inside the rootless container** so the media
helper can mount its own private `/proc`; other system-path masks remain. No
`--privileged`, added host capabilities, host network/PID namespace or host-wide
kernel-policy change is used. See the supported options in
[Podman's documentation](https://docs.podman.io/en/stable/markdown/podman-run.1.html).

The image explicitly installs Glycin loaders, librsvg, Bubblewrap, fonts, GI,
GStreamer (including explicit OpenH264/libav plugins), Xvfb and the input/device test dependencies. The early runtime probe
records versions, H.264 decoder factories, UID mapping and security state, exercises Bubblewrap including a private `/proc` and decodes
bundled SVG/PNG files with Glycin's own namespace/seccomp isolation intact. It fails
before GUI assertions when the runtime cannot provide those boundaries. Production
sandbox code and decoder settings are unchanged.

`gui-checks.sh` retains all 38 original GUI runs (37 named cases and the default
case). Each gets a private D-Bus/Xvfb session, a timeout, log and exit status. Every
case runs even if a preceding case fails, and **any** failed/timed-out case fails
the job. GTK diagnostics stay visible. Portal/native worker checks still run after
a GUI failure when the runtime preflight passed. CI retains `runtime.log`, `unit.log`,
`gui/results.tsv`, per-case logs and a job summary instead of presenting an early
partial GUI run as full validation.

Known baseline limitations remain in [desktop testing](desktop.md): the default
emoji-picker assertion and GTK/Adwaita height-measurement diagnostics must be
reproduced/fixed separately. Their continued visibility is intentional. A green
sandbox/icon preflight does not certify the entire GUI suite or real hardware.

## Focused checks

```sh
python3 -m unittest discover -s tests -p windows_packaging_test.py
bash -n packaging/ci/run-fedora.sh packaging/ci/gui-checks.sh
# In the configured Fedora environment, using synthetic data only:
python3 packaging/ci/runtime_probe.py
bash packaging/ci/gui-checks.sh symbolic-icons
```

The Git fixtures verify clean source, ignored build outputs, staged/tracked/
untracked edits, explicit dirty previews, changes during a build and independence
from Git's implicit EOL defaults. Archive tests reject corrupted, missing, extra,
unsafe or duplicate inputs and installed-version drift. Native Windows package
capture/build and Linux GUI results are recorded separately below when run.
