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
MAX_FILES = 8
MAX_CONTEXT_CHARS = 28000
MAX_FILE_CHARS = 5200
ALLOWED_PREFIXES = (
    "app/",
    "services/platform-api/src/",
    "services/platform-api/tests/",
    "services/platform-api/migrations/",
    "tests/",
    "docs/",
)

def git(*args, check=True, input_text=None):
    return subprocess.run(
        ["git", *args], cwd=ROOT, check=check, text=True,
        input=input_text, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )

def allowed(path, tracked):
    path = path.replace("\\", "/")
    if path.startswith("/") or any(part in (".", "..") for part in path.split("/")):
        return False
    low = path.lower()
    if not path.startswith(ALLOWED_PREFIXES):
        return False
    if any(x in low for x in (".env", "credentials", "service-account", "secret", "private-key")):
        return False
    if low.endswith((".lock", ".min.js", ".map", ".pem", ".key", ".p12", ".pfx")):
        return False
    if path.startswith("services/platform-api/migrations/") and path in tracked:
        return False
    return True

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
    preferred = [
        "docs/MAP.md", "docs/DELIVERY_PLAN.md", "docs/AI_AGENT_WORKFLOW.md",
        "README.md", "app/page.tsx", "app/properties/page.tsx",
        "app/properties/[id]/page.tsx", "app/properties/[id]/enquiry-form.tsx",
        "services/platform-api/src/elite_estate_api.rs",
        "services/platform-api/src/routes.rs",
        "services/platform-api/src/domains/elite_estate.rs",
        "services/platform-api/src/main.rs",
        "services/platform-api/migrations/0001_elite_estate.sql",
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
    verification = read_text("PRODUCT_FACTORY_VERIFICATION.md")
    if verification:
        chunks.append("\n--- FAILED VERIFICATION EVIDENCE ---\n" + scrub(verification[:12000]))
    return "\n".join(chunks)

EDIT_SCHEMA = {
    "type": "object",
    "properties": {
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
    "required": ["edits"],
    "additionalProperties": False,
}

def apply_edits(edits, tracked):
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
    for path, content in staged.items():
        target = ROOT / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")
    return sorted(staged)

def main():
    if not ROOT.is_dir() or not (ROOT / ".git").exists():
        print("::error::Target working tree is missing.")
        return 2
    tracked = git("ls-files").stdout.splitlines()
    task = os.environ.get("OBJECTIVE", "Complete the highest-priority bounded AUREL delivery slice.")
    verification = read_text("PRODUCT_FACTORY_VERIFICATION.md")
    if verification:
        task += "\n\nRepair the actual failures listed in PRODUCT_FACTORY_VERIFICATION.md. Preserve all checks."
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
        "You are a cautious local coding fallback for a production product. "
        "Inspect the provided project evidence; choose one small, complete, testable increment. "
        "Return only JSON matching the supplied schema, with an edits array of exact path/old/new replacements. "
        "For existing files, old must be an exact unique snippet copied from the provided file. "
        "For a new file, old must be empty and new must contain the complete file. "
        "Never return a unified diff, markdown fence or explanation. "
        "Never invent live data, credentials, legal approvals, customers, property inventory or tests. "
        "Do not edit workflows, action definitions, package manifests, lockfiles, credentials, deployment files, "
        "production secrets or existing SQL migrations. Add tests for behavior changes. "
        "Do not weaken or delete tests. Keep changes minimal. No extra keys or explanations."
    )
    if skill:
        system += "\n\nApply this Caveman skill to reduce prose only. Never compress code, paths, identifiers, errors, contracts, negations or tests:\n" + skill
    messages = [
        {"role": "system", "content": system},
        {"role": "user", "content": "OBJECTIVE:\n" + task + "\n\nREPOSITORY EVIDENCE:\n" + context + "\n\nProduce minimal structured edits only, following the JSON schema."},
    ]
    tracked_set = set(tracked)
    for attempt in range(3):
        payload = {
            "model": MODEL, "stream": False, "messages": messages,
            "format": EDIT_SCHEMA,
            "options": {"temperature": 0.1, "num_ctx": 12288, "num_predict": 4096},
        }
        request = urllib.request.Request(
            OLLAMA_URL, data=json.dumps(payload).encode(),
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(request, timeout=1800) as response:
                answer = json.loads(response.read())["message"]["content"]
        except (urllib.error.URLError, TimeoutError, KeyError, ValueError) as exc:
            print(f"::error::Qwen inference failed: {type(exc).__name__}: {exc}")
            return 3
        try:
            response_json = json.loads(answer)
            apply_edits(response_json.get("edits"), tracked_set)
            break
        except (ValueError, OSError, UnicodeError, json.JSONDecodeError) as exc:
            error = str(exc)[:1000]
            messages.extend([
                {"role": "assistant", "content": answer[:12000]},
                {"role": "user", "content": "The structured edit was rejected: " + error + ". Return corrected JSON with exact old text from the provided files."},
            ])
            if attempt == 2:
                print("::error::Qwen could not produce valid structured edits: " + error)
                return 4
    changed = set(git("diff", "--name-only").stdout.splitlines())
    changed.update(git("ls-files", "--others", "--exclude-standard").stdout.splitlines())
    changed.discard("PRODUCT_FACTORY_VERIFICATION.md")
    if not changed or any(not allowed(path, tracked_set) for path in changed):
        print("::error::Final patch failed the path allowlist.")
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
