#!/usr/bin/env python3
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
        if check_id == "tests":
            count = item.get("tests_discovered")
            if not isinstance(count, int) or count <= 0:
                errors.append("test command discovered zero tests")
        if workspace is not None:
            artifact = Path(str(item.get("artifact", "")))
            if artifact.is_absolute() or ".." in artifact.parts:
                errors.append(f"check {check_id!r} artifact path is unsafe")
                continue
            target = workspace / artifact
            if not target.is_file():
                errors.append(f"check {check_id!r} artifact is missing")
                continue
            digest = hashlib.sha256(target.read_bytes()).hexdigest()
            if item.get("artifact_sha256") != digest:
                errors.append(f"check {check_id!r} artifact SHA-256 does not match its contents")

    if level == "CODE_VERIFIED":
        return errors

    api = manifest.get("api")
    if not isinstance(api, dict) or api.get("route_exists") is not True or api.get("route_test_passed") is not True:
        errors.append("required API route is absent or has no passing route-level test")
    db = manifest.get("database")
    if not isinstance(db, dict) or db.get("write_attempted") is not True or db.get("read_after_write") is not True or not db.get("readback_artifact"):
        errors.append("database write is not proven by persisted read-after-write evidence")
    ui = manifest.get("ui")
    if not isinstance(ui, dict) or ui.get("data_source") != "live_api" or ui.get("api_available") is not True or ui.get("browser_test_passed") is not True:
        errors.append("UI is not proven to use the live API in a passing browser test")
    if level in {"USER_JOURNEY_VERIFIED", "PRODUCTION_VERIFIED"}:
        deployment = manifest.get("deployment")
        if not isinstance(deployment, dict) or deployment.get("state") != "READY" or deployment.get("deployed_sha") != expected_sha or deployment.get("smoke_test_passed") is not True:
            errors.append("deployment is not READY, exact-SHA matched, and smoke-tested")
    children = manifest.get("children")
    if not isinstance(children, list) or not children or any(
        not isinstance(c, dict) or c.get("state") not in LEVELS for c in children
    ):
        errors.append("parent completion contains a failed, missing, or unverified child task")
    retry = manifest.get("retry")
    if not isinstance(retry, dict) or retry.get("artifact_present") is not True or (
        retry.get("provider_acceptance_known") is not True and not retry.get("idempotency_key")
    ):
        errors.append("retry has no artifact or ambiguous provider acceptance without an idempotency key")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--expected-sha", required=True)
    parser.add_argument("--diff-file", required=True, help="newline-delimited paths from git diff --name-only")
    parser.add_argument("--level", choices=LEVELS, default="PRODUCTION_VERIFIED")
    parser.add_argument("--workspace", help="root for verifying evidence artifact bytes and SHA-256")
    args = parser.parse_args()
    try:
        manifest = json.loads(Path(args.manifest).read_text(encoding="utf-8"))
        changed = [line.strip() for line in Path(args.diff_file).read_text(encoding="utf-8").splitlines() if line.strip()]
        if not isinstance(manifest, dict):
            raise ValueError("manifest root must be an object")
    except Exception as exc:
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
