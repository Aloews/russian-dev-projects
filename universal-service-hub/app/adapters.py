from __future__ import annotations

from dataclasses import dataclass
from typing import Any
from urllib.parse import quote

import httpx

from app.outbound import UnsafeDestinationError, ensure_public_destination


class ProviderAdapterError(Exception):
    def __init__(
        self,
        kind: str,
        *,
        status_code: int | None = None,
        retryable: bool = False,
    ) -> None:
        super().__init__(kind)
        self.kind = kind
        self.status_code = status_code
        self.retryable = retryable


@dataclass
class AdapterResult:
    content: str
    usage: dict[str, Any]


class BaseAIAdapter:
    def __init__(self, transport: httpx.AsyncBaseTransport | None = None) -> None:
        self.transport = transport

    async def post_json(
        self,
        url: str,
        *,
        headers: dict[str, str],
        payload: dict[str, Any],
        timeout: float,
    ) -> dict[str, Any]:
        if self.transport is None:
            try:
                await ensure_public_destination(url)
            except UnsafeDestinationError as exc:
                raise ProviderAdapterError("provider_destination_blocked") from exc
        try:
            async with httpx.AsyncClient(
                timeout=timeout,
                transport=self.transport,
                follow_redirects=False,
            ) as client:
                response = await client.post(url, headers=headers, json=payload)
        except (httpx.ConnectError, httpx.ConnectTimeout, httpx.PoolTimeout) as exc:
            raise ProviderAdapterError("provider_connection_failed", retryable=True) from exc
        except httpx.HTTPError as exc:
            raise ProviderAdapterError("provider_network_error") from exc

        if not response.is_success:
            status_code = response.status_code
            raise ProviderAdapterError(
                f"provider_http_{status_code}",
                status_code=status_code,
                retryable=status_code == 429,
            )
        try:
            data = response.json()
        except ValueError as exc:
            raise ProviderAdapterError("provider_invalid_json") from exc
        if not isinstance(data, dict):
            raise ProviderAdapterError("provider_invalid_response")
        return data


class OpenAICompatibleAdapter(BaseAIAdapter):
    async def chat(
        self,
        *,
        base_url: str,
        api_key: str,
        model: str,
        messages: list[dict[str, str]],
        temperature: float,
        max_tokens: int,
        timeout: float,
        idempotency_key: str | None = None,
    ) -> AdapterResult:
        headers = {"Authorization": f"Bearer {api_key}"}
        if idempotency_key:
            headers["Idempotency-Key"] = idempotency_key
        data = await self.post_json(
            f"{base_url.rstrip('/')}/chat/completions",
            headers=headers,
            payload={
                "model": model,
                "messages": messages,
                "temperature": temperature,
                "max_tokens": max_tokens,
            },
            timeout=timeout,
        )
        try:
            content = data["choices"][0]["message"]["content"]
        except (KeyError, IndexError, TypeError) as exc:
            raise ProviderAdapterError("provider_invalid_response") from exc
        if isinstance(content, list):
            content = "".join(
                item.get("text", "")
                for item in content
                if isinstance(item, dict) and isinstance(item.get("text"), str)
            )
        if not isinstance(content, str):
            raise ProviderAdapterError("provider_invalid_response")
        usage = data.get("usage", {})
        return AdapterResult(content=content, usage=usage if isinstance(usage, dict) else {})


class AnthropicAdapter(BaseAIAdapter):
    async def chat(
        self,
        *,
        base_url: str,
        api_key: str,
        model: str,
        messages: list[dict[str, str]],
        temperature: float,
        max_tokens: int,
        timeout: float,
        idempotency_key: str | None = None,
    ) -> AdapterResult:
        system = "\n\n".join(item["content"] for item in messages if item["role"] == "system")
        turns = [
            {"role": item["role"], "content": item["content"]}
            for item in messages
            if item["role"] != "system"
        ]
        if not turns:
            raise ProviderAdapterError("provider_invalid_request")
        headers = {
            "x-api-key": api_key,
            "anthropic-version": "2023-06-01",
        }
        if idempotency_key:
            headers["Idempotency-Key"] = idempotency_key
        payload: dict[str, Any] = {
            "model": model,
            "messages": turns,
            "temperature": temperature,
            "max_tokens": max_tokens,
        }
        if system:
            payload["system"] = system
        data = await self.post_json(
            f"{base_url.rstrip('/')}/v1/messages",
            headers=headers,
            payload=payload,
            timeout=timeout,
        )
        blocks = data.get("content", [])
        if not isinstance(blocks, list):
            raise ProviderAdapterError("provider_invalid_response")
        content = "".join(
            block.get("text", "")
            for block in blocks
            if isinstance(block, dict)
            and block.get("type") == "text"
            and isinstance(block.get("text"), str)
        )
        if not content:
            raise ProviderAdapterError("provider_empty_response")
        usage = data.get("usage", {})
        return AdapterResult(content=content, usage=usage if isinstance(usage, dict) else {})


class GeminiAdapter(BaseAIAdapter):
    async def chat(
        self,
        *,
        base_url: str,
        api_key: str,
        model: str,
        messages: list[dict[str, str]],
        temperature: float,
        max_tokens: int,
        timeout: float,
        idempotency_key: str | None = None,
    ) -> AdapterResult:
        system = "\n\n".join(item["content"] for item in messages if item["role"] == "system")
        contents = [
            {
                "role": "model" if item["role"] == "assistant" else "user",
                "parts": [{"text": item["content"]}],
            }
            for item in messages
            if item["role"] != "system"
        ]
        if not contents:
            raise ProviderAdapterError("provider_invalid_request")
        payload: dict[str, Any] = {
            "contents": contents,
            "generationConfig": {
                "temperature": temperature,
                "maxOutputTokens": max_tokens,
            },
        }
        if system:
            payload["systemInstruction"] = {"parts": [{"text": system}]}
        data = await self.post_json(
            f"{base_url.rstrip('/')}/v1beta/models/{quote(model, safe='')}:generateContent",
            headers={"x-goog-api-key": api_key},
            payload=payload,
            timeout=timeout,
        )
        try:
            parts = data["candidates"][0]["content"]["parts"]
        except (KeyError, IndexError, TypeError) as exc:
            raise ProviderAdapterError("provider_invalid_response") from exc
        if not isinstance(parts, list):
            raise ProviderAdapterError("provider_invalid_response")
        content = "".join(
            part.get("text", "")
            for part in parts
            if isinstance(part, dict) and isinstance(part.get("text"), str)
        )
        if not content:
            raise ProviderAdapterError("provider_empty_response")
        usage = data.get("usageMetadata", {})
        return AdapterResult(content=content, usage=usage if isinstance(usage, dict) else {})


def create_ai_adapter(
    adapter_type: str,
    transport: httpx.AsyncBaseTransport | None = None,
) -> BaseAIAdapter:
    adapters = {
        "openai_compatible": OpenAICompatibleAdapter,
        "anthropic": AnthropicAdapter,
        "gemini": GeminiAdapter,
    }
    adapter_class = adapters.get(adapter_type)
    if adapter_class is None:
        raise ValueError("Unsupported AI adapter type")
    return adapter_class(transport=transport)
