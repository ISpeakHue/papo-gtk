"""EN-US: Exercise release-gate failures with synthetic reports, without desktop/network access.
PT-BR: Testa falhas da aprovação de release com relatórios sintéticos, sem desktop nem rede.
"""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "packaging/windows"))
import release_gate as gate
from build import sha256, write_json

COMMIT = "a" * 40
REPO = "fixture/papo"


class ReleaseGate(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.build_path = self.root / "BUILD-INFO.json"
        self.info = {"schema": 1, "source": {"commit": COMMIT, "status": "", "untracked_sha256": {}},
                     "source_dirty": False, "runtime": {"manifest_sha256": "b" * 64},
                     "ci": {"GITHUB_SHA": COMMIT, "GITHUB_RUN_ID": "42", "GITHUB_RUN_ATTEMPT": "1"},
                     "build": {"packaging_only": False, "allow_dirty": False,
                               "target": "x86_64-pc-windows-gnu", "profile": "release",
                               "environment": {"RUSTFLAGS": "-D warnings"}, "cargo_lock_sha256": "c" * 64},
                     "native_inputs": {"installed": {"rust": "1"},
                                       "files": [{"file": "rust-1.pkg.tar.zst", "sha256": "d" * 64},
                                                 {"file": "ucrt64.db", "sha256": "e" * 64}]},
                     "toolchain": {"rustc.exe": {"version": "rustc fixture", "path": "C:/ucrt64/bin"}}}
        self.jobs = {name: {"result": "success"} for name in gate.JOBS}
        self.reports = self.root / "reports"
        self.reports.mkdir()
        write_json(self.build_path, self.info)
        for name in ("python-native.json", "python-workers.json"):
            write_json(self.root / name, {"status": "pass", "run": 3, "discovered": {"fixtures.py": 3},
                                         "skipped": [], "unexpected_skips": [], "failures": [], "errors": []})
        for name, cases in gate.REPORTS.items():
            report = {"schema": 1, "status": "pass", "source_commit": COMMIT,
                      "build_info_sha256": sha256(self.build_path),
                      "runtime_manifest_sha256": self.info["runtime"]["manifest_sha256"],
                      "archive": {"name": "Papo.zip", "sha256": "f" * 64},
                      "immutable_runtime": True, "machine_path_unchanged": True,
                      "extracted_package_removed": True,
                      "instrumented_ui": {"source_commit": COMMIT, "source_dirty": False, "sha256": "1" * 64},
                      "audio_output": "discard", "native_keys": name == "desktop-ui-auto.json",
                      "renderer_request": "cairo" if "cairo" in name else "auto",
                      "cases": [{"name": case, "status": "pass", "timed_out": False,
                                 "expected_failure": case.startswith("missing-"),
                                 "exit_code": 1 if case.startswith("missing-") else 0} for case in sorted(cases)]}
            write_json(self.reports / name, report)
        installer_info = {"schema": 1, "source_commit": COMMIT, "source_dirty": False,
                          "archive": {"name": "Papo.zip", "sha256": "f" * 64},
                          "installer": {"name": "Papo.exe", "sha256": "3" * 64},
                          "build_info_sha256": sha256(self.build_path),
                          "runtime_manifest_sha256": self.info["runtime"]["manifest_sha256"]}
        write_json(self.root / "INSTALLER-INFO.json", installer_info)
        write_json(self.reports / "installer-runtime.json", {**installer_info, "status": "pass", "machine_path_unchanged": True,
                   "cases": [{"name": case, "status": "pass", "timed_out": False, "exit_code": 0} for case in sorted(gate.INSTALLER_CASES)]})

    def aggregate(self):
        return gate.aggregate(self.build_path, self.reports, self.jobs, COMMIT, REPO, "42", "1")

    def mutate(self, name, update):
        path = self.reports / name
        value = gate.read(path)
        update(value)
        write_json(path, value)

    def test_complete_candidate_preserves_pending_desktop_acceptance(self):
        report = self.aggregate()
        self.assertEqual(report["automated_status"], "pass")
        self.assertFalse(report["release_qualified"])
        self.assertEqual(len(report["manual_acceptance"]["targets"]), 4)
        self.assertEqual(len(report["manual_acceptance"]["features"]), 26)
        self.assertEqual(len(report["evidence"]), 5)

    def test_failed_skipped_cancelled_and_unknown_job_results_never_pass(self):
        for name in gate.JOBS:
            for result in ("failure", "skipped", "cancelled", "pending", ""):
                with self.subTest(name=name, result=result):
                    self.jobs[name]["result"] = result
                    with self.assertRaisesRegex(RuntimeError, "Required job"):
                        self.aggregate()
            self.jobs[name]["result"] = "success"

    def test_missing_job_is_not_a_pass(self):
        self.jobs.pop("linux-regression")
        with self.assertRaisesRegex(RuntimeError, "missing"):
            self.aggregate()

    def test_python_summary_cannot_hide_skipped_native_or_missing_tests(self):
        path = self.root / "python-native.json"
        original = gate.read(path)
        for update in ({"run": 0}, {"discovered": {}}, {"discovered": {"fixtures.py": 0}},
                       {"skipped": [{"test": "native.fixture", "reason": "no compiler"}]}):
            write_json(path, {**original, **update})
            with self.assertRaisesRegex(RuntimeError, "Python checks"):
                self.aggregate()

    def test_dirty_or_packaging_only_build_cannot_pass(self):
        for path, key, value in (((), "source_dirty", True), (("source",), "status", " M src.rs"),
                                  (("source",), "untracked_sha256", {"new.rs": "0" * 64}),
                                  (("build",), "allow_dirty", True), (("build",), "packaging_only", True)):
            bad = copy.deepcopy(self.info)
            target = bad
            for part in path:
                target = target[part]
            target[key] = value
            write_json(self.build_path, bad)
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                self.aggregate()

    def test_other_commit_run_or_attempt_is_rejected(self):
        for commit, run, attempt in (("2" * 40, "42", "1"), (COMMIT, "43", "1"), (COMMIT, "42", "2")):
            with self.subTest(commit=commit, run=run, attempt=attempt), self.assertRaises(RuntimeError):
                gate.aggregate(self.build_path, self.reports, self.jobs, commit, REPO, run, attempt)

    def test_all_report_bindings_and_integrity_flags_are_required(self):
        for name in gate.REPORTS:
            original = gate.read(self.reports / name)
            for field, value in (("source_commit", "2" * 40), ("build_info_sha256", "2" * 64),
                                 ("runtime_manifest_sha256", "2" * 64), ("status", "skipped"),
                                 ("immutable_runtime", False), ("machine_path_unchanged", False),
                                 ("extracted_package_removed", False), ("archive", {"name": "other.zip", "sha256": "2" * 64})):
                bad = {**original, field: value}
                write_json(self.reports / name, bad)
                with self.subTest(name=name, field=field), self.assertRaises(RuntimeError):
                    self.aggregate()
            write_json(self.reports / name, original)

    def test_missing_or_duplicate_reports_fail(self):
        path = self.reports / "portable-runtime.json"
        path.unlink()
        with self.assertRaisesRegex(RuntimeError, "Missing or duplicate"):
            self.aggregate()
        write_json(path, {})
        write_json(self.reports / "duplicate" / path.name, {})
        with self.assertRaisesRegex(RuntimeError, "Missing or duplicate"):
            self.aggregate()

    def test_installer_requires_matching_payload_and_complete_acceptance(self):
        path = self.reports / "installer-runtime.json"
        original = gate.read(path)
        for update in ({"status": "skipped"}, {"cases": []}, {"source_commit": "2" * 40},
                       {"machine_path_unchanged": False}, {"installer": {"name": "Papo.exe", "sha256": "2" * 64}}):
            write_json(path, {**original, **update})
            with self.assertRaises(RuntimeError):
                self.aggregate()
        write_json(path, {})
        write_json(self.reports / "duplicate" / path.name, {})
        with self.assertRaisesRegex(RuntimeError, "Missing or duplicate"):
            self.aggregate()

    def test_empty_partial_duplicate_or_timed_out_cases_fail(self):
        path = self.reports / "media-ui-cairo.json"
        original = gate.read(path)
        cases = original["cases"]
        variants = [[], cases[:-1], cases + [cases[0]],
                    [{**c, "status": "skipped"} for c in cases],
                    [{**c, "timed_out": True} for c in cases],
                    [{**c, "exit_code": 1} for c in cases]]
        for variant in variants:
            write_json(path, {**original, "cases": variant})
            with self.assertRaises(RuntimeError):
                self.aggregate()

    def test_wrong_ui_source_renderer_and_missing_native_keys_fail(self):
        path = self.reports / "desktop-ui-auto.json"
        original = gate.read(path)
        for update in ({"renderer_request": "cairo"}, {"native_keys": False}, {"audio_output": "device"},
                       {"instrumented_ui": {"source_commit": COMMIT, "source_dirty": True}}):
            write_json(path, {**original, **update})
            with self.assertRaises(RuntimeError):
                self.aggregate()

    def test_mixed_instrumented_binaries_and_false_missing_component_pass_fail(self):
        self.mutate("media-ui-auto.json", lambda r: r["instrumented_ui"].update(sha256="2" * 64))
        with self.assertRaisesRegex(RuntimeError, "different instrumented"):
            self.aggregate()
        self.mutate("media-ui-auto.json", lambda r: r["instrumented_ui"].update(sha256="1" * 64))
        self.mutate("portable-runtime.json", lambda r: next(c for c in r["cases"] if c["name"] == "missing-python").update(exit_code=0))
        with self.assertRaisesRegex(RuntimeError, "contradicts"):
            self.aggregate()

    def test_powershell_bom_is_supported(self):
        path = self.reports / "portable-runtime.json"
        path.write_text(path.read_text(), encoding="utf-8-sig")
        self.assertEqual(self.aggregate()["automated_status"], "pass")

    def run_metadata(self):
        return {"id": 42, "run_attempt": 1, "repository": {"full_name": REPO},
                "head_repository": {"full_name": REPO}, "path": ".github/workflows/windows.yml",
                "event": "push", "status": "completed", "conclusion": "success", "head_sha": COMMIT}

    def test_rebuild_requires_authenticated_successful_same_revision_full_run(self):
        report, run = self.aggregate(), self.run_metadata()
        gate.authorize_rebuild(report, run, COMMIT, REPO)
        for key, value in (("path", ".github/workflows/other.yml"), ("head_sha", "2" * 40),
                           ("event", "pull_request"), ("conclusion", "failure"), ("status", "in_progress"),
                           ("id", 43), ("run_attempt", 2), ("head_repository", {"full_name": "fork/papo"})):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                gate.authorize_rebuild(report, {**run, key: value}, COMMIT, REPO)
        for key, value in (("mode", "packaging_only"), ("automated_status", "skipped"),
                           ("repository", "fork/papo"), ("jobs", {})):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                gate.authorize_rebuild({**report, key: value}, run, COMMIT, REPO)

    def test_rebuild_rejects_tool_dependency_lock_or_flag_drift(self):
        report = self.aggregate()
        gate_path, run_path = self.root / "gate.json", self.root / "run.json"
        write_json(gate_path, report)
        write_json(run_path, self.run_metadata())
        self.assertFalse(gate.check_rebuild(gate_path, run_path, self.info, REPO)["release_qualified"])
        for section, key, value in (("build", "cargo_lock_sha256", "2" * 64),
                                    ("build", "environment", {}), ("build", "profile", "debug"),
                                    ("toolchain", "rustc.exe", {"version": "new"}),
                                    ("native_inputs", "installed", {"rust": "2"}),
                                    ("native_inputs", "files", [{"file": "rust-1.pkg.tar.zst", "sha256": "2" * 64}])):
            bad = copy.deepcopy(self.info)
            bad[section][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(RuntimeError, "changed"):
                gate.check_rebuild(gate_path, run_path, bad, REPO)

    def test_repository_index_or_tool_path_change_does_not_replace_package_identity(self):
        other = copy.deepcopy(self.info)
        other["native_inputs"]["files"][1]["sha256"] = "2" * 64
        other["toolchain"]["rustc.exe"]["path"] = "D:/ucrt64/bin"
        self.assertEqual(gate.fingerprint(other), gate.fingerprint(self.info))

    def test_cli_writes_failure_evidence_and_returns_failure(self):
        jobs = self.root / "jobs.json"
        output = self.root / "gate.json"
        write_json(jobs, {})
        result = subprocess.run([sys.executable, str(ROOT / "packaging/windows/release_gate.py"),
                                 "--build-info", str(self.build_path), "--reports", str(self.reports),
                                 "--jobs", str(jobs), "--commit", COMMIT, "--repository", REPO,
                                 "--run-id", "42", "--attempt", "1", "--output", str(output)], capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(gate.read(output)["automated_status"], "fail")

    def test_build_rejects_packaging_only_without_evidence_before_compiling(self):
        result = subprocess.run([sys.executable, str(ROOT / "packaging/windows/build.py"),
                                 "--native-inputs", str(self.root), "--packaging-only"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn("requires --tested-gate", result.stderr)

    def test_python_runner_rejects_empty_discovery_and_unexpected_skips(self):
        fixtures = self.root / "python-fixtures"
        fixtures.mkdir()
        (fixtures / "fixture_test.py").write_text('import unittest\nclass Check(unittest.TestCase):\n'
                                                 ' @unittest.skip("intentional fixture")\n'
                                                 ' def test_skip(self): pass\n', encoding="utf-8")
        output = self.root / "python.json"
        command = [sys.executable, str(ROOT / "packaging/ci/python_checks.py"), "--directory", str(fixtures),
                   "--output", str(output)]
        for pattern in ("missing.py", "fixture_test.py"):
            result = subprocess.run([*command, "--pattern", pattern], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(gate.read(output)["status"], "fail")
        result = subprocess.run([*command, "--pattern", "fixture_test.py", "--allow-skip", "fixture_test.Check.test_skip"], capture_output=True)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(len(gate.read(output)["skipped"]), 1)


if __name__ == "__main__":
    unittest.main()
