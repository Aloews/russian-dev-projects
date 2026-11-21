import copy
import unittest
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from validate_completion_evidence import validate

SHA = "a" * 40
BASE = {
    "candidate_sha": SHA,
    "tested_sha": SHA,
    "changed_paths": ["src/service.py"],
    "required_check_ids": ["tests"],
    "checks": [{"id": "tests", "executed": True, "exit_code": 0, "tests_discovered": 8, "candidate_sha": SHA}],
    "api": {"route_exists": True, "route_test_passed": True},
    "database": {"write_attempted": True, "read_after_write": True, "readback_artifact": "artifacts/db-readback.json"},
    "ui": {"data_source": "live_api", "api_available": True, "browser_test_passed": True},
    "deployment": {"state": "READY", "deployed_sha": SHA, "smoke_test_passed": True},
    "children": [{"id": "child-1", "state": "USER_JOURNEY_VERIFIED"}],
    "retry": {"artifact_present": True, "provider_acceptance_known": True, "idempotency_key": None},
}

class AdversarialCompletionTests(unittest.TestCase):
    def rejected(self, mutate, changed=None):
        manifest = copy.deepcopy(BASE)
        mutate(manifest)
        self.assertTrue(validate(manifest, SHA, changed if changed is not None else ["src/service.py"]))

    def test_01_implementation_claim_without_changed_files(self):
        self.assertTrue(validate(copy.deepcopy(BASE), SHA, []))

    def test_02_tests_claimed_but_command_not_executed(self):
        self.rejected(lambda m: m["checks"][0].update(executed=False))

    def test_03_test_command_discovers_zero_tests(self):
        self.rejected(lambda m: m["checks"][0].update(tests_discovered=0))

    def test_04_unit_tests_pass_but_required_api_route_missing(self):
        self.rejected(lambda m: m["api"].update(route_exists=False))

    def test_05_api_success_but_database_write_not_persisted(self):
        self.rejected(lambda m: m["database"].update(read_after_write=False))

    def test_06_ui_uses_mock_data_or_api_is_unavailable(self):
        self.rejected(lambda m: m["ui"].update(data_source="mock"))

    def test_07_deployment_not_ready_or_wrong_sha(self):
        self.rejected(lambda m: m["deployment"].update(state="BUILDING"))

    def test_08_candidate_sha_changes_after_tests(self):
        self.rejected(lambda m: m.update(tested_sha="b" * 40))

    def test_09_parent_claims_complete_with_failed_child(self):
        self.rejected(lambda m: m["children"][0].update(state="FAILED"))

    def test_10_retry_has_no_artifact_or_ambiguous_provider_acceptance(self):
        self.rejected(lambda m: m["retry"].update(artifact_present=False))

    def test_10b_retry_requires_idempotency_for_ambiguous_acceptance(self):
        self.rejected(lambda m: m["retry"].update(idempotency_key="retry-1"))

    def test_complete_evidence_is_accepted(self):
        self.assertEqual(validate(copy.deepcopy(BASE), SHA, ["src/service.py"]), [])

    def test_manifest_diff_must_match_actual_diff(self):
        self.assertTrue(validate(copy.deepcopy(BASE), SHA, ["src/other.py"]))

if __name__ == "__main__":
    unittest.main()
