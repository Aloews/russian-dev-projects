from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ACTION = ROOT / ".github" / "actions" / "product-factory-agent" / "action.yml"
WORKFLOW = ROOT / ".github" / "workflows" / "product-factory.yml"


class ProductFactoryAgentPermissionTests(unittest.TestCase):
    def test_agent_has_explicit_workspace_scoped_file_permissions(self):
        action = ACTION.read_text(encoding="utf-8")
        settings = action.split("settings: |", 1)[1].split("\n    - name:", 1)[0]
        for rule in (
            "Read(./**)",
            "Write(./**)",
            "Edit(./**)",
            "Glob(./**)",
            "Grep(./**)",
        ):
            with self.subTest(rule=rule):
                self.assertIn(f'"{rule}"', settings)

    def test_sensitive_read_and_write_denials_remain(self):
        action = ACTION.read_text(encoding="utf-8")
        settings = action.split("settings: |", 1)[1].split("\n    - name:", 1)[0]
        for rule in (
            "Read(./.env)",
            "Read(./**/.env)",
            "Read(./.git/**)",
            "Read(./.mcp.json)",
            "Write(./.env)",
            "Write(./.git/**)",
            "Write(./.github/workflows/**)",
            "Write(./.github/actions/**)",
            "Edit(./.github/workflows/**)",
            "Edit(./.github/actions/**)",
        ):
            with self.subTest(rule=rule):
                self.assertIn(f'"{rule}"', settings)

    def test_reads_and_writes_remain_confined_to_working_directories(self):
        action = ACTION.read_text(encoding="utf-8")
        settings = action.split("settings: |", 1)[1].split("\n    - name:", 1)[0]
        self.assertIn('"blockReadsOutsideWorkingDirectories": true', settings)
        self.assertIn('"blockWritesOutsideWorkingDirectories": true', settings)

    def test_agent_is_started_in_target_product_working_directory(self):
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("CLAUDE_WORKING_DIR: \${{ github.workspace }}/target", workflow)
        action = ACTION.read_text(encoding="utf-8")
        self.assertIn("Verify target workspace source files are readable", action)
        self.assertIn('test -r "target/$path"', action)


if __name__ == "__main__":
    unittest.main()
