# Windows isolated media helpers (step 6)

This contract covers [step 6](../../MULTIPLATFORM_PLAN.md), underneath the shared
poster and video-to-animation helpers. Voice workers are separate: they need
consented device/network access and do not run in this decoder sandbox. Inline
GTK playback is also a separate path; this does not claim all playback codecs
are isolated.

## Native boundary

`src/platform/sandbox/windows.rs` owns async cancellation and result buffers.
`windows.cpp` provides a narrow C ABI around native AppContainer, ACL and Job
Object APIs, compiled with the same UCRT64 C++ toolchain as the runtime. No shell,
PowerShell or externally installed sandbox executable is used during conversion.

Each invocation creates a random, per-process `Papo.Media.<PID>.<UUID>` profile.
The process starts suspended with its AppContainer SID and one application-owned
capability SID. That capability grants only read/execute access to the runtime;
it grants no network, device, credential or user-data capability. A shared
read/execute ACE on the runtime avoids adding thousands of per-job ACEs. It is
not inherited by application settings, caches or the user's home directory.
Windows system resources available to ordinary AppContainers remain governed by
Windows policy; this is not a virtual filesystem containing only three paths.

The source is copied, with a 256 MiB bound and cancellation checks, into a
separately protected read-only subtree. The original path and containing directory
are never granted. Profile-root child deletion is denied to the container so it
cannot replace that subtree. Scratch belongs only to this invocation's profile.
Different conversions have distinct AppContainer identities.

Python receives an explicit bundled `PYTHONHOME`, no user site, no current-directory
module search, no bytecode writes and a clean environment containing only runtime
and scratch settings plus the Windows system directory. Parent cookies, proxy
settings and desktop addresses are not inherited. Only stdin (EOF), stdout and
stderr pipe handles appear in `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`; private file,
socket and job handles are not inherited. Before resuming, the broker verifies
the token's AppContainer SID, capability SID/count and low integrity level, and
successfully assigns the process to its job. Setup failures never launch an
unrestricted fallback.

The private plugin directory contains only the software conversion subset:
core elements/typefind/playback/appsink, video conversion/rate, MP4/WebM demuxing,
libav/VPx, video/audio parsers, audio conversion, Opus and Vorbis. DLLs are copied
from the verified runtime into the read-only subtree. Plugin scanning stays in
the container process; the registry is private scratch. Hardware/device/network
plugin discovery is unnecessary for these silent conversions. A compromised
decoder's direct socket calls remain subject to AppContainer network policy.

## Ownership and limits

| Resource | Enforcement |
| --- | --- |
| Process tree | Non-inheritable Job Object, kill on last handle close; no breakaway. Root is assigned before resume; up to four processes. |
| Memory | 512 MiB committed memory per process; 768 MiB across the job. |
| CPU | 15 seconds of user CPU across the job. |
| Time | Internal 25-second wall deadline, including setup. Shared poster/animation callers keep their shorter 9/20-second deadlines. Cancellation signals a private event and terminates the tree. |
| IPC | stdout capped at 48 MiB + 4 KiB; stderr capped at 64 KiB while draining. Overflow terminates the job before adding the excess to Rust's result. |
| Input/script | Input copy capped at 256 MiB; script excludes NUL and is bounded so the Windows command line fits. Scripts are trusted application code. |
| Scratch | Private per-job profile; bounded lifetime, not a filesystem quota. No claim of a hard disk-byte limit. |

Normal completion, timeout, overflow, errors and cancellation terminate descendants
and remove the profile after handles close. Abrupt application death closes the
kernel job and kills descendants. A later invocation reaps only validated
Papo-owned profiles whose creator PID has exited; live or inaccessible processes
are left alone. PID reuse can delay removal, without granting a later decoder
access to that orphan. The explicit parent-exit probe checks tree termination and
subsequent reaping. This does not protect against a compromised Windows kernel,
an administrator or another unrestricted process running as the same user.

## Explicit acceptance

Run in a disposable desktop/account with the packaged runtime:

```powershell
.\Papo.com --check-media-runtime
```

The command uses synthetic MP4/MOV/WebM fixtures. It checks denied private-file
access and input modification, then requires valid posters and nonempty converted
animations. Windows adds native credential/handle/runtime-write denial, TCP4,
TCP6 and UDP loopback probes, channel overflow, committed-memory quota, internal
deadline, cancellation of a root and descendant, abrupt parent exit and cleanup.
The listeners must see no successful traffic. Credential acceptance creates and
deletes only one random synthetic entry and never enumerates real credentials.

The private `--windows-media-parent-probe` argument is only a synthetic process
lifecycle fixture; it does not start normal session/UI code. Ordinary tests do
not run the native credential probe. macOS continues to fail closed until a
separate isolation adapter is implemented. Linux retains Bubblewrap/Flatpak
isolation and the existing media validation rules.

Native CI executes the explicit check against the assembled package, clearing
development runtime overrides first. Native Windows VM acceptance and the
capability-enablement decision are recorded below when the full gate passes.

## References

- [AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)
- [Native AppContainer launch and capability access checks](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer)
- [Restricted handle inheritance](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute)
- [ACL propagation to existing child objects](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setnamedsecurityinfow)
- [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [Profile cleanup requirements](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-deleteappcontainerprofile)
