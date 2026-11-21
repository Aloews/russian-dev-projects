#!/usr/bin/env python3
"""Fail-closed parser for Product Factory test-gate output."""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


TEST_GATE = re.compile(
    r"^(?:Rust|Python|Go|Gradle|Maven|Frontend|Browser end-to-end) tests$",
    re.IGNORECASE,
)


def _xml_counts(workspace: Path) -> tuple[int, int, int] | None:
    files = []
    for pattern in (
        "build/test-results/test/TEST-*.xml",
        "target/surefire-reports/TEST-*.xml",
        "target/failsafe-reports/TEST-*.xml",
    ):
        files.extend(workspace.glob(pattern))
    if not files:
        return None
    tests = skipped = failed = 0
    for file in files:
        try:
            root = ET.parse(file).getroot()
            for suite in ([root] if root.tag.endswith("testsuite") else root.findall(".//testsuite")):
                tests += int(suite.attrib.get("tests", "0"))
                skipped += int(suite.attrib.get("skipped", "0"))
                failed += int(suite.attrib.get("failures", "0")) + int(suite.attrib.get("errors", "0"))
        except (ET.ParseError, OSError, ValueError):
            return None
    return tests - skipped - failed, skipped, failed


def _counts(label: str, body: str, workspace: Path) -> tuple[int, int, int] | None:
    # Structured Go JSON output counts individual tests, not package summaries.
    if label.lower() == "go tests":
        passed = skipped = failed = 0
        valid = False
        for line in body.splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(event, dict) and event.get("Test") and event.get("Action") in {"pass", "skip", "fail"}:
                valid = True
                if event["Action"] == "pass":
                    passed += 1
                elif event["Action"] == "skip":
                    skipped += 1
                else:
                    failed += 1
        return (passed, skipped, failed) if valid else None

    xml_result = _xml_counts(workspace) if label.lower() in {"gradle tests", "maven tests"} else None
    if xml_result is not None:
        return xml_result

    patterns = [
        (r"test result:\s*ok\.\s*(\d+)\s+passed;\s*(\d+)\s+failed;\s*(\d+)\s+ignored", "rust"),
        (r"(\d+)\s+passed(?:,\s*(\d+)\s+skipped)?", "pytest"),
        (r"Tests:\s*(\d+)\s+passed(?:,\s*(\d+)\s+skipped)?", "jest"),
        (r"Tests\s+(\d+)\s+passed(?:\s*\|\s*(\d+)\s+skipped)?", "vitest"),
        (r"(\d+)\s+passing(?:\s*\((\d+)\s+pending\))?", "mocha"),
        (r"(\d+)\s+passed\s*\(", "playwright"),
        (r"(\d+)\s+tests?\s+passed", "generic"),
    ]
    candidates: list[tuple[int, int, int]] = []
    for pattern, _ in patterns:
        for match in re.finditer(pattern, body, re.IGNORECASE):
            passed = int(match.group(1))
            skipped = int(match.group(2) or 0) if match.lastindex and match.lastindex >= 2 else 0
            failed = int(match.group(3) or 0) if match.lastindex and match.lastindex >= 3 else 0
            candidates.append((passed, skipped, failed))
    if not candidates:
        return None
    # A zero-exit wrapper must not conceal a runner-reported failure.
    failed_matches = [int(value) for value in re.findall(r"(\d+)\s+(?:failed|failures|errors?)", body, re.IGNORECASE)]
    best = max(candidates, key=lambda item: item[0])
    return best[0], best[1], max(best[2], max(failed_matches, default=0))


def parse_report(report: Path, workspace: Path) -> dict:
    raw = report.read_text(encoding="utf-8", errors="replace")
    sections = re.split(r"(?m)^## (.+?)\s*$", raw)
    gates = []
    errors = []
    for index in range(1, len(sections), 2):
        label = sections[index].strip()
        body = sections[index + 1] if index + 1 < len(sections) else ""
        if not TEST_GATE.fullmatch(label):
            continue
        exit_matches = re.findall(r"(?m)^exit_code=(\d+)\s*$", body)
        if not exit_matches or exit_matches[-1] != "0":
            errors.append(f"{label}: missing successful exit_code evidence")
            continue
        if re.search(r"no tests? (?:found|ran)|no test files|no test cases", body, re.IGNORECASE):
            errors.append(f"{label}: runner reports no tests")
            continue
        counts = _counts(label, body, workspace)
        if counts is None:
            errors.append(f"{label}: cannot determine executed test count from supported runner output")
            continue
        passed, skipped, failed = counts
        if failed > 0:
            errors.append(f"{label}: runner reports {failed} failed/error tests")
            continue
        if passed <= 0:
            errors.append(f"{label}: zero tests passed (all skipped or no executed tests)")
            continue
        command_match = re.search(r"(?m)^\$\s*(.+)$", body)
        gates.append({
            "id": re.sub(r"[^a-z0-9]+", "-", label.lower()).strip("-"),
            "label": label,
            "command": command_match.group(1).strip() if command_match else "unknown",
            "tests_discovered": passed + skipped,
            "tests_passed": passed,
            "tests_skipped": skipped,
            "tests_failed": failed,
        })
    if not gates and not errors:
        errors.append("no test gates were executed")
    return {
        "test_gates": gates,
        "tests_discovered": sum(g["tests_discovered"] for g in gates),
        "tests_passed": sum(g["tests_passed"] for g in gates),
        "tests_skipped": sum(g["tests_skipped"] for g in gates),
        "tests_failed": sum(g["tests_failed"] for g in gates),
        "errors": errors,
        "report_sha256": hashlib.sha256(report.read_bytes()).hexdigest(),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = parse_report(args.report, args.workspace)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    if result["errors"]:
        for error in result["errors"]:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
