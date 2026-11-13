from __future__ import annotations

import re
from contextlib import asynccontextmanager
from datetime import UTC, datetime
from time import perf_counter
from typing import Annotated

import httpx
from fastapi import Depends, FastAPI, HTTPException, Path
from sqlalchemy import func, select, text
from sqlalchemy.ext.asyncio import AsyncSession

from app.auth import require_api_token
from app.config import get_settings
from app.database import create_database, get_session
from app.models import Base, ProviderRecord
from app.schemas import (
    HealthCheckResult,
    ProviderHealthProbeRequest,
    ProviderSelectionRequest,
    ProviderSelectionResponse,
    ProviderUpsertRequest,
    ProviderView,
)

PROVIDER_ID_PATTERN = re.compile(r"^[a-zA-Z0-9._-]+$")


def view(record: ProviderRecord) -> ProviderView:
    return ProviderView(
        id=record.id,
        name=record.name,
        capabilities=record.capabilities,
        priority=record.priority,
        enabled=record.enabled,
        health_status=record.health_status,
        checked_at=record.checked_at,
        latency_ms=record.latency_ms,
    )


async def probe_provider(record: ProviderRecord) -> HealthCheckResult:
    checked_at = datetime.now(UTC)
    if not record.enabled:
        record.health_status = "unhealthy"
        record.health_detail = "Provider is disabled"
        record.latency_ms = None
    elif not record.health_url:
        record.health_status = "unknown"
        record.health_detail = "No health endpoint configured"
        record.latency_ms = None
    else:
        started = perf_counter()
        try:
            async with httpx.AsyncClient(
                timeout=record.timeout_seconds,
                follow_redirects=False,
            ) as client:
                response = await client.get(record.health_url)
            if response.is_success:
                record.health_status = "healthy"
                record.health_detail = None
            else:
                record.health_status = "unhealthy"
                record.health_detail = f"HTTP {response.status_code}"
        except httpx.HTTPError as exc:
            record.health_status = "unhealthy"
            record.health_detail = type(exc).__name__
        record.latency_ms = round((perf_counter() - started) * 1000, 2)
    record.checked_at = checked_at
    status = record.health_status if record.health_status != "unknown" else "not_configured"
    return HealthCheckResult(
        provider_id=record.id,
        status=status,
        latency_ms=record.latency_ms,
        checked_at=checked_at,
        detail=record.health_detail,
    )


@asynccontextmanager
async def lifespan(app: FastAPI):
    settings = get_settings()
    if settings.environment.lower() == "production":
        if settings.api_token is None:
            raise RuntimeError("USH_API_TOKEN must be configured in production")
        if not settings.database_url.startswith("postgresql+asyncpg://"):
            raise RuntimeError("USH_DATABASE_URL must use PostgreSQL in production")
    initial_providers = settings.providers()
    engine, session_factory = create_database(settings.database_url)
    app.state.engine = engine
    app.state.session_factory = session_factory
    try:
        if settings.database_url.startswith("sqlite"):
            async with engine.begin() as connection:
                await connection.run_sync(Base.metadata.create_all)
        async with engine.connect() as connection:
            await connection.execute(text("SELECT 1"))
        async with session_factory() as session:
            for config in initial_providers:
                if await session.get(ProviderRecord, config.id) is None:
                    session.add(
                        ProviderRecord(
                            id=config.id,
                            name=config.name,
                            capabilities=config.capabilities,
                            priority=config.priority,
                            enabled=config.enabled,
                            health_url=config.health_url,
                            timeout_seconds=config.timeout_seconds,
                        )
                    )
            await session.commit()
        yield
    finally:
        await engine.dispose()


app = FastAPI(title="Universal Service Hub", version="0.3.0", lifespan=lifespan)


@app.get("/healthz")
async def healthz():
    return {"status": "ok", "timestamp": datetime.now(UTC)}


@app.get("/readyz")
async def readyz(session: Annotated[AsyncSession, Depends(get_session)]):
    try:
        count = await session.scalar(select(func.count()).select_from(ProviderRecord))
    except Exception as exc:
        raise HTTPException(status_code=503, detail="Database is unavailable") from exc
    return {"status": "ready", "providers_configured": count or 0}


@app.get(
    "/v1/providers",
    response_model=list[ProviderView],
    dependencies=[Depends(require_api_token)],
)
async def list_providers(session: Annotated[AsyncSession, Depends(get_session)]):
    result = await session.scalars(
        select(ProviderRecord).order_by(ProviderRecord.priority.desc(), ProviderRecord.id.asc())
    )
    return [view(record) for record in result.all()]


@app.put(
    "/v1/providers/{provider_id}",
    response_model=ProviderView,
    dependencies=[Depends(require_api_token)],
)
async def upsert_provider(
    payload: ProviderUpsertRequest,
    session: Annotated[AsyncSession, Depends(get_session)],
    provider_id: Annotated[str, Path(min_length=1, max_length=80, pattern=PROVIDER_ID_PATTERN.pattern)],
):
    record = await session.get(ProviderRecord, provider_id)
    if record is None:
        record = ProviderRecord(id=provider_id, name=payload.name)
        session.add(record)
    record.name = payload.name
    record.capabilities = payload.capabilities
    record.priority = payload.priority
    record.enabled = payload.enabled
    record.health_url = payload.health_url
    record.timeout_seconds = payload.timeout_seconds
    record.health_status = "unknown"
    record.checked_at = None
    record.latency_ms = None
    record.health_detail = None
    await session.commit()
    await session.refresh(record)
    return view(record)


@app.delete(
    "/v1/providers/{provider_id}",
    status_code=204,
    dependencies=[Depends(require_api_token)],
)
async def delete_provider(
    provider_id: Annotated[str, Path(min_length=1, max_length=80, pattern=PROVIDER_ID_PATTERN.pattern)],
    session: Annotated[AsyncSession, Depends(get_session)],
):
    record = await session.get(ProviderRecord, provider_id)
    if record is None:
        raise HTTPException(status_code=404, detail="Provider not found")
    await session.delete(record)
    await session.commit()


@app.post(
    "/v1/providers/health-check",
    response_model=list[HealthCheckResult],
    dependencies=[Depends(require_api_token)],
)
async def health_check(
    payload: ProviderHealthProbeRequest,
    session: Annotated[AsyncSession, Depends(get_session)],
):
    if payload.provider_id:
        record = await session.get(ProviderRecord, payload.provider_id)
        if record is None:
            raise HTTPException(status_code=404, detail="Provider not found")
        records = [record]
    else:
        result = await session.scalars(select(ProviderRecord))
        records = result.all()
    checks = [await probe_provider(record) for record in records]
    await session.commit()
    return checks


@app.post(
    "/v1/providers/select",
    response_model=ProviderSelectionResponse,
    dependencies=[Depends(require_api_token)],
)
async def select_provider(
    payload: ProviderSelectionRequest,
    session: Annotated[AsyncSession, Depends(get_session)],
):
    result = await session.scalars(
        select(ProviderRecord).where(ProviderRecord.enabled.is_(True))
    )
    excluded = set(payload.exclude)
    candidates = [
        record
        for record in result.all()
        if payload.capability in record.capabilities
        and record.id not in excluded
        and record.health_status != "unhealthy"
    ]
    if not candidates:
        return ProviderSelectionResponse(
            capability=payload.capability,
            provider=None,
            reason="No enabled provider is eligible for this capability",
        )
    selected = min(candidates, key=lambda record: (-record.priority, record.id))
    return ProviderSelectionResponse(
        capability=payload.capability,
        provider=view(selected),
        reason="Highest-priority eligible provider",
    )
