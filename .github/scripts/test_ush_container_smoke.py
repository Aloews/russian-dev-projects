from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "universal-service-hub-ci.yml"


class ContainerSmokeWorkflowTests(unittest.TestCase):
    def test_production_startup_guard_runs_after_smoke_server_is_stopped(self):
        workflow = WORKFLOW.read_text(encoding="utf-8")
        smoke = workflow.split("Container smoke test", 1)[1]
        guard = "if docker run --rm --network host"
        guard_index = smoke.index(guard)
        before_guard = smoke[:guard_index]
        self.assertIn('docker stop ush-smoke >/dev/null', before_guard)


if __name__ == "__main__":
    unittest.main()
