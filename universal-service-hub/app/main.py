from __future__ import annotations

from contextlib import asynccontextmanager
from datetime import datetime, timezone

from fastapi import FastAPI, HTTPException, Request

from app.config import get_settings
from app.registry import ProviderRegistry
from app.schemas import (
    HealthCheckResult,
    ProviderHealthProbeRequest,
    ProviderSelectionRequest,
    ProviderSelectionResponse,
    ProviderView,
)


def view(state) -> ProviderView:
    return ProviderView(
        id=state.config.id,
        name=state.config.name,
        capabilities=state.config.capabilities,
        priority=state.config.priority,
        enabled=state.config.enabled,
        health_status=state.health_status,
        checked_at=state.checked_at,
    )


@asynccontextmanager
async def lifespan(app: FastAPI):
    settings = get_settings()
    app.state.registry = ProviderRegistry(settings.providers())
    yield


app = FastAPI(title="Universal Service Hub", version="0.1.0", lifespan=lifespan)


@app.get("/healthz")
async def healthz():
    return {"status": "ok", "timestamp": datetime.now(timezone.utc)}


@app.get("/readyz")
async def readyz(request: Request):
    registry: ProviderRegistry = request.app.state.registry
    return {"status": "ready", "providers_configured": len(registry.list())}


@app.get("/v1/providers", response_model=list[ProviderView])
async def list_providers(request: Request):
    registry: ProviderRegistry = request.app.state.registry
    return [view(state) for state in registry.list()]


@app.post("/v1/providers/health-check", response_model=list[HealthCheckResult])
async def health_check(payload: ProviderHealthProbeRequest, request: Request):
    registry: ProviderRegistry = request.app.state.registry
    states = registry.list()
    if payload.provider_id:
        if registry.get(payload.provider_id) is None:
            raise HTTPException(status_code=404, detail="Provider not found")
        states = [await registry.probe(payload.provider_id)]
    else:
        states = await registry.probe_all()
    return [
        HealthCheckResult(
            provider_id=state.config.id,
            status=state.health_status if state.health_status != "unknown" else "not_configured",
            latency_ms=state.latency_ms,
            checked_at=state.checked_at or datetime.now(timezone.utc),
            detail=state.detail,
        )
        for state in states
    ]


@app.post("/v1/providers/select", response_model=ProviderSelectionResponse)
async def select_provider(payload: ProviderSelectionRequest, request: Request):
    registry: ProviderRegistry = request.app.state.registry
    selected = registry.select(payload.capability, set(payload.exclude))
    if selected is None:
        return ProviderSelectionResponse(
            capability=payload.capability,
            provider=None,
            reason="No enabled provider is eligible for this capability",
        )
    return ProviderSelectionResponse(
        capability=payload.capability,
        provider=view(selected),
        reason="Highest-priority eligible provider",
    )
