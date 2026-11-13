from fastapi.testclient import TestClient

from app.config import Settings, get_settings
from app.main import app

TEST_TOKEN = "test-token-that-is-long-enough-for-validation-123"


def client_with_providers(monkeypatch, providers: str, database_path):
    monkeypatch.setenv("USH_PROVIDERS_JSON", providers)
    monkeypatch.setenv("USH_API_TOKEN", TEST_TOKEN)
    monkeypatch.setenv("USH_DATABASE_URL", f"sqlite+aiosqlite:///{database_path}")
    monkeypatch.setenv("USH_ENVIRONMENT", "development")
    get_settings.cache_clear()
    return TestClient(app, headers={"Authorization": f"Bearer {TEST_TOKEN}"})


def provider_payload(priority: int = 100, enabled: bool = True):
    return {
        "name": "Provider",
        "capabilities": ["compute", "ai"],
        "priority": priority,
        "enabled": enabled,
        "timeout_seconds": 3.0,
    }


def test_health_and_empty_registry(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "health.db") as client:
        assert client.get("/healthz").json()["status"] == "ok"
        assert client.get("/readyz").json()["providers_configured"] == 0
        assert client.get("/v1/providers").json() == []


def test_provider_api_rejects_missing_and_invalid_tokens(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "auth.db") as client:
        missing = client.get("/v1/providers", headers={"Authorization": ""})
        invalid = client.get("/v1/providers", headers={"Authorization": "Bearer wrong-token"})
        assert missing.status_code == 401
        assert invalid.status_code == 401


def test_provider_api_rejects_unconfigured_authentication(monkeypatch, tmp_path):
    monkeypatch.setenv("USH_PROVIDERS_JSON", "[]")
    monkeypatch.setenv("USH_DATABASE_URL", f"sqlite+aiosqlite:///{tmp_path / 'no-auth.db'}")
    monkeypatch.delenv("USH_API_TOKEN", raising=False)
    get_settings.cache_clear()
    with TestClient(app) as client:
        response = client.get("/v1/providers")
        assert response.status_code == 503
    get_settings.cache_clear()


def test_provider_registry_crud_and_persistence(monkeypatch, tmp_path):
    database_path = tmp_path / "persistent.db"
    with client_with_providers(monkeypatch, "[]", database_path) as client:
        created = client.put("/v1/providers/primary", json=provider_payload(priority=100))
        assert created.status_code == 200
        assert created.json()["id"] == "primary"
        assert client.get("/v1/providers").json()[0]["name"] == "Provider"

    with client_with_providers(monkeypatch, "[]", database_path) as client:
        providers = client.get("/v1/providers").json()
        assert len(providers) == 1
        assert providers[0]["id"] == "primary"
        deleted = client.delete("/v1/providers/primary")
        assert deleted.status_code == 204
        assert client.get("/v1/providers").json() == []


def test_selects_highest_priority_eligible_provider(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "selection.db") as client:
        client.put("/v1/providers/backup", json=provider_payload(priority=10))
        client.put("/v1/providers/primary", json=provider_payload(priority=100))
        response = client.post("/v1/providers/select", json={"capability": "ai"})
        assert response.status_code == 200
        assert response.json()["provider"]["id"] == "primary"


def test_selection_excludes_disabled_and_explicitly_excluded_providers(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "exclusion.db") as client:
        client.put("/v1/providers/primary", json=provider_payload(priority=100, enabled=False))
        client.put("/v1/providers/backup", json=provider_payload(priority=10))
        response = client.post("/v1/providers/select", json={"capability": "compute"})
        assert response.json()["provider"]["id"] == "backup"
        response = client.post(
            "/v1/providers/select",
            json={"capability": "compute", "exclude": ["backup"]},
        )
        assert response.json()["provider"] is None


def test_unknown_provider_returns_404(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "missing.db") as client:
        response = client.post(
            "/v1/providers/health-check",
            json={"provider_id": "missing"},
        )
        assert response.status_code == 404


def test_invalid_provider_configuration_fails_fast(monkeypatch, tmp_path):
    monkeypatch.setenv("USH_PROVIDERS_JSON", '{"not":"an array"}')
    monkeypatch.setenv("USH_API_TOKEN", TEST_TOKEN)
    monkeypatch.setenv("USH_DATABASE_URL", f"sqlite+aiosqlite:///{tmp_path / 'invalid.db'}")
    get_settings.cache_clear()
    try:
        with TestClient(app):
            raise AssertionError("invalid provider configuration should fail during startup")
    except TypeError as exc:
        assert "JSON array" in str(exc)
    finally:
        get_settings.cache_clear()


def test_production_requires_api_token(monkeypatch, tmp_path):
    monkeypatch.setenv("USH_ENVIRONMENT", "production")
    monkeypatch.setenv("USH_PROVIDERS_JSON", "[]")
    monkeypatch.setenv("USH_DATABASE_URL", "postgresql+asyncpg://user:pass@localhost/hub")
    monkeypatch.delenv("USH_API_TOKEN", raising=False)
    get_settings.cache_clear()
    try:
        with TestClient(app):
            raise AssertionError("production startup must fail without an API token")
    except RuntimeError as exc:
        assert "USH_API_TOKEN" in str(exc)
    finally:
        get_settings.cache_clear()


def test_production_requires_postgresql(monkeypatch, tmp_path):
    monkeypatch.setenv("USH_ENVIRONMENT", "production")
    monkeypatch.setenv("USH_PROVIDERS_JSON", "[]")
    monkeypatch.setenv("USH_DATABASE_URL", f"sqlite+aiosqlite:///{tmp_path / 'production.db'}")
    monkeypatch.setenv("USH_API_TOKEN", TEST_TOKEN)
    get_settings.cache_clear()
    try:
        with TestClient(app):
            raise AssertionError("production startup must require PostgreSQL")
    except RuntimeError as exc:
        assert "PostgreSQL" in str(exc)
    finally:
        get_settings.cache_clear()


def test_health_probe_without_endpoint_is_not_configured(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "probe.db") as client:
        client.put("/v1/providers/local", json=provider_payload())
        response = client.post(
            "/v1/providers/health-check",
            json={"provider_id": "local"},
        )
        assert response.status_code == 200
        assert response.json()[0]["status"] == "not_configured"


def test_environment_seed_does_not_overwrite_database_changes(monkeypatch, tmp_path):
    database_path = tmp_path / "seed.db"
    original = (
        '[{"id":"primary","name":"Primary","capabilities":["ai"],"priority":90}]'
    )
    changed = (
        '[{"id":"primary","name":"Changed","capabilities":["compute"],"priority":5}]'
    )
    with client_with_providers(monkeypatch, original, database_path) as client:
        assert client.get("/v1/providers").json()[0]["priority"] == 90

    with client_with_providers(monkeypatch, changed, database_path) as client:
        provider = client.get("/v1/providers").json()[0]
        assert provider["name"] == "Primary"
        assert provider["priority"] == 90
        assert provider["capabilities"] == ["ai"]


def test_invalid_provider_id_is_rejected(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "invalid-id.db") as client:
        response = client.put("/v1/providers/not valid", json=provider_payload())
        assert response.status_code == 422


def test_normalizes_standard_postgres_urls():
    settings = Settings(database_url="postgresql://user:pass@db.example/hub")
    assert settings.database_url == "postgresql+asyncpg://user:pass@db.example/hub"


def ai_provider_payload(priority: int, api_key_env: str = "TEST_AI_API_KEY"):
    return {
        "name": "AI Provider",
        "capabilities": ["ai.chat"],
        "priority": priority,
        "enabled": True,
        "adapter_type": "openai_compatible",
        "model": "model-test",
        "base_url": "https://api.example/v1",
        "api_key_env": api_key_env,
        "supports_idempotency": True,
    }


def test_ai_chat_routes_to_configured_adapter(monkeypatch, tmp_path):
    from app.adapters import AdapterResult

    class FakeAdapter:
        async def chat(self, **kwargs):
            assert kwargs["api_key"] == "fake-ai-key"
            return AdapterResult(content="generated", usage={"total_tokens": 4})

    monkeypatch.setenv("TEST_AI_API_KEY", "fake-ai-key")
    monkeypatch.setattr("app.main.create_ai_adapter", lambda adapter_type: FakeAdapter())
    with client_with_providers(monkeypatch, "[]", tmp_path / "ai.db") as client:
        provider = client.put(
            "/v1/providers/starimg",
            json=ai_provider_payload(priority=100, api_key_env="TEST_AI_API_KEY"),
        )
        assert provider.status_code == 200
        assert provider.json()["credentials_configured"] is True
        response = client.post(
            "/v1/ai/chat",
            json={"messages": [{"role": "user", "content": "Hello"}]},
        )
        assert response.status_code == 200
        assert response.json()["provider_id"] == "starimg"
        assert response.json()["content"] == "generated"
        assert "fake-ai-key" not in response.text
        events = client.get("/v1/audit").json()
        ai_event = next(item for item in events if item["action"] == "ai.chat.success")
        assert ai_event["resource_id"] == "starimg"
        assert ai_event["details"]["usage"]["total_tokens"] == 4
        assert "Hello" not in str(ai_event)
        assert "fake-ai-key" not in str(ai_event)


def test_ai_chat_fails_over_only_with_idempotency_key(monkeypatch, tmp_path):
    from app.adapters import AdapterResult, ProviderAdapterError

    class FailThenSucceedAdapter:
        def __init__(self):
            self.calls = 0

        async def chat(self, **kwargs):
            self.calls += 1
            if self.calls == 1:
                raise ProviderAdapterError(
                    "provider_http_429",
                    status_code=429,
                    retryable=True,
                )
            return AdapterResult(content="backup response", usage={})

    fake_adapter = FailThenSucceedAdapter()
    monkeypatch.setenv("TEST_AI_API_KEY", "fake-ai-key")
    monkeypatch.setattr("app.main.create_ai_adapter", lambda adapter_type: fake_adapter)
    with client_with_providers(monkeypatch, "[]", tmp_path / "failover.db") as client:
        client.put(
            "/v1/providers/primary",
            json=ai_provider_payload(priority=100),
        )
        client.put(
            "/v1/providers/backup",
            json=ai_provider_payload(priority=50),
        )
        response = client.post(
            "/v1/ai/chat",
            json={
                "messages": [{"role": "user", "content": "Hello"}],
                "idempotency_key": "request-key-12345",
            },
        )
        assert response.status_code == 200
        assert response.json()["provider_id"] == "backup"
        assert response.json()["attempts"] == 2
        providers = {item["id"]: item for item in client.get("/v1/providers").json()}
        assert providers["primary"]["health_status"] == "unknown"


def test_ai_chat_does_not_retry_billable_request_without_idempotency(monkeypatch, tmp_path):
    from app.adapters import ProviderAdapterError

    class AlwaysFailAdapter:
        async def chat(self, **kwargs):
            raise ProviderAdapterError(
                "provider_http_503",
                status_code=503,
                retryable=True,
            )

    monkeypatch.setenv("TEST_AI_API_KEY", "fake-ai-key")
    monkeypatch.setattr("app.main.create_ai_adapter", lambda adapter_type: AlwaysFailAdapter())
    with client_with_providers(monkeypatch, "[]", tmp_path / "no-retry.db") as client:
        client.put("/v1/providers/primary", json=ai_provider_payload(priority=100))
        client.put("/v1/providers/backup", json=ai_provider_payload(priority=50))
        response = client.post(
            "/v1/ai/chat",
            json={"messages": [{"role": "user", "content": "Hello"}]},
        )
        assert response.status_code == 502


def test_ai_provider_rejects_insecure_or_secret_bearing_base_url(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "unsafe-url.db") as client:
        payload = ai_provider_payload(priority=100)
        payload["base_url"] = "https://api.example/v1?api_key=leak"
        response = client.put("/v1/providers/unsafe", json=payload)
        assert response.status_code == 422


def test_ai_chat_requires_provider_secret(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "missing-key.db") as client:
        client.put(
            "/v1/providers/starimg",
            json=ai_provider_payload(priority=100, api_key_env="MISSING_STARIMG_KEY"),
        )
        response = client.post(
            "/v1/ai/chat",
            json={"messages": [{"role": "user", "content": "Hello"}]},
        )
        assert response.status_code == 503
        provider = client.get("/v1/providers").json()[0]
        assert provider["credentials_configured"] is False
        assert "api_key_env" not in provider
        assert "base_url" not in provider


def test_gemini_cannot_be_marked_idempotent(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "gemini.db") as client:
        payload = {
            "name": "Gemini",
            "capabilities": ["ai.chat"],
            "adapter_type": "gemini",
            "model": "gemini-test",
            "base_url": "https://generativelanguage.googleapis.com",
            "api_key_env": "GEMINI_API_KEY",
            "supports_idempotency": True,
        }
        response = client.put("/v1/providers/gemini", json=payload)
        assert response.status_code == 422


def test_ai_chat_returns_rate_limit_without_cross_provider_retry(monkeypatch, tmp_path):
    from app.adapters import ProviderAdapterError

    class RateLimitedAdapter:
        def __init__(self):
            self.calls = 0

        async def chat(self, **kwargs):
            self.calls += 1
            raise ProviderAdapterError(
                "provider_http_429",
                status_code=429,
                retryable=True,
            )

    adapter = RateLimitedAdapter()
    monkeypatch.setenv("TEST_AI_API_KEY", "fake-ai-key")
    monkeypatch.setattr("app.main.create_ai_adapter", lambda adapter_type: adapter)
    with client_with_providers(monkeypatch, "[]", tmp_path / "rate-limit.db") as client:
        client.put("/v1/providers/primary", json=ai_provider_payload(priority=100))
        client.put("/v1/providers/backup", json=ai_provider_payload(priority=50))
        response = client.post(
            "/v1/ai/chat",
            json={"messages": [{"role": "user", "content": "Hello"}]},
        )
        assert response.status_code == 429
        assert adapter.calls == 1


def test_provider_endpoints_reject_private_and_non_https_destinations(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "outbound-policy.db") as client:
        payload = ai_provider_payload(priority=100)
        payload["base_url"] = "https://127.0.0.1/v1"
        response = client.put("/v1/providers/private-ip", json=payload)
        assert response.status_code == 422

        payload = ai_provider_payload(priority=100)
        payload["base_url"] = "http://api.example/v1"
        response = client.put("/v1/providers/plain-http", json=payload)
        assert response.status_code == 422

        payload = provider_payload()
        payload["health_url"] = "https://169.254.169.254/latest/meta-data/"
        response = client.put("/v1/providers/metadata", json=payload)
        assert response.status_code == 422


def test_provider_mutations_are_audited_without_secret_values(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "audit.db") as client:
        payload = ai_provider_payload(priority=100)
        client.put("/v1/providers/audited", json=payload)
        client.delete("/v1/providers/audited")
        events = client.get("/v1/audit").json()
        actions = [item["action"] for item in events]
        assert "provider.upsert" in actions
        assert "provider.delete" in actions
        assert all(item["actor"] == "service-token" for item in events)
        assert "TEST_AI_API_KEY" not in str(events)


def test_project_tokens_are_hashed_scoped_and_revocable(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "projects.db") as client:
        created = client.post(
            "/v1/projects",
            json={
                "id": "alpha",
                "name": "Alpha Project",
                "allowed_capabilities": ["ai.chat"],
                "allowed_provider_ids": ["primary"],
            },
        )
        assert created.status_code == 200
        token_response = client.post(
            "/v1/projects/alpha/tokens",
            json={"name": "production", "role": "operator"},
        )
        assert token_response.status_code == 200
        raw_token = token_response.json()["token"]
        assert len(raw_token) >= 40

        listed = client.get("/v1/projects/alpha/tokens").json()
        assert len(listed) == 1
        assert "token" not in listed[0]
        assert "token_hash" not in listed[0]
        assert raw_token not in str(listed)

        denied_admin = client.get(
            "/v1/providers",
            headers={"Authorization": f"Bearer {raw_token}"},
        )
        assert denied_admin.status_code == 403

        revoked = client.delete(
            f"/v1/projects/alpha/tokens/{token_response.json()['id']}"
        )
        assert revoked.status_code == 204
        denied_after_revoke = client.post(
            "/v1/ai/chat",
            headers={"Authorization": f"Bearer {raw_token}"},
            json={"messages": [{"role": "user", "content": "Hello"}]},
        )
        assert denied_after_revoke.status_code == 401


def test_project_token_can_only_route_allowed_provider_and_capability(monkeypatch, tmp_path):
    from app.adapters import AdapterResult

    class FakeAdapter:
        async def chat(self, **kwargs):
            return AdapterResult(content="scoped response", usage={"total_tokens": 2})

    monkeypatch.setenv("TEST_AI_API_KEY", "fake-ai-key")
    monkeypatch.setattr("app.main.create_ai_adapter", lambda adapter_type: FakeAdapter())
    with client_with_providers(monkeypatch, "[]", tmp_path / "scoped.db") as client:
        client.put("/v1/providers/primary", json=ai_provider_payload(priority=100))
        client.put("/v1/providers/backup", json=ai_provider_payload(priority=50))
        client.post(
            "/v1/projects",
            json={
                "id": "alpha",
                "name": "Alpha Project",
                "allowed_capabilities": ["ai.chat"],
                "allowed_provider_ids": ["primary"],
            },
        )
        token_response = client.post(
            "/v1/projects/alpha/tokens",
            json={"name": "runtime", "role": "operator"},
        )
        token = token_response.json()["token"]
        headers = {"Authorization": f"Bearer {token}"}

        response = client.post(
            "/v1/ai/chat",
            headers=headers,
            json={"messages": [{"role": "user", "content": "Hello"}]},
        )
        assert response.status_code == 200
        assert response.json()["provider_id"] == "primary"

        denied = client.post(
            "/v1/providers/select",
            headers=headers,
            json={"capability": "compute"},
        )
        assert denied.status_code == 403


def test_project_viewer_cannot_invoke_ai(monkeypatch, tmp_path):
    with client_with_providers(monkeypatch, "[]", tmp_path / "viewer.db") as client:
        client.post(
            "/v1/projects",
            json={
                "id": "read-only",
                "name": "Read Only",
                "allowed_capabilities": ["ai.chat"],
            },
        )
        token_response = client.post(
            "/v1/projects/read-only/tokens",
            json={"name": "viewer", "role": "viewer"},
        )
        response = client.post(
            "/v1/ai/chat",
            headers={"Authorization": f"Bearer {token_response.json()['token']}"},
            json={"messages": [{"role": "user", "content": "Hello"}]},
        )
        assert response.status_code == 403
