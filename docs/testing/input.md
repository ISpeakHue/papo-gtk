# Composer, clipboard and input methods

Commands run from the repository root. See [Testing](../../TESTING.md) for setup.
Windows desktop and real input-method acceptance is tracked separately in
[Windows desktop/input testing](windows-desktop.md). X11 key injection and direct
preedit-signal tests do not prove Windows dead-key or IME behavior.
Dated validation results describe their original fixture runs, rather than a
current guarantee for every server, desktop or driver.

## Clipboard image attachments

Copy an image (for example, a screenshot) and paste in the message composer with
Ctrl+V or Shift+Insert. It should appear as `imagem-colada.png` in the pending
attachments, preserving any caption. It is uploaded only when you send. Ordinary
text paste continues to insert text. Removing the attachment deletes its temporary
file; failed uploads retain it for retry, and switching channels preserves its draft.

The mapped clipboard check uses a private test clipboard and mock API. It covers
native paste, both shortcuts, PNG MIME clipboard data, caption preservation,
upload failure/retry, channel-scoped completions, permission changes, attachment
limits, sending while an image is being prepared, and temporary-file cleanup.

```sh
PAPO_UI_CASE=clipboard dbus-run-session -- xvfb-run -a cargo test --locked ui_smoke -- --ignored --test-threads=1
cargo test --locked clipboard_file_lives_until_the_last_draft_or_upload_owner_is_dropped
```

The clipboard check also runs in the complete GTK workflow.

## Multiline composer and native input methods

Enter sends a message; Shift+Enter inserts a newline. The native text editor
wraps at the available width and grows from 40 to 160 pixels, then scrolls
vertically. Multiline drafts use a 4-pixel top/bottom inset; shorter drafts stay
centered in the toolbar's minimum height. Mentions retain their IDs, highlighting and caret position across
multiline edits, failed sends and restored channel drafts. Input-method preedit
and commit keys are handled by GTK before sending or autocomplete shortcuts.

```sh
cargo test --locked --offline
PAPO_UI_CASE=multiline PAPO_NATIVE_INPUT_TEST=1 GTK_IM_MODULE=simple dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

The native test requires Python, `setxkbmap` and libXtst. It sends real keyboard
events only to its named window on a private X11 display. It checks dead-key
`á`/`é`, Unicode `ç` committed with Enter without sending, Shift+Enter, mention
completion, wrapping, height limits, failed-send preservation and Enter-to-send.
Its keyboard layout change stays inside that test display. Without
`PAPO_NATIVE_INPUT_TEST`, the full GTK smoke test exercises native editing signals.
Dead keys are pressed separately, with GTK updates dispatched between the accent
and the letter, matching human typing. Set `PAPO_NATIVE_INPUT_LAYOUT=br` to run
the same private-display check with the Brazilian layout.

The AppImage includes GTK clients for IBus and Fcitx5 and preserves the desktop's
input-method selection. Its runtime check loads both modules and their bundled
dependencies. The 20261013-input build passed the native keyboard test with the
bundled GTK runtime using `simple`, `ibus` and `fcitx`; those private tests do not
run desktop input-method daemons. Testers should also check their actual GNOME/KDE
session: type dead-key accents, paste text, compose multiple lines, and confirm
Enter commits active input-method candidates before sending a message.

On Wayland, the composer prefers the IBus/Fcitx GTK client selected by
`XMODIFIERS`, respecting available explicit `GTK_IM_MODULE` and GTK settings
(including preference lists and GTK aliases). If that client is missing, or a
terminal provides no framework hint, the composer uses the registered
`gtk-im-context-simple` context for
dead keys, Compose and Unicode input. The AppImage already includes both framework
clients. Source installs needing full IBus/Fcitx candidate input should install
their GTK4 client (Fedora: `ibus-gtk4` or `fcitx5-gtk4`). This fallback cannot provide
the framework's language engines. The root chooses a consistent process-local
input method before constructing login fields; desktop settings remain unchanged.
Preedit updates stay in GTK rather than rebuilding the component; the placeholder
updates directly, and forced word-completion hints are omitted.

The default native GTK path dropped accents in both controls of a desktop check
on this GNOME Wayland system. The user confirmed dead-key typing works in the
updated composer with the registered built-in context. The full-application
check was also confirmed by the user on the GNOME Wayland desktop. A manual
comparison is available using a private localhost mock server, without opening
the user's account or contacting the remote backend:

```sh
cargo test --locked --offline ui::chat::multiline::desktop_accent_check -- --ignored --test-threads=1 --nocapture
```

This opens a visible test window for at most three minutes and reports only
whether accented text was entered, not the typed contents. Explicitly close it
with **Concluir teste**. Automated X11 checks alone do not validate the desktop's
Wayland input-method connection. The full-app check embeds the initially
disabled composer in the actual main application hierarchy, with local startup
requests and the normal activity/shortcut handlers.

A composer-only fallback still failed after actual login: diagnostics showed no
dead-key/preedit events reaching a focused, editable composer with the correct
built-in module. The user confirmed that `GTK_IM_MODULE=simple` restores accents
in the real server session. Startup now selects the transport for the entire
application through process-local GTK settings, so login and chat inherit the
same method. Available framework clients and valid explicit preferences retain
priority. This removes the need for that temporary launch variable.

The previous fallback depended on `XMODIFIERS` and skipped any nonempty GTK
preference, including an unavailable module. Selection now handles an absent
hint and missing clients, runs again on map, and follows later GTK settings
changes. It changes the widget module only when the selection differs, preserving
active composition during ordinary updates. Tests cover those preference cases
and full-app startup with real Brazilian dead keys on a private X11 display:

```sh
PAPO_UI_CASE=full-app-input PAPO_NATIVE_INPUT_TEST=1 GTK_IM_MODULE=simple dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```

Run `PAPO_UI_CASE=full-app-input` without `XMODIFIERS` or `GTK_IM_MODULE` in an
isolated Wayland test session to check the no-hint fallback and process-local
settings updates. This policy check does not simulate physical Wayland dead keys;
the desktop interactive check verifies those. CI also runs the full-app X11
keyboard check separately.

## Diagnosing input in the running application

If a local test works but the application launched in another terminal/runtime
still fails, opt-in composer diagnostics are available:

```sh
PAPO_INPUT_DIAGNOSTICS=1 cargo run
```

After reproducing the failure, inspect `target/papo-input-diagnostic.json`.
The ignored local artifact records the runtime GTK version, backend, known
input-module preferences, registered framework clients, focus/editability and
aggregate key/preedit/buffer-change event counts. It never stores key values,
typed text, message contents, credentials or server/session data. Unknown module
preference values are masked. `test_build` distinguishes test windows from the
actual application. The hooks and file writes are disabled unless explicitly
requested with `PAPO_INPUT_DIAGNOSTICS=1`. Normal startup and typing incur no
diagnostic file writes.

The `login-input` regression focuses and edits the actual login username field,
then transitions the existing root stack to the main chat while retaining the
login controller. It checks subsequent composition and confirms that committing
Unicode input with Enter never sends a message. The private Wayland run checks
that both views inherit the same process-wide Compose setting; CI uses actual
Brazilian dead-key events on its private X11 display:

```sh
PAPO_UI_CASE=login-input PAPO_NATIVE_INPUT_TEST=1 GTK_IM_MODULE=simple dbus-run-session -- xvfb-run -a cargo test --locked --offline ui_smoke -- --ignored --test-threads=1
```
