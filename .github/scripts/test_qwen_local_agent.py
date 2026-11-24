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
    def test_retry_prompt_requires_single_source_edit_and_corrects_validation(self):
        for validation_error in (
            "Qwen review report is missing or too short",
            "Qwen response lacks a regression test edit",
        ):
            retry = agent.build_retry_messages(
                [{"role": "user", "content": "objective"}],
                validation_error,
            )
            prompt = retry[-1]["content"]
            self.assertIn("exactly one", prompt)
            self.assertIn("non-test source-file edit", prompt)
            self.assertIn("no test edit", prompt)
            self.assertIn("do not choose documentation", prompt.lower())
            self.assertIn("review between 80 and 400 characters", prompt)
            self.assertIn("Fix the specific validation error above", prompt)
            self.assertIn(validation_error, prompt)

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
        permitted = ["tests/test_api.py"]
        self.assertEqual(agent.validate_test_recovery({
            "review": "R" * 100,
            "edits": [{"path": "tests/test_api.py", "old": "assert old", "new": "assert new"}],
        }, permitted), "R" * 100)
        with self.assertRaisesRegex(ValueError, "non-test path"):
            agent.validate_test_recovery({
                "review": "R" * 100,
                "edits": [{"path": "src/api.py", "old": "x", "new": "y"}],
            }, permitted)

    def test_jsx_recovery_is_rejected_when_project_has_no_test_runner(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                (agent.ROOT / "package.json").write_text(json.dumps({
                    "scripts": {"build": "next build", "lint": "eslint ."},
                    "dependencies": {"next": "15.5.29", "react": "19.1.10"},
                }), encoding="utf-8")
                response = {
                    "review": "R" * 100,
                    "edits": [{
                        "path": "tests/enquiry_form.test.tsx",
                        "old": "",
                        "new": "import { render } from '@testing-library/react'; jest.mock('next/navigation');",
                    }],
                }
                with self.assertRaisesRegex(ValueError, "does not declare a runnable"):
                    agent.validate_test_recovery(response, ["tests/enquiry_form.test.tsx"])
            finally:
                agent.ROOT = previous_root

    def test_jsx_recovery_rejects_undeclared_test_libraries(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                (agent.ROOT / "package.json").write_text(json.dumps({
                    "scripts": {"test": "jest"},
                    "devDependencies": {"jest": "30.0.0"},
                    "dependencies": {"react": "19.1.10"},
                }), encoding="utf-8")
                response = {
                    "review": "R" * 100,
                    "edits": [{
                        "path": "tests/enquiry_form.test.tsx",
                        "old": "",
                        "new": "import { render } from '@testing-library/react'; jest.mock('next/navigation');",
                    }],
                }
                with self.assertRaisesRegex(ValueError, "undeclared package"):
                    agent.validate_test_recovery(response, ["tests/enquiry_form.test.tsx"])
            finally:
                agent.ROOT = previous_root

    def test_typescript_recovery_rejects_rust_syntax_and_rs_imports(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                (agent.ROOT / "package.json").write_text(json.dumps({
                    "scripts": {"test": "vitest run"},
                    "devDependencies": {"vitest": "3.0.0"},
                }), encoding="utf-8")
                response = {
                    "review": "R" * 100,
                    "edits": [{
                        "path": "tests/enquiry_form.test.ts",
                        "old": "",
                        "new": "import { Property } from '../services/platform-api/src/domains/elite_estate.rs';\\nconst state = VerificationState::Verified;",
                    }],
                }
                with self.assertRaisesRegex(ValueError, "Rust syntax|Rust source file"):
                    agent.validate_test_recovery(response, ["tests/enquiry_form.test.ts"])
            finally:
                agent.ROOT = previous_root

    def test_test_recovery_prompt_is_separate_and_behavior_focused(self):
        paths = ["tests/api_regression.test.py"]
        messages = agent.build_test_recovery_messages(
            {"path": "src/api.py", "old": "A", "new": "B"},
            "fix API",
            "source evidence",
            paths,
        )
        self.assertEqual(len(messages), 2)
        self.assertEqual(messages[0]["role"], "system")
        self.assertIn("ONLY the regression-test recovery phase", messages[0]["content"])
        self.assertIn("exactly one regression-test file edit", messages[0]["content"])
        self.assertIn("MUST be exactly one of the permitted test paths", messages[0]["content"])
        self.assertIn("Create only one focused regression-test edit", messages[-1]["content"])
        self.assertIn("tests/api_regression.test.py", messages[-1]["content"])
        self.assertIn("not merely assert True", messages[0]["content"])
        self.assertIn("src/api.py", messages[-1]["content"])
        self.assertIn("Do not include source-code edits", messages[-1]["content"])

    def test_test_recovery_prompt_includes_actual_package_test_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            previous_root = agent.ROOT
            try:
                agent.ROOT = Path(directory)
                (agent.ROOT / "package.json").write_text(json.dumps({
                    "scripts": {"lint": "eslint .", "build": "next build"},
                    "dependencies": {"next": "15.5.29", "react": "19.1.10"},
                }), encoding="utf-8")
                messages = agent.build_test_recovery_messages(
                    {"path": "app/form.tsx", "old": "A", "new": "B"},
                    "fix form submission",
                    "repository evidence",
                    ["tests/form.test.tsx"],
                )
                self.assertIn("PROJECT TEST CONFIGURATION", messages[-1]["content"])
                self.assertIn('"lint": "eslint ."', messages[-1]["content"])
                self.assertIn('"build": "next build"', messages[-1]["content"])
            finally:
                agent.ROOT = previous_root

    def test_rust_source_gets_new_cargo_integration_test_path_when_none_exist(self):
        tracked = [
            "services/platform-api/Cargo.toml",
            "services/platform-api/src/domains/elite_estate.rs",
        ]
        paths = agent.test_recovery_paths(
            {"path": "services/platform-api/src/domains/elite_estate.rs"},
            tracked,
        )
        self.assertEqual(paths, ["services/platform-api/tests/elite_estate_regression.rs"])

    def test_test_recovery_schema_enforces_only_permitted_paths(self):
        paths = ["tests/elite_estate_regression.rs"]
        schema = agent.build_test_recovery_schema(paths)
        self.assertEqual(schema["properties"]["edits"]["maxItems"], 1)
        self.assertEqual(
            schema["properties"]["edits"]["items"]["properties"]["path"]["enum"],
            paths,
        )

    def test_test_recovery_rejects_path_outside_allowlist(self):
        with self.assertRaisesRegex(ValueError, "outside the test allowlist"):
            agent.validate_test_recovery({
                "review": "R" * 100,
                "edits": [{"path": "tests/test_other.py", "old": "A", "new": "B"}],
            }, ["tests/test_api.py"])

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

    def test_symlink_parent_cannot_redirect_to_protected_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".git").mkdir()
            protected = root / ".github"
            protected.mkdir()
            source = protected / "secret.py"
            original = "VALUE = 1\n"
            source.write_text(original, encoding="utf-8")
            (root / "src").symlink_to(protected, target_is_directory=True)
            edit = {"path": "src/secret.py", "old": "VALUE = 1", "new": "VALUE = 2"}
            previous_root = agent.ROOT
            agent.ROOT = root
            try:
                with self.assertRaisesRegex(ValueError, "symbolic-link parent"):
                    agent.validate_edit_preconditions(edit, {"src/secret.py"})
                with self.assertRaisesRegex(ValueError, "symbolic-link parent"):
                    agent.apply_edits([edit], {"src/secret.py"})
                answer = json.dumps({"edits": [{"path": "src/secret.py", "old": "", "new": "VALUE = 2\n"}]})
                self.assertEqual(agent.rejected_existing_file_context(answer), "")
                self.assertEqual(agent.read_text("src/secret.py"), "")
                self.assertEqual(source.read_text(encoding="utf-8"), original)
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

    def test_retry_prompt_requests_one_source_edit_only(self):
        retry = agent.build_retry_messages(
            [{"role": "user", "content": "objective"}],
            "too many source edits",
        )
        prompt = retry[-1]["content"]
        self.assertIn("exactly one", prompt)
        self.assertIn("non-test source-file edit", prompt)
        self.assertIn("no test edit", prompt)
        self.assertIn("documentation edit", prompt)

    def test_retry_prompt_requires_nonempty_exact_anchor_for_existing_file_conflict(self):
        evidence = "--- EXISTING FILE (NOT A NEW FILE): app/form.tsx ---\nconst oldValue = true;\n"
        retry = agent.build_retry_messages(
            [{"role": "user", "content": "objective"}],
            "new-file edit targets an existing file: app/form.tsx; provide an exact old snippet",
            evidence,
        )
        prompt = retry[-1]["content"]
        self.assertIn("CRITICAL EXISTING-FILE ANCHOR RECOVERY", prompt)
        self.assertIn("old MUST be a non-empty exact unique snippet", prompt)
        self.assertIn('old="" is forbidden', prompt)
        self.assertIn("app/form.tsx", prompt)
        self.assertIn("const oldValue = true;", prompt)
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
        self.assertEqual(agent.MAX_OUTPUT_TOKENS, 3072)
        self.assertEqual(agent.MAX_CAVEMAN_SKILL_CHARS, 2000)
        self.assertEqual(agent.MAX_REALITY_FIRST_SKILL_CHARS, 4000)
        self.assertEqual(agent.EDIT_SCHEMA["properties"]["edits"]["maxItems"], 2)
        self.assertEqual(agent.SINGLE_EDIT_SCHEMA["properties"]["edits"]["maxItems"], 1)
        self.assertEqual(agent.generation_budget(0), 1536)
        self.assertEqual(agent.generation_budget(1), 3072)
        self.assertEqual(agent.generation_budget(2), 3072)

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

    def test_main_retries_overbroad_source_output_with_single_edit_schema(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".git").mkdir()
            source_path = root / "src" / "main.py"
            extra_path = root / "src" / "extra.py"
            test_path = root / "tests" / "test_main.py"
            for path in (source_path, extra_path, test_path):
                path.parent.mkdir(parents=True, exist_ok=True)
            source_path.write_text("VALUE = 1\n", encoding="utf-8")
            extra_path.write_text("EXTRA = 1\n", encoding="utf-8")
            test_path.write_text("assert VALUE == 1\n", encoding="utf-8")

            review = "Implementation review: one narrowly scoped source replacement is bounded and has a focused regression test. "
            recovery_review = "Regression test review: this assertion verifies the behavior change and fails before the implementation. "
            overbroad = json.dumps({
                "review": review,
                "edits": [
                    {"path": "src/main.py", "old": "VALUE = 1", "new": "VALUE = 2"},
                    {"path": "src/extra.py", "old": "EXTRA = 1", "new": "EXTRA = 2"},
                ],
            })
            implementation = json.dumps({
                "review": review,
                "edits": [{"path": "src/main.py", "old": "VALUE = 1", "new": "VALUE = 2"}],
            })
            recovery = json.dumps({
                "review": recovery_review,
                "edits": [{"path": "tests/test_main.py", "old": "assert VALUE == 1", "new": "assert VALUE == 2"}],
            })

            def fake_git(*args, **kwargs):
                if args == ("ls-files",):
                    return SimpleNamespace(stdout="src/main.py\nsrc/extra.py\ntests/test_main.py\n", stderr="", returncode=0)
                if args == ("diff", "--name-only"):
                    return SimpleNamespace(stdout="src/main.py\ntests/test_main.py\n", stderr="", returncode=0)
                if args == ("ls-files", "--others", "--exclude-standard"):
                    return SimpleNamespace(stdout="", stderr="", returncode=0)
                if args == ("diff", "--check"):
                    return SimpleNamespace(stdout="", stderr="", returncode=0)
                raise AssertionError("Unexpected git command: " + repr(args))

            previous_root = agent.ROOT
            agent.ROOT = root
            try:
                with (
                    patch.object(agent, "git", side_effect=fake_git),
                    patch.object(agent, "make_context", return_value="bounded source and test context"),
                    patch.object(agent, "ask_ollama", side_effect=[overbroad, implementation, recovery]) as ask,
                    patch.dict(os.environ, {
                        "OBJECTIVE": "retry one source edit after overbroad output",
                        "CAVEMAN_SKILL_PATH": "",
                        "REALITY_FIRST_SKILL_PATH": "",
                    }),
                    redirect_stdout(io.StringIO()),
                ):
                    result = agent.main()

                self.assertEqual(result, 0)
                self.assertEqual(ask.call_count, 3)
                for call in ask.call_args_list:
                    self.assertEqual(call.kwargs["format_schema"]["properties"]["edits"]["maxItems"], 1)
                self.assertEqual(source_path.read_text(encoding="utf-8"), "VALUE = 2\n")
                self.assertEqual(extra_path.read_text(encoding="utf-8"), "EXTRA = 1\n")
                self.assertEqual(test_path.read_text(encoding="utf-8"), "assert VALUE == 2\n")
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
            rejected_recovery = json.dumps({
                "review": recovery_review,
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
                    patch.object(agent, "ask_ollama", side_effect=[implementation, rejected_recovery, recovery]) as ask,
                    patch.dict(os.environ, {
                        "OBJECTIVE": "test two-pass Qwen recovery",
                        "CAVEMAN_SKILL_PATH": "",
                        "REALITY_FIRST_SKILL_PATH": "",
                    }),
                    redirect_stdout(io.StringIO()),
                ):
                    result = agent.main()

                self.assertEqual(result, 0)
                self.assertEqual(ask.call_count, 3)
                self.assertIn("max_predict", ask.call_args_list[1].kwargs)
                self.assertEqual(
                    ask.call_args_list[1].kwargs["format_schema"]["properties"]["edits"]["items"]["properties"]["path"]["enum"],
                    ["tests/test_main.py"],
                )
                self.assertIn("tests/test_main.py", ask.call_args_list[2].args[0][-1]["content"])
                self.assertEqual(source_path.read_text(encoding="utf-8"), "VALUE = 2\n")
                self.assertEqual(test_path.read_text(encoding="utf-8"), "assert VALUE == 2\n")
                review_report = (root / "docs" / "PRODUCT_FACTORY_QWEN_REVIEW.md").read_text(encoding="utf-8")
                self.assertIn("Regression-test recovery:", review_report)
                self.assertIn(recovery_review, review_report)
                self.assertIn("bounded candidate review", review_report)
            finally:
                agent.ROOT = previous_root


    def test_edit_preflight_rejects_stale_anchor_before_test_recovery(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".git").mkdir()
            source_path = root / "src" / "main.py"
            test_path = root / "tests" / "test_main.py"
            source_path.parent.mkdir(parents=True)
            test_path.parent.mkdir(parents=True)
            source_path.write_text("VALUE = 1\n", encoding="utf-8")
            test_path.write_text("def test_regression():\n    assert True\n", encoding="utf-8")

            bad_source = json.dumps({
                "review": "Implementation review: the proposed source anchor is intentionally stale and must be rejected before test recovery runs.",
                "edits": [{"path": "src/main.py", "old": "VALUE = 2", "new": "VALUE = 3"}],
            })
            good_source = json.dumps({
                "review": "Implementation review: update the exact existing value using a unique source anchor and preserve the surrounding module.",
                "edits": [{"path": "src/main.py", "old": "VALUE = 1", "new": "VALUE = 2"}],
            })
            recovery = json.dumps({
                "review": "Regression test review: this focused test asserts the changed behavior and fails against the previous value.",
                "edits": [{"path": "tests/test_main.py", "old": "assert True", "new": "assert VALUE == 2"}],
            })

            def fake_git(*args, **kwargs):
                if args == ("ls-files",):
                    return SimpleNamespace(stdout="src/main.py\ntests/test_main.py\n", stderr="", returncode=0)
                if args == ("diff", "--name-only"):
                    return SimpleNamespace(stdout="src/main.py\ntests/test_main.py\n", stderr="", returncode=0)
                if args == ("ls-files", "--others", "--exclude-standard"):
                    return SimpleNamespace(stdout="", stderr="", returncode=0)
                if args == ("diff", "--check"):
                    return SimpleNamespace(stdout="", stderr="", returncode=0)
                raise AssertionError("Unexpected git command: " + repr(args))

            previous_root = agent.ROOT
            agent.ROOT = root
            try:
                with (
                    patch.object(agent, "git", side_effect=fake_git),
                    patch.object(agent, "make_context", return_value="bounded source and test context"),
                    patch.object(agent, "ask_ollama", side_effect=[bad_source, good_source, recovery]) as ask,
                    patch.dict(os.environ, {
                        "OBJECTIVE": "preflight exact source anchors before test recovery",
                        "CAVEMAN_SKILL_PATH": "",
                        "REALITY_FIRST_SKILL_PATH": "",
                    }),
                    redirect_stdout(io.StringIO()),
                ):
                    result = agent.main()

                self.assertEqual(result, 0)
                self.assertEqual(ask.call_count, 3)
                retry_prompt = ask.call_args_list[1].args[0][-1]["content"]
                self.assertIn("EXISTING FILE (NOT A NEW FILE): src/main.py", retry_prompt)
                self.assertIn("VALUE = 1", retry_prompt)
                self.assertEqual(source_path.read_text(encoding="utf-8"), "VALUE = 2\n")
                self.assertEqual(
                    test_path.read_text(encoding="utf-8"),
                    "def test_regression():\n    assert VALUE == 2\n",
                )
            finally:
                agent.ROOT = previous_root

    def test_main_returns_after_final_invalid_json_without_unbound_review(self):
        self.assertEqual(self._run_main_with_responses(["not-json"] * agent.MAX_GENERATION_ATTEMPTS), 4)

    def test_main_returns_after_final_validation_error_without_unbound_review(self):
        invalid = json.dumps({"review": "short", "edits": []})
        self.assertEqual(self._run_main_with_responses([invalid] * agent.MAX_GENERATION_ATTEMPTS), 4)

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
        self.assertEqual(agent.generation_budget(1), 3072)
        self.assertEqual(agent.generation_budget(2), 3072)
        self.assertEqual(agent.MAX_GENERATION_ATTEMPTS, 3)
        self.assertGreater(agent.generation_budget(1), agent.generation_budget(0))

    def test_review_schema_minimum_matches_validator(self):
        self.assertEqual(agent.EDIT_SCHEMA["properties"]["review"]["minLength"], 80)

    def test_qwen_output_schema_limits_each_field_to_a_compact_patch(self):
        properties = agent.EDIT_SCHEMA["properties"]["edits"]["items"]["properties"]
        self.assertEqual(agent.EDIT_SCHEMA["properties"]["review"]["maxLength"], 500)
        self.assertEqual(properties["old"]["maxLength"], 600)
        self.assertEqual(properties["new"]["maxLength"], 1800)
        self.assertEqual(agent.MAX_OUTPUT_TOKENS, 3072)

    def test_qwen_output_limits_are_bounded(self):
        self.assertEqual(agent.MAX_FILES, 2)
        self.assertEqual(agent.EDIT_SCHEMA["properties"]["review"]["maxLength"], 500)

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
