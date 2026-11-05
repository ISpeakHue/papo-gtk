#!/usr/bin/env python3
"""EN-US: Verify that the source behind the reviewed UX contracts has not changed; no server access.
PT-BR: Verifica se o código dos contratos UX revisados mudou, sem acessar servidores.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def verify(backend, review):
    differences = []
    for name, expected in review["files"].items():
        path = backend / name
        actual = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
        if actual != expected:
            differences.append({"file": name, "reviewed_sha256": expected, "current_sha256": actual})
    return differences


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    backend = args.backend.resolve()
    review = json.loads((ROOT / "docs/backend/ux-contracts.json").read_text(encoding="utf-8"))
    current = subprocess.check_output(["git", "-C", str(backend), "rev-parse", "HEAD"], text=True).strip()
    differences = verify(backend, review)
    result = {"schema": 1, "status": "review_required" if differences else "matches_reviewed_source",
              "reviewed_commit": review["source_commit"], "current_commit": current,
              "scope": "Source comparison only; not a running-server capability probe or backend test suite.",
              "differences": differences, "reviewed_contracts": review["contracts"]}
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    raise SystemExit(2 if differences else 0)


if __name__ == "__main__":
    main()
