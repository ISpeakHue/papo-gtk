"""EN-US: Beta exports retain only allowlisted numeric measurements.
PT-BR: Exportações beta preservam somente medições numéricas permitidas.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "packaging/windows"))
from beta_report import aggregates, compare


class WindowsBeta(unittest.TestCase):
    def test_unknown_fields_labels_and_raw_logs_cannot_leak(self):
        text = 'Authorization: Bearer SECRET\nCookie: Auth=SECRET\nPapo aggregate performance: ' + json.dumps({
            "request_us": {"samples": 2, "total": 30, "max": 20, "url": "SECRET"},
            "private_username": "SECRET", "message": "SECRET"})
        self.assertEqual(aggregates(text), {"request_us": {"samples": 2, "total": 30, "max": 20}})
        self.assertNotIn("SECRET", json.dumps(aggregates(text)))

    def test_strings_boolean_negative_huge_and_inconsistent_samples_are_rejected(self):
        for value in ("SECRET", True, -1, 2**64):
            text = 'Papo aggregate performance: ' + json.dumps({"request_us": {"samples": 1, "total": value, "max": 1}})
            with self.subTest(value=value), self.assertRaises(ValueError):
                aggregates(text)
        with self.assertRaises(ValueError):
            aggregates('Papo aggregate performance: {"request_us":{"samples":1,"total":1,"max":2}}')

    def test_oversized_missing_and_unknown_only_reports_are_rejected(self):
        for text in ("x" * (4 * 1024**2 + 1), "raw log", 'Papo aggregate performance: {"private": 1}'):
            with self.assertRaises(ValueError):
                aggregates(text)

    def test_cross_platform_comparisons_require_matching_conditions_and_copy_no_extra_fields(self):
        left = {"source_commit": "a" * 40, "platform": "windows", "scenario": "warm-channel", "renderer": "cairo",
                "scale": 100, "network": "normal", "keyboard": "pt-br", "audio": "none",
                "measurements": {"startup_ms": None, "working_set_start_bytes": 10,
                                 "working_set_end_bytes": 15, "working_set_peak_bytes": 20}}
        right = {**left, "platform": "linux", "private_text": "SECRET"}
        self.assertNotIn("SECRET", json.dumps(compare(left, right)))
        with self.assertRaises(ValueError):
            compare(left, {**right, "measurements": {**right["measurements"], "startup_ms": "SECRET"}})
        for key, value in (("source_commit", "b" * 40), ("scenario", "cold-channel"), ("renderer", "auto"), ("platform", "windows")):
            with self.subTest(key=key), self.assertRaises(ValueError):
                compare(left, {**right, key: value})

    def test_cli_requires_consent_before_creating_output(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "report.json"
            command = [sys.executable, str(ROOT / "packaging/windows/beta_report.py"), "--performance", "missing.log",
                       "--output", str(output), "--platform", "windows", "--source-commit", "a" * 40,
                       "--scenario", "startup", "--renderer", "auto", "--scale", "100", "--gpu", "unknown",
                       "--keyboard", "pt-br", "--audio", "none", "--network", "normal",
                       "--working-set-start-bytes", "1", "--working-set-end-bytes", "1", "--working-set-peak-bytes", "1"]
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("--consent", result.stderr)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
