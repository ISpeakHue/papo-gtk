"""EN-US: Real Git fixtures and native-input integrity checks, without a Windows runtime.
PT-BR: Fixtures Git reais e verificações dos pacotes, sem exigir um runtime Windows.
"""
import copy
import stat
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "packaging/windows"))
from build import source_state, require_source, sha256, write_json, packaged_environment, require_gstreamer, remove_stage
from native_inputs import verify_archive, package_set, local_packages, restore_gtk, seed_cache


class SourceProvenance(unittest.TestCase):
    def test_repeated_stage_cleanup_handles_copied_readonly_data(self):
        distribution = self.root / 'dist'
        stage = distribution / 'preview'
        data = stage / 'runtime/share/zoneinfo/iso3166.tab'
        data.parent.mkdir(parents=True)
        data.write_text('fixture', encoding='ascii')
        data.chmod(stat.S_IREAD)
        remove_stage(stage, distribution)
        self.assertFalse(stage.exists())
        self.assertTrue(distribution.is_dir())

    def test_stage_cleanup_refuses_parent_and_outside_directories(self):
        distribution = self.root / 'dist'
        distribution.mkdir()
        outside = self.root / 'keep'
        outside.mkdir()
        for candidate in (self.root, distribution, outside):
            with self.subTest(candidate=candidate), self.assertRaisesRegex(RuntimeError, 'cleanup'):
                remove_stage(candidate, distribution)
        self.assertTrue(outside.is_dir())

    def test_unfixed_gstreamer_cannot_be_packaged(self):
        for inventory in ('mingw-w64-ucrt-x86_64-gstreamer 1.28.8-1', '',
                          'mingw-w64-ucrt-x86_64-gstreamer 1.29.0-1'):
            with self.subTest(inventory=inventory), self.assertRaisesRegex(RuntimeError, 'Unicode-path'):
                require_gstreamer(inventory)
        require_gstreamer('mingw-w64-ucrt-x86_64-gstreamer 1.28.8-1.1\n')

    def test_packaged_check_cannot_inherit_developer_media_or_preview_overrides(self):
        environment = {'GST_PLUGIN_LOADING_WHITELIST': 'opus,vpx', 'GST_REGISTRY_1_0': 'sdk.bin',
                       'PAPO_WINDOWS_CAPTURE_PREVIEW': '1', 'PAPO_PYTHON': 'sdk/python.exe',
                       'PythonPath': 'sdk/modules', 'GI_TYPELIB_PATH': 'sdk/typelibs',
                       'GIO_EXTRA_MODULES': 'sdk/modules', 'GSETTINGS_SCHEMA_DIR': 'sdk/schemas',
                       'FONTCONFIG_FILE': 'sdk/fonts.conf', 'XDG_DATA_DIRS': 'sdk/share',
                       'GSK_RENDERER': 'cairo', 'GTK_A11Y': 'none', 'SystemRoot': 'C:/Windows'}
        self.assertEqual(packaged_environment(environment),
                         {'GSK_RENDERER': 'cairo', 'GTK_A11Y': 'none', 'SystemRoot': 'C:/Windows'})

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "core.autocrlf", "false")
        self.git("config", "core.filemode", "false")
        (self.root / "source.rs").write_bytes(b"fn main() {}\n")
        (self.root / ".gitignore").write_bytes(b"target/\ndist/\n")
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

    def test_cache_seed_uses_exact_identity_epoch_and_keeps_signature(self):
        signature = Path(str(self.path) + '.sig')
        signature.write_bytes(b'signature fixture')
        (self.root / 'example-2-1-any.pkg.tar.zst').write_bytes(b'newer unrelated version')
        with tempfile.TemporaryDirectory() as output, patch('native_inputs.msys_path', return_value='/cache/package'), \
                patch('native_inputs.run', return_value='example 1:1-1'):
            seed_cache(output, self.root, {'example': '1:1-1'})
            self.assertEqual({p.name for p in Path(output).iterdir()}, {self.path.name, signature.name})
            self.assertEqual((Path(output) / self.path.name).read_bytes(), self.path.read_bytes())

    def test_cache_seed_rejects_mislabeled_archives_and_excludes_local_backports(self):
        with tempfile.TemporaryDirectory() as output, patch('native_inputs.msys_path', return_value='/cache/package'), \
                patch('native_inputs.run', return_value='other-package 1-1') as query:
            with self.assertRaisesRegex(RuntimeError, 'identity mismatch'):
                seed_cache(output, self.root, {'example': '1-1'})
            self.assertFalse(list(Path(output).iterdir()))
            query.reset_mock()
            seed_cache(output, self.root, {'example': '1-1'}, {'example'})
            query.assert_not_called()

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

    def test_local_backport_must_match_installed_package_and_be_unique(self):
        with patch("native_inputs.msys_path", return_value="/fixture/package"), patch("native_inputs.run", return_value="example 1-1"):
            inventory = local_packages([self.path], {"example": "1-1"})
            self.assertEqual(inventory["example"]["archive"], self.path.name)
            with self.assertRaisesRegex(RuntimeError, "installed versions"):
                local_packages([self.path], {"example": "2-1"})
            with self.assertRaisesRegex(RuntimeError, "duplicated"):
                local_packages([self.path, self.path], {"example": "1-1"})

    def test_gtk_reuse_requires_exact_recipe_dependencies_and_archive_integrity(self):
        name = "mingw-w64-ucrt-x86_64-gtk4"
        self.manifest["installed"] = {name: "4.24.1-1.1", "dependency": "1-1"}
        self.manifest["locally_built"] = {name: {"version": "4.24.1-1.1", "archive": self.path.name}}
        recipes = ROOT / "packaging/windows/gtk"
        for source in recipes.iterdir():
            (self.root / source.name).write_bytes(source.read_bytes())
        (self.root / "gtk-4.24.1.tar.xz").write_bytes(b"synthetic source")
        def save_inputs():
            self.manifest["files"] = [{"file": path.name, "size": path.stat().st_size, "sha256": sha256(path)}
                                      for path in self.root.iterdir() if path.name != "NATIVE-INPUTS.json"]
            self.save()
        save_inputs()
        installed = {name: "4.24.1-1", "dependency": "1-1"}
        with tempfile.TemporaryDirectory() as output, patch("native_inputs.msys_path", return_value="/fixture/package"), patch("native_inputs.run", return_value=f"{name} 4.24.1-1.1"):
            self.assertTrue(restore_gtk(self.root, output, installed))
            self.assertEqual((Path(output) / self.path.name).read_bytes(), self.path.read_bytes())
            self.assertFalse(restore_gtk(self.root, output, {**installed, "dependency": "2-1"}))
            (self.root / "PKGBUILD").write_text("different recipe", encoding="utf-8")
            save_inputs()
            self.assertFalse(restore_gtk(self.root, output, installed))
            self.path.write_bytes(b"corrupted")
            with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
                restore_gtk(self.root, output, installed)


if __name__ == "__main__":
    unittest.main()
