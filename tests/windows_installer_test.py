"""EN-US: Validate exact installer payloads and hostile paths using synthetic packages only.
PT-BR: Valida conteúdos exatos e caminhos hostis com pacotes exclusivamente sintéticos.
"""
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "packaging/windows"))
import installer
from build import sha256, write_json


def fixture(root):
    stage = root / "Papo-fixture"
    for name in ("Papo.exe", "Papo.com", "LICENSE", "runtime/bin/papo-gtk.exe",
                 "runtime/bin/papo-shell.exe", "runtime/bin/python.exe", "runtime/RUNTIME-MANIFEST.json"):
        path = stage / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"synthetic compilation fixture; not a runnable runtime")
    info = {"schema": 1, "version": "1.0.2", "source": {"commit": "a" * 40, "status": "", "untracked_sha256": {}},
            "source_dirty": False, "build": {"allow_dirty": False},
            "binary_sha256": sha256(stage / "runtime/bin/papo-gtk.exe"),
            "runtime": {"manifest_sha256": sha256(stage / "runtime/RUNTIME-MANIFEST.json")}}
    write_json(stage / "BUILD-INFO.json", info)
    archive = Path(shutil.make_archive(str(stage), "zip", root, stage.name))
    archive.with_suffix(".zip.sha256").write_text(f"{sha256(archive)}  {archive.name}\n")
    return stage, archive


class WindowsInstaller(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.stage, self.archive = fixture(self.root)

    def rewrite_archive(self, mutate=None):
        self.archive.unlink()
        with zipfile.ZipFile(self.archive, "w") as bundle:
            for path in self.stage.rglob("*"):
                if path.is_file():
                    bundle.write(path, str(path.relative_to(self.root)))
            if mutate:
                mutate(bundle)
        self.archive.with_suffix(".zip.sha256").write_text(sha256(self.archive))

    def test_inventory_and_generated_removal_match_the_zip_exactly(self):
        _, files = installer.prepare(self.stage, self.archive, self.root / "recipe", self.root / "setup.exe")
        text = (self.root / "recipe/payload.nsh").read_text()
        self.assertEqual(len(files), 8)
        self.assertEqual(text.count('\nFile "'), 8)
        self.assertEqual(text.count('\nDelete "'), 8)
        self.assertNotIn("RMDir /r", text)
        self.assertNotIn("APPDATA", text)
        self.assertIn("CreateFileW", text)
        self.assertIn("GetFileAttributesW", text)

    def test_changed_or_unrecorded_staged_files_reject_packaging(self):
        original = (self.stage / "Papo.exe").read_bytes()
        for name in ("Papo.exe", "extra.txt"):
            (self.stage / name).write_bytes(b"changed")
            with self.assertRaises(ValueError):
                installer.inventory(self.stage, self.archive)
            (self.stage / "Papo.exe").write_bytes(original)

    def test_changed_zip_checksum_is_rejected(self):
        self.archive.with_suffix(".zip.sha256").write_text("0" * 64)
        with self.assertRaisesRegex(ValueError, "checksum"):
            installer.inventory(self.stage, self.archive)

    def test_hostile_windows_names_are_rejected(self):
        for name in ("../outside", "/absolute", "C:outside", "bad\\path", "name:stream", "file.",
                     "folder/ ", "NUL.txt", "com1.log", "dir/../file", "dir//file", "a\nfile", "a" * 221):
            with self.subTest(name=name), self.assertRaises(ValueError):
                installer.safe_name(name)

    def test_archive_root_duplicate_and_case_collision_are_rejected(self):
        for name in ("../outside", "other-root/Papo.exe", "Papo-fixture/papo.exe"):
            self.rewrite_archive(lambda z: z.writestr(name, b"bad"))
            with self.subTest(name=name), self.assertRaises(ValueError):
                installer.inventory(self.stage, self.archive)

    def test_installer_metadata_cannot_be_supplied_by_the_payload(self):
        for name in ("Uninstall.exe", "install.INI"):
            (self.stage / name).write_bytes(b"collision")
            self.rewrite_archive()
            with self.assertRaisesRegex(ValueError, "metadata"):
                installer.inventory(self.stage, self.archive)
            (self.stage / name).unlink()

    def test_dirty_and_mismatched_payload_identity_are_rejected(self):
        info_path = self.stage / "BUILD-INFO.json"
        original = json.loads(info_path.read_text())
        for field, value in (("source_dirty", True), ("binary_sha256", "0" * 64), ("version", "1.0.2;exec")):
            write_json(info_path, {**original, field: value})
            self.rewrite_archive()
            with self.subTest(field=field), self.assertRaises(ValueError):
                installer.inventory(self.stage, self.archive)

    def test_dollar_signs_remain_literal_in_generated_payload_paths(self):
        (self.stage / "$PLUGINSDIR.txt").write_bytes(b"literal payload name")
        self.rewrite_archive()
        installer.prepare(self.stage, self.archive, self.root / "recipe", self.root / "setup.exe")
        text = (self.root / "recipe/payload.nsh").read_text()
        self.assertIn('Delete "$INSTDIR\\$$PLUGINSDIR.txt"', text)


if __name__ == "__main__":
    unittest.main()
