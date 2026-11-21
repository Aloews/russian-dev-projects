"""Fail-closed Starimg Anthropic Messages capability probe for Product Factory."""

from __future__ import annotations

import json
import os
import re
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

MODEL_ID = re.compile(r"[A-Za-z0-9._:/-]{1,200}")


def _request(url: str, body: dict[str, Any] | None = None, headers: dict[str, str] | None = None):
    request = urllib.request.Request(
        url,
        data=None if body is None else json.dumps(body).encode("utf-8"),
        headers=headers or {},
        method="GET" if body is None else "POST",
    )
    with urllib.request.urlopen(request, timeout=90) as response:
        return response.status, json.loads(response.read().decode("utf-8"))


def probe(
    base_url: str,
    api_key: str,
    fable_model: str = "",
    claude_model: str = "",
) -> dict[str, Any]:
    result: dict[str, Any] = {
        "status": 0,
        "tool_ok": False,
        "model": "",
        "base_url": "",
        "error_type": "missing_key" if not api_key else "not_probed",
    }
    if not api_key:
        return result

    origin = base_url.rstrip("/")
    if origin.endswith("/v1"):
        origin = origin[:-3]
    parsed = urllib.parse.urlsplit(origin)
    if (
        parsed.scheme != "https"
        or not parsed.netloc
        or parsed.username
        or parsed.password
        or parsed.query
        or parsed.fragment
        or any(char in origin for char in "\r\n\t")
    ):
        result["error_type"] = "invalid_base_url"
        return result

    try:
        status, catalogue = _request(
            origin + "/v1/models",
            headers={"Authorization": "Bearer " + api_key},
        )
        entries = catalogue.get("data") or catalogue.get("models") or []
        model_ids = sorted(
            {
                str(item.get("id"))
                for item in entries
                if isinstance(item, dict)
                and isinstance(item.get("id"), str)
                and MODEL_ID.fullmatch(item["id"])
            }
        )
        fables = [
            model
            for model in model_ids
            if re.search(r"fable.*5[._-]?2|5[._-]?2.*fable", model, re.IGNORECASE)
        ]
        claudes = [
            model
            for model in model_ids
            if re.search(r"claude.*opus|opus.*claude", model, re.IGNORECASE)
        ]
        if not claudes:
            claudes = [
                model
                for model in model_ids
                if re.search(r"claude.*sonnet|sonnet.*claude", model, re.IGNORECASE)
            ]

        if fable_model:
            model = fable_model
            if model not in model_ids:
                result.update(status=status, error_type="configured_fable_model_not_found")
                return result
        elif fables:
            model = fables[0]
        elif claude_model:
            model = claude_model
            if model not in model_ids:
                result.update(status=status, error_type="configured_claude_model_not_found")
                return result
        else:
            model = claudes[0] if claudes else ""

        if status != 200 or not model or not MODEL_ID.fullmatch(model):
            result.update(status=status, error_type="no_verified_anthropic_model")
            return result

        headers = {
            "x-api-key": api_key,
            "anthropic-version": "2023-06-01",
            "content-type": "application/json",
        }
        body = {
            "model": model,
            "max_tokens": 64,
            "messages": [
                {"role": "user", "content": "Call ping exactly once with value STARIMG_TOOL_OK."}
            ],
            "tools": [
                {
                    "name": "ping",
                    "description": "A harmless tool-call compatibility probe.",
                    "input_schema": {
                        "type": "object",
                        "properties": {"value": {"type": "string"}},
                        "required": ["value"],
                        "additionalProperties": False,
                    },
                }
            ],
            "tool_choice": {"type": "tool", "name": "ping"},
        }
        status, response = _request(origin + "/v1/messages", body, headers)
        blocks = response.get("content") or []
        tool_ok = status == 200 and any(
            isinstance(block, dict)
            and block.get("type") == "tool_use"
            and block.get("name") == "ping"
            and isinstance(block.get("input"), dict)
            and block["input"].get("value") == "STARIMG_TOOL_OK"
            for block in blocks
        )
        result.update(
            status=status,
            tool_ok=tool_ok,
            model=model if tool_ok else "",
            base_url=origin if tool_ok else "",
            error_type=None if tool_ok else "tool_call_not_verified",
        )
    except urllib.error.HTTPError as error:
        result.update(status=error.code, error_type="provider_http_error")
    except (TimeoutError, ConnectionError, urllib.error.URLError, OSError):
        result["error_type"] = "provider_connection_error"
    except (ValueError, TypeError, KeyError, json.JSONDecodeError):
        result["error_type"] = "invalid_provider_response"
    except Exception as error:  # fail closed without exposing provider payloads or credentials
        result["error_type"] = type(error).__name__
    return result


def main() -> int:
    result = probe(
        os.environ.get("STARIMG_BASE_URL", "https://ai.starimg.ru"),
        os.environ.get("STARIMG_API_KEY", ""),
        os.environ.get("STARIMG_FABLE_MODEL", "").strip(),
        os.environ.get("STARIMG_CLAUDE_MODEL", "").strip(),
    )
    diagnostic = Path(os.environ.get("RUNNER_TEMP", ".")) / "starimg-diagnostics.json"
    diagnostic.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    output_path = os.environ.get("GITHUB_OUTPUT")
    if output_path:
        with open(output_path, "a", encoding="utf-8") as output:
            output.write("tool_ok=" + str(result["tool_ok"]).lower() + "\n")
            if result["tool_ok"]:
                output.write("model=" + result["model"] + "\n")
                output.write("base_url=" + result["base_url"] + "\n")
    print(
        "Starimg Anthropic Messages: HTTP "
        + str(result["status"])
        + "; tool_call="
        + str(result["tool_ok"])
        + "; model="
        + (result["model"] or "none")
        + "; error="
        + str(result["error_type"] or "none")
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
