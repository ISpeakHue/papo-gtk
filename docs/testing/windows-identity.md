# Windows identity and native startup (step 4)

Implementation and acceptance for step 4 of [the port plan](../../MULTIPLATFORM_PLAN.md).
Windows remains an experimental preview. This work establishes identity and an
activation prototype. The [step 9 adapter](windows-notifications.md) now implements
production message delivery and durable tickets; its Windows desktop acceptance
is recorded separately from the historical step 4 evidence.

## Stable identity and executable resources

[data/windows/identity.json](../../data/windows/identity.json) is the shell identity
source. The AppUserModelID is `br.com.papo.gtk`, matching GTK's existing application
ID. The protocol is `papo-notification`; its toast stub CLSID is
`{DBE5A52E-3D00-553D-94AB-21403BD46288}`. These identifiers are independent of the
version and installation directory, including upgrades and package relocation.

`resources.py` builds icon, VERSIONINFO and application-manifest resources for the
native launchers, GTK executable and shell adapter. The ICO contains the existing
128/256 px Papo artwork, with no external theme dependency. Version fields follow
Cargo.toml. All executables request `asInvoker`; no routine launch requires UAC.

The step 4 manifest deliberately used **system DPI awareness**, matching GDK's default
Win32 behavior. Step 11 removes the fixed DPI declaration from `papo-gtk.exe` so
GDK selects its own mode; the bootstrap/shell retain their native manifest.
Neither policy forces GDK's opt-in per-monitor mode or a global scale override.
Physical fractional/mixed-monitor acceptance is still pending; see
[desktop testing](windows-desktop.md) and
[Microsoft's DPI manifest guidance](https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process).

## GUI and diagnostics

- `Papo.exe` and the actual `runtime/bin/papo-gtk.exe` use the Windows GUI subsystem.
  Normal startup creates no console.
- `Papo.com` uses the console subsystem, retaining synchronous CLI results and
  redirected stdout/stderr. `Papo.cmd` forwards there for compatibility.
- `Papo.com --diagnostics` starts the application with intentional terminal
  diagnostics. Existing secret-safe tracing filters remain in effect.
- `Papo.com --startup-info` reports the running process ID, console state and
  bounded activation counters; it reads no credentials or message content.
- Runtime commands such as `Papo.com --check-packaged-runtime` remain available.

The native bootstrap establishes runtime discovery before loading GTK and shows
an actionable GUI error if the runtime is missing. Windows startup sets the
explicit AppUserModelID before GTK creates a window. A native instance pipe
handles activation; Windows no longer relies on session D-Bus for uniqueness.
Linux/macOS application startup and macOS runtime-layout hooks are unchanged.

## Desktop registration and installer requirements

ZIP registration is explicit and optional:

```powershell
.\Papo.com --register-desktop
.\Papo.com --desktop-info
.\Papo.com --unregister-desktop
```

Registration creates the current user's Start menu `Papo.lnk`, with the matching
AppUserModelID, icon, GUI target and `System.AppUserModel.ToastActivatorCLSID`.
Only Papo's HKCU protocol/AppUserModelID/stub-CLSID keys are written. Ownership
markers prevent overwriting another application's registration. Re-registering
updates an owned installation path; an older copy cannot unregister an identity
now owned by a newer location. Unregistration removes only Papo's matching entries
and shortcut. No machine PATH, HKLM settings or credentials are changed.

The selected installer format is **NSIS, per-user**, with `RequestExecutionLevel user`
and `%LOCALAPPDATA%\Programs\Papo` as the default destination. Step 13 implements
the [versioned installer and recovery contract](windows-beta.md); native installer
and beta qualification remain pending. Step 4's standard-user Windows prototype verified the registration,
notification-history and warm/cold activation contract without an MSIX identity
or COM server. The installer should invoke this same registration adapter after copying
files, and invoke ownership-checked unregistration before removing them. It must
handle rollback, upgrades and user data separately. NSIS's
[per-user example](https://github.com/NSIS-Dev/nsis/blob/master/Examples/install-per-user.nsi)
and [execution-level documentation](https://nsis.sourceforge.io/Reference/RequestExecutionLevel)
cover its non-administrative installation model.

The notification prototype uses the documented unpackaged **protocol-only/stub
CLSID** option, without the Windows App SDK or a COM activation server. It supports
click actions and notification history; it does not support inline text input or
in-process COM callbacks. Those are not current Papo notification requirements.
See [Microsoft's desktop activation options](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/toast-desktop-apps)
and [shortcut identity requirements](https://learn.microsoft.com/en-us/windows/win32/shell/enable-desktop-toast-with-appusermodelid).

## Activation and account boundaries

The OS launches `Papo.exe --activate papo-notification://open/<UUID>`. Only a
canonical, nonempty UUID is accepted: no account, destination URL, message text,
credentials, query string or arbitrary command is accepted from the shell.

A named pipe is scoped by Windows user SID and desktop session. Its protected
DACL grants only that user access; remote clients are rejected. First-instance
creation is exclusive. Messages/replies are capped at 512 bytes, the queue at 32
entries and connection work at two seconds. A second process forwards activation
and exits; it does not create another GTK window. Foreground permission is handed
to the existing process where Windows permits it. GTK consumes events on its main
thread, and the owning process releases the pipe on exit.

Step 4's `activation::Router` prototype separates untrusted tickets from local,
account/session-bound destinations, with a 64-entry memory limit. Its offline
policy probe is retained below. Production notifications now use step 9's
[durable actor and DPAPI registry](windows-notifications.md), with a 128-entry
limit, cold restoration after authentication, permission rechecks and revocation.
Unknown tickets still only present the current window/login. No ticket logs in
a user or switches accounts. `Papo.com --check-notifications` exercises the new
packaged WinRT/storage path without visible alerts; the older policy probe remains:

```powershell
.\Papo.com --check-activation-policy
```

## Notification prototype and validation

The shell adapter is a small statically linked UCRT64 C++ program using system
COM/WinRT interfaces. A narrow ABI prefix avoids MinGW's unrelated
`IReference<boolean>`/`BYTE` template conflict. Native tests load the real XML and
toast factories and set tag/group metadata without desktop registration.

Explicit prototype commands use synthetic content only:

```powershell
.\Papo.com --test-toast 70b0d588-40ac-4314-86bf-130ad1c9b2ee
.\Papo.com --test-toast-info
.\Papo.com --clear-test-toast
```

`check-identity.ps1` is an explicit integration test for a disposable standard-user
Windows desktop. It verifies PE subsystems, icons, versions, manifests, effective
DPI mode, registration and normal GUI startup. It checks Start menu/protocol
activation, toast history, real UI clicks with the app running and closed, process
reuse and console state, and captures native shell visuals. `-ExternalClicks`
waits for an operator's real mouse input and verifies delivery independently;
the VM wrapper selects this mode because shell accessibility labels vary. It cleans up only its
own window, prototype toast and registration. Do not run it against another user's
existing Papo instance or settings.

The [clean VM harness](../../packaging/windows/clean-vm/README.md) supports
`prepare.py --test identity` for a fresh offline test guest. Runtime acceptance and
full desktop/release acceptance remain separate gates.

## Accepted evidence (2026-10-22)

[Native CI run 39703346154](https://github.com/ISpeakHue/papo-gtk/actions/runs/39703346154)
passed Windows compilation with warnings denied, all 24 packaging tests,
11 platform tests (including real named-pipe checks), native GTK startup/icon
checks, optimized packaging and an independent ZIP-only runtime check. The macOS
foundation compiled and linked tests in the same run.

The [offline VM report and inspected screenshots](evidence/windows-identity-2026-10-22/README.md)
record a successful standard-user run on Windows 11 Enterprise Evaluation 26H2,
build 26300.9457. All four native executables have correct resources/subsystems;
normal startup has no console and uses system DPI awareness (96 DPI in this guest).
Start search, Explorer, taskbar and notification history display Papo's identity.
Real notification-history clicks restored process 600 and cold-started process
1424; both delivered activation to GTK without a console. Cleanup removed the
owned registration and test toast; machine PATH was unchanged.

This establishes step 4's identity/activation design and the per-user NSIS choice.
It does not qualify Windows Home/Pro, real hardware, mixed-DPI scaling, a finished
installer or production notifications. The offline fixture uses no real account;
step 9 now implements account routing and trusted cold restoration, with its new
Windows desktop acceptance still pending.
An optional GLib session-D-Bus warning is retained in the captured stderr; native
single-instance forwarding passed independently of D-Bus.
