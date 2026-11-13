from __future__ import annotations

import asyncio
import os
import re
from contextlib import asynccontextmanager
from datetime import UTC, datetime
from time import perf_counter
from typing import Annotated

import httpx
from fastapi import Depends, FastAPI, HTTPException, Path, Query
from sqlalchemy import func, select, text
from sqlalchemy.ext.asyncio import AsyncSession

from app.adapters import ProviderAdapterError, create_ai_adapter
from app.auth import require_api_token
from app.config import get_settings
from app.database import create_database, get_session
from app.models import AuditEvent, Base, ProviderRecord
from app.outbound import UnsafeDestinationError, ensure_public_destination
from app.schemas import (
    AIChatRequest,
    AIChatResponse,
    AuditEventView,
    HealthCheckResult,
    ProviderHealthProbeRequest,
    ProviderSelectionRequest,
    ProviderSelectionResponse,
    ProviderUpsertRequest,
    ProviderView,
)

PROVIDER_ID_PATTERN = re.compile(r"^[a-zA-Z0-9._-]+$")


def audit_event(
    session: AsyncSession,
    *,
    action: str,
    resource_type: str,
    resource_id: str,
    details: dict | None = None,
) -> None:
    session.add(
        AuditEvent(
            actor="service-token",
            action=action,
            resource_type=resource_type,
            resource_id=resource_id,
            details=details or {},
        )
    )


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
        adapter_type=record.adapter_type,
        model=record.model,
        credentials_configured=bool(record.api_key_env and os.environ.get(record.api_key_env)),
        supports_idempotency=record.supports_idempotency,
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
            await ensure_public_destination(record.health_url)
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
        except UnsafeDestinationError as exc:
            record.health_status = "unhealthy"
            record.health_detail = str(exc)
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
                            adapter_type=config.adapter_type,
                            model=config.model,
                            base_url=config.base_url,
                            api_key_env=config.api_key_env,
                            supports_idempotency=config.supports_idempotency,
                        )
                    )
            await session.commit()
        yield
    finally:
        await engine.dispose()


app = FastAPI(title="Universal Service Hub", version="0.4.0", lifespan=lifespan)


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


@app.get(
    "/v1/audit",
    response_model=list[AuditEventView],
    dependencies=[Depends(require_api_token)],
)
async def list_audit_events(
    session: Annotated[AsyncSession, Depends(get_session)],
    limit: Annotated[int, Query(ge=1, le=100)] = 50,
):
    result = await session.scalars(
        select(AuditEvent).order_by(AuditEvent.occurred_at.desc(), AuditEvent.id.desc()).limit(limit)
    )
    return [AuditEventView.model_validate(event, from_attributes=True) for event in result.all()]


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
    record.adapter_type = payload.adapter_type
    record.model = payload.model
    record.base_url = payload.base_url
    record.api_key_env = payload.api_key_env
    record.supports_idempotency = payload.supports_idempotency
    record.health_status = "unknown"
    record.checked_at = None
    record.latency_ms = None
    record.health_detail = None
    audit_event(
        session,
        action="provider.upsert",
        resource_type="provider",
        resource_id=provider_id,
        details={
            "adapter_type": payload.adapter_type,
            "model": payload.model,
            "capabilities": payload.capabilities,
            "enabled": payload.enabled,
            "priority": payload.priority,
        },
    )
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
    audit_event(
        session,
        action="provider.delete",
        resource_type="provider",
        resource_id=provider_id,
        details={"adapter_type": record.adapter_type},
    )
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
        result = await session.scalars(
            select(ProviderRecord)
            .order_by(ProviderRecord.priority.desc(), ProviderRecord.id.asc())
            .limit(payload.limit)
        )
        records = result.all()
    semaphore = asyncio.Semaphore(10)

    async def bounded_probe(record: ProviderRecord) -> HealthCheckResult:
        async with semaphore:
            return await probe_provider(record)

    checks = await asyncio.gather(*(bounded_probe(record) for record in records))
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


@app.post(
    "/v1/ai/chat",
    response_model=AIChatResponse,
    dependencies=[Depends(require_api_token)],
)
async def ai_chat(
    payload: AIChatRequest,
    session: Annotated[AsyncSession, Depends(get_session)],
):
    result = await session.scalars(
        select(ProviderRecord)
        .where(ProviderRecord.enabled.is_(True))
        .order_by(ProviderRecord.priority.desc(), ProviderRecord.id.asc())
    )
    excluded = set(payload.exclude)
    candidates = [
        record
        for record in result.all()
        if payload.capability in record.capabilities
        and record.id not in excluded
        and record.health_status != "unhealthy"
        and record.adapter_type != "none"
        and record.model
        and record.base_url
        and record.api_key_env
    ]
    if not candidates:
        raise HTTPException(status_code=503, detail="No eligible AI provider is configured")

    configured_candidates = [
        (record, os.environ.get(record.api_key_env or ""))
        for record in candidates
    ]
    configured_candidates = [
        (record, api_key) for record, api_key in configured_candidates if api_key
    ]
    if not configured_candidates:
        raise HTTPException(status_code=503, detail="No eligible AI provider has configured credentials")

    attempts = 0
    for record, api_key in configured_candidates:
        attempts += 1
        started = perf_counter()
        adapter = create_ai_adapter(record.adapter_type)
        messages = [message.model_dump() for message in payload.messages]
        try:
            result = await adapter.chat(
                base_url=record.base_url or "",
                api_key=api_key or "",
                model=record.model or "",
                messages=messages,
                temperature=payload.temperature,
                max_tokens=payload.max_tokens,
                timeout=record.timeout_seconds,
                idempotency_key=payload.idempotency_key,
            )
        except ProviderAdapterError as exc:
            record.checked_at = datetime.now(UTC)
            record.latency_ms = round((perf_counter() - started) * 1000, 2)
            if exc.status_code != 429:
                record.health_status = "unhealthy"
                record.health_detail = exc.kind
            else:
                record.health_detail = "provider_rate_limited"
            audit_event(
                session,
                action="ai.chat.failure",
                resource_type="provider",
                resource_id=record.id,
                details={
                    "adapter_type": record.adapter_type,
                    "model": record.model,
                    "failure": exc.kind,
                    "status_code": exc.status_code,
                },
            )
            await session.commit()
            can_failover = (
                exc.retryable
                and payload.idempotency_key is not None
                and record.supports_idempotency
                and record.adapter_type != "gemini"
            )
            if can_failover:
                continue
            status_code = 429 if exc.status_code == 429 else 502
            raise HTTPException(
                status_code=status_code,
                detail="AI provider request failed",
            ) from exc

        latency_ms = round((perf_counter() - started) * 1000, 2)
        record.health_status = "healthy"
        record.health_detail = None
        record.checked_at = datetime.now(UTC)
        record.latency_ms = latency_ms
        safe_usage = {
            key: value
            for key, value in result.usage.items()
            if isinstance(value, (int, float)) and not isinstance(value, bool)
        }
        audit_event(
            session,
            action="ai.chat.success",
            resource_type="provider",
            resource_id=record.id,
            details={
                "adapter_type": record.adapter_type,
                "model": record.model,
                "latency_ms": latency_ms,
                "attempts": attempts,
                "usage": safe_usage,
            },
        )
        await session.commit()
        return AIChatResponse(
            provider_id=record.id,
            model=record.model or "",
            content=result.content,
            usage=result.usage,
            latency_ms=latency_ms,
            attempts=attempts,
        )

    raise HTTPException(status_code=502, detail="All eligible AI providers failed")
