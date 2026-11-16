#!/usr/bin/env python3
"""Evidence-first weekly technology radar for Universal Service Hub.

Reads only public GitHub release metadata from an explicit source allowlist.
It never installs, executes, or adopts a discovered technology.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import unittest
from datetime import datetime, timezone
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

SOURCES = (
    "openai/openai-python",
    "openai/openai-agents-python",
    "modelcontextprotocol/python-sdk",
    "huggingface/transformers",
    "ollama/ollama",
    "vllm-project/vllm",
    "langchain-ai/langgraph",
    "vercel/ai",
    "supabase/supabase",
    "neondatabase/neon",
)


def fetch_releases(repo: str, token: str | None = None) -> list[dict]:
    request = Request(
        f"https://api.github.com/repos/{repo}/releases?per_page=10",
        headers={
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "universal-service-hub-technology-radar",
            **({"Authorization": f"Bearer {token}"} if token else {}),
        },
    )
    with urlopen(request, timeout=20) as response:
        payload = json.loads(response.read().decode("utf-8"))
    if not isinstance(payload, list):
        raise ValueError(f"Unexpected releases response for {repo}")
    releases = []
    for item in payload:
        if not isinstance(item, dict) or item.get("draft"):
            continue
        tag = item.get("tag_name")
        if not isinstance(tag, str) or not tag:
            continue
        releases.append({
            "tag": tag,
            "name": (item.get("name") or tag)[:160],
            "published_at": item.get("published_at"),
            "prerelease": bool(item.get("prerelease")),
            "url": item.get("html_url"),
        })
    return releases


def build_snapshot(results: dict[str, list[dict]], previous: dict | None) -> dict:
    prior_tags = {
        repo: {item.get("tag") for item in values if isinstance(item, dict)}
        for repo, values in (previous or {}).get("sources", {}).items()
        if isinstance(values, list)
    }
    first_baseline = previous is None
    sources = {}
    newly_observed = []
    errors = []
    for repo, value in results.items():
        if isinstance(value, dict) and "error" in value:
            errors.append({"repo": repo, "error": str(value["error"])[:300]})
            continue
        releases = value
        sources[repo] = releases
        if not first_baseline:
            for release in releases:
                if release["tag"] not in prior_tags.get(repo, set()):
                    newly_observed.append({"repo": repo, **release})
    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "baseline_initial": first_baseline,
        "sources": sources,
        "newly_observed": newly_observed,
        "source_errors": errors,
        "coverage": {"configured": len(results), "successful": len(sources), "failed": len(errors)},
        "decision": "WATCH_ONLY",
        "note": "Release discovery is a candidate signal, not a quality, security, cost or adoption verdict.",
    }


def render_markdown(report: dict) -> str:
    lines = [
        "# Universal Service Hub — technology radar",
        "",
        f"Generated: {report['generated_at']}",
        "",
        f"Coverage: {report['coverage']['successful']}/{report['coverage']['configured']} sources succeeded.",
        "",
        "## Newly observed releases",
        "",
    ]
    if report["baseline_initial"]:
        lines.append("Initial baseline captured; no release is labelled new on the first run.")
    elif report["newly_observed"]:
        for item in report["newly_observed"]:
            lines.append(
                f"- [{item['repo']} {item['tag']}]({item.get('url') or 'https://github.com/' + item['repo'] + '/releases'})"
                f" — {item.get('published_at') or 'publication date unavailable'}"
                f"{' (pre-release)' if item.get('prerelease') else ''}"
            )
    else:
        lines.append("No newly observed release tags in the configured source set.")
    lines += ["", "## Latest release snapshots", ""]
    for repo, releases in report["sources"].items():
        latest = releases[0] if releases else None
        if latest:
            lines.append(f"- {repo}: {latest['tag']} ({latest.get('published_at') or 'date unavailable'})")
        else:
            lines.append(f"- {repo}: no public release returned")
    if report["source_errors"]:
        lines += ["", "## Sources requiring retry", ""]
        lines.extend(f"- {item['repo']}: {item['error']}" for item in report["source_errors"])
    lines += [
        "",
        "## Decision boundary",
        "",
        "All discoveries are WATCH-only. No package, model, provider, workflow or production setting was changed.",
        "A candidate requires a separate benchmark, cost/privacy/security review and approval before adoption.",
        "",
    ]
    return "\n".join(lines)


def run(output: Path, previous_path: Path | None) -> int:
    token = os.environ.get("GITHUB_TOKEN")
    previous = None
    if previous_path and previous_path.exists():
        previous = json.loads(previous_path.read_text(encoding="utf-8"))
    results = {}
    for repo in SOURCES:
        try:
            results[repo] = fetch_releases(repo, token)
        except (HTTPError, URLError, TimeoutError, ValueError, json.JSONDecodeError) as exc:
            results[repo] = {"error": f"{type(exc).__name__}: {exc}"}
    report = build_snapshot(results, previous)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    output.with_suffix(".md").write_text(render_markdown(report), encoding="utf-8")
    print(render_markdown(report))
    if not report["coverage"]["successful"]:
        print("All configured sources failed; refusing a false successful scan.", file=sys.stderr)
        return 1
    if report["coverage"]["failed"]:
        print("Partial source failure: inspect the report and retry failed sources.", file=sys.stderr)
    return 0


class RadarTests(unittest.TestCase):
    def test_first_run_is_baseline_not_new_release(self):
        report = build_snapshot({"org/repo": [{"tag": "v1"}]}, None)
        self.assertTrue(report["baseline_initial"])
        self.assertEqual(report["newly_observed"], [])

    def test_new_tag_detected_and_existing_tag_ignored(self):
        old = {"sources": {"org/repo": [{"tag": "v1"}]}}
        report = build_snapshot(
            {"org/repo": [{"tag": "v2"}, {"tag": "v1"}]}, old
        )
        self.assertEqual([x["tag"] for x in report["newly_observed"]], ["v2"])

    def test_failed_source_is_not_reported_as_empty_success(self):
        report = build_snapshot({"org/repo": {"error": "HTTP 503"}}, None)
        self.assertEqual(report["coverage"]["successful"], 0)
        self.assertEqual(report["coverage"]["failed"], 1)
        self.assertEqual(report["source_errors"][0]["error"], "HTTP 503")

    def test_only_watch_decision_is_emitted(self):
        report = build_snapshot({"org/repo": []}, None)
        self.assertEqual(report["decision"], "WATCH_ONLY")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("technology-radar/report.json"))
    parser.add_argument("--previous", type=Path, default=Path("technology-radar/previous.json"))
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(RadarTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        raise SystemExit(0 if result.wasSuccessful() else 1)
    raise SystemExit(run(args.output, args.previous))
