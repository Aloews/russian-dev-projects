import copy
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from validate_completion_evidence import validate

SHA = "a" * 40
BASE = {
    "candidate_sha": SHA,
    "tested_sha": SHA,
    "changed_paths": ["src/service.py"],
    "required_check_ids": ["tests"],
    "checks": [{"id": "tests", "executed": True, "exit_code": 0, "tests_discovered": 8, "tests_passed": 8, "tests_skipped": 0, "tests_failed": 0, "candidate_sha": SHA, "command": "pytest -q", "artifact": "artifacts/tests.log"}],
    "api": {"route_exists": True, "route_test_passed": True},
    "database": {"write_attempted": True, "read_after_write": True, "readback_artifact": "artifacts/db-readback.json"},
    "ui": {"data_source": "live_api", "api_available": True, "browser_test_passed": True},
    "deployment": {"state": "READY", "deployed_sha": SHA, "smoke_test_passed": True},
    "children": [{"id": "child-1", "state": "USER_JOURNEY_VERIFIED"}],
    "retry": {"artifact_present": True, "provider_acceptance_known": True, "idempotency_key": None},
}

class AdversarialCompletionTests(unittest.TestCase):
    def rejected(self, mutate, changed=None):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            manifest = copy.deepcopy(BASE)
            self.attach_integration_artifacts(manifest, workspace)
            mutate(manifest)
            self.assertTrue(validate(
                manifest,
                SHA,
                changed if changed is not None else ["src/service.py"],
                workspace=workspace,
            ))

    def test_01_implementation_claim_without_changed_files(self):
        self.assertTrue(validate(copy.deepcopy(BASE), SHA, []))

    def test_code_level_does_not_claim_runtime_integration(self):
        code = copy.deepcopy(BASE)
        code["api"] = {}
        code["database"] = {}
        code["ui"] = {}
        code["deployment"] = {}
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            artifact = workspace / "artifacts/tests.log"
            artifact.parent.mkdir()
            artifact.write_text("8 passed", encoding="utf-8")
            code["checks"][0]["artifact_sha256"] = hashlib.sha256(artifact.read_bytes()).hexdigest()
            self.assertEqual(validate(code, SHA, ["src/service.py"], "CODE_VERIFIED", workspace), [])

    def test_02_tests_claimed_but_command_not_executed(self):
        self.rejected(lambda m: m["checks"][0].update(executed=False))

    def test_03_test_command_discovers_zero_tests(self):
        self.rejected(lambda m: m["checks"][0].update(tests_discovered=0))

    def test_skipped_tests_cannot_be_marked_code_verified(self):
        self.rejected(lambda m: m["checks"][0].update(tests_skipped=1))

    def test_zero_passed_tests_cannot_be_marked_code_verified(self):
        self.rejected(lambda m: m["checks"][0].update(tests_passed=0))

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
        self.rejected(lambda m: m["retry"].update(provider_acceptance_known=False))

    def attach_integration_artifacts(self, manifest, workspace):
        test_artifact = workspace / manifest["checks"][0]["artifact"]
        test_artifact.parent.mkdir(parents=True, exist_ok=True)
        test_artifact.write_text("8 passed", encoding="utf-8")
        manifest["checks"][0]["artifact_sha256"] = hashlib.sha256(test_artifact.read_bytes()).hexdigest()
        records = [
            (manifest["api"], "artifact", "artifact_sha256", "api.json"),
            (manifest["database"], "readback_artifact", "readback_sha256", "db.json"),
            (manifest["ui"], "artifact", "artifact_sha256", "ui.json"),
            (manifest["deployment"], "artifact", "artifact_sha256", "deployment.json"),
            (manifest["retry"], "artifact", "artifact_sha256", "retry.json"),
        ]
        for record, path_key, hash_key, filename in records:
            path = workspace / filename
            path.write_text(filename, encoding="utf-8")
            record[path_key] = filename
            record[hash_key] = hashlib.sha256(path.read_bytes()).hexdigest()
        for index, child in enumerate(manifest["children"]):
            filename = f"child-{index}.json"
            path = workspace / filename
            path.write_text(filename, encoding="utf-8")
            child["artifact"] = filename
            child["artifact_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()

    def test_complete_evidence_is_accepted(self):
        manifest = copy.deepcopy(BASE)
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            self.attach_integration_artifacts(manifest, workspace)
            self.assertEqual(validate(manifest, SHA, ["src/service.py"], workspace=workspace), [])

    def test_database_readback_artifact_must_match_its_hash(self):
        manifest = copy.deepcopy(BASE)
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            self.attach_integration_artifacts(manifest, workspace)
            (workspace / manifest["database"]["readback_artifact"]).write_text("not a readback", encoding="utf-8")
            self.assertTrue(validate(manifest, SHA, ["src/service.py"], workspace=workspace))

    def test_api_boolean_claim_without_artifact_is_rejected(self):
        manifest = copy.deepcopy(BASE)
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            self.attach_integration_artifacts(manifest, workspace)
            manifest["api"].pop("artifact")
            self.assertTrue(validate(manifest, SHA, ["src/service.py"], workspace=workspace))

    def test_manifest_diff_must_match_actual_diff(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            manifest = copy.deepcopy(BASE)
            self.attach_integration_artifacts(manifest, workspace)
            self.assertTrue(validate(manifest, SHA, ["src/other.py"], workspace=workspace))

    def test_code_evidence_rejects_missing_artifact(self):
        manifest = copy.deepcopy(BASE)
        manifest["checks"][0]["artifact_sha256"] = "0" * 64
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            self.assertTrue(validate(manifest, SHA, ["src/service.py"], "CODE_VERIFIED", workspace))

    def test_code_evidence_verifies_artifact_bytes(self):
        manifest = copy.deepcopy(BASE)
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            artifact = workspace / "artifacts" / "tests.log"
            artifact.parent.mkdir()
            artifact.write_text("8 passed", encoding="utf-8")
            manifest["checks"][0]["artifact"] = "artifacts/tests.log"
            manifest["checks"][0]["artifact_sha256"] = hashlib.sha256(artifact.read_bytes()).hexdigest()
            self.assertEqual(validate(manifest, SHA, ["src/service.py"], "CODE_VERIFIED", workspace), [])
            artifact.write_text("fabricated", encoding="utf-8")
            self.assertTrue(validate(manifest, SHA, ["src/service.py"], "CODE_VERIFIED", workspace))


    def test_boolean_values_are_not_valid_exit_codes_or_test_counts(self):
        self.rejected(lambda m: m["checks"][0].update(exit_code=True))
        self.rejected(lambda m: m["checks"][0].update(tests_discovered=True))

    def test_evidence_symlink_cannot_escape_workspace(self):
        manifest = copy.deepcopy(BASE)
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            outside = workspace.parent / (workspace.name + "-outside-evidence")
            outside.write_text("8 passed", encoding="utf-8")
            try:
                link = workspace / "tests.log"
                link.symlink_to(outside)
                manifest["checks"][0]["artifact"] = "tests.log"
                manifest["checks"][0]["artifact_sha256"] = hashlib.sha256(outside.read_bytes()).hexdigest()
                self.assertTrue(validate(manifest, SHA, ["src/service.py"], "CODE_VERIFIED", workspace))
            finally:
                outside.unlink(missing_ok=True)

    def test_wrong_verification_level_is_rejected(self):
        self.assertTrue(validate(copy.deepcopy(BASE), SHA, ["src/service.py"], "MADE_UP"))

    def test_metadata_only_validation_is_rejected(self):
        manifest = copy.deepcopy(BASE)
        self.assertTrue(validate(manifest, SHA, ["src/service.py"], "CODE_VERIFIED"))

    def test_duplicate_required_check_ids_are_rejected(self):
        self.rejected(lambda m: m.update(required_check_ids=["tests", "tests"]))

    def test_duplicate_check_records_are_rejected(self):
        self.rejected(lambda m: m["checks"].append(copy.deepcopy(m["checks"][0])))

    def test_required_tests_check_must_have_machine_counted_totals(self):
        self.rejected(lambda m: m["checks"][0].pop("tests_discovered"))

    def test_malformed_check_ids_are_rejected_without_exception(self):
        self.rejected(lambda m: m["checks"][0].update(id=["not-a-string"]))
        self.rejected(lambda m: m.update(required_check_ids=["tests", []]))

if __name__ == "__main__":
    unittest.main()
