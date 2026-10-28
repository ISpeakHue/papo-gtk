#!/usr/bin/env python3
"""EN-US: Export opt-in beta measurements with fixed fields; never copy diagnostic logs or account data.
PT-BR: Exporta medições beta opcionais com campos fixos; nunca copia logs nem dados de contas.
"""
import argparse
import json
from pathlib import Path
import re

METRICS = set("request_us request_queue_us rate_limits chat_update_us inbox_update_us search_update_us "
              "notification_rows_built search_widgets_built member_widgets_built encoded_cache_hits "
              "encoded_cache_misses encoded_cache_bytes external_avatar_hits animation_frames frame_paint_us "
              "frame_budget_us frame_paint_over_budget gif_decode_us gif_decode_wait_us media_decode_us "
              "media_decode_wait_us resident_preview_bytes resident_preview_hits media_first_frames".split())
SCENARIOS = ("startup", "cold-channel", "warm-channel", "channel-switch-10", "older-media",
             "call-cycles-10", "sleep-resume", "permission-change", "network-recovery")


def aggregates(text):
    if len(text.encode("utf-8")) > 4 * 1024**2:
        raise ValueError("Use a diagnostic excerpt of at most 4 MiB")
    rows = None
    for line in text.splitlines():
        if line.startswith("Papo aggregate performance: "):
            rows = json.loads(line.removeprefix("Papo aggregate performance: "))
    if not isinstance(rows, dict):
        raise ValueError("No final Papo aggregate performance report found")
    result = {}
    for label in METRICS & rows.keys():
        sample = rows[label]
        if not isinstance(sample, dict):
            raise ValueError("Malformed aggregate sample")
        row = {key: sample[key] for key in ("samples", "total", "max")}
        if any(type(value) is not int or not 0 <= value <= 2**64 - 1 for value in row.values()):
            raise ValueError("Only bounded integer counters can be exported")
        if row["max"] > row["total"]:
            raise ValueError("Inconsistent aggregate sample")
        result[label] = row
    if not result:
        raise ValueError("No approved numeric metrics found")
    return result


def compare(left, right):
    keys = ("source_commit", "scenario", "renderer", "scale", "network", "keyboard", "audio")
    if any(left[key] != right[key] for key in keys) or {left["platform"], right["platform"]} != {"windows", "linux"}:
        raise ValueError("Comparison requires the same source/scenario/settings on Windows and Linux")
    for platform in (left, right):
        for key in ("startup_ms", "working_set_start_bytes", "working_set_end_bytes", "working_set_peak_bytes"):
            value = platform["measurements"][key]
            if key == "startup_ms" and value is None:
                continue
            if type(value) is not int or not 0 <= value <= 2**63 - 1:
                raise ValueError("Comparison accepts only numeric measurements")
    return {platform["platform"]: {"startup_ms": platform["measurements"]["startup_ms"],
                                   "working_set_start_bytes": platform["measurements"]["working_set_start_bytes"],
                                   "working_set_end_bytes": platform["measurements"]["working_set_end_bytes"],
                                   "working_set_peak_bytes": platform["measurements"]["working_set_peak_bytes"]}
            for platform in (left, right)}


def bounded(value):
    number = int(value)
    if not 0 <= number <= 2**63 - 1:
        raise argparse.ArgumentTypeError("Expected a nonnegative measurement")
    return number


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--consent", action="store_true", help="Explicitly export the selected local measurements")
    parser.add_argument("--performance", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--platform", choices=("windows", "linux"), required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--package-sha256")
    parser.add_argument("--scenario", choices=SCENARIOS, required=True)
    parser.add_argument("--renderer", choices=("auto", "cairo"), required=True)
    parser.add_argument("--scale", choices=(100, 125, 150, 200), type=int, required=True)
    parser.add_argument("--gpu", choices=("amd", "intel", "nvidia", "microsoft", "other", "unknown"), required=True)
    parser.add_argument("--keyboard", choices=("pt-br", "us", "ime", "other"), required=True)
    parser.add_argument("--audio", choices=("default", "usb", "bluetooth", "none"), required=True)
    parser.add_argument("--network", choices=("normal", "offline", "interrupted", "relay"), required=True)
    parser.add_argument("--startup-ms", type=bounded)
    for name in ("start", "end", "peak"):
        parser.add_argument(f"--working-set-{name}-bytes", type=bounded, required=True)
    parser.add_argument("--compare", type=Path)
    args = parser.parse_args()
    if not args.consent:
        parser.error("No report written: explicit --consent is required")
    if not re.fullmatch(r"[0-9a-f]{40}", args.source_commit):
        parser.error("Use the exact full source commit")
    if args.package_sha256 and not re.fullmatch(r"[0-9a-f]{64}", args.package_sha256):
        parser.error("Invalid package hash")
    if args.platform == "windows" and not args.package_sha256:
        parser.error("Windows beta reports require the tested installer/ZIP SHA-256")
    if args.performance.stat().st_size > 4 * 1024**2:
        parser.error("Diagnostic excerpt exceeds 4 MiB")
    metrics = aggregates(args.performance.read_text(encoding="utf-8-sig"))
    measurements = {"startup_ms": args.startup_ms, "working_set_start_bytes": args.working_set_start_bytes,
                    "working_set_end_bytes": args.working_set_end_bytes, "working_set_peak_bytes": args.working_set_peak_bytes}
    if measurements["working_set_peak_bytes"] < max(args.working_set_start_bytes, args.working_set_end_bytes):
        parser.error("Peak working set is below the start/end measurement")
    report = {"schema": 1, "kind": "opt-in-beta-measurements", "source_commit": args.source_commit,
              "package_sha256": args.package_sha256,
              **{key: getattr(args, key) for key in ("platform", "scenario", "renderer", "scale", "gpu", "keyboard", "audio", "network")},
              "measurements": measurements, "aggregates": metrics,
              "release_qualified": False, "scope": "Manually sampled beta measurements; no automatic collection or upload."}
    if args.compare:
        report["comparison"] = compare(report, json.loads(args.compare.read_text(encoding="utf-8-sig")))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print("Selected numeric beta measurements exported locally. Nothing uploaded.")


if __name__ == "__main__":
    main()
