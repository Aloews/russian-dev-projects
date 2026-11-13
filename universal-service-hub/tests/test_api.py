from fastapi.testclient import TestClient

from app.config import get_settings
from app.main import app

TEST_TOKEN = "test-token-that-is-long-enough-for-validation-123"


def client_with_providers(monkeypatch, providers: str):
    monkeypatch.setenv("USH_PROVIDERS_JSON", providers)
    monkeypatch.setenv("USH_API_TOKEN", TEST_TOKEN)
    get_settings.cache_clear()
    return TestClient(app, headers={"Authorization": f"Bearer {TEST_TOKEN}"})


def test_health_and_empty_registry(monkeypatch):
    with client_with_providers(monkeypatch, "[]") as client:
        assert client.get("/healthz").json()["status"] == "ok"
        assert client.get("/readyz").json()["providers_configured"] == 0
        assert client.get("/v1/providers").json() == []


def test_provider_api_rejects_missing_and_invalid_tokens(monkeypatch):
    with client_with_providers(monkeypatch, "[]") as client:
        missing = client.get("/v1/providers", headers={"Authorization": ""})
        invalid = client.get("/v1/providers", headers={"Authorization": "Bearer wrong-token"})
        assert missing.status_code == 401
        assert invalid.status_code == 401


def test_provider_api_rejects_unconfigured_authentication(monkeypatch):
    monkeypatch.setenv("USH_PROVIDERS_JSON", "[]")
    monkeypatch.delenv("USH_API_TOKEN", raising=False)
    get_settings.cache_clear()
    with TestClient(app) as client:
        response = client.get("/v1/providers")
        assert response.status_code == 503
    get_settings.cache_clear()


def test_selects_highest_priority_eligible_provider(monkeypatch):
    providers = (
        '[{"id":"backup","name":"Backup","capabilities":["ai"],"priority":10},'
        '{"id":"primary","name":"Primary","capabilities":["ai"],"priority":100}]'
    )
    with client_with_providers(monkeypatch, providers) as client:
        response = client.post("/v1/providers/select", json={"capability": "ai"})
        assert response.status_code == 200
        assert response.json()["provider"]["id"] == "primary"


def test_excludes_unhealthy_and_explicitly_excluded_providers(monkeypatch):
    providers = (
        '[{"id":"primary","name":"Primary","capabilities":["compute"],"priority":100},'
        '{"id":"backup","name":"Backup","capabilities":["compute"],"priority":10}]'
    )
    with client_with_providers(monkeypatch, providers) as client:
        registry = app.state.registry
        registry.get("primary").health_status = "unhealthy"
        response = client.post("/v1/providers/select", json={"capability": "compute"})
        assert response.json()["provider"]["id"] == "backup"
        response = client.post(
            "/v1/providers/select",
            json={"capability": "compute", "exclude": ["backup"]},
        )
        assert response.json()["provider"] is None


def test_unknown_provider_returns_404(monkeypatch):
    with client_with_providers(monkeypatch, "[]") as client:
        response = client.post(
            "/v1/providers/health-check",
            json={"provider_id": "missing"},
        )
        assert response.status_code == 404


def test_invalid_provider_configuration_fails_fast(monkeypatch):
    monkeypatch.setenv("USH_PROVIDERS_JSON", '{"not":"an array"}')
    monkeypatch.setenv("USH_API_TOKEN", TEST_TOKEN)
    get_settings.cache_clear()
    try:
        with TestClient(app):
            raise AssertionError("invalid configuration should fail during startup")
    except TypeError as exc:
        assert "JSON array" in str(exc)
    finally:
        get_settings.cache_clear()


def test_production_requires_api_token(monkeypatch):
    monkeypatch.setenv("USH_ENVIRONMENT", "production")
    monkeypatch.setenv("USH_PROVIDERS_JSON", "[]")
    monkeypatch.delenv("USH_API_TOKEN", raising=False)
    get_settings.cache_clear()
    try:
        with TestClient(app):
            raise AssertionError("production startup must fail without an API token")
    except RuntimeError as exc:
        assert "USH_API_TOKEN" in str(exc)
    finally:
        get_settings.cache_clear()
