#!/usr/bin/env python3
"""Deterministic structural checks for repository agent contracts and lifecycle docs."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

REQUIRED = {
    "AGENTS.md": (
        "## Instruction hierarchy",
        "## Coding-agent compatibility",
        "## Required discovery before changes",
        "## Product development lifecycle",
        "## Agent coordination",
        "## Security and permissions",
        "## Verification and evidence",
        "## Definition of Done",
    ),
    ".github/AGENTS.md": (
        "## Control-plane safety",
        "## Workflow changes",
        "## Product Factory boundaries",
        "## Evidence",
    ),
    ".github/product-factory/AGENTS.md": (
        "## Product-agnostic operation",
        "## Role separation",
        "## Product lifecycle gates",
        "## Model, provider and budget policy",
        "## Completion contract",
    ),
    "universal-service-hub/docs/PROJECT_ONBOARDING.md": ("## Agent instruction onboarding",),
    ".github/product-factory/PRODUCT_DEVELOPMENT_LIFECYCLE.md": (
        "## Governing metric",
        "## Stage gates",
        "## Product discovery and prioritization",
        "## API documentation",
        "## Quality and release evidence",
    ),
    "universal-service-hub/docs/CODING_AGENTS.md": ("## Instruction loading and precedence", "## Provider routing status"),
    "universal-service-hub/AGENTS.md": (
        "## Required workflow",
        "## Provider integration rules",
        "## Safety boundaries",
        "## API and documentation contract",
        "## Verification and Definition of Done",
    ),
    "universal-service-hub/docs/PRODUCT_FACTORY.md": (
        "## Current workflow configuration",
        "## Provider and model truth",
        "## Product-agnostic verification",
        "## Completion and release rules",
    ),
    "universal-service-hub/docs/CODING_AGENTS.md": (
        "## Instruction loading and precedence",
        "## Provider routing status",
        "## Self-hosting status and transition",
    ),
}

TEMPLATE = ".github/product-factory/templates/AGENTS.md"


def fail(message: str) -> None:
    print(f"FAIL: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    for relative, headings in REQUIRED.items():
        path = ROOT / relative
        if not path.is_file():
            fail(f"required agent/lifecycle document missing: {relative}")
        text = path.read_text(encoding="utf-8")
        if not text.strip():
            fail(f"document is empty: {relative}")
        if not text.startswith("# "):
            fail(f"document must begin with a level-one Markdown heading: {relative}")
        for heading in headings:
            if heading not in text:
                fail(f"required section {heading!r} missing from {relative}")
        if "{{" in text:
            fail(f"unresolved template placeholder found in operational document: {relative}")

    template = ROOT / TEMPLATE
    template_text = template.read_text(encoding="utf-8")
    placeholders = re.findall(r"\{\{[A-Z0-9_]+\}\}", template_text)
    if "Claude Code" not in template_text or "@AGENTS.md" not in template_text:
        fail("target-project AGENTS template must define Claude Code AGENTS.md compatibility")
    if len(placeholders) < 10:
        fail("target-project AGENTS template appears incomplete: expected explicit project placeholders")

    lifecycle = (ROOT / ".github/product-factory/PRODUCT_DEVELOPMENT_LIFECYCLE.md").read_text(encoding="utf-8")
    if "cost per accepted, verified product increment" not in lifecycle:
        fail("lifecycle must optimize for cost per accepted, verified increment")
    if "OpenAPI" not in lifecycle or "Swagger UI" not in lifecycle:
        fail("lifecycle must define the OpenAPI/Swagger contract relationship")

    self_hosting_sections = {
        "AGENTS.md": "## Universal Service Hub self-hosting rule",
        ".github/AGENTS.md": "## Hub self-hosting and bootstrap boundary",
        ".github/product-factory/AGENTS.md": "## Use of the Universal Service Hub",
        TEMPLATE: "## Platform orchestration boundary",
        "universal-service-hub/AGENTS.md": "## Self-hosting development contract",
        "universal-service-hub/docs/CODING_AGENTS.md": "## Self-hosting status and transition",
        "universal-service-hub/docs/MAP.md": "## Self-hosting and provider diagnosis",
    }
    for relative, marker in self_hosting_sections.items():
        content = (ROOT / relative).read_text(encoding="utf-8")
        if marker not in content:
            fail(f"self-hosting contract section {marker!r} missing from {relative}")

    claude_imports = (
        "CLAUDE.md",
        ".github/CLAUDE.md",
        ".github/product-factory/CLAUDE.md",
        "universal-service-hub/CLAUDE.md",
        ".github/product-factory/templates/CLAUDE.md",
    )
    for relative in claude_imports:
        path = ROOT / relative
        if not path.is_file() or "@AGENTS.md" not in path.read_text(encoding="utf-8"):
            fail(f"Claude instruction bridge must import canonical AGENTS.md: {relative}")

    # Check that the written policy is actually injected into both hosted and local agents.
    required_integration_markers = {
        ".github/actions/product-factory-agent/action.yml": (
            "product-factory-governance/SKILL.md",
            "TARGET_AGENTS_TEMPLATE.md",
        ),
        ".github/actions/product-factory-qwen-agent/action.yml": (
            "PRODUCT_FACTORY_CONTRACT_PATH",
            "TARGET_AGENTS_TEMPLATE_PATH",
        ),
        ".github/scripts/qwen_local_agent.py": (
            "PRODUCT_FACTORY_CONTRACT_PATH",
            "TARGET_AGENTS_TEMPLATE_PATH",
            "Apply this shared Product Factory agent contract",
        ),
        ".github/workflows/universal-hub-codex-claude.yml": (
            "test -s AGENTS.md",
            "Read repository-root AGENTS.md, .github/AGENTS.md and universal-service-hub/AGENTS.md",
        ),
        ".github/workflows/product-factory.yml": (
            "product-factory-governance/SKILL.md",
            "Audit the target repository's root and nested AGENTS.md",
        ),
    }
    for relative, markers in required_integration_markers.items():
        path = ROOT / relative
        if not path.is_file():
            fail(f"agent integration file missing: {relative}")
        content = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in content:
                fail(f"agent policy injection marker {marker!r} missing from {relative}")

    print(
        "Agent documentation validation passed: "
        f"{len(REQUIRED)} operational documents and {len(placeholders)} template placeholders checked."
    )


if __name__ == "__main__":
    main()
