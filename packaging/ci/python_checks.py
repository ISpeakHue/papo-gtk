#!/usr/bin/env python3
"""EN-US: Record Python test skips explicitly; reject empty discovery and unapproved skips.
PT-BR: Registra testes Python ignorados; rejeita descoberta vazia e exclusões não aprovadas.
"""
import argparse
import json
from pathlib import Path
import unittest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=Path("tests"))
    parser.add_argument("--pattern", action="append", required=True)
    parser.add_argument("--allow-skip", action="append", default=[])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    suite = unittest.TestSuite()
    counts = {}
    for pattern in args.pattern:
        tests = unittest.defaultTestLoader.discover(str(args.directory), pattern=pattern)
        counts[pattern] = tests.countTestCases()
        suite.addTests(tests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    skipped = [{"test": test.id(), "reason": reason} for test, reason in result.skipped]
    unexpected = [row for row in skipped if row["test"] not in args.allow_skip]
    passed = result.wasSuccessful() and all(counts.values()) and not unexpected
    report = {"schema": 1, "status": "pass" if passed else "fail", "discovered": counts,
              "run": result.testsRun, "skipped": skipped, "unexpected_skips": unexpected,
              "failures": [test.id() for test, _ in result.failures],
              "errors": [test.id() for test, _ in result.errors]}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if not passed:
        raise SystemExit("Python checks failed, discovered no tests, or skipped required checks")


if __name__ == "__main__":
    main()
