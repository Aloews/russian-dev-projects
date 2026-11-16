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
    ".github/product-factory/PRODUCT_DEVELOPMENT_LIFECYCLE.md": (
        "## Governing metric",
        "## Stage gates",
        "## Product discovery and prioritization",
        "## API documentation",
        "## Quality and release evidence",
    ),
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
        if relative != TEMPLATE and "{{" in text:
            fail(f"unresolved template placeholder found in operational document: {relative}")

    template = ROOT / TEMPLATE
    template_text = template.read_text(encoding="utf-8")
    placeholders = re.findall(r"\{\{[A-Z0-9_]+\}\}", template_text)
    if len(placeholders) < 10:
        fail("target-project AGENTS template appears incomplete: expected explicit project placeholders")

    lifecycle = (ROOT / ".github/product-factory/PRODUCT_DEVELOPMENT_LIFECYCLE.md").read_text(encoding="utf-8")
    if "cost per accepted, verified product increment" not in lifecycle:
        fail("lifecycle must optimize for cost per accepted, verified increment")
    if "OpenAPI" not in lifecycle or "Swagger UI" not in lifecycle:
        fail("lifecycle must define the OpenAPI/Swagger contract relationship")

    print(
        "Agent documentation validation passed: "
        f"{len(REQUIRED)} operational documents and {len(placeholders)} template placeholders checked."
    )


if __name__ == "__main__":
    main()
