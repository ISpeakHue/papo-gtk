"""EN-US: Real Git fixtures and native-input integrity checks, without a Windows runtime.
PT-BR: Fixtures Git reais e verificações dos pacotes, sem exigir um runtime Windows.
"""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "packaging/windows"))
from build import source_state, require_source, sha256, write_json
from native_inputs import verify_archive, package_set


class SourceProvenance(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "core.autocrlf", "false")
        self.git("config", "core.filemode", "false")
        (self.root / "source.rs").write_text("fn main() {}\n", encoding="utf-8")
        (self.root / ".gitignore").write_text("target/\ndist/\n", encoding="utf-8")
        self.commit()

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, text=True).strip()

    def commit(self):
        self.git("add", ".")
        self.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                 "-c", "commit.gpgsign=false", "commit", "-qm", "fixture")

    def test_clean_checkout_and_ignored_outputs_remain_clean(self):
        before = source_state(self.root)
        self.assertEqual(before["commit"], self.git("rev-parse", "HEAD"))
        self.assertEqual(before["status"], "")
        (self.root / "target").mkdir()
        (self.root / "target/generated.exe").write_bytes(b"binary")
        require_source(before, source_state(self.root))

    def test_tracked_staged_and_untracked_changes_reject_by_default(self):
        for staged in (False, True):
            (self.root / "source.rs").write_text("// changed\n", encoding="utf-8")
            if staged:
                self.git("add", "source.rs")
            state = source_state(self.root)
            self.assertTrue(state["diff_stat"])
            with self.assertRaisesRegex(RuntimeError, "dirty"):
                require_source(state)
        self.commit()
        (self.root / "new.rs").write_text("// new\n", encoding="utf-8")
        state = source_state(self.root)
        self.assertIn("new.rs", state["untracked_sha256"])
        with self.assertRaisesRegex(RuntimeError, "dirty"):
            require_source(state)

    def test_explicit_dirty_preview_still_rejects_changes_during_build(self):
        source = self.root / "source.rs"
        source.write_text("// first\n", encoding="utf-8")
        before = source_state(self.root)
        require_source(before, allow_dirty=True)
        source.write_text("// other\n", encoding="utf-8")
        after = source_state(self.root)
        self.assertEqual(before["diff_numstat"], after["diff_numstat"])
        with self.assertRaisesRegex(RuntimeError, "during compilation"):
            require_source(before, after, allow_dirty=True)

    def test_eol_policy_is_independent_of_git_autocrlf_defaults(self):
        (self.root / ".gitattributes").write_bytes((ROOT / ".gitattributes").read_bytes())
        self.commit()
        # EN-US: Simulate checkout by Git for Windows, then inspection by MSYS2 Git.
        # PT-BR: Simula checkout pelo Git for Windows e inspeção pelo Git do MSYS2.
        self.git("config", "core.autocrlf", "true")
        (self.root / "source.rs").unlink()
        self.git("checkout", "--", "source.rs")
        self.assertNotIn(b"\r\n", (self.root / "source.rs").read_bytes())
        self.git("config", "core.autocrlf", "false")
        require_source(source_state(self.root))

    def test_untracked_same_size_mutation_is_detected(self):
        path = self.root / "new.rs"
        path.write_bytes(b"first")
        before = source_state(self.root)
        path.write_bytes(b"other")
        with self.assertRaisesRegex(RuntimeError, "during compilation"):
            require_source(before, source_state(self.root), allow_dirty=True)


class NativeInputIntegrity(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.path = self.root / "example-1-1-any.pkg.tar.zst"
        self.path.write_bytes(b"package fixture")
        self.manifest = {"schema": 1, "installed": {"example": "1-1"},
                         "files": [{"file": self.path.name, "size": self.path.stat().st_size,
                                    "sha256": sha256(self.path)}]}
        self.save()

    def save(self):
        write_json(self.root / "NATIVE-INPUTS.json", self.manifest)

    def test_exact_inputs_match_installed_environment(self):
        verified = verify_archive(self.root, installed="example 1-1\n")
        self.assertEqual(verified["installed"], {"example": "1-1"})
        self.assertEqual(len(verified["manifest_sha256"]), 64)

    def test_package_corruption_and_version_drift_fail_closed(self):
        with self.assertRaisesRegex(RuntimeError, "versions differ"):
            verify_archive(self.root, installed="example 2-1\n")
        self.path.write_bytes(b"changed fixture")
        with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
            verify_archive(self.root)

    def test_missing_or_extra_input_is_rejected(self):
        (self.root / "unexpected.pkg.tar.zst").write_bytes(b"extra")
        with self.assertRaisesRegex(RuntimeError, "unrecorded"):
            verify_archive(self.root)
        (self.root / "unexpected.pkg.tar.zst").unlink()
        self.path.unlink()
        with self.assertRaises(FileNotFoundError):
            verify_archive(self.root)

    def test_unsafe_and_duplicate_names_are_rejected(self):
        for name in ("../escape", "..\\escape", "C:escape", self.path.name):
            with self.subTest(name=name):
                original = copy.deepcopy(self.manifest)
                self.manifest["files"].append({**self.manifest["files"][0], "file": name})
                self.save()
                with self.assertRaisesRegex(RuntimeError, "Unsafe or duplicate"):
                    verify_archive(self.root)
                self.manifest = original

    def test_duplicate_package_names_are_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "duplicate"):
            package_set("example 1-1\nexample 2-1\n")


if __name__ == "__main__":
    unittest.main()
