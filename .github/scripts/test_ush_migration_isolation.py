from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "universal-service-hub-ci.yml"
SCRIPT = ROOT / "universal-service-hub" / "scripts" / "ci_migration_smoke.sh"


class MigrationSmokeIsolationTests(unittest.TestCase):
    def test_workflow_uses_isolated_migration_database_script(self):
        workflow = WORKFLOW.read_text(encoding="utf-8")
        step = workflow.split("- name: Migration smoke test", 1)[1].split("\n      - name:", 1)[0]
        self.assertIn("bash scripts/ci_migration_smoke.sh", step)
        self.assertIn("USH_MIGRATION_TEST_DB_NAME: ush_migration_test", step)
        self.assertNotIn("USH_DATABASE_URL:", step)

    def test_smoke_creates_fresh_database_and_runs_upgrade_twice(self):
        script = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("createdb", script)
        self.assertIn("Refusing to reuse existing migration test database", script)
        self.assertEqual(script.count("alembic upgrade head"), 2)
        self.assertIn("USH_DATABASE_URL", script)
        self.assertNotIn("/ush_test", script)


if __name__ == "__main__":
    unittest.main()
