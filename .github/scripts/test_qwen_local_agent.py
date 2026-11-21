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
                self.assertIn("EXISTING FILE CONFLICT", context)
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


    def test_contextual_replacement_disambiguates_repeated_snippet(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "README.md"
                source.write_text("# First\\nA paragraph.\\n# Second\\nA paragraph.\\n", encoding="utf-8")
                changed = agent.apply_edits(
                    [{"path": "README.md", "old": "A paragraph.", "new": "Updated.", "before": "# Second\\n"}],
                    {"README.md"},
                )
                self.assertEqual(changed, ["README.md"])
                self.assertEqual(source.read_text(encoding="utf-8"), "# First\\nA paragraph.\\n# Second\\nUpdated.\\n")
            finally:
                agent.ROOT = previous_root

    def test_contextual_replacement_rejects_ambiguous_context(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "README.md"
                source.write_text("# First\\nA paragraph.\\n# Second\\nA paragraph.\\n", encoding="utf-8")
                with self.assertRaises(ValueError):
                    agent.apply_edits(
                        [{"path": "README.md", "old": "A paragraph.", "new": "Updated.", "before": "# Missing\\n"}],
                        {"README.md"},
                    )
            finally:
                agent.ROOT = previous_root


    def test_symlink_parent_cannot_redirect_into_protected_repository_path(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                protected = agent.ROOT / ".github/workflows"
                protected.mkdir(parents=True)
                (agent.ROOT / "src").mkdir()
                (agent.ROOT / "src/link").symlink_to(protected, target_is_directory=True)
                with self.assertRaises(ValueError):
                    agent.apply_edits(
                        [{"path": "src/link/release.yml", "old": "", "new": "name: unsafe\\n"}],
                        set(),
                    )
                self.assertFalse((protected / "release.yml").exists())
            finally:
                agent.ROOT = previous_root

    def test_read_context_does_not_follow_symlinked_parent(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                protected = agent.ROOT / ".github/workflows"
                protected.mkdir(parents=True)
                (protected / "secret.py").write_text("SECRET = 'do-not-read'\\n", encoding="utf-8")
                (agent.ROOT / "src").mkdir()
                (agent.ROOT / "src/link").symlink_to(protected, target_is_directory=True)
                self.assertEqual(agent.read_text("src/link/secret.py"), "")
            finally:
                agent.ROOT = previous_root


    def test_symlink_loop_fails_closed_without_crashing(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                (agent.ROOT / "src").mkdir()
                (agent.ROOT / "src/loop").symlink_to("loop")
                self.assertEqual(agent.read_text("src/loop/file.py"), "")
                with self.assertRaises(ValueError):
                    agent.apply_edits(
                        [{"path": "src/loop/file.py", "old": "", "new": "VALUE = 1\\n"}],
                        set(),
                    )
            finally:
                agent.ROOT = previous_root


    def test_windows_separator_cannot_bypass_symlink_parent_guard(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                protected = agent.ROOT / ".github/workflows"
                protected.mkdir(parents=True)
                (agent.ROOT / "src").mkdir()
                (agent.ROOT / "src/link").symlink_to(protected, target_is_directory=True)
                with self.assertRaises(ValueError):
                    agent.apply_edits(
                        [{"path": "src\\\\link\\\\release.yml", "old": "", "new": "name: unsafe\\n"}],
                        set(),
                    )
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
