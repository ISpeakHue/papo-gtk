#!/usr/bin/env python3
"""EN-US: Validate source/artifact-bound manual acceptance; missing device/upgrade checks stay pending.
PT-BR: Valida aceitação manual vinculada ao código/artefatos; testes ausentes permanecem pendentes.
"""
import argparse
import json
import re
from pathlib import Path

CHECKS = {"multi_server", "reconnect_pending_permissions", "keyboard_reader_scaling",
          "microphone_output", "direct_turn_call", "capture_permission_cleanup", "upgrade_and_rollback"}
PLATFORMS = {"linux-appimage", "linux-flatpak", "windows-installer", "windows-portable"}


def evaluate(receipt, fingerprint, windows_ci=None):
    missing = []
    if receipt.get("source_fingerprint") != fingerprint:
        missing.append("source fingerprint does not match")
    rows = receipt.get("platforms", {})
    accepted = {}
    ci_ok = bool(windows_ci and windows_ci.get("conclusion") == "success"
        and windows_ci.get("name") == "Windows release checks"
        and windows_ci.get("head_sha") == receipt.get("source_commit")
        and re.fullmatch(r"[0-9a-f]{40}", windows_ci.get("head_sha", ""))
        and windows_ci.get("repository", {}).get("full_name") == "ISpeakHue/papo-gtk"
        and windows_ci.get("html_url") == f"https://github.com/ISpeakHue/papo-gtk/actions/runs/{windows_ci.get('id')}")
    if windows_ci is not None and not ci_ok:
        missing.append("Windows CI is not successful or does not match the tested source/repository/workflow")
    for platform in sorted(PLATFORMS):
        if platform.startswith("windows-") and ci_ok:
            accepted[platform] = {"status": "pass-automated", "evidence": windows_ci["html_url"]}
            continue
        row = rows.get(platform, {})
        if not row.get("tested_by") or not row.get("tested_at") or not row.get("environment"):
            missing.append(f"{platform}: missing operator/date/environment")
        if not re.fullmatch(r"[0-9a-f]{64}", row.get("artifact_sha256", "")):
            missing.append(f"{platform}: missing artifact SHA-256")
        checks = row.get("checks", {})
        for check in sorted(CHECKS):
            result = checks.get(check, {})
            if result.get("status") != "pass" or not result.get("evidence"):
                missing.append(f"{platform}: {check} has no passing evidence")
    return {"schema": 1, "status": "pending" if missing else "pass", "missing": missing, "automated_acceptance": accepted,
            "scope": "Operator-attested desktop/device/upgrade evidence; does not install, publish, sign or infer support status"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--source-fingerprint", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--windows-ci-report", type=Path, help="GitHub run API record; explicitly accept source-matched successful Windows automation instead of manual Windows rows")
    args = parser.parse_args()
    try:
        result = evaluate(json.loads(args.receipt.read_text()), args.source_fingerprint, json.loads(args.windows_ci_report.read_text()) if args.windows_ci_report else None)
    except (ValueError, OSError, TypeError) as error:
        result = {"schema": 1, "status": "pending", "error": str(error)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    raise SystemExit(result["status"] != "pass")


if __name__ == "__main__":
    main()
