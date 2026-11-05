#!/usr/bin/env python3
"""EN-US: Fail closed on incomplete benchmarks and compare bounded workloads with declared tolerances.
PT-BR: Rejeita medições incompletas e compara cargas limitadas com tolerâncias declaradas.
"""
import argparse
import json
import math
import hashlib
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parents[1]
SCENARIOS = {"startup_ui_ready", "cold_channel_text", "cold_channel_media", "warm_channel",
             "scrolling", "long_history", "media_residency", "media_requests"}


def number(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
        raise ValueError("Missing, negative or non-finite measurement")
    return value


def evaluate(report, reference, budgets, structural_only=False, asserted_profile=None):
    if not all(isinstance(value, dict) for value in (report, reference, budgets)):
        raise ValueError("Reports and budgets must be objects")
    for key in ("cpu_paint_p95_us", "mounted_rows", "resident_cache_bytes", "encoded_cache_bytes", "rss_checkpoint_bytes"):
        number(budgets.get(key))
    if report.get("status") != "pass" or not report.get("runs"):
        raise ValueError("A successful, nonempty baseline is required")
    violations = []
    measurements = {}
    reference_values = {}
    for run in reference.get("runs", []):
        for sample in run["samples"]:
            if "elapsed_us" in sample:
                reference_values.setdefault(sample["scenario"], []).append(number(sample["elapsed_us"]))
    for run in report["runs"]:
        samples = run.get("samples", [])
        if SCENARIOS - {s.get("scenario") for s in samples}:
            raise ValueError("Missing baseline scenarios")
        for sample in samples:
            name = sample["scenario"]
            detail = sample.get("details", {})
            if name in SCENARIOS - {"media_requests"}:
                measurements.setdefault(name, []).append(number(sample.get("elapsed_us")))
            if name == "media_requests":
                if number(detail.get("cold")) != 8 or number(detail.get("warm")) != 0:
                    violations.append("media_requests: expected eight cold and zero warm requests")
            if name == "warm_channel" and number(detail.get("additional_media_requests")) != 0:
                violations.append("warm_channel: downloaded media again")
        frames = [s["details"] for s in samples if s.get("scenario") == "scroll_frames"]
        history = [s["details"] for s in samples if s.get("scenario") == "history_memory_checkpoint"]
        media = [s["details"] for s in samples if s.get("scenario") == "media_memory_checkpoint"]
        if not frames or {100, 1000, 5000} - {s.get("messages") for s in history} or set(range(1, 9)) - {s.get("channels") for s in media}:
            raise ValueError("Missing frame/history/cache checkpoints")
        for frame in frames:
            if number(frame.get("frames")) < 30:
                violations.append("scroll_frames: fewer than 30 sampled frames")
            if not structural_only and number(frame.get("paint_p95_us")) > budgets["cpu_paint_p95_us"]:
                violations.append("scroll_frames: GTK CPU paint p95 exceeds budget")
        for checkpoint in history:
            if number(checkpoint.get("mounted_rows")) > budgets["mounted_rows"]:
                violations.append("history: mounted row budget exceeded")
        for checkpoint in media:
            for field in ("resident_cache_bytes", "encoded_cache_bytes"):
                if number(checkpoint.get(field)) > budgets[field]:
                    violations.append(f"media cache: {field} exceeded")
        rss = [number(s["details"]["process_resident_bytes"]) for s in samples
               if s.get("scenario") in {"history_memory_checkpoint", "media_memory_checkpoint"}]
        if max(rss) > budgets["rss_checkpoint_bytes"]:
            violations.append("process RSS checkpoint exceeds synthetic-workload envelope")
    comparisons = {}
    if not structural_only:
        profile = asserted_profile or report.get("profile")
        if profile != "release" or not str(report.get("platform", "")).startswith("Linux") or report.get("machine") != reference.get("machine"):
            raise ValueError("Timing budgets require a Linux release report with matching CPU architecture; use structural-only elsewhere")
        if len(report["runs"]) < 3:
            raise ValueError("Timing comparisons require at least three fresh-process runs")
        if report.get("renderer_requested") != reference.get("renderer_requested") or report.get("window_size") != reference.get("window_size"):
            raise ValueError("Renderer or window geometry differs from the reference")
        for name, values in measurements.items():
            if name not in reference_values:
                raise ValueError(f"Reference lacks {name}")
            baseline = statistics.median(reference_values[name])
            allowance = budgets["timing"][name]
            limit = baseline * number(allowance["ratio"]) + number(allowance["slack_us"])
            current = statistics.median(values)
            comparisons[name] = {"reference_us": baseline, "median_us": current, "limit_us": limit}
            if current > limit:
                violations.append(f"{name}: {current} us exceeds {limit} us")
    return {"schema": 1, "status": "fail" if violations else "pass", "violations": sorted(set(violations)),
            "timing_checked": not structural_only, "comparisons": comparisons,
            "binary_source_verified": report.get("binary_source_verified", False),
            "profile_asserted_by_operator": asserted_profile,
            "scope": "Synthetic CPU/request/cache regression envelope, not compositor latency, leak detection or release/device acceptance"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--reference", type=Path, default=ROOT / "docs/testing/evidence/performance-2026-11-04/baseline.json")
    parser.add_argument("--budgets", type=Path, default=ROOT / "docs/testing/performance-budgets.json")
    parser.add_argument("--structural-only", action="store_true")
    parser.add_argument("--assert-profile", choices=("release",), help="Explicit assertion for an externally built executable; recorded, never treated as source verification")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = evaluate(json.loads(args.report.read_text()), json.loads(args.reference.read_text()),
                          json.loads(args.budgets.read_text()), args.structural_only, args.assert_profile)
        result.update(report_sha256=hashlib.sha256(args.report.read_bytes()).hexdigest(), reference_sha256=hashlib.sha256(args.reference.read_bytes()).hexdigest(), budgets_sha256=hashlib.sha256(args.budgets.read_bytes()).hexdigest())
    except (ValueError, KeyError, TypeError, OSError) as error:
        result = {"schema": 1, "status": "fail", "error": str(error)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    raise SystemExit(result["status"] != "pass")


if __name__ == "__main__":
    main()
