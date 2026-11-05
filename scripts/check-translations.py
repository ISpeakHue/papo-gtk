#!/usr/bin/env python3
"""EN-US: Validate embedded catalogs and literal translation references without compiling GTK.
PT-BR: Valida catálogos embutidos e referências literais sem compilar o GTK.
"""
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def placeholders(text):
    return set(re.findall(r"(?<!\{)\{([^{}]*)\}(?!\})", text))


def validate(catalogs):
    pt, en = catalogs
    errors = []
    if pt.keys() != en.keys(): errors.append("Catalog keys differ")
    for key in pt.keys() & en.keys():
        a, b = pt[key], en[key]
        if type(a) is not type(b): errors.append(f"Catalog type mismatch: {key}"); continue
        if isinstance(a, dict):
            if set(a) != {"one", "other"} or set(b) != {"one", "other"}: errors.append(f"Invalid plural forms: {key}"); continue
            pairs = [(a[name], b[name]) for name in ("one", "other")]
        else: pairs = [(a, b)]
        for source, target in pairs:
            if not isinstance(source, str) or not isinstance(target, str) or not target:
                errors.append(f"Invalid translation: {key}")
            elif placeholders(source) != placeholders(target): errors.append(f"Placeholder mismatch: {key}")
    return errors


def main():
    catalogs = [json.loads((ROOT / "locales" / f"{name}.json").read_text(encoding="utf-8")) for name in ("pt-BR", "en-US")]
    errors = validate(catalogs)
    pattern = re.compile(r'i18n::(?:tr|fmt|plural)\(\s*"((?:\\.|[^"\\])*)"')
    for path in (ROOT / "src").rglob("*.rs"):
        for match in pattern.finditer(path.read_text(encoding="utf-8")):
            key = json.loads('"' + match[1].replace("\n", "\\n") + '"')
            if key not in catalogs[0]: errors.append(f"Missing key in {path.relative_to(ROOT)}: {key}")
    if errors: raise SystemExit("\n".join(errors))
    print(f"PT-BR/EN-US: {len(catalogs[0])} matching entries; placeholders and static references valid")


if __name__ == "__main__": main()
