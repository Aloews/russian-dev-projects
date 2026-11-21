#!/usr/bin/env python3
"""Fail-closed validator for machine-readable Product Factory completion evidence."""
from __future__ import annotations
import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

SHA = re.compile(r"^[0-9a-f]{40}$")

def validate(manifest: dict[str, Any], expected_sha: str, changed_paths: list[str]) -> list[str]:
    errors: list[str] = []
    if not SHA.fullmatch(expected_sha):
        errors.append("expected candidate SHA is not a full 40-character Git SHA")
    candidate = manifest.get("candidate_sha")
    if candidate != expected_sha:
        errors.append("candidate SHA does not match the checked-out candidate")
    if not changed_paths:
        errors.append("no changed files: implementation claim has no diff")
    declared = manifest.get("changed_paths")
    if not isinstance(declared, list) or set(declared) != set(changed_paths):
        errors.append("manifest changed_paths do not exactly match the inspected diff")

    required_ids = manifest.get("required_check_ids")
    if not isinstance(required_ids, list) or not required_ids or "tests" not in required_ids:
        errors.append("required_check_ids must be non-empty and include tests")
        required_ids = []
    checks = manifest.get("checks")
    if not isinstance(checks, list):
        checks = []
    by_id = {c.get("id"): c for c in checks if isinstance(c, dict)}
    for check_id in required_ids:
        item = by_id.get(check_id)
        if not item or item.get("executed") is not True or item.get("exit_code") != 0 or not item.get("command") or not item.get("artifact"):
            errors.append(f"required check {check_id!r} was not executed successfully")
            continue
        if item.get("candidate_sha") != expected_sha:
            errors.append(f"required check {check_id!r} was not run on the candidate SHA")
        if check_id == "tests" and (not isinstance(item.get("tests_discovered"), int) or item["tests_discovered"] <= 0):
            errors.append("test command discovered zero tests")

    api = manifest.get("api")
    if not isinstance(api, dict) or api.get("route_exists") is not True or api.get("route_test_passed") is not True:
        errors.append("required API route is absent or has no passing route-level test")

    db = manifest.get("database")
    if not isinstance(db, dict) or db.get("write_attempted") is not True or db.get("read_after_write") is not True or not db.get("readback_artifact"):
        errors.append("database write is not proven by persisted read-after-write evidence")

    ui = manifest.get("ui")
    if not isinstance(ui, dict) or ui.get("data_source") != "live_api" or ui.get("api_available") is not True or ui.get("browser_test_passed") is not True:
        errors.append("UI is not proven to use the live API in a passing browser test")

    deployment = manifest.get("deployment")
    if not isinstance(deployment, dict) or deployment.get("state") != "READY" or deployment.get("deployed_sha") != expected_sha or deployment.get("smoke_test_passed") is not True:
        errors.append("deployment is not READY, exact-SHA matched, and smoke-tested")

    if manifest.get("tested_sha") != expected_sha:
        errors.append("candidate SHA changed after tests or test evidence targets another SHA")

    children = manifest.get("children")
    if not isinstance(children, list) or not children or any(not isinstance(c, dict) or c.get("state") not in {"CODE_VERIFIED", "INTEGRATION_VERIFIED", "USER_JOURNEY_VERIFIED", "PRODUCTION_VERIFIED"} for c in children):
        errors.append("parent completion contains a failed, missing, or unverified child task")

    retry = manifest.get("retry")
    if not isinstance(retry, dict) or retry.get("artifact_present") is not True or (retry.get("provider_acceptance_known") is not True and not retry.get("idempotency_key")):
        errors.append("retry has no artifact or ambiguous provider acceptance without an idempotency key")
    return errors

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--expected-sha", required=True)
    parser.add_argument("--diff-file", required=True, help="newline-delimited paths from git diff --name-only")
    args = parser.parse_args()
    try:
        manifest = json.loads(Path(args.manifest).read_text(encoding="utf-8"))
        changed = [line.strip() for line in Path(args.diff_file).read_text(encoding="utf-8").splitlines() if line.strip()]
        if not isinstance(manifest, dict):
            raise ValueError("manifest root must be an object")
    except Exception as exc:
        print(f"INVALID EVIDENCE: {exc}", file=sys.stderr)
        return 2
    errors = validate(manifest, args.expected_sha, changed)
    if errors:
        print("NOT_VERIFIED")
        for error in errors:
            print(f"- {error}")
        return 1
    print("EVIDENCE_SCHEMA_VALID: all required evidence fields are present and SHA-bound")
    print("This validates evidence structure, not the truth of external attestations.")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
