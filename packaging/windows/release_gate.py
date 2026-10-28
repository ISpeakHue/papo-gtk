#!/usr/bin/env python3
"""EN-US: Bind full CI results to one clean source revision and one tested Windows ZIP.
PT-BR: Vincula os resultados completos da CI a uma revisão limpa e um ZIP Windows testado.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

from build import sha256, write_json

JOBS = {"windows", "portable-runtime", "media-ui-runtime", "desktop-ui-runtime", "installer-runtime",
        "linux-regression", "macos-foundation"}
PORTABLE = {"version", "capabilities", "runtime", "notifications", "storage", "isolation",
            "relocated-runtime", "relocated-notifications", "missing-python", "missing-gtk",
            "missing-capture", "missing-shell"}
MEDIA = {"media-emojis", "draft-previews", "attachment-media", "mov-media", "backend-gifs",
         "media-cache", "recent-media", "status-viewer", "send-video", "video-audio"}
AUTO_MEDIA = {"mov-media", "backend-gifs", "status-viewer", "video-audio"}
DESKTOP = {"desktop-input", "login-input", "multiline", "edit-mentions", "search-recycling",
           "account-settings", "voice-controls", "symbolic-icons", "status-viewer",
           "windows-notifications", "notification-window", "notification-filters", "voice-chat",
           "native_voice_worker_drop_releases_process", "native_call_worker_drop_closes_descendant_pipes",
           "native_call_cues_complete_and_cancel_without_devices"}
REPORTS = {"portable-runtime.json": PORTABLE, "media-ui-cairo.json": MEDIA,
           "media-ui-auto.json": AUTO_MEDIA, "desktop-ui-auto.json": DESKTOP}
INSTALLER_CASES = {"install", "upgrade", "stale-uninstaller", "locked-uninstall", "interrupted-recovery", "uninstall", "data-preserved"}
FEATURE_CASES = {
    "W01": ["runtime", "relocated-runtime", "missing-python", "missing-gtk", "missing-capture", "missing-shell"],
    "W03": ["storage"], "W04": ["storage", "login-input"], "W06": ["voice-chat"],
    "W07": ["search-recycling"], "W09": ["media-emojis"], "W10": ["draft-previews", "send-video"],
    "W11": ["attachment-media", "status-viewer"], "W12": ["backend-gifs"],
    "W13": ["mov-media", "video-audio"], "W14": ["isolation"],
    "W15": ["notification-window", "notification-filters"], "W16": ["notifications", "windows-notifications"],
    "W17": ["native_call_cues_complete_and_cancel_without_devices"],
    "W18": ["native_voice_worker_drop_releases_process", "native_call_worker_drop_closes_descendant_pipes"],
    "W19": ["capabilities", "runtime"], "W22": ["account-settings"],
    "W23": ["desktop-input", "login-input", "edit-mentions", "multiline"],
    "W24": ["symbolic-icons", "status-viewer"], "W25": ["media-cache", "recent-media"],
    "W26": sorted(INSTALLER_CASES),
}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def read(path):
    # EN-US: Windows PowerShell 5.1 writes UTF-8 with a BOM.
    # PT-BR: O Windows PowerShell 5.1 grava UTF-8 com BOM.
    return json.loads(Path(path).read_text(encoding="utf-8-sig"))


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def fingerprint(info):
    # EN-US: Repository indexes may change; exact package contents and tool versions may not.
    # PT-BR: Índices dos repositórios podem mudar; pacotes exatos e versões das ferramentas não.
    native = info["native_inputs"]
    build = info["build"]
    return digest({"installed": native["installed"],
                   "packages": sorted((entry["file"], entry["sha256"]) for entry in native["files"]
                                      if ".pkg.tar." in entry["file"]),
                   "tools": {name: tool["version"] for name, tool in info["toolchain"].items()},
                   "build": {name: build[name] for name in ("target", "profile", "environment", "cargo_lock_sha256")}})


def clean_build(info, commit):
    require(info["schema"] == 1 and info["source"]["commit"] == commit, "Package source revision mismatch")
    require(re.fullmatch(r"[0-9a-f]{40}", commit), "Expected a complete source commit")
    require(info["source_dirty"] is False and info["source"]["status"] == ""
            and not info["source"]["untracked_sha256"] and info["build"]["allow_dirty"] is False,
            "Dirty previews cannot pass the release gate")
    require(info["build"]["packaging_only"] is False, "Packaging-only builds cannot qualify a release")
    require(info["ci"]["GITHUB_SHA"] == commit, "Build CI/source mismatch")


def aggregate(info_path, reports, jobs, commit, repository, run_id, attempt):
    info = read(info_path)
    clean_build(info, commit)
    require(str(info["ci"]["GITHUB_RUN_ID"]) == str(run_id)
            and str(info["ci"]["GITHUB_RUN_ATTEMPT"]) == str(attempt), "Build belongs to another workflow run/attempt")
    require(JOBS <= jobs.keys(), "Required CI job results are missing")
    for name in sorted(JOBS):
        require(jobs[name]["result"] == "success", f"Required job {name}: {jobs[name]['result']}")
    source_checks = []
    for name in ("python-native.json", "python-workers.json"):
        path = info_path.parent / name
        tests = read(path)
        allowed = {"call_sound_test.CallSoundTests.test_auto_output_children_get_papos_identity_before_playback"} if name == "python-workers.json" else set()
        require(tests["status"] == "pass" and tests["run"] > 0 and tests["discovered"]
                and all(n > 0 for n in tests["discovered"].values()) and not tests["failures"]
                and not tests["errors"] and not tests["unexpected_skips"]
                and all(row["test"] in allowed and row["reason"] for row in tests["skipped"]),
                f"Required Python checks missing, failed or skipped: {name}")
        source_checks.append({"file": name, "sha256": sha256(path), "run": tests["run"], "skipped": tests["skipped"]})
    archive = None
    evidence = []
    instrumented = None
    for name, cases in REPORTS.items():
        paths = list(Path(reports).rglob(name))
        require(len(paths) == 1, f"Missing or duplicate evidence: {name}")
        path = paths[0]
        report = read(path)
        require(report["schema"] == 1 and report["status"] == "pass", f"Report did not pass: {name}")
        require(report["source_commit"] == commit and report["build_info_sha256"] == sha256(info_path)
                and report["runtime_manifest_sha256"] == info["runtime"]["manifest_sha256"],
                f"Evidence source/build/runtime mismatch: {name}")
        require(re.fullmatch(r"[0-9a-f]{64}", report["archive"]["sha256"]), "Invalid ZIP hash")
        require(archive is None or archive == report["archive"], "Checks used different ZIPs")
        archive = report["archive"]
        for field in ("immutable_runtime", "machine_path_unchanged", "extracted_package_removed"):
            require(report[field] is True, f"Missing integrity/cleanup evidence: {name}/{field}")
        actual = report["cases"]
        require(len(actual) == len(cases) and {case["name"] for case in actual} == cases,
                f"Missing, duplicate or unexpected cases: {name}")
        require(all(case["status"] == "pass" and case["timed_out"] is False for case in actual),
                f"Skipped, failed or timed-out case: {name}")
        if name == "portable-runtime.json":
            require(all(case["expected_failure"] is case["name"].startswith("missing-")
                        and ((case["exit_code"] != 0) if case["expected_failure"] else (case["exit_code"] == 0))
                        for case in actual), "Portable process result contradicts the reported pass")
        else:
            require(all(case["exit_code"] == 0 for case in actual), f"UI exit failure: {name}")
            require(report["instrumented_ui"]["source_commit"] == commit
                    and report["instrumented_ui"]["source_dirty"] is False, "UI source mismatch")
            ui_hash = report["instrumented_ui"]["sha256"]
            require(re.fullmatch(r"[0-9a-f]{64}", ui_hash) and (instrumented is None or instrumented == ui_hash),
                    "Checks used different instrumented executables")
            instrumented = ui_hash
            require(report["audio_output"] == "discard", "Unexpected device-dependent CI audio mode")
            require(report["renderer_request"] == ("cairo" if "cairo" in name else "auto"), "Renderer mismatch")
            if name == "desktop-ui-auto.json":
                require(report["native_keys"] is True, "Native desktop input check was not requested")
        evidence.append({"file": str(path.relative_to(reports)), "sha256": sha256(path),
                         "cases": sorted(cases)})
    paths = list(Path(reports).rglob("installer-runtime.json"))
    require(len(paths) == 1, "Missing or duplicate installer acceptance report")
    installer = read(paths[0])
    installer_path = info_path.parent / "INSTALLER-INFO.json"
    metadata = read(installer_path)
    require(metadata["schema"] == 1 and metadata["source_dirty"] is False
            and metadata["source_commit"] == commit and metadata["archive"] == archive
            and metadata["build_info_sha256"] == sha256(info_path)
            and metadata["runtime_manifest_sha256"] == info["runtime"]["manifest_sha256"], "Installer payload identity mismatch")
    require(installer["schema"] == 1 and installer["status"] == "pass"
            and installer["archive"] == archive and installer["installer"] == metadata["installer"]
            and installer["source_commit"] == commit and installer["build_info_sha256"] == sha256(info_path)
            and installer["runtime_manifest_sha256"] == info["runtime"]["manifest_sha256"]
            and installer["machine_path_unchanged"] is True, "Installer evidence identity/status mismatch")
    require(len(installer["cases"]) == len(INSTALLER_CASES) and {case["name"] for case in installer["cases"]} == INSTALLER_CASES
            and all(case["status"] == "pass" and case["timed_out"] is False and case["exit_code"] == 0
                    for case in installer["cases"]), "Missing, skipped or failed installer cases")
    require(re.fullmatch(r"[0-9a-f]{64}", metadata["installer"]["sha256"]), "Invalid installer hash")
    evidence.append({"file": str(paths[0].relative_to(reports)), "sha256": sha256(paths[0]), "cases": sorted(INSTALLER_CASES)})
    return {"schema": 1, "automated_status": "pass", "mode": "full", "repository": repository,
            "source_commit": commit, "run_id": str(run_id), "run_attempt": str(attempt),
            "jobs": jobs, "archive": archive, "build_info_sha256": sha256(info_path),
            "installer": metadata["installer"], "installer_info_sha256": sha256(installer_path),
            "build_fingerprint": fingerprint(info), "evidence": evidence, "source_checks": source_checks,
            "release_qualified": False,
            "manual_acceptance": {"status": "pending", "matrix": "docs/testing/windows.md",
                                  "targets": ["W25H", "W25P", "W26H", "W26P"],
                                  "features": [f"W{n:02}" for n in range(1, 27)]},
            "feature_evidence": {f"W{n:02}": {"automated_cases": FEATURE_CASES.get(f"W{n:02}", []),
                                               "source_tests": "windows: full regular suite; focused policy/native probes",
                                               "native_desktop_acceptance": "pending"} for n in range(1, 27)},
            "scope": "Automated candidate gate; Windows Server CI does not qualify Windows 11 desktop support.",
            "platform_exclusions": {"windows": "X11/XTest and Linux portals are Linux-only checks",
                                    "macos": "Compile/link foundation only; no macOS runtime acceptance"}}


def authorize_rebuild(gate, run, commit, repository):
    # EN-US: The workflow obtains run metadata from GitHub's API, never from the artifact itself.
    # PT-BR: O workflow obtém metadados da execução pela API do GitHub, nunca pelo próprio artefato.
    require(run["repository"]["full_name"] == repository
            and run["head_repository"]["full_name"] == repository, "Tested run must belong to this repository")
    require(run["path"] == ".github/workflows/windows.yml" and run["event"] in ("push", "workflow_dispatch")
            and run["status"] == "completed" and run["conclusion"] == "success", "Expected a successful trusted full workflow")
    require(run["head_sha"] == commit == gate["source_commit"], "Rebuild source must exactly match the tested revision")
    require(gate["schema"] == 1 and gate["repository"] == repository and gate["mode"] == "full"
            and gate["automated_status"] == "pass", "Only a full automated gate can authorize rebuilding")
    require(gate["run_id"] == str(run["id"]) and gate["run_attempt"] == str(run["run_attempt"]),
            "Test evidence belongs to another workflow run/attempt")
    require(JOBS <= gate["jobs"].keys() and all(gate["jobs"][name]["result"] == "success" for name in JOBS),
            "Incomplete tested-run gate")


def check_rebuild(gate_path, run_path, info, repository):
    gate = read(gate_path)
    authorize_rebuild(gate, read(run_path), info["source"]["commit"], repository)
    require(not info["source"]["status"] and not info["source"]["untracked_sha256"], "Dirty rebuild rejected")
    require(fingerprint(info) == gate["build_fingerprint"],
            "Native inputs, tools or build options changed; run the full workflow again")
    return {"run_id": gate["run_id"], "run_attempt": gate["run_attempt"],
            "gate_sha256": sha256(gate_path), "release_qualified": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-info", type=Path, required=True)
    parser.add_argument("--reports", type=Path, required=True)
    parser.add_argument("--jobs", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--attempt", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        report = aggregate(args.build_info, args.reports, read(args.jobs), args.commit,
                           args.repository, args.run_id, args.attempt)
    except (RuntimeError, KeyError, TypeError, ValueError, OSError) as error:
        write_json(args.output, {"schema": 1, "automated_status": "fail", "release_qualified": False,
                                 "error": str(error), "source_commit": args.commit, "mode": "full"})
        raise SystemExit(f"Windows candidate gate failed: {error}") from error
    write_json(args.output, report)
    print("Automated Windows candidate gate passed. Windows 11 manual acceptance remains pending.")


if __name__ == "__main__":
    main()
