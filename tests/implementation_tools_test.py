"""EN-US: Reject incomplete baselines and stale backend source reviews.
PT-BR: Rejeita baselines incompletas e revisões desatualizadas do backend.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


def module(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / f"{name}.py")
    loaded = importlib.util.module_from_spec(spec); spec.loader.exec_module(loaded)
    return loaded


baseline = module("performance-baseline")
contracts = module("check-backend-contracts")
translations = module("check-translations")


class BaselineTests(unittest.TestCase):
    def output(self, warm_requests=0):
        samples = [{"scenario": name, "elapsed_us": 100, "details": {}} for name in sorted(baseline.SCENARIOS)]
        next(s for s in samples if s["scenario"] == "warm_channel")["details"]["additional_media_requests"] = warm_requests
        return "\n".join("Papo baseline: " + json.dumps(s) for s in samples)

    def test_complete_output_and_aggregated_median(self):
        samples = baseline.parse_samples(self.output())
        report = baseline.summarize([{"samples": samples}, {"samples": [{"scenario": "warm_channel", "elapsed_us": 300}]}])
        self.assertEqual(report["warm_channel"]["median_us"], 200)
        self.assertEqual(report["warm_channel"]["samples"], 2)

    def test_zero_tests_or_missing_scenarios_are_not_success(self):
        for text in ["running 0 tests\ntest result: ok", 'Papo baseline: {"scenario":"startup_ui_ready"}']:
            with self.assertRaises(ValueError): baseline.parse_samples(text)

    def test_repeat_media_downloads_are_not_success(self):
        with self.assertRaises(ValueError): baseline.parse_samples(self.output(warm_requests=1))

    def test_malformed_json_is_not_success(self):
        with self.assertRaises(ValueError): baseline.parse_samples("Papo baseline: invalid")


class ContractReviewTests(unittest.TestCase):
    def test_source_changes_and_missing_files_require_review(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); path = root / "routes.go"; path.write_bytes(b"reviewed source")
            review = {"files": {"routes.go": hashlib.sha256(path.read_bytes()).hexdigest()}}
            self.assertEqual(contracts.verify(root, review), [])
            path.write_bytes(b"changed behavior")
            self.assertEqual(contracts.verify(root, review)[0]["file"], "routes.go")
            path.unlink()
            self.assertIsNone(contracts.verify(root, review)[0]["current_sha256"])

    def test_pinned_review_does_not_claim_future_contracts_supported(self):
        review = json.loads((ROOT / "docs/backend/ux-contracts.json").read_text())
        self.assertEqual(review["contracts"]["human_message_idempotency"], "missing")
        self.assertEqual(review["contracts"]["webhook_message_idempotency"], "present")
        self.assertNotEqual(review["contracts"]["media_history"], "present")


class TranslationTests(unittest.TestCase):
    def test_translation_cannot_drop_a_placeholder_or_plural_form(self):
        for target in ["Hello", {"one": "{count} item"}]:
            source = {"one": "{count} item", "other": "{count} items"} if isinstance(target, dict) else "Hello {name}"
            self.assertTrue(translations.validate([{"key": source}, {"key": target}]))

    def test_missing_keys_and_invalid_values_fail(self):
        for target in [{}, {"key": ""}, {"key": 12}]:
            self.assertTrue(translations.validate([{"key": "Texto"}, target]))

    def test_matching_placeholders_can_change_order(self):
        self.assertEqual(translations.validate([{"key": "{name}: {count}"}, {"key": "{count} for {name}"}]), [])



class PerformanceBudgetTests(unittest.TestCase):
    def setUp(self):
        import copy
        self.checker = module('check-performance-budgets')
        self.reference = json.loads((ROOT / 'docs/testing/evidence/performance-2026-11-04/baseline.json').read_text())
        self.report = copy.deepcopy(self.reference)
        self.budgets = json.loads((ROOT / 'docs/testing/performance-budgets.json').read_text())

    def test_reference_passes_and_minor_timing_variation_is_allowed(self):
        for run in self.report['runs']:
            for sample in run['samples']:
                if 'elapsed_us' in sample: sample['elapsed_us'] *= 1.1
        self.assertEqual(self.checker.evaluate(self.report, self.reference, self.budgets)['status'], 'pass')

    def test_large_regression_and_warm_requests_fail(self):
        for run in self.report['runs']:
            for sample in run['samples']:
                if sample['scenario'] == 'warm_channel':
                    sample['elapsed_us'] = 2000000
                    sample['details']['additional_media_requests'] = 1
        result = self.checker.evaluate(self.report, self.reference, self.budgets)
        self.assertEqual(result['status'], 'fail')
        self.assertTrue(any('warm_channel' in item for item in result['violations']))

    def test_zero_tests_missing_checkpoints_and_nan_are_rejected(self):
        import copy
        for mode in ('empty', 'missing', 'nan'):
            report = copy.deepcopy(self.report)
            if mode == 'empty': report['runs'] = []
            elif mode == 'missing': report['runs'][0]['samples'] = [s for s in report['runs'][0]['samples'] if s['scenario'] != 'scroll_frames']
            else: report['runs'][0]['samples'][0]['elapsed_us'] = float('nan')
            with self.assertRaises(ValueError): self.checker.evaluate(report, self.reference, self.budgets)

    def test_structural_checks_cover_cache_rows_and_memory_without_linux_timings(self):
        self.report['profile'] = 'debug'
        self.report['platform'] = 'Windows-test'
        result = self.checker.evaluate(self.report, self.reference, self.budgets, structural_only=True)
        self.assertEqual(result['status'], 'pass')
        self.assertFalse(result['timing_checked'])
        for sample in self.report['runs'][0]['samples']:
            if sample['scenario'] == 'history_memory_checkpoint': sample['details']['mounted_rows'] = 999
            if sample['scenario'] == 'media_memory_checkpoint': sample['details']['resident_cache_bytes'] = 1 << 30
        self.assertEqual(self.checker.evaluate(self.report, self.reference, self.budgets, True)['status'], 'fail')
        with self.assertRaises(ValueError): self.checker.evaluate(self.report, self.reference, self.budgets)


class ReleaseAcceptanceTests(unittest.TestCase):
    def test_template_and_stale_evidence_never_claim_release_acceptance(self):
        checker=module('check-release-acceptance')
        template=json.loads((ROOT/'docs/testing/release-acceptance-template.json').read_text())
        self.assertEqual(checker.evaluate(template,'test-source')['status'],'pending')
        receipt={'source_fingerprint':'test-source','platforms':{}}
        for platform in checker.PLATFORMS:
            receipt['platforms'][platform]={'tested_by':'Synthetic operator','tested_at':'2026-11-05','environment':'Synthetic fixture',
                'artifact_sha256':'a'*64,'checks':{c:{'status':'pass','evidence':'Synthetic test evidence'} for c in checker.CHECKS}}
        self.assertEqual(checker.evaluate(receipt,'test-source')['status'],'pass')
        self.assertEqual(checker.evaluate(receipt,'different-source')['status'],'pending')
        receipt['platforms']['windows-installer']['checks']['upgrade_and_rollback']['status']='pending'
        self.assertEqual(checker.evaluate(receipt,'test-source')['status'],'pending')


    def test_successful_source_bound_windows_ci_can_replace_windows_manual_rows(self):
        checker=module('check-release-acceptance')
        receipt={'source_fingerprint':'test-source','source_commit':'b'*40,'platforms':{}}
        ci={'id':123,'name':'Windows release checks','conclusion':'success','head_sha':'b'*40,
            'repository':{'full_name':'ISpeakHue/papo-gtk'},'html_url':'https://github.com/ISpeakHue/papo-gtk/actions/runs/123'}
        result=checker.evaluate(receipt,'test-source',ci)
        self.assertEqual(len(result['automated_acceptance']),2)
        self.assertTrue(all('windows-' not in item for item in result['missing']))
        ci['head_sha']='c'*40
        self.assertFalse(checker.evaluate(receipt,'test-source',ci)['automated_acceptance'])


if __name__ == "__main__": unittest.main()
