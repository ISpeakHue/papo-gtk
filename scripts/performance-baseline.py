#!/usr/bin/env python3
"""EN-US: Run isolated synthetic GUI baselines and retain comparable machine-readable samples.
PT-BR: Executa baselines sintéticas isoladas e preserva amostras comparáveis em JSON.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import time
import contextlib
import shutil
import signal

ROOT = Path(__file__).resolve().parents[1]
SCENARIOS = {"startup_ui_ready", "cold_channel_text", "cold_channel_media", "warm_channel",
             "scrolling", "long_history", "media_residency", "media_requests"}


def parse_samples(output):
    samples = []
    for line in output.splitlines():
        prefix = "Papo baseline: "
        if prefix in line:
            sample = json.loads(line.split(prefix, 1)[1])
            if not isinstance(sample, dict) or not isinstance(sample.get("scenario"), str):
                raise ValueError("Invalid baseline sample")
            samples.append(sample)
    missing = SCENARIOS - {s["scenario"] for s in samples}
    if missing:
        raise ValueError(f"Missing baseline scenarios: {sorted(missing)}")
    if any(s.get("details", {}).get("additional_media_requests", 0) for s in samples if s["scenario"] == "warm_channel"):
        raise ValueError("Warm channel revisit downloaded media again")
    return samples


def summarize(runs):
    groups = {}
    for run in runs:
        for sample in run["samples"]:
            if "elapsed_us" in sample:
                groups.setdefault(sample["scenario"], []).append(sample["elapsed_us"])
    return {name: {"samples": len(values), "median_us": statistics.median(values),
                   "min_us": min(values), "max_us": max(values)} for name, values in sorted(groups.items())}


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def source_fingerprint():
    names = git("ls-files", "--cached", "--others", "--exclude-standard", "-z").split("\0")
    manifest = {}
    for name in sorted(set(names)):
        if name and (name.startswith(("src/", "tests/", "assets/", "locales/", "scripts/", "packaging/")) or name in ("Cargo.toml", "Cargo.lock", "build.rs")):
            path = ROOT / name
            if path.is_file(): manifest[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    return hashlib.sha256(json.dumps(manifest, sort_keys=True).encode()).hexdigest()


@contextlib.contextmanager
def private_display(environment):
    if platform.system() != "Linux" or shutil.which("xvfb-run"):
        yield
        return
    if not shutil.which("Xvnc"):
        raise RuntimeError("Install xvfb-run or Xvnc, or pass --native-display")
    # EN-US: Only a private Unix VNC socket; no TCP listener or shared desktop session.
    # PT-BR: Apenas um socket Unix VNC privado, sem listener TCP nem sessão compartilhada do desktop.
    with tempfile.TemporaryDirectory(prefix="papo-display-") as directory, tempfile.TemporaryFile() as ready, tempfile.TemporaryFile() as log:
        process = subprocess.Popen(["Xvnc", "-displayfd", str(ready.fileno()), "-geometry", "1920x1080", "-depth", "24",
                                    "-SecurityTypes", "None", "-rfbport", "-1", "-rfbunixpath", str(Path(directory)/"vnc.sock"),
                                    "-rfbunixmode", "0600", "-localhost", "-nolisten", "tcp"],
                                   pass_fds=(ready.fileno(),), stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                ready.seek(0); number = ready.read().decode("ascii").strip()
                if number.isdigit(): break
                if process.poll() is not None:
                    log.seek(0)
                    raise RuntimeError("Private Xvnc failed to start: " + log.read().decode("utf-8", errors="replace")[-2000:])
                time.sleep(0.05)
            else: raise RuntimeError("Private Xvnc startup timed out")
            environment["DISPLAY"] = ":" + number
            yield
        finally:
            process.terminate()
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: process.kill(); process.wait()


def run_gui(command, environment, private):
    with private_display(environment) if private else contextlib.nullcontext():
        process = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace",
                                   start_new_session=os.name != "nt")
        try:
            output, _ = process.communicate(timeout=240)
            return subprocess.CompletedProcess(command, process.returncode, output)
        except subprocess.TimeoutExpired:
            if os.name == "nt":
                subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True)
            else:
                os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--profile", choices=("debug", "release"), default="release")
    parser.add_argument("--binary", type=Path, help="Existing instrumented Rust test executable")
    parser.add_argument("--native-display", action="store_true", help="Use the current desktop instead of private Linux Xvfb")
    parser.add_argument("--output", type=Path, default=ROOT / "target/performance/baseline.json")
    args = parser.parse_args()
    if not 1 <= args.runs <= 20:
        parser.error("--runs must be between 1 and 20")
    binary = args.binary
    external_binary = binary is not None
    if binary is None:
        command = ["cargo", "test", "--locked", "--no-run", "--message-format=json"]
        if args.profile == "release": command.append("--release")
        built = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, text=True, check=True)
        executables = [r["executable"] for line in built.stdout.splitlines() if (r := json.loads(line)).get("reason") == "compiler-artifact"
                       and r.get("profile", {}).get("test") and r.get("executable") and r["target"]["name"] == "papo-gtk"]
        if len(executables) != 1: raise RuntimeError("Expected one instrumented application test executable")
        binary = Path(executables[0])
    binary = binary.resolve()
    output = args.output.resolve(); output.parent.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    for key in ("PAPO_NATIVE_INPUT_TEST", "PAPO_NATIVE_FULLSCREEN_TEST", "PAPO_DESIGN_PREVIEW", "PAPO_SCROLL_MESSAGES", "PAPO_INPUT_DIAGNOSTICS"):
        environment.pop(key, None)
    environment.update(PAPO_UI_CASE="performance-baselines", PAPO_PERFORMANCE="1", PAPO_TEST_AUDIO_OUTPUT="discard", GSK_RENDERER="cairo")
    command = [str(binary), "ui_smoke", "--ignored", "--exact", "--test-threads=1", "--nocapture"]
    # EN-US: Exact matching prevents silently measuring zero tests after a test rename.
    # PT-BR: A correspondência exata evita medir silenciosamente zero testes após renomeações.
    command[1] = "ui::root::tests::ui_smoke_window_has_header_controls_and_server_password"
    private = platform.system() == "Linux" and not args.native_display
    if private:
        environment.update(GDK_BACKEND="x11")
        if shutil.which("xvfb-run"):
            command = ["xvfb-run", "-a", "-s", "-screen 0 1920x1080x24", *command]
        command = ["dbus-run-session", "--", *command]
    report = {"schema": 1, "status": "running", "source_commit": git("rev-parse", "HEAD"),
              "source_dirty": bool(git("status", "--porcelain")), "source_fingerprint": source_fingerprint(),
              "binary_source_verified": not external_binary, "platform": platform.platform(),
              "machine": platform.machine(), "profile": "external-unverified" if external_binary else args.profile,
              "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "display": "private-x11" if private else "native", "renderer_requested": "cairo", "window_size": [1000, 720],
              "scope": "Synthetic local fixtures, not live-server latency or GPU/compositor presentation; RSS includes native runtime and allocator memory.",
              "runs": []}
    for index in range(args.runs):
        with tempfile.TemporaryDirectory(prefix="papo-baseline-") as temporary:
            environment["PAPO_TEST_CONFIG_DIR"] = temporary
            started = time.monotonic()
            try:
                result = run_gui(command, environment, private)
            except subprocess.TimeoutExpired as error:
                report["status"] = "fail"; report["error"] = "GUI baseline timed out"
                output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
                raise SystemExit(report["error"]) from error
            log = output.with_name(f"{output.stem}-run-{index + 1}.log")
            log.write_text(result.stdout, encoding="utf-8")
            if result.returncode:
                report["status"] = "fail"; report["error"] = f"GUI baseline exited {result.returncode}; see {log.name}"
                output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
                raise SystemExit(report["error"])
            try:
                samples = parse_samples(result.stdout)
                aggregate = [json.loads(line.split("Papo aggregate performance: ", 1)[1]) for line in result.stdout.splitlines()
                             if "Papo aggregate performance: " in line][-1]
            except (ValueError, IndexError) as error:
                report.update(status="fail", error=f"Invalid baseline output: {error}")
                output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
                raise SystemExit(report["error"]) from error
            report["runs"].append({"process_wall_ms": round((time.monotonic() - started) * 1000, 2),
                                   "samples": samples, "aggregate_metrics": aggregate})
            print(f"Baseline run {index + 1}/{args.runs}: {len(samples)} samples", flush=True)
    report.update(status="pass", summary=summarize(report["runs"]))
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
