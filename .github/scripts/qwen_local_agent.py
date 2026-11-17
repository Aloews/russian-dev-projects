#!/usr/bin/env python3
"""Bounded local Qwen fallback for Product Factory. Emits a validated patch; never commits or publishes."""
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(os.environ.get("TARGET_WORKDIR", "target")).resolve()
MODEL = os.environ.get("OLLAMA_MODEL", "qwen2.5-coder:3b")
OLLAMA_URL = os.environ.get("OLLAMA_URL", "http://127.0.0.1:11434/api/chat")
MAX_FILES = 6
MAX_CONTEXT_CHARS = 16000
MAX_FILE_CHARS = 3500
SOURCE_SUFFIXES = (
    ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs",
    ".go", ".java", ".kt", ".swift", ".cs", ".rb", ".php",
    ".vue", ".svelte", ".html", ".css", ".scss", ".sql",
    ".md", ".mdx", ".yaml", ".yml", ".toml",
)
PROTECTED_DIRS = {
    ".git", ".github", ".claude", ".next", ".venv", "node_modules",
    "vendor", "dist", "build", "coverage", "target", "migrations",
}
PROTECTED_NAMES = {
    "package.json", "package-lock.json", "pnpm-lock.yaml", "yarn.lock",
    "cargo.toml", "cargo.lock", "pyproject.toml", "poetry.lock",
    "requirements.txt", "dockerfile", "docker-compose.yml",
    "makefile", "justfile",
}

def git(*args, check=True, input_text=None):
    return subprocess.run(
        ["git", *args], cwd=ROOT, check=check, text=True,
        input=input_text, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )

def allowed(path, tracked):
    path = path.replace("\\", "/")
    if path.startswith("/") or any(part in (".", "..") for part in path.split("/")):
        return False
    prefixes = [item.strip().strip("/") for item in os.environ.get("ALLOWED_PREFIXES", "").split(";") if item.strip()]
    if prefixes and not any(path == prefix or path.startswith(prefix + "/") for prefix in prefixes):
        return False
    low = path.lower()
    parts = low.split("/")
    if any(part in PROTECTED_DIRS for part in parts):
        return False
    name = parts[-1]
    if name in PROTECTED_NAMES or name.endswith((".lock", ".min.js", ".map", ".pem", ".key", ".p12", ".pfx")):
        return False
    if any(x in low for x in (".env", "credentials", "service-account", "secret", "private-key")):
        return False
    if name.startswith(".") and name not in {".gitignore", ".dockerignore"}:
        return False
    return Path(name).suffix.lower() in SOURCE_SUFFIXES

def read_text(path):
    try:
        p = ROOT / path
        if p.is_symlink() or not p.is_file() or p.stat().st_size > 100000:
            return ""
        return p.read_text(encoding="utf-8")[:MAX_FILE_CHARS]
    except (OSError, UnicodeError):
        return ""

def scrub(text):
    text = re.sub(r"(?i)(bearer\s+)[A-Za-z0-9._~-]{12,}", r"\1[REDACTED]", text)
    text = re.sub(r"(?i)((?:api[_-]?key|token|password|secret)\s*[:=]\s*)['\"]?[^\s'\"]{8,}", r"\1[REDACTED]", text)
    return text

def make_context(task, tracked):
    tokens = set(re.findall(r"[a-zA-Z0-9_/-]{3,}", task.lower()))
    configured_context = [item.strip() for item in os.environ.get("CONTEXT_FILES", "").split(";") if item.strip()]
    preferred = configured_context or [
        "docs/MAP.md", "docs/DELIVERY_PLAN.md", "docs/AI_AGENT_WORKFLOW.md",
        "AGENTS.md", "README.md", "README.rst",
    ]
    candidates = [p for p in preferred if p in tracked]
    ranked = []
    tracked_set = set(tracked)
    for path in tracked:
        if not allowed(path, tracked_set) or path in candidates:
            continue
        body = read_text(path)
        if not body:
            continue
        score = sum(4 for token in tokens if token in path.lower())
        score += sum(1 for token in tokens if token in body[:18000].lower())
        if score:
            ranked.append((score, path))
    ranked.sort(key=lambda item: (-item[0], item[1]))
    chosen = candidates + [p for _, p in ranked[:12]]
    chunks = []
    total = 0
    for path in chosen:
        body = read_text(path)
        if not body:
            continue
        chunk = f"\n--- FILE: {path} ---\n{scrub(body)}\n"
        if total + len(chunk) > MAX_CONTEXT_CHARS:
            continue
        chunks.append(chunk)
        total += len(chunk)
    verification_path = os.environ.get("VERIFICATION_FILE", "PRODUCT_FACTORY_VERIFICATION.md")
    verification = read_text(verification_path)
    if verification:
        chunks.append("\n--- FAILED VERIFICATION EVIDENCE ---\n" + scrub(verification[:12000]))
    return "\n".join(chunks)

EDIT_SCHEMA = {
    "type": "object",
    "properties": {
        "review": {"type": "string", "minLength": 1, "maxLength": 7000},
        "edits": {
            "type": "array",
            "minItems": 1,
            "maxItems": MAX_FILES,
            "items": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "minLength": 1, "maxLength": 240},
                    "old": {"type": "string", "maxLength": 6000},
                    "new": {"type": "string", "minLength": 1, "maxLength": 12000},
                },
                "required": ["path", "old", "new"],
                "additionalProperties": False,
            },
        },
    },
    "required": ["review", "edits"],
    "additionalProperties": False,
}

def rejected_existing_file_context(answer):
    """Return bounded current-file evidence when the model incorrectly proposes a new file."""
    try:
        payload = json.loads(answer)
    except (TypeError, json.JSONDecodeError):
        return ""
    sections = []
    for edit in payload.get("edits", []) if isinstance(payload, dict) else []:
        if not isinstance(edit, dict) or edit.get("old") != "":
            continue
        path = edit.get("path")
        if not isinstance(path, str) or not allowed(path, set()):
            continue
        target = ROOT / path
        try:
            resolved = target.resolve()
            if not resolved.is_relative_to(ROOT.resolve()) or target.is_symlink() or not target.is_file():
                continue
            current = target.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            continue
        sections.append(
            "\n--- EXISTING FILE (NOT A NEW FILE): " + path + " ---\n"
            + scrub(current[:12000])
            + ("\n[FILE CONTENT TRUNCATED]\n" if len(current) > 12000 else "")
        )
    return "\n".join(sections)


def prepare_edits(edits, tracked):
    """Validate and stage every edit in memory without mutating the workspace."""
    if not isinstance(edits, list) or not edits or len(edits) > MAX_FILES:
        raise ValueError("model returned no edits or exceeded the edit limit")
    staged = {}
    for edit in edits:
        if not isinstance(edit, dict):
            raise ValueError("edit item is not an object")
        path, old, new = edit.get("path"), edit.get("old"), edit.get("new")
        if not isinstance(path, str) or not isinstance(old, str) or not isinstance(new, str) or not new:
            raise ValueError("edit is missing path, old or new text")
        if not allowed(path, tracked):
            raise ValueError("edit path is outside the source allowlist")
        target = ROOT / path
        resolved_target = target.resolve()
        if not resolved_target.is_relative_to(ROOT.resolve()):
            raise ValueError("edit path resolves outside the target repository")
        if target.is_symlink():
            raise ValueError("symbolic-link edits are not permitted")
        if path in staged:
            current = staged[path]
        elif target.exists():
            if not target.is_file() or target.stat().st_size > 100000:
                raise ValueError("existing file is not a bounded regular text file")
            current = target.read_text(encoding="utf-8")
        else:
            current = None
        if old == "":
            if current is not None or path in tracked:
                raise ValueError("new-file edit targets an existing file")
            current = new
        else:
            if current is None:
                raise ValueError("update edit targets a missing file")
            if current.count(old) != 1:
                raise ValueError("old text must match exactly once in " + path)
            current = current.replace(old, new, 1)
        if len(current) > 100000 or "\0" in current:
            raise ValueError("edited file exceeds safe text limits")
        staged[path] = current
    return staged


def write_staged_edits(staged):
    """Apply a fully validated in-memory edit set after all response gates pass."""
    for path in staged:
        target = ROOT / path
        if target.is_symlink() or not target.resolve().is_relative_to(ROOT.resolve()):
            raise ValueError("edit destination changed or escaped before write")
        if target.exists() and not target.is_file():
            raise ValueError("edit destination is no longer a regular file")
    for path, content in staged.items():
        target = ROOT / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")


def apply_edits(edits, tracked):
    staged = prepare_edits(edits, tracked)
    write_staged_edits(staged)
    return sorted(staged)



def append_policy_section(prompt, heading, body):
    return f"{prompt}\n\n{heading}\n{body}"

def ask_ollama(messages, format_schema=None, max_predict=1200, timeout_seconds=480):
    payload = {
        "model": MODEL,
        "stream": False,
        "messages": messages,
        "options": {"temperature": 0.1, "num_ctx": 12288, "num_predict": max_predict},
    }
    if format_schema is not None:
        payload["format"] = format_schema
    request = urllib.request.Request(
        OLLAMA_URL, data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
        data = json.loads(response.read())
    answer = data.get("message", {}).get("content")
    if not isinstance(answer, str) or not answer.strip():
        raise ValueError("Qwen returned an empty or invalid response")
    return answer


def main():
    if not ROOT.is_dir() or not (ROOT / ".git").exists():
        print("::error::Target working tree is missing.")
        return 2
    tracked = git("ls-files").stdout.splitlines()
    task = os.environ.get("OBJECTIVE", "Complete the highest-priority bounded, testable product increment.")
    verification_path = os.environ.get("VERIFICATION_FILE", "PRODUCT_FACTORY_VERIFICATION.md")
    verification = read_text(verification_path)
    if verification:
        task += "\n\nRepair the actual failures listed in the supplied verification evidence. Preserve all checks."
    skill_path = os.environ.get("CAVEMAN_SKILL_PATH", "")
    skill = ""
    if skill_path:
        try:
            skill = Path(skill_path).read_text(encoding="utf-8")[:7000]
        except (OSError, UnicodeError):
            print("::error::Pinned Caveman skill could not be loaded.")
            return 2
    context = make_context(task, tracked)
    if not context.strip():
        print("::error::No safe source context found for Qwen.")
        return 2
    system = (
        "You are a cautious, product-agnostic local coding fallback for any software product. "
        "Inspect the provided project evidence and project map; infer the actual stack and product domain from files, then choose one small, complete, testable increment. "
        "Return only JSON matching the supplied schema, with an edits array of exact path/old/new replacements. "
        "For existing files, old must be an exact unique snippet copied from the provided file. "
        "For a new file, old must be empty and new must contain the complete file. "
        "Never return a unified diff, markdown fence or explanation. "
        "Never invent live data, credentials, legal approvals, customers, property inventory or tests. "
        "Do not edit workflows, action definitions, dependency manifests, lockfiles, credentials, deployment files, "
        "production secrets, infrastructure definitions or database migrations. Add tests for behavior changes. "
        "Do not weaken or delete tests. Keep changes minimal. No extra keys or explanations."
    )
    reality_path = os.environ.get("REALITY_FIRST_SKILL_PATH", "")
    if reality_path:
        try:
            reality_skill = Path(reality_path).read_text(encoding="utf-8")[:10000]
        except (OSError, UnicodeError):
            print("::error::Reality-First Business skill could not be loaded.")
            return 2
        system += "\n\nApply this Reality-First Business skill as a hard execution standard:\n" + reality_skill
    contract_path = os.environ.get("PRODUCT_FACTORY_CONTRACT_PATH", "")
    if not contract_path:
        print("::error::Shared Product Factory agent contract path is missing.")
        return 2
    try:
        factory_contract = Path(contract_path).read_text(encoding="utf-8")[:12000]
    except (OSError, UnicodeError):
        print("::error::Shared Product Factory agent contract could not be loaded.")
        return 2
    system = append_policy_section(system, "Apply this shared Product Factory agent contract as mandatory execution policy:", factory_contract)
    template_path = os.environ.get("TARGET_AGENTS_TEMPLATE_PATH", "")
    if not template_path:
        print("::error::Target AGENTS.md template path is missing.")
        return 2
    try:
        target_agents_template = Path(template_path).read_text(encoding="utf-8")[:9000]
    except (OSError, UnicodeError):
        print("::error::Target AGENTS.md template could not be loaded.")
        return 2
    system = append_policy_section(system, "Use this template when onboarding a target project or correcting missing/outdated local agent instructions. Replace every placeholder with verified facts; never copy placeholders into a product repository:", target_agents_template)
    if skill:
        system += "\n\nApply this Caveman skill to reduce prose only. Never compress code, paths, identifiers, errors, contracts, negations or tests:\n" + skill
    system += (
        "\n\nYou are a coordinated Qwen2.5-Coder 3B engineering council. In ONE inference pass, "
        "perform six explicitly separated roles: (1) architecture analyst, (2) product and user-journey analyst, "
        "(3) security/privacy analyst, (4) QA/test analyst, (5) implementation engineer, and "
        "(6) independent post-change reviewer. First reason through architecture, product, security and QA; "
        "then make only the smallest coherent implementation changes; finally review your proposed changes "
        "for regressions and missing tests. The 'review' field must contain concise findings for each role, "
        "evidence-backed file references, acceptance criteria and any remaining unknowns. "
        "Do not claim tests were run. Return only JSON matching the schema, including both review and edits."
    )
    messages = [
        {"role": "system", "content": system},
        {"role": "user", "content": "OBJECTIVE:\n" + task + "\n\nREPOSITORY EVIDENCE:\n" + context + "\n\nProduce a review report and minimal exact structured edits. For existing files, old must be an exact unique snippet. For new files, old must be empty."},
    ]
    tracked_set = set(tracked)
    validation_error = None
    answer = ""
    for attempt in range(2):
        try:
            answer = ask_ollama(
                messages,
                format_schema=EDIT_SCHEMA,
                max_predict=2048 if attempt == 0 else 1024,
                timeout_seconds=480 if attempt == 0 else 180,
            )
            response_json = json.loads(answer)
            staged_edits = prepare_edits(response_json.get("edits"), tracked_set)
            changed_paths = sorted(staged_edits)
            if not changed_paths:
                raise ValueError("Qwen returned no source or test edits")
            source_changes = [path for path in changed_paths if Path(path).suffix.lower() not in {".md", ".mdx", ".yaml", ".yml", ".toml"}]
            if not source_changes:
                raise ValueError("Qwen returned documentation-only edits")
            review_text = response_json.get("review")
            if not isinstance(review_text, str) or len(review_text.strip()) < 80 or "timed out" in review_text.lower():
                raise ValueError("Qwen review report is missing or contains an execution error")
            break
        except (TimeoutError, urllib.error.URLError) as exc:
            print("::error::Qwen inference timed out or could not connect: " + type(exc).__name__ + ": " + str(exc)[:500])
            return 4
        except (KeyError, ValueError, json.JSONDecodeError, OSError, UnicodeError) as exc:
            validation_error = str(exc)[:1000]
            if attempt == 0:
                existing_file_evidence = rejected_existing_file_context(answer)
                messages.extend([
                    {"role": "assistant", "content": answer[:10000]},
                    {"role": "user", "content": (
                        "Your previous JSON edit was rejected: " + validation_error + ". "
                        "Correct the JSON. For an existing file, old must be an exact unique non-empty snippet copied from that file; "
                        "never mark an existing path as a new file. Use old='' only for a path that is confirmed absent. "
                        "Do not overwrite or recreate existing files. "
                        + existing_file_evidence
                        + "\nReturn only the corrected JSON schema."
                    )},
                ])
                continue
            print("::error::Qwen multi-role engineering pass failed validation: " + validation_error)
            return 4
    review_relative = os.environ.get("QWEN_REVIEW_PATH", "docs/PRODUCT_FACTORY_QWEN_REVIEW.md")
    if Path(review_relative).is_absolute() or not allowed(review_relative, tracked_set):
        print("::error::Qwen review path is outside the configured allowlist.")
        return 5
    report_path = ROOT / review_relative
    if not report_path.resolve().is_relative_to(ROOT.resolve()) or report_path.is_symlink():
        print("::error::Qwen review path escapes the target repository.")
        return 5
    try:
        write_staged_edits(staged_edits)
    except (OSError, ValueError) as exc:
        print("::error::Qwen staged edits could not be safely applied: " + str(exc)[:500])
        return 5
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        "# Qwen2.5-Coder 3B multi-role engineering review\n\n"
        + "OBJECTIVE: " + task + "\n\n"
        + review_text[:7000] + "\n",
        encoding="utf-8",
    )
    print("Qwen multi-role engineering council completed in one inference pass.")
    changed = set(git("diff", "--name-only").stdout.splitlines())
    changed.update(git("ls-files", "--others", "--exclude-standard").stdout.splitlines())
    changed.discard("PRODUCT_FACTORY_VERIFICATION.md")
    qwen_touched = set(changed_paths) | {review_relative}
    if not changed or not qwen_touched or any(not allowed(path, tracked_set) for path in qwen_touched):
        print("::error::Qwen edits or review artifact failed the configured path allowlist.")
        return 5
    check = git("diff", "--check", check=False)
    if check.returncode:
        print("::error::Whitespace validation failed.")
        print(check.stderr[-1200:])
        return 6
    print("Qwen fallback produced a bounded patch: " + ", ".join(sorted(changed)))
    return 0

if __name__ == "__main__":
    sys.exit(main())
