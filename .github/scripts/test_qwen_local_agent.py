from __future__ import annotations

import importlib.util
import io
import os
import json
import tempfile
from contextlib import redirect_stdout
from types import SimpleNamespace
from unittest.mock import patch
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
    def test_retry_prompt_requires_source_test_pair_and_corrects_validation(self):
        retry = agent.build_retry_messages(
            [{"role": "user", "content": "objective"}],
            "Qwen review report is missing or too short",
        )
        self.assertIn("one non-test source edit and one regression-test edit", retry[-1]["content"])
        self.assertIn("do not choose documentation", retry[-1]["content"])
        self.assertIn("review must be 80-1000 characters", retry[-1]["content"])
        self.assertIn("Fix the specific validation error above", retry[-1]["content"])
        self.assertIn("Qwen review report is missing or too short", retry[-1]["content"])
        self.assertNotIn("previous answer lacked the required source-plus-test pair", retry[-1]["content"])


        retry = agent.build_retry_messages(
            [{"role": "user", "content": "objective"}],
            "Qwen response lacks a regression test edit",
        )
        self.assertIn("one non-test source edit and one regression-test edit", retry[-1]["content"])
        self.assertIn("do not choose documentation", retry[-1]["content"])

    def test_validation_error_requires_concrete_source_edit(self):
        with self.assertRaisesRegex(ValueError, "non-test source edit"): 
            agent.validate_response({
                "review": "A" * 100,
                "edits": [{"path": "docs/MAP.md", "old": "A", "new": "B"}],
            })

    def test_candidate_requires_regression_test_edit(self):
        with self.assertRaisesRegex(ValueError, "regression test edit"):
            agent.validate_response({
                "review": "A" * 100,
                "edits": [{"path": "src/api.py", "old": "VALUE = 1", "new": "VALUE = 2"}],
            })

    def test_source_only_implementation_is_accepted_for_test_recovery(self):
        response = {
            "review": "A" * 100,
            "edits": [{"path": "src/api.py", "old": "VALUE = 1", "new": "VALUE = 2"}],
        }
        self.assertEqual(agent.validate_response(response, require_test=False), "A" * 100)

    def test_test_recovery_requires_exactly_one_test_file(self):
        self.assertEqual(agent.validate_test_recovery({
            "review": "R" * 100,
            "edits": [{"path": "tests/test_api.py", "old": "assert old", "new": "assert new"}],
        }), "R" * 100)
        with self.assertRaisesRegex(ValueError, "non-test path"):
            agent.validate_test_recovery({
                "review": "R" * 100,
                "edits": [{"path": "src/api.py", "old": "x", "new": "y"}],
            })

    def test_test_recovery_prompt_is_separate_and_behavior_focused(self):
        messages = agent.build_test_recovery_messages(
            [{"role": "system", "content": "system"}],
            {"path": "src/api.py", "old": "A", "new": "B"},
            "fix API",
            "source evidence",
        )
        self.assertIn("ONLY one focused regression-test edit", messages[-1]["content"])
        self.assertIn("not merely assert True", messages[-1]["content"])
        self.assertIn("src/api.py", messages[-1]["content"])
        self.assertIn("RECOVERY-CALL OVERRIDE", messages[0]["content"])
        self.assertIn("exactly one edit", messages[0]["content"])
        self.assertIn("Do not return production source edits", messages[0]["content"])

    def test_source_and_regression_test_edits_are_accepted(self):
        review = "A" * 100
        self.assertEqual(
            agent.validate_response({
                "review": review,
                "edits": [
                    {"path": "src/api.py", "old": "VALUE = 1", "new": "VALUE = 2"},
                    {"path": "tests/test_api.py", "old": "VALUE = 1", "new": "VALUE = 2"},
                ],
            }),
            review,
        )

    def test_test_path_detection_covers_common_conventions(self):
        self.assertTrue(agent.is_test_path("tests/test_api.py"))
        self.assertTrue(agent.is_test_path("src/api.test.ts"))
        self.assertTrue(agent.is_test_path("src/api.spec.tsx"))
        self.assertTrue(agent.is_test_path("src/api_test.rs"))
        self.assertFalse(agent.is_test_path("src/api.ts"))

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
        with self.assertRaisesRegex(ValueError, "non-test source edit"): 
            agent.validate_response({
                "review": "A" * 100,
                "edits": [{"path": "docs/MAP.md", "old": "A", "new": "B"}],
            })

    def test_qwen_generation_budget_is_bounded_for_small_model(self):
        self.assertEqual(agent.MAX_FILES, 2)
        self.assertEqual(agent.MAX_CONTEXT_CHARS, 6000)
        self.assertEqual(agent.MAX_OUTPUT_TOKENS, 4096)
        self.assertEqual(agent.MAX_CAVEMAN_SKILL_CHARS, 2000)
        self.assertEqual(agent.MAX_REALITY_FIRST_SKILL_CHARS, 4000)
        self.assertEqual(agent.EDIT_SCHEMA["properties"]["edits"]["maxItems"], 2)
        self.assertEqual(agent.generation_budget(0), 1536)
        self.assertEqual(agent.generation_budget(1), 4096)
        self.assertEqual(agent.generation_budget(2), 4096)

    def test_test_context_is_reserved_when_many_source_files_rank_higher(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                tracked = []
                for index in range(12):
                    path = f"src/authentication_{index:02d}.py"
                    file_path = agent.ROOT / path
                    file_path.parent.mkdir(parents=True, exist_ok=True)
                    file_path.write_text("authentication handler\n" * 20, encoding="utf-8")
                    tracked.append(path)
                test_path = "tests/test_fallback_auth.py"
                test_file = agent.ROOT / test_path
                test_file.parent.mkdir(parents=True)
                test_file.write_text("def test_fallback_auth_regression():\n    assert True\n", encoding="utf-8")
                tracked.append(test_path)
                context = agent.make_context("authentication handler", tracked)
                self.assertIn(test_path, context.split("--- SOURCE/TEST FILE CANDIDATES", 1)[1].split("--- SOURCE/TEST FILE:", 1)[0])
                self.assertIn("SOURCE/TEST FILE: src/authentication_00.py", context)
                self.assertIn("SOURCE/TEST FILE: tests/test_fallback_auth.py", context)
                self.assertLess(
                    context.index("SOURCE/TEST FILE: src/authentication_00.py"),
                    context.index("SOURCE/TEST FILE: tests/test_fallback_auth.py"),
                )
            finally:
                agent.ROOT = previous_root

    def test_source_and_test_context_are_prioritized_before_docs(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                source = agent.ROOT / "src/api.py"
                doc = agent.ROOT / "docs/MAP.md"
                source.parent.mkdir(parents=True)
                doc.parent.mkdir(parents=True)
                source.write_text("def api():\n    return True\n" + "# code\n" * 100, encoding="utf-8")
                doc.write_text("# project map\\n" + "documentation-only context\\n" * 400, encoding="utf-8")
                context = agent.make_context("API endpoint implementation", ["src/api.py", "docs/MAP.md"])
                self.assertIn("SOURCE/TEST FILE CANDIDATES", context)
                self.assertIn("SOURCE/TEST FILE: src/api.py", context)
                self.assertLess(context.index("SOURCE/TEST FILE: src/api.py"), context.index("PROJECT DOC: docs/MAP.md"))
            finally:
                agent.ROOT = previous_root

class ProductFactoryRecoveryTests(unittest.TestCase):
    def _run_main_with_responses(self, responses):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            agent.ROOT = Path(directory)
            (agent.ROOT / ".git").mkdir()
            git_result = SimpleNamespace(stdout="src/main.py\ntests/test_qwen_local_agent.py\n")
            try:
                with (
                    patch.object(agent, "git", return_value=git_result),
                    patch.object(agent, "make_context", return_value="bounded source context"),
                    patch.object(agent, "ask_ollama", side_effect=responses) as ask,
                    patch.dict(os.environ, {
                        "OBJECTIVE": "retry-exhaustion regression",
                        "CAVEMAN_SKILL_PATH": "",
                        "REALITY_FIRST_SKILL_PATH": "",
                    }),
                    redirect_stdout(io.StringIO()),
                ):
                    result = agent.main()
                self.assertEqual(ask.call_count, agent.MAX_GENERATION_ATTEMPTS)
                self.assertFalse((Path(directory) / "docs" / "PRODUCT_FACTORY_QWEN_REVIEW.md").exists())
                return result
            finally:
                agent.ROOT = previous_root

    def test_main_runs_separate_test_recovery_and_preserves_both_reviews(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".git").mkdir()
            source_path = root / "src" / "main.py"
            test_path = root / "tests" / "test_main.py"
            source_path.parent.mkdir(parents=True)
            test_path.parent.mkdir(parents=True)
            source_path.write_text("VALUE = 1\n", encoding="utf-8")
            test_path.write_text("assert VALUE == 1\n", encoding="utf-8")

            implementation_review = "Implementation review: a minimal behavior change is required and the exact source replacement is bounded. " + "I"
            recovery_review = "Regression test review: this assertion exercises the new behavior and fails against the previous implementation. " + "R"
            implementation = json.dumps({
                "review": implementation_review,
                "edits": [{
                    "path": "src/main.py",
                    "old": "VALUE = 1",
                    "new": "VALUE = 2",
                }],
            })
            recovery = json.dumps({
                "review": recovery_review,
                "edits": [{
                    "path": "tests/test_main.py",
                    "old": "assert VALUE == 1",
                    "new": "assert VALUE == 2",
                }],
            })
            git_result = SimpleNamespace(
                stdout="src/main.py\ntests/test_main.py\n",
                stderr="",
                returncode=0,
            )

            previous_root = agent.ROOT
            agent.ROOT = root
            try:
                with (
                    patch.object(agent, "git", return_value=git_result),
                    patch.object(agent, "make_context", return_value="bounded source and test context"),
                    patch.object(agent, "ask_ollama", side_effect=[implementation, recovery]) as ask,
                    patch.dict(os.environ, {
                        "OBJECTIVE": "test two-pass Qwen recovery",
                        "CAVEMAN_SKILL_PATH": "",
                        "REALITY_FIRST_SKILL_PATH": "",
                    }),
                    redirect_stdout(io.StringIO()),
                ):
                    result = agent.main()

                self.assertEqual(result, 0)
                self.assertEqual(ask.call_count, 2)
                self.assertIn("max_predict", ask.call_args_list[1].kwargs)
                self.assertEqual(source_path.read_text(encoding="utf-8"), "VALUE = 2\n")
                self.assertEqual(test_path.read_text(encoding="utf-8"), "assert VALUE == 2\n")
                review_report = (root / "docs" / "PRODUCT_FACTORY_QWEN_REVIEW.md").read_text(encoding="utf-8")
                self.assertIn("Regression-test recovery:", review_report)
                self.assertIn(recovery_review, review_report)
                self.assertIn("bounded candidate review", review_report)
            finally:
                agent.ROOT = previous_root

    def test_main_returns_after_final_invalid_json_without_unbound_review(self):
        self.assertEqual(self._run_main_with_responses(["not-json", "not-json"]), 4)

    def test_main_returns_after_final_validation_error_without_unbound_review(self):
        invalid = json.dumps({"review": "short", "edits": []})
        self.assertEqual(self._run_main_with_responses([invalid, invalid]), 4)

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
        self.assertEqual(agent.generation_budget(0), 1536)
        self.assertEqual(agent.generation_budget(1), 4096)
        self.assertGreater(agent.generation_budget(1), agent.generation_budget(0))

    def test_review_schema_minimum_matches_validator(self):
        self.assertEqual(agent.EDIT_SCHEMA["properties"]["review"]["minLength"], 80)

    def test_qwen_output_limits_are_bounded(self):
        self.assertEqual(agent.MAX_FILES, 2)
        self.assertEqual(agent.EDIT_SCHEMA["properties"]["review"]["maxLength"], 1200)

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
