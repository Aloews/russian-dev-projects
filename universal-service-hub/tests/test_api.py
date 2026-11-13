from fastapi.testclient import TestClient

from app.config import get_settings
from app.main import app


def client_with_providers(monkeypatch, providers: str):
    monkeypatch.setenv("USH_PROVIDERS_JSON", providers)
    get_settings.cache_clear()
    return TestClient(app)


def test_health_and_empty_registry(monkeypatch):
    with client_with_providers(monkeypatch, "[]") as client:
        assert client.get("/healthz").json()["status"] == "ok"
        assert client.get("/readyz").json()["providers_configured"] == 0
        assert client.get("/v1/providers").json() == []


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
    get_settings.cache_clear()
    try:
        with TestClient(app):
            raise AssertionError("invalid configuration should fail during startup")
    except ValueError as exc:
        assert "JSON array" in str(exc)
    finally:
        get_settings.cache_clear()
