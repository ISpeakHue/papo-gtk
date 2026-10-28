"""EN-US: Release policy and signing invariants use synthetic files, never credentials or GitHub writes.
PT-BR: Política de release e invariantes usam arquivos sintéticos, nunca credenciais nem gravações no GitHub.
"""
import copy
import datetime as dt
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "packaging/windows"))
from authenticode import fields, signing_only
from build import sha256, write_json
import publish as publication
import release
from windows_bundle_test import pe_image


class SigningInvariant(unittest.TestCase):
    def signed(self, data):
        data = bytearray(data)
        checksum, security = fields(data)
        offset = (len(data) + 7) // 8 * 8
        data += b"\0" * (offset - len(data)) + b"synthetic cert!!"
        struct.pack_into("<I", data, checksum, 12345)
        struct.pack_into("<II", data, security, offset, len(data) - offset)
        return data

    def test_appended_signature_changes_only_expected_header_fields(self):
        before = pe_image([])
        signing_only(before, self.signed(before))
        before += b"overlay"
        signing_only(before, self.signed(before))

    def test_nsis_pe32_and_application_pe32plus_are_supported(self):
        before = pe_image([])
        struct.pack_into("<H", before, 152, 0x10b)
        struct.pack_into("<I", before, 152 + 92, 16)
        signing_only(before, self.signed(before))

    def test_code_mutation_nonzero_padding_and_overlays_are_rejected(self):
        before = pe_image([]) + b"x"
        signed = self.signed(before)
        for offset in (512, len(before)):
            bad = bytearray(signed)
            bad[offset] ^= 1
            with self.assertRaises(ValueError):
                signing_only(before, bad)
        with self.assertRaises(ValueError):
            signing_only(before, signed + b"extra")

    def test_truncation_missing_or_overlapping_security_directory_is_rejected(self):
        before = pe_image([])
        for malformed in (b"not PE", before[:70], before[:160]):
            with self.assertRaises(ValueError):
                fields(malformed)
        after = self.signed(before)
        _, security = fields(before)
        struct.pack_into("<II", after, security, 512, len(after) - 512)
        with self.assertRaises(ValueError):
            signing_only(before, after)

    def test_already_signed_input_and_missing_certificate_are_rejected(self):
        before = pe_image([])
        signed = self.signed(before)
        with self.assertRaises(ValueError):
            signing_only(signed, self.signed(signed))
        with self.assertRaises(ValueError):
            signing_only(before, before)


class DesktopQualification(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.candidate = {"source_commit": "a" * 40, "archive": {"sha256": "b" * 64}, "installer": {"sha256": "c" * 64}}
        release.acceptance_template(SimpleNamespace(candidate=None, output=self.root / "template.json"))
        self.record = release.read(self.root / "template.json")
        self.file = self.root / "executed.log"
        self.file.write_text("Synthetic acceptance fixture; no native checks executed.\n")
        self.row = {"status": "pass", "evidence": [{"file": self.file.name, "sha256": sha256(self.file)}]}

    def completed(self):
        row = copy.deepcopy(self.record)
        row.update(source_commit=self.candidate["source_commit"], archive_sha256="b" * 64, installer_sha256="c" * 64,
                   backend_revision="d" * 40, blockers=[], servicing_reviewed_utc=dt.datetime.now(dt.timezone.utc).isoformat())
        for target in row["targets"].values():
            target.update(native=True, standard_user=True, kb="KB1234567")
            target["features"] = {feature: copy.deepcopy(self.row) for feature in release.FEATURES}
        row["device_success_paths"] = {name: copy.deepcopy(self.row) for name in release.DEVICES}
        row["beta"] = {**copy.deepcopy(self.row), "blockers": []}
        return row

    def test_template_is_explicitly_unqualified_and_never_overwrites_evidence(self):
        self.assertEqual(len(self.record["targets"]), 4)
        self.assertTrue(all(feature["status"] == "not_run" for row in self.record["targets"].values() for feature in row["features"].values()))
        with self.assertRaises(RuntimeError):
            release.desktop_acceptance(self.record, self.candidate, self.root)
        with self.assertRaises(RuntimeError):
            release.acceptance_template(SimpleNamespace(candidate=None, output=self.root / "template.json"))

    def test_complete_bound_fixture_is_accepted(self):
        release.desktop_acceptance(self.completed(), self.candidate, self.root)

    def test_other_source_artifact_or_backend_is_rejected(self):
        for name, value in (("source_commit", "e" * 40), ("archive_sha256", "e" * 64),
                            ("installer_sha256", "e" * 64), ("backend_revision", "unknown"), ("blockers", ["open"])):
            row = self.completed()
            row[name] = value
            with self.subTest(name=name), self.assertRaises(RuntimeError):
                release.desktop_acceptance(row, self.candidate, self.root)

    def test_native_standard_user_edition_architecture_and_build_are_required(self):
        for name, value in (("native", False), ("standard_user", False), ("edition", "Server"),
                            ("architecture", "arm64"), ("family", "26H1"), ("build", "10.0.26200.1"), ("kb", "")):
            row = self.completed()
            row["targets"]["W25H"][name] = value
            with self.subTest(name=name), self.assertRaises(RuntimeError):
                release.desktop_acceptance(row, self.candidate, self.root)

    def test_all_rows_features_device_paths_beta_and_fresh_servicing_are_required(self):
        for mutate in (lambda r: r["targets"].pop("W26P"), lambda r: r["targets"]["W25H"]["features"].pop("W17"),
                       lambda r: r["targets"]["W25H"]["features"]["W23"].update(status="skipped"),
                       lambda r: r["device_success_paths"].pop("camera"), lambda r: r["beta"].update(blockers=["crash"]),
                       lambda r: r.update(servicing_reviewed_utc=None),
                       lambda r: r.update(servicing_reviewed_utc=(dt.datetime.now(dt.timezone.utc) - dt.timedelta(days=31)).isoformat())):
            row = self.completed()
            mutate(row)
            with self.assertRaises(RuntimeError):
                release.desktop_acceptance(row, self.candidate, self.root)

    def test_missing_altered_or_escaping_evidence_is_rejected(self):
        for name in ("missing.log", "../outside.log", "C:outside", "/absolute.log"):
            row = self.completed()
            row["beta"]["evidence"][0]["file"] = name
            with self.assertRaises((RuntimeError, ValueError)):
                release.desktop_acceptance(row, self.candidate, self.root)
        self.file.write_text("changed")
        with self.assertRaises(RuntimeError):
            release.desktop_acceptance(self.completed(), self.candidate, self.root)


class Publication(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.candidate_dir = self.root / "candidate"
        self.candidate_dir.mkdir()
        self.candidate = {"schema": 1, "version": "1.1.0", "repository": "fixture/papo", "source_commit": "a" * 40,
                          "signer_certificate_sha256": "f" * 64}
        package = self.candidate_dir / "Papo.zip"
        with zipfile.ZipFile(package, "w") as bundle:
            bundle.writestr("Papo/THIRD-PARTY-NOTICES.txt", "synthetic notices")
            bundle.writestr("Papo/runtime/share/licenses/fixture/LICENSE", "synthetic runtime license")
        for kind, name in (("archive", "Papo.zip"), ("installer", "Papo.exe"), ("source", "source.tar.gz")):
            path = self.candidate_dir / name
            if not path.exists():
                path.write_bytes(b"synthetic fixture, not runnable")
            self.candidate[kind] = release.artifact(path)
        write_json(self.candidate_dir / "candidate.json", self.candidate)
        (self.candidate_dir / "RELEASE-NOTES.md").write_text("Qualified fixture notes")
        self.qualification = self.root / "qualification.json"
        self.record = {"schema": 1, "status": "pass", "release_qualified": True, "source_commit": "a" * 40,
                       "version": "1.1.0", "repository": "fixture/papo", "signer_certificate_sha256": "f" * 64,
                       "candidate_sha256": sha256(self.candidate_dir / "candidate.json"),
                       "release_notes_sha256": sha256(self.candidate_dir / "RELEASE-NOTES.md"),
                       "qualified_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
                       "artifacts": {key: self.candidate[key] for key in ("archive", "installer", "source")}}
        write_json(self.qualification, self.record)
        self.output = self.root / "publish"
        self.source = {"commit": "a" * 40, "status": ""}

    def stage(self):
        with patch.object(publication, "source_state", return_value=self.source):
            return publication.stage(self.candidate_dir, self.qualification, self.output)

    def test_local_staging_contains_checksums_exact_source_and_dependency_licenses(self):
        with patch.object(publication.subprocess, "run") as mutation:
            self.stage()
            mutation.assert_not_called()
        self.assertTrue((self.output / "source.tar.gz").is_file())
        self.assertIn("Papo.exe", (self.output / "SHA256SUMS").read_text())
        self.assertTrue((self.output / "Papo-1.1.0-windows-notices.tar.gz").is_file())

    def test_pending_failed_and_stale_qualification_is_never_publishable(self):
        for update in ({"status": "fail"}, {"release_qualified": False},
                       {"qualified_utc": (dt.datetime.now(dt.timezone.utc) - dt.timedelta(days=8)).isoformat()}):
            write_json(self.qualification, {**self.record, **update})
            with self.assertRaises(RuntimeError):
                self.stage()
        self.assertFalse(self.output.exists())

    def test_modified_candidate_installer_notes_dirty_or_other_revision_is_rejected(self):
        for source in ({"commit": "a" * 40, "status": " M source.rs"}, {"commit": "b" * 40, "status": ""}):
            self.source = source
            with self.assertRaises(RuntimeError):
                self.stage()
        self.source = {"commit": "a" * 40, "status": ""}
        (self.candidate_dir / "Papo.exe").write_bytes(b"modified")
        with self.assertRaises(RuntimeError):
            self.stage()

    def test_wrong_remote_tag_and_existing_release_never_create_or_edit_a_release(self):
        self.stage()
        with patch.object(publication.subprocess, "check_output", return_value=json.dumps({"object": {"type": "commit", "sha": "b" * 40}})), \
             patch.object(publication.subprocess, "run") as mutation:
            with self.assertRaises(RuntimeError):
                publication.publish(self.output, self.candidate)
            mutation.assert_not_called()
        with patch.object(publication.subprocess, "check_output", return_value=json.dumps({"object": {"type": "commit", "sha": "a" * 40}})), \
             patch.object(publication.subprocess, "run", return_value=SimpleNamespace(returncode=0, stderr="")) as mutation:
            with self.assertRaises(RuntimeError):
                publication.publish(self.output, self.candidate)
            self.assertEqual(mutation.call_args.args[0][1], "api")

    def test_corrupt_uploaded_digest_leaves_release_as_a_draft(self):
        self.stage()
        assets = [{"name": file.name, "digest": "sha256:" + "0" * 64} for file in self.output.iterdir() if file.name != "publication.json"]
        responses = [json.dumps({"object": {"type": "commit", "sha": "a" * 40}}), json.dumps({"draft": True, "assets": assets})]
        with patch.object(publication.subprocess, "check_output", side_effect=responses), \
             patch.object(publication.subprocess, "run", side_effect=[SimpleNamespace(returncode=1, stderr="HTTP 404"), SimpleNamespace(returncode=0)]) as mutation:
            with self.assertRaisesRegex(RuntimeError, "digest"):
                publication.publish(self.output, self.candidate)
            self.assertIn("--draft", mutation.call_args.args[0])
            self.assertFalse(any(call.args[0][1:3] == ["release", "edit"] for call in mutation.call_args_list))

    def test_matching_uploaded_digests_promote_only_the_verified_draft(self):
        self.stage()
        assets = [{"name": file.name, "digest": "sha256:" + sha256(file)}
                  for file in self.output.iterdir() if file.name != "publication.json"]
        responses = [json.dumps({"object": {"type": "tag", "sha": "e" * 40}}),
                     json.dumps({"object": {"type": "commit", "sha": "a" * 40}}),
                     json.dumps({"draft": True, "assets": assets})]
        with patch.object(publication.subprocess, "check_output", side_effect=responses), \
             patch.object(publication.subprocess, "run", side_effect=[SimpleNamespace(returncode=1, stderr="HTTP 404"),
                                                                      SimpleNamespace(returncode=0), SimpleNamespace(returncode=0)]) as mutation:
            publication.publish(self.output, self.candidate)
            commands = [call.args[0] for call in mutation.call_args_list]
            self.assertEqual(commands[1][1:3], ["release", "create"])
            self.assertIn("--draft", commands[1])
            self.assertIn("--verify-tag", commands[1])
            self.assertEqual(commands[2], ["gh", "release", "edit", "v1.1.0", "--repo", "fixture/papo", "--draft=false"])

    def test_tampered_staging_never_uploads_assets(self):
        self.stage()
        for tamper in (lambda: (self.output / "Papo.exe").write_bytes(b"modified"),
                       lambda: (self.output / "private-extra.txt").write_text("not an approved asset")):
            tamper()
            with patch.object(publication.subprocess, "check_output", return_value=json.dumps({"object": {"type": "commit", "sha": "a" * 40}})), \
                 patch.object(publication.subprocess, "run", return_value=SimpleNamespace(returncode=1, stderr="HTTP 404")) as mutation:
                with self.assertRaises(RuntimeError):
                    publication.publish(self.output, self.candidate)
                self.assertEqual(len(mutation.call_args_list), 1)
                self.assertEqual(mutation.call_args.args[0][1], "api")


if __name__ == "__main__":
    unittest.main()
