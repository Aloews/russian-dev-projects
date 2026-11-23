from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "product-factory.yml"
VERIFY_ACTION = ROOT / ".github" / "actions" / "product-factory-verify" / "action.yml"


def job_section(workflow: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        workflow,
    )
    if not match:
        raise AssertionError(f"Workflow job {name!r} was not found")
    return match.group(1)


class ProductFactoryVerificationWiringTests(unittest.TestCase):
    def test_all_verification_cycles_receive_target_repo_and_base_branch(self):
        workflow = WORKFLOW.read_text(encoding="utf-8")
        expected = {
            "TARGET_REPO": "${{ fromJSON(github.event.issue.body || '{}').target_repo || github.event.inputs.target_repo || 'Aloews/full' }}",
            "BASE_BRANCH": "${{ fromJSON(github.event.issue.body || '{}').base_branch || github.event.inputs.base_branch || 'foundation/luxury-estate-platform' }}",
        }
        for job_name in ("verify_0", "verify_1", "verify_2"):
            section = job_section(workflow, job_name)
            job_header = section.split("    steps:", 1)[0]
            with self.subTest(job=job_name):
                for key, value in expected.items():
                    self.assertIn(f"      {key}: {value}", job_header)
                self.assertIn("target_repo: ${{ env.TARGET_REPO }}", section)
                self.assertIn("base_branch: ${{ env.BASE_BRANCH }}", section)

    def test_verifier_checks_out_the_requested_target_repository_and_base_ref(self):
        action = VERIFY_ACTION.read_text(encoding="utf-8")
        self.assertIn("repository: ${{ inputs.target_repo }}", action)
        self.assertIn("ref: ${{ inputs.base_branch }}", action)
        self.assertIn("path: target", action)

    def test_verifier_rejects_missing_inputs_and_wrong_repository_checkout(self):
        action = VERIFY_ACTION.read_text(encoding="utf-8")
        self.assertIn("Validate target repository inputs", action)
        self.assertIn("Invalid or missing target_repo input", action)
        self.assertIn("Invalid or missing base_branch input", action)
        self.assertIn("Verify target checkout identity and base ref", action)
        self.assertIn("unexpected repository", action)
        self.assertIn("CHECKED_OUT_SHA", action)
        self.assertIn("REMOTE_BASE_SHA", action)


if __name__ == "__main__":
    unittest.main()
