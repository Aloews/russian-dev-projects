from fastapi.testclient import TestClient

from app.config import get_settings
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
