from __future__ import annotations

import json

import httpx
import pytest

from app.adapters import (
    AnthropicAdapter,
    GeminiAdapter,
    OpenAICompatibleAdapter,
    ProviderAdapterError,
)


@pytest.mark.asyncio
async def test_openai_compatible_adapter_normalizes_chat_response():
    def handler(request: httpx.Request) -> httpx.Response:
        assert str(request.url) == "https://api.example/v1/chat/completions"
        assert request.headers["Authorization"] == "Bearer test-secret"
        assert request.headers["Idempotency-Key"] == "request-12345"
        payload = json.loads(request.content)
        assert payload["model"] == "model-a"
        assert payload["messages"][0]["content"] == "Hello"
        return httpx.Response(
            200,
            json={
                "choices": [{"message": {"content": "Hello back"}}],
                "usage": {"total_tokens": 7},
            },
        )

    adapter = OpenAICompatibleAdapter(transport=httpx.MockTransport(handler))
    result = await adapter.chat(
        base_url="https://api.example/v1",
        api_key="test-secret",
        model="model-a",
        messages=[{"role": "user", "content": "Hello"}],
        temperature=0.2,
        max_tokens=100,
        timeout=2,
        idempotency_key="request-12345",
    )
    assert result.content == "Hello back"
    assert result.usage == {"total_tokens": 7}


@pytest.mark.asyncio
async def test_anthropic_adapter_maps_system_and_message_content():
    def handler(request: httpx.Request) -> httpx.Response:
        assert str(request.url) == "https://api.example/v1/messages"
        assert request.headers["x-api-key"] == "test-secret"
        payload = json.loads(request.content)
        assert payload["system"] == "Be concise"
        assert payload["messages"] == [{"role": "user", "content": "Hello"}]
        return httpx.Response(
            200,
            json={
                "content": [{"type": "text", "text": "First"}, {"type": "text", "text": " second"}],
                "usage": {"input_tokens": 2, "output_tokens": 3},
            },
        )

    adapter = AnthropicAdapter(transport=httpx.MockTransport(handler))
    result = await adapter.chat(
        base_url="https://api.example",
        api_key="test-secret",
        model="claude-test",
        messages=[
            {"role": "system", "content": "Be concise"},
            {"role": "user", "content": "Hello"},
        ],
        temperature=0.1,
        max_tokens=100,
        timeout=2,
    )
    assert result.content == "First second"
    assert result.usage["output_tokens"] == 3


@pytest.mark.asyncio
async def test_gemini_adapter_uses_header_auth_and_maps_content():
    def handler(request: httpx.Request) -> httpx.Response:
        assert str(request.url) == (
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-test:generateContent"
        )
        assert request.headers["x-goog-api-key"] == "test-secret"
        payload = json.loads(request.content)
        assert payload["contents"][0]["role"] == "user"
        return httpx.Response(
            200,
            json={
                "candidates": [{"content": {"parts": [{"text": "Gemini response"}]}}],
                "usageMetadata": {"totalTokenCount": 9},
            },
        )

    adapter = GeminiAdapter(transport=httpx.MockTransport(handler))
    result = await adapter.chat(
        base_url="https://generativelanguage.googleapis.com",
        api_key="test-secret",
        model="gemini-test",
        messages=[{"role": "user", "content": "Hello"}],
        temperature=0.3,
        max_tokens=100,
        timeout=2,
    )
    assert result.content == "Gemini response"
    assert result.usage["totalTokenCount"] == 9


@pytest.mark.asyncio
async def test_retryable_provider_status_is_normalized_without_body_leak():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(503, text="private upstream diagnostic")

    adapter = OpenAICompatibleAdapter(transport=httpx.MockTransport(handler))
    with pytest.raises(ProviderAdapterError) as raised:
        await adapter.chat(
            base_url="https://api.example/v1",
            api_key="test-secret",
            model="model-a",
            messages=[{"role": "user", "content": "Hello"}],
            temperature=0.2,
            max_tokens=100,
            timeout=2,
        )
    assert raised.value.retryable is False
    assert raised.value.status_code == 503
    assert "private upstream diagnostic" not in str(raised.value)
