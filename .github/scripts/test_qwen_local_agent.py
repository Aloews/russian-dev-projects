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


ROOT = Path(__file__).resolve().parents[2]
ACTION = ROOT / ".github" / "actions" / "product-factory-agent" / "action.yml"

class QwenEditSafetyTests(unittest.TestCase):
    def test_retry_prompt_requires_concrete_source_or_test_change(self):
        retry = agent.build_retry_messages(
            [{"role": "user", "content": "objective"}],
            "Qwen response contains no source or test edits",
        )
        self.assertIn("exactly one concrete implementation edit", retry[-1]["content"])
        self.assertIn("do not choose documentation", retry[-1]["content"])

    def test_validation_error_requires_concrete_source_edit(self):
        with self.assertRaisesRegex(ValueError, "small concrete implementation change"):
            agent.validate_response({
                "review": "A" * 100,
                "edits": [{"path": "docs/MAP.md", "old": "A", "new": "B"}],
            })

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

    def test_similar_full_file_replacement_with_empty_old_is_bounded(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "src/main.py"
                source.parent.mkdir(parents=True)
                original = "def get_value():\\n    return 1\\n\\n" + "# retained content\\n" * 30
                source.write_text(original, encoding="utf-8")
                replacement = original.replace("return 1", "return 2") + "# verified\\n"
                changed = agent.apply_edits(
                    [{"path": "src/main.py", "old": "", "new": replacement}],
                    {"src/main.py"},
                )
                self.assertEqual(changed, ["src/main.py"])
                self.assertEqual(source.read_text(encoding="utf-8"), replacement)
            finally:
                agent.ROOT = previous_root

    def test_unrelated_full_file_replacement_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "src/main.py"
                source.parent.mkdir(parents=True)
                original = "A" * 600
                source.write_text(original, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "substantially similar"):
                    agent.apply_edits(
                        [{"path": "src/main.py", "old": "", "new": "B" * 400}],
                        {"src/main.py"},
                    )
                self.assertEqual(source.read_text(encoding="utf-8"), original)
            finally:
                agent.ROOT = previous_root

    def test_short_fragment_cannot_replace_existing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "src/main.py"
                source.parent.mkdir(parents=True)
                original = "A" * 600
                source.write_text(original, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "complete replacement"):
                    agent.apply_edits(
                        [{"path": "src/main.py", "old": "", "new": "B" * 100}],
                        {"src/main.py"},
                    )
                self.assertEqual(source.read_text(encoding="utf-8"), original)
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

    def test_rejected_existing_file_context_includes_file_after_old_text_mismatch(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "README.md"
                source.write_text("# Actual heading\\nActual body\\n", encoding="utf-8")
                answer = json.dumps({
                    "edits": [{"path": "README.md", "old": "# Stale heading", "new": "# Updated heading"}]
                })
                context = agent.rejected_existing_file_context(answer)
                self.assertIn("EXISTING FILE (NOT A NEW FILE)", context)
                self.assertIn("# Actual heading", context)
                self.assertNotIn("# Stale heading", context)
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

    def test_retry_does_not_replay_malformed_model_output(self):
        base = [
            {"role": "system", "content": "system contract"},
            {"role": "user", "content": "original objective"},
        ]
        malformed = '{"review":"unterminated'
        retry = agent.build_retry_messages(base, "Unterminated string", "")
        self.assertEqual(retry[:2], base)
        self.assertNotIn(malformed, json.dumps(retry))
        self.assertIn("Discard that response", retry[-1]["content"])
        self.assertIn("complete JSON object", retry[-1]["content"])

    def test_retry_keeps_bounded_existing_file_evidence(self):
        base = [{"role": "user", "content": "objective"}]
        evidence = "EXISTING FILE: src/main.py\nVALUE = 1\n"
        retry = agent.build_retry_messages(base, "old text mismatch", evidence)
        self.assertIn("VALUE = 1", retry[-1]["content"])

    def test_retry_prompt_caps_error_and_existing_file_context(self):
        retry = agent.build_retry_messages(
            [{"role": "user", "content": "objective"}],
            "E" * 1000,
            "C" * 6000,
        )
        self.assertLess(len(retry[-1]["content"]), 6000)
        self.assertNotIn("E" * 301, retry[-1]["content"])

    def test_response_validation_rejects_missing_review_before_editing(self):
        with self.assertRaisesRegex(ValueError, "review report"):
            agent.validate_response({
                "review": "too short",
                "edits": [{"path": "src/main.py", "old": "A", "new": "B"}],
            })

    def test_response_validation_rejects_documentation_only_candidate(self):
        with self.assertRaisesRegex(ValueError, "no source or test edits"):
            agent.validate_response({
                "review": "A" * 100,
                "edits": [{"path": "docs/MAP.md", "old": "A", "new": "B"}],
            })

    def test_qwen_generation_budget_is_bounded_for_small_model(self):
        self.assertEqual(agent.MAX_FILES, 2)
        self.assertEqual(agent.MAX_CONTEXT_CHARS, 8500)
        self.assertEqual(agent.MAX_OUTPUT_TOKENS, 4096)
        self.assertEqual(agent.EDIT_SCHEMA["properties"]["edits"]["maxItems"], 2)
        self.assertEqual(agent.generation_budget(0), 3072)
        self.assertEqual(agent.generation_budget(1), 4096)
        self.assertEqual(agent.generation_budget(2), 4096)

class ProductFactoryRecoveryTests(unittest.TestCase):
    def test_ollama_length_termination_is_reported_as_incomplete(self):
        with self.assertRaisesRegex(agent.IncompleteOllamaResponse, "num_predict limit"):
            agent.parse_ollama_response({
                "message": {"content": '{"review":"partial'},
                "done_reason": "length",
            })

    def test_ollama_complete_response_is_accepted(self):
        self.assertEqual(
            agent.parse_ollama_response({
                "message": {"content": '{"review":"complete"}'},
                "done_reason": "stop",
            }),
            '{"review":"complete"}',
        )

    def test_retry_generation_budget_increases_instead_of_shrinking(self):
        self.assertEqual(agent.generation_budget(0), 3072)
        self.assertEqual(agent.generation_budget(1), 4096)
        self.assertGreater(agent.generation_budget(1), agent.generation_budget(0))

    def test_qwen_output_limits_are_bounded(self):
        self.assertEqual(agent.MAX_FILES, 2)
        self.assertEqual(agent.EDIT_SCHEMA["properties"]["review"]["maxLength"], 2000)

    def test_agent_session_explicitly_adds_target_workspace(self):
        action_path = SCRIPT.parents[1] / "actions" / "product-factory-agent" / "action.yml"
        action = action_path.read_text(encoding="utf-8")
        self.assertIn('--add-dir "${{ github.workspace }}/target"', action)
        self.assertIn('"additionalDirectories": ["${{ github.workspace }}/target"]', action)

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
        action = ACTION.read_text(encoding="utf-8")
        self.assertIn("CLAUDE_WORKING_DIR: " + "$" + "{{ github.workspace }}/target", action)
        self.assertIn("Verify target workspace source files are readable", action)
        self.assertIn('test -r "target/$path"', action)


if __name__ == "__main__":
    unittest.main()
