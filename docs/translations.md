# Interface languages

Papo embeds `locales/pt-BR.json` and `locales/en-US.json` in the executable;
AppImage, Flatpak and Windows bundles need no external catalog installation.
The default is the system language: Portuguese locales select PT-BR and all
other locales fall back to EN-US. Windows uses the native user locale; other
systems inspect the message-locale environment.

Account settings → Preferences → Appearance includes a device-local language
choice. Save preferences and restart to apply it to all windows together. The
choice is not sent to the backend. It changes `LANGUAGE` for native toolkit
translations but never changes `LC_CTYPE`, `LC_ALL` or the input-method module;
keyboard composition continues to use the existing desktop input settings.

Use `i18n::tr` for fixed interface text, `i18n::fmt` for named substitutions and
`i18n::plural` for counted labels. Catalogs use Portuguese message IDs and `one`
/ `other` plural forms. Both supported languages use the singular form at one.
Values inserted into a translated template are scanned only once: braces in a
nickname or message cannot become another substitution. Message bodies,
channel/user/role names, code, emoji identifiers, URLs, API fields and server
errors are not translated. Accessible names and tooltips use the same catalogs.
Date/time display uses day/month and 24-hour time in PT-BR, month/day and 12-hour
time in EN-US. API timestamps and stored dates retain their original formats.

Validate changes with:

```sh
python3 scripts/check-translations.py
python3 -m unittest discover -s tests -p implementation_tools_test.py -v
cargo test --locked i18n::tests
PAPO_UI_CASE=translations cargo test --locked ui_smoke -- --ignored --test-threads=1
```

The checker rejects missing keys, incompatible value types, missing plural forms,
empty translations and changed substitution names. The English GTK fixture also
checks that a nickname matching a Portuguese message ID remains unchanged. GTK
fixtures always use disposable settings and synthetic loopback accounts.
