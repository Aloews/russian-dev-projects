from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("qwen_local_agent.py")
SPEC = importlib.util.spec_from_file_location("qwen_local_agent", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
agent = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(agent)


class QwenEditSafetyTests(unittest.TestCase):
    def test_source_file_is_allowed(self):
        self.assertTrue(agent.allowed("src/main.py", {"src/main.py"}))

    def test_path_traversal_is_rejected(self):
        self.assertFalse(agent.allowed("src/../.env", set()))

    def test_credentials_and_workflows_are_rejected(self):
        self.assertFalse(agent.allowed(".github/workflows/release.yml", set()))
        self.assertFalse(agent.allowed("src/credentials.py", set()))
        self.assertFalse(agent.allowed("src/.env", set()))

    def test_dependency_manifests_and_migrations_are_rejected(self):
        self.assertFalse(agent.allowed("package.json", set()))
        self.assertFalse(agent.allowed("migrations/0001_schema.sql", set()))

    def test_exact_unique_replacement_is_applied(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "src/main.py"
                source.parent.mkdir(parents=True)
                source.write_text("VALUE = 1\n", encoding="utf-8")
                changed = agent.apply_edits(
                    [{"path": "src/main.py", "old": "VALUE = 1", "new": "VALUE = 2"}],
                    {"src/main.py"},
                )
                self.assertEqual(changed, ["src/main.py"])
                self.assertEqual(source.read_text(encoding="utf-8"), "VALUE = 2\n")
            finally:
                agent.ROOT = previous_root

    def test_new_file_conflict_context_includes_existing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "src/main.py"
                source.parent.mkdir(parents=True)
                source.write_text("VALUE = 1\n", encoding="utf-8")
                answer = json.dumps({
                    "edits": [{"path": "src/main.py", "old": "", "new": "VALUE = 2\n"}]
                })
                context = agent.rejected_existing_file_context(answer)
                self.assertIn("EXISTING FILE (NOT A NEW FILE)", context)
                self.assertIn("VALUE = 1", context)
            finally:
                agent.ROOT = previous_root


    def test_ambiguous_replacement_context_includes_existing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "README.md"
                source.write_text("# Product\\n\\nA paragraph.\\n\\nA paragraph.\\n", encoding="utf-8")
                answer = json.dumps({
                    "edits": [{"path": "README.md", "old": "A paragraph.", "new": "A revised paragraph."}]
                })
                context = agent.rejected_existing_file_context(answer)
                self.assertIn("EXISTING FILE CONFLICT", context)
                self.assertIn("occurs 2 times", context)
                self.assertIn("A paragraph.", context)
            finally:
                agent.ROOT = previous_root

    def test_missing_replacement_context_includes_existing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "README.md"
                source.write_text("# Product\\n\\nActual content.\\n", encoding="utf-8")
                answer = json.dumps({
                    "edits": [{"path": "README.md", "old": "Not in file", "new": "Replacement"}]
                })
                context = agent.rejected_existing_file_context(answer)
                self.assertIn("does not occur", context)
                self.assertIn("Actual content.", context)
            finally:
                agent.ROOT = previous_root

    def test_symlink_parent_cannot_escape_repository(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as outside:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                (agent.ROOT / "src").mkdir()
                (agent.ROOT / "src/escape").symlink_to(Path(outside), target_is_directory=True)
                with self.assertRaises(ValueError):
                    agent.apply_edits(
                        [{"path": "src/escape/pwn.py", "old": "", "new": "VALUE = 1\n"}],
                        set(),
                    )
                self.assertFalse((Path(outside) / "pwn.py").exists())
            finally:
                agent.ROOT = previous_root

    def test_non_unique_replacement_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "src/main.py"
                source.parent.mkdir(parents=True)
                source.write_text("VALUE = 1\nVALUE = 1\n", encoding="utf-8")
                with self.assertRaises(ValueError):
                    agent.apply_edits(
                        [{"path": "src/main.py", "old": "VALUE = 1", "new": "VALUE = 2"}],
                        {"src/main.py"},
                    )
            finally:
                agent.ROOT = previous_root


if __name__ == "__main__":
    unittest.main()
