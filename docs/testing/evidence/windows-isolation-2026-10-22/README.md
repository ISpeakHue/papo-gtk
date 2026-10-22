# Step 6 Windows media isolation acceptance

Native code gate: **PASS-CI**, 2026-10-22. See the
[implementation contract](../../windows-isolation.md) and
[step 6](../../../../MULTIPLATFORM_PLAN.md).

## Source and CI

- [Full native CI](https://github.com/ISpeakHue/papo-gtk/actions/runs/39838842551)
  tested `994a9d517f48883172cb332773a8be67d47cbaff`: Windows all-target compilation
  with warnings treated as errors, 24 packaging tests, 13 platform tests,
  synthetic voice, icon/startup/composer-clipboard GUI cases, 10 session tests
  (one opt-in test ignored), four upload tests, packaged storage and media
  isolation, independent ZIP-only runtime acceptance, and macOS compile/link.
  [Conclusions](CODE-CI-RESULT.json), [selected logs](native-ci-excerpt.txt) and
  the packaged [media report](ci-media.stdout.txt) retain the results.
- The [package run](https://github.com/ISpeakHue/papo-gtk/actions/runs/39864866170)
  uses a later documentation-only source commit to correct the preview README.
  It repeats packaging, storage, isolation and independent ZIP-only acceptance;
  it does not replace the full native code run. macOS/native application unit
  tests are intentionally skipped in this packaging-only run.
- [Source comparison](SOURCE-COMPARISON.json) verifies 190 current native
  application/package inputs byte for byte against both revisions, including
  the C++ isolation bridge. Documentation changes are outside that comparison.
  This is source equivalence, not a claim of byte-identical builds.

## Boundary and assertions

Every decoder starts suspended with a distinct AppContainer identity, one
runtime-read capability, a verified low-integrity token and restricted inherited
handles. Input is sealed outside its writable profile. A Job Object enforces
process-tree lifetime and resource quotas; it does not supply file/network
isolation by itself. There is no unrestricted conversion fallback.

| Check | Native result |
| --- | --- |
| Private files and original source | Reads denied; only the protected staged copy is granted. |
| Input and runtime mutation | Writes, attribute changes and staged-directory replacement denied. Runtime manifest contents remain unchanged. |
| Native credentials | One random synthetic entry cannot be read; no existing credentials enumerated. |
| Inherited handles | Private inheritable fixture handle unavailable. Only exact missing/wrong-handle native statuses after readiness count as denial; unrelated crashes fail. |
| Network | No TCP4/TCP6/UDP loopback listener receives traffic. No network capability granted. |
| IPC and memory | Oversized stdout/stderr rejected; a 600 MiB allocation fails under the 512 MiB process quota. |
| Deadlines/cancellation | Internal deadline terminates work; cancellation kills ready root/child and removes the profile/input. |
| Abrupt parent exit | Job close kills root/child; next invocation reaps the owned orphan profile/input. |
| Healthy conversions | MP4, QuickTime MOV and WebM produce three valid posters and three nonempty animations. |

## Scope

This completes the decoder-backend gate only. Full packaged inline/viewer media
UX, Home/Pro qualification, physical GPU/device paths, real-backend integration,
notifications, screen capture and installers remain later steps. Voice workers
need separate network/device permissions. Inline playback is not claimed to be
sandboxed. macOS remains compile/link groundwork with its converter capability
disabled. Scratch has a bounded lifetime, not a hard disk-space quota.

`SHA256SUMS` covers retained evidence bytes. Binaries, ISO, VM/TPM disks and
fixture passwords are excluded.
