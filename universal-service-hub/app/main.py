from __future__ import annotations

import asyncio
import hashlib
import os
import re
import secrets
from contextlib import asynccontextmanager
from datetime import UTC, datetime, timedelta
from time import perf_counter
from typing import Annotated
from uuid import uuid4

import httpx
from fastapi import Depends, FastAPI, HTTPException, Path, Query, Request
from sqlalchemy import func, or_, select, text
from sqlalchemy.ext.asyncio import AsyncSession

from app.adapters import ProviderAdapterError, create_ai_adapter
from app.audit import add_audit_event
from app.auth import Actor, require_admin, require_scope
from app.config import get_settings
from app.database import create_database, get_session
from app.models import ApiKeyRecord, AuditLogRecord, Base, ProjectRecord, ProviderRecord
from app.schemas import (
    AIChatRequest,
    AIChatResponse,
    ApiKeyCreateRequest,
    ApiKeyCreatedResponse,
    ApiKeyView,
    AuditLogView,
    HealthCheckResult,
    ProjectCreateRequest,
    ProjectView,
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
        project_id=record.project_id,
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


def apply_project_scope(statement, actor: Actor):
    if actor.project_id is not None:
        return statement.where(
            or_(
                ProviderRecord.project_id.is_(None),
                ProviderRecord.project_id == actor.project_id,
            )
        )
    return statement


def can_access_provider(record: ProviderRecord, actor: Actor) -> bool:
    return (
        actor.project_id is None
        or record.project_id is None
        or record.project_id == actor.project_id
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


app = FastAPI(title="Universal Service Hub", version="0.5.0", lifespan=lifespan)


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


@app.post("/v1/projects", response_model=ProjectView)
async def create_project(
    payload: ProjectCreateRequest,
    request: Request,
    session: Annotated[AsyncSession, Depends(get_session)],
    actor: Annotated[Actor, Depends(require_admin)],
):
    if await session.get(ProjectRecord, payload.id) is not None:
        raise HTTPException(status_code=409, detail="Project already exists")
    project = ProjectRecord(id=payload.id, name=payload.name, enabled=True)
    session.add(project)
    await session.flush()
    add_audit_event(
        session,
        actor=actor,
        request=request,
        action="project.create",
        resource_type="project",
        resource_id=project.id,
        project_id=project.id,
    )
    await session.commit()
    await session.refresh(project)
    return ProjectView.model_validate(project, from_attributes=True)


@app.get(
    "/v1/projects",
    response_model=list[ProjectView],
    dependencies=[Depends(require_admin)],
)
async def list_projects(session: Annotated[AsyncSession, Depends(get_session)]):
    result = await session.scalars(select(ProjectRecord).order_by(ProjectRecord.id.asc()))
    return [ProjectView.model_validate(project, from_attributes=True) for project in result.all()]


@app.post("/v1/api-keys", response_model=ApiKeyCreatedResponse)
async def create_api_key(
    payload: ApiKeyCreateRequest,
    request: Request,
    session: Annotated[AsyncSession, Depends(get_session)],
    actor: Annotated[Actor, Depends(require_admin)],
):
    if payload.project_id is not None:
        project = await session.get(ProjectRecord, payload.project_id)
        if project is None or not project.enabled:
            raise HTTPException(status_code=404, detail="Project not found")
    token = f"ush_live_{secrets.token_urlsafe(32)}"
    key_id = str(uuid4())
    expires_at = datetime.now(UTC) + timedelta(days=payload.expires_in_days)
    key = ApiKeyRecord(
        id=key_id,
        name=payload.name,
        token_hash=hashlib.sha256(token.encode("utf-8")).hexdigest(),
        token_prefix=token[:16],
        role=payload.role,
        project_id=payload.project_id,
        scopes=payload.scopes,
        expires_at=expires_at,
    )
    session.add(key)
    add_audit_event(
        session,
        actor=actor,
        request=request,
        action="api_key.create",
        resource_type="api_key",
        resource_id=key_id,
        project_id=payload.project_id,
        event_metadata={"role": payload.role, "scopes": payload.scopes},
    )
    await session.commit()
    return ApiKeyCreatedResponse(
        id=key_id,
        name=payload.name,
        role=payload.role,
        project_id=payload.project_id,
        scopes=payload.scopes,
        token=token,
        expires_at=expires_at,
    )


@app.get(
    "/v1/api-keys",
    response_model=list[ApiKeyView],
    dependencies=[Depends(require_admin)],
)
async def list_api_keys(session: Annotated[AsyncSession, Depends(get_session)]):
    result = await session.scalars(
        select(ApiKeyRecord).order_by(ApiKeyRecord.created_at.desc())
    )
    return [ApiKeyView.model_validate(key, from_attributes=True) for key in result.all()]


@app.delete("/v1/api-keys/{key_id}", status_code=204)
async def revoke_api_key(
    key_id: Annotated[str, Path(min_length=36, max_length=36)],
    request: Request,
    session: Annotated[AsyncSession, Depends(get_session)],
    actor: Annotated[Actor, Depends(require_admin)],
):
    key = await session.get(ApiKeyRecord, key_id)
    if key is None:
        raise HTTPException(status_code=404, detail="API key not found")
    if key.enabled:
        key.enabled = False
        key.revoked_at = datetime.now(UTC)
        add_audit_event(
            session,
            actor=actor,
            request=request,
            action="api_key.revoke",
            resource_type="api_key",
            resource_id=key.id,
            project_id=key.project_id,
            event_metadata={"role": key.role},
        )
        await session.commit()


@app.get(
    "/v1/audit-logs",
    response_model=list[AuditLogView],
    dependencies=[Depends(require_admin)],
)
async def list_audit_logs(
    session: Annotated[AsyncSession, Depends(get_session)],
    limit: Annotated[int, Query(ge=1, le=500)] = 100,
    project_id: str | None = None,
):
    statement = select(AuditLogRecord)
    if project_id:
        statement = statement.where(AuditLogRecord.project_id == project_id)
    result = await session.scalars(
        statement.order_by(AuditLogRecord.created_at.desc()).limit(limit)
    )
    return [AuditLogView.model_validate(item, from_attributes=True) for item in result.all()]


@app.get(
    "/v1/providers",
    response_model=list[ProviderView],
)
async def list_providers(
    session: Annotated[AsyncSession, Depends(get_session)],
    actor: Annotated[Actor, Depends(require_scope("providers:read"))],
):
    statement = apply_project_scope(select(ProviderRecord), actor)
    result = await session.scalars(
        statement.order_by(ProviderRecord.priority.desc(), ProviderRecord.id.asc())
    )
    return [view(record) for record in result.all()]


@app.put(
    "/v1/providers/{provider_id}",
    response_model=ProviderView,
)
async def upsert_provider(
    payload: ProviderUpsertRequest,
    request: Request,
    session: Annotated[AsyncSession, Depends(get_session)],
    actor: Annotated[Actor, Depends(require_scope("providers:write"))],
    provider_id: Annotated[str, Path(min_length=1, max_length=80, pattern=PROVIDER_ID_PATTERN.pattern)],
):
    if actor.project_id is not None and payload.project_id != actor.project_id:
        raise HTTPException(status_code=403, detail="Provider is outside the API key project")
    if payload.project_id is not None:
        project = await session.get(ProjectRecord, payload.project_id)
        if project is None or not project.enabled:
            raise HTTPException(status_code=404, detail="Project not found")
    record = await session.get(ProviderRecord, provider_id)
    if record is not None and not can_access_provider(record, actor):
        raise HTTPException(status_code=404, detail="Provider not found")
    if record is None:
        record = ProviderRecord(id=provider_id, name=payload.name)
        session.add(record)
    record.project_id = payload.project_id
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
    add_audit_event(
        session,
        actor=actor,
        request=request,
        action="provider.upsert",
        resource_type="provider",
        resource_id=record.id,
        project_id=record.project_id,
        event_metadata={"adapter_type": record.adapter_type, "model": record.model},
    )
    await session.commit()
    await session.refresh(record)
    return view(record)


@app.delete(
    "/v1/providers/{provider_id}",
    status_code=204,
)
async def delete_provider(
    request: Request,
    provider_id: Annotated[str, Path(min_length=1, max_length=80, pattern=PROVIDER_ID_PATTERN.pattern)],
    session: Annotated[AsyncSession, Depends(get_session)],
    actor: Annotated[Actor, Depends(require_scope("providers:write"))],
):
    record = await session.get(ProviderRecord, provider_id)
    if record is None or not can_access_provider(record, actor):
        raise HTTPException(status_code=404, detail="Provider not found")
    await session.delete(record)
    add_audit_event(
        session,
        actor=actor,
        request=request,
        action="provider.delete",
        resource_type="provider",
        resource_id=provider_id,
        project_id=record.project_id,
    )
    await session.commit()


@app.post(
    "/v1/providers/health-check",
    response_model=list[HealthCheckResult],
)
async def health_check(
    payload: ProviderHealthProbeRequest,
    request: Request,
    session: Annotated[AsyncSession, Depends(get_session)],
    actor: Annotated[Actor, Depends(require_scope("health:probe"))],
):
    if payload.provider_id:
        record = await session.get(ProviderRecord, payload.provider_id)
        if record is None or not can_access_provider(record, actor):
            raise HTTPException(status_code=404, detail="Provider not found")
        records = [record]
    else:
        statement = apply_project_scope(select(ProviderRecord), actor)
        result = await session.scalars(
            statement.order_by(ProviderRecord.priority.desc(), ProviderRecord.id.asc())
            .limit(payload.limit)
        )
        records = result.all()
    semaphore = asyncio.Semaphore(10)

    async def bounded_probe(record: ProviderRecord) -> HealthCheckResult:
        async with semaphore:
            return await probe_provider(record)

    checks = await asyncio.gather(*(bounded_probe(record) for record in records))
    add_audit_event(
        session,
        actor=actor,
        request=request,
        action="provider.health_check",
        resource_type="provider",
        resource_id=payload.provider_id,
        outcome="success",
        event_metadata={"checked_count": len(checks)},
    )
    await session.commit()
    return checks


@app.post(
    "/v1/providers/select",
    response_model=ProviderSelectionResponse,
)
async def select_provider(
    payload: ProviderSelectionRequest,
    request: Request,
    session: Annotated[AsyncSession, Depends(get_session)],
    actor: Annotated[Actor, Depends(require_scope("routing:select"))],
):
    statement = apply_project_scope(
        select(ProviderRecord).where(ProviderRecord.enabled.is_(True)),
        actor,
    )
    result = await session.scalars(statement)
    excluded = set(payload.exclude)
    candidates = [
        record
        for record in result.all()
        if payload.capability in record.capabilities
        and record.id not in excluded
        and record.health_status != "unhealthy"
    ]
    if not candidates:
        add_audit_event(
            session,
            actor=actor,
            request=request,
            action="provider.select",
            resource_type="provider",
            resource_id=None,
            outcome="no_provider",
            event_metadata={"capability": payload.capability},
        )
        await session.commit()
        return ProviderSelectionResponse(
            capability=payload.capability,
            provider=None,
            reason="No enabled provider is eligible for this capability",
        )
    selected = min(candidates, key=lambda record: (-record.priority, record.id))
    add_audit_event(
        session,
        actor=actor,
        request=request,
        action="provider.select",
        resource_type="provider",
        resource_id=selected.id,
        project_id=selected.project_id,
        event_metadata={"capability": payload.capability},
    )
    await session.commit()
    return ProviderSelectionResponse(
        capability=payload.capability,
        provider=view(selected),
        reason="Highest-priority eligible provider",
    )


@app.post(
    "/v1/ai/chat",
    response_model=AIChatResponse,
)
async def ai_chat(
    payload: AIChatRequest,
    request: Request,
    session: Annotated[AsyncSession, Depends(get_session)],
    actor: Annotated[Actor, Depends(require_scope("ai:chat"))],
):
    statement = apply_project_scope(
        select(ProviderRecord).where(ProviderRecord.enabled.is_(True)),
        actor,
    ).order_by(ProviderRecord.priority.desc(), ProviderRecord.id.asc())
    result = await session.scalars(statement)
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
        add_audit_event(
            session,
            actor=actor,
            request=request,
            action="ai.chat",
            resource_type="ai_request",
            resource_id=None,
            outcome="no_provider",
            event_metadata={"capability": payload.capability},
        )
        await session.commit()
        raise HTTPException(status_code=503, detail="No eligible AI provider is configured")

    configured_candidates = [
        (record, os.environ.get(record.api_key_env or ""))
        for record in candidates
    ]
    configured_candidates = [
        (record, api_key) for record, api_key in configured_candidates if api_key
    ]
    if not configured_candidates:
        add_audit_event(
            session,
            actor=actor,
            request=request,
            action="ai.chat",
            resource_type="ai_request",
            resource_id=None,
            outcome="credentials_missing",
            event_metadata={"candidate_count": len(candidates)},
        )
        await session.commit()
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
            can_failover = (
                exc.retryable
                and payload.idempotency_key is not None
                and record.supports_idempotency
                and record.adapter_type != "gemini"
            )
            add_audit_event(
                session,
                actor=actor,
                request=request,
                action="ai.provider_attempt",
                resource_type="provider",
                resource_id=record.id,
                outcome="retryable_failure" if can_failover else "failed",
                project_id=record.project_id,
                event_metadata={"error_kind": exc.kind, "status_code": exc.status_code},
            )
            await session.commit()
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
        add_audit_event(
            session,
            actor=actor,
            request=request,
            action="ai.chat",
            resource_type="provider",
            resource_id=record.id,
            outcome="success",
            project_id=record.project_id,
            event_metadata={"model": record.model, "attempts": attempts, "latency_ms": latency_ms},
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

    add_audit_event(
        session,
        actor=actor,
        request=request,
        action="ai.chat",
        resource_type="ai_request",
        resource_id=None,
        outcome="all_providers_failed",
        event_metadata={"attempts": attempts},
    )
    await session.commit()
    raise HTTPException(status_code=502, detail="All eligible AI providers failed")
