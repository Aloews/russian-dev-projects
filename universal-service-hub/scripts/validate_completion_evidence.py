"""Fail-closed validator for SHA-bound Product Factory evidence."""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

SHA = re.compile(r"^[0-9a-f]{40}$")
LEVELS = ("CODE_VERIFIED", "INTEGRATION_VERIFIED", "USER_JOURNEY_VERIFIED", "PRODUCTION_VERIFIED")


def _artifact_error(workspace: Path | None, relative: Any, digest: Any, label: str) -> str | None:
    if workspace is None:
        return f"{label} cannot be independently checked without an evidence workspace"
    if not isinstance(relative, str) or not relative:
        return f"{label} artifact path is missing"
    path = Path(relative)
    if path.is_absolute() or ".." in path.parts:
        return f"{label} artifact path is unsafe"
    root = workspace.resolve()
    target = (root / path).resolve()
    if not target.is_relative_to(root):
        return f"{label} artifact resolves outside the evidence workspace"
    if not target.is_file():
        return f"{label} artifact is missing"
    actual = hashlib.sha256(target.read_bytes()).hexdigest()
    if not isinstance(digest, str) or digest != actual:
        return f"{label} artifact SHA-256 does not match its contents"
    return None


def validate(manifest: dict[str, Any], expected_sha: str, changed_paths: list[str],
             level: str = "PRODUCTION_VERIFIED", workspace: Path | None = None) -> list[str]:
    errors: list[str] = []
    if level not in LEVELS:
        return ["unknown verification level"]
    if not SHA.fullmatch(expected_sha):
        errors.append("expected candidate SHA is not a full 40-character Git SHA")
    if manifest.get("candidate_sha") != expected_sha:
        errors.append("candidate SHA does not match the checked-out candidate")
    if manifest.get("tested_sha") != expected_sha:
        errors.append("candidate SHA changed after tests or test evidence targets another SHA")
    if workspace is None:
        errors.append("an evidence workspace is required; metadata-only validation is not a verification")
    if not changed_paths:
        errors.append("no changed files: implementation claim has no diff")
    declared = manifest.get("changed_paths")
    if not isinstance(declared, list) or set(declared) != set(changed_paths):
        errors.append("manifest changed_paths do not exactly match the inspected diff")

    required_ids = manifest.get("required_check_ids")
    valid_required_ids = (
        isinstance(required_ids, list)
        and all(isinstance(item, str) and item for item in required_ids)
    )
    if (
        not valid_required_ids
        or not required_ids
        or "tests" not in required_ids
        or len(required_ids) != len(set(required_ids))
    ):
        errors.append("required_check_ids must be unique, non-empty strings and include tests")
        required_ids = []
    checks = manifest.get("checks")
    if not isinstance(checks, list):
        checks = []
    check_ids = [item.get("id") for item in checks if isinstance(item, dict)]
    valid_check_ids = [item for item in check_ids if isinstance(item, str) and item]
    if len(valid_check_ids) != len(check_ids):
        errors.append("every check record must have a non-empty string ID")
    if len(valid_check_ids) != len(set(valid_check_ids)):
        errors.append("checks contain duplicate IDs; evidence cannot be resolved unambiguously")
    by_id = {
        item.get("id"): item
        for item in checks
        if isinstance(item, dict) and isinstance(item.get("id"), str)
    }
    for check_id in required_ids:
        item = by_id.get(check_id)
        if not item or item.get("executed") is not True or type(item.get("exit_code")) is not int or item.get("exit_code") != 0 or not isinstance(item.get("command"), str) or not item.get("command").strip() or not item.get("artifact"):
            errors.append(f"required check {check_id!r} was not executed successfully")
            continue
        if check_id == "tests" and "tests_discovered" not in item:
            errors.append("required tests check has no machine-counted test totals")
        if item.get("candidate_sha") != expected_sha:
            errors.append(f"required check {check_id!r} was not run on the candidate SHA")
        if "tests_discovered" in item:
            count = item.get("tests_discovered")
            passed = item.get("tests_passed")
            skipped = item.get("tests_skipped")
            failed = item.get("tests_failed", 0)
            if type(count) is not int or count <= 0:
                errors.append(f"check {check_id!r} discovered zero tests")
            if type(passed) is not int or passed <= 0:
                errors.append(f"check {check_id!r} has no positively passed tests")
            if type(skipped) is not int or skipped != 0:
                errors.append(f"check {check_id!r} contains skipped tests")
            if type(failed) is not int or failed != 0:
                errors.append(f"check {check_id!r} contains failed tests")
        error = _artifact_error(
            workspace,
            item.get("artifact"),
            item.get("artifact_sha256"),
            f"check {check_id!r}",
        )
        if error:
            errors.append(error)

    if level == "CODE_VERIFIED":
        return errors

    api = manifest.get("api")
    if not isinstance(api, dict) or api.get("route_exists") is not True or api.get("route_test_passed") is not True:
        errors.append("required API route is absent or has no passing route-level test")
    else:
        error = _artifact_error(workspace, api.get("artifact"), api.get("artifact_sha256"), "API route/test")
        if error:
            errors.append(error)
    db = manifest.get("database")
    if not isinstance(db, dict) or db.get("write_attempted") is not True or db.get("read_after_write") is not True:
        errors.append("database write is not proven by persisted read-after-write evidence")
    else:
        error = _artifact_error(workspace, db.get("readback_artifact"), db.get("readback_sha256"), "database read-after-write")
        if error:
            errors.append(error)
    ui = manifest.get("ui")
    if not isinstance(ui, dict) or ui.get("data_source") != "live_api" or ui.get("api_available") is not True or ui.get("browser_test_passed") is not True:
        errors.append("UI is not proven to use the live API in a passing browser test")
    else:
        error = _artifact_error(workspace, ui.get("artifact"), ui.get("artifact_sha256"), "live-API browser test")
        if error:
            errors.append(error)
    if level in {"USER_JOURNEY_VERIFIED", "PRODUCTION_VERIFIED"}:
        deployment = manifest.get("deployment")
        if not isinstance(deployment, dict) or deployment.get("state") != "READY" or deployment.get("deployed_sha") != expected_sha or deployment.get("smoke_test_passed") is not True:
            errors.append("deployment is not READY, exact-SHA matched, and smoke-tested")
        else:
            error = _artifact_error(workspace, deployment.get("artifact"), deployment.get("artifact_sha256"), "deployment READY/smoke test")
            if error:
                errors.append(error)
    children = manifest.get("children")
    if not isinstance(children, list) or not children or any(
        not isinstance(child, dict) or child.get("state") not in LEVELS for child in children
    ):
        errors.append("parent completion contains a failed, missing, or unverified child task")
    elif workspace is None:
        errors.append("child task evidence cannot be independently checked without an evidence workspace")
    else:
        for index, child in enumerate(children):
            error = _artifact_error(workspace, child.get("artifact"), child.get("artifact_sha256"), f"child {index} evidence")
            if error:
                errors.append(error)
    retry = manifest.get("retry")
    if not isinstance(retry, dict) or retry.get("artifact_present") is not True or (
        retry.get("provider_acceptance_known") is not True and not retry.get("idempotency_key")
    ):
        errors.append("retry has no artifact or ambiguous provider acceptance without an idempotency key")
    else:
        error = _artifact_error(workspace, retry.get("artifact"), retry.get("artifact_sha256"), "provider retry/idempotency")
        if error:
            errors.append(error)
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--expected-sha", required=True)
    parser.add_argument("--diff-file", required=True, help="newline-delimited paths from git diff --name-only")
    parser.add_argument("--level", choices=LEVELS, default="PRODUCTION_VERIFIED")
    parser.add_argument("--workspace", required=True, help="root for verifying evidence artifact bytes and SHA-256")
    args = parser.parse_args()
    try:
        manifest = json.loads(Path(args.manifest).read_text(encoding="utf-8"))
        changed = [line.strip() for line in Path(args.diff_file).read_text(encoding="utf-8").splitlines() if line.strip()]
        if not isinstance(manifest, dict):
            raise TypeError("manifest root must be an object")
    except (OSError, UnicodeError, TypeError, ValueError) as exc:
        print(f"INVALID EVIDENCE: {exc}", file=sys.stderr)
        return 2
    errors = validate(manifest, args.expected_sha, changed, args.level,
                      Path(args.workspace).resolve() if args.workspace else None)
    if errors:
        print("NOT_VERIFIED")
        for error in errors:
            print(f"- {error}")
        return 1
    print(f"{args.level}: required evidence structure, candidate SHA and available artifact hashes verified")
    if args.level != "CODE_VERIFIED":
        print("External runtime attestations still require independent verification.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
