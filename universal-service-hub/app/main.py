from __future__ import annotations

import asyncio
import hashlib
import os
import re
import secrets
from contextlib import asynccontextmanager
from datetime import UTC, datetime
from time import perf_counter
from typing import Annotated

import httpx
from fastapi import Depends, FastAPI, HTTPException, Path, Query
from sqlalchemy import func, select, text
from sqlalchemy.ext.asyncio import AsyncSession

from app.adapters import ProviderAdapterError, create_ai_adapter
from app.auth import Principal, require_admin, require_project_operator
from app.config import get_settings
from app.database import create_database, get_session
from app.models import AuditEvent, Base, ProjectRecord, ProjectTokenRecord, ProviderRecord
from app.outbound import UnsafeDestinationError, ensure_public_destination
from app.schemas import (
    AIChatRequest,
    AIChatResponse,
    AuditEventView,
    ProjectCreateRequest,
    ProjectTokenCreateRequest,
    ProjectTokenCreated,
    ProjectTokenView,
    ProjectView,
    ProjectUpdateRequest,
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
    actor: str = "service-token",
) -> None:
    session.add(
        AuditEvent(
            actor=actor,
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


app = FastAPI(title="Universal Service Hub", version="0.5.0", lifespan=lifespan)



async def scope_project_providers(
    session: AsyncSession,
    principal: Principal,
    capability: str,
    records: list[ProviderRecord],
) -> list[ProviderRecord]:
    if principal.is_admin:
        return records
    project = await session.get(ProjectRecord, principal.project_id)
    if project is None:
        raise HTTPException(status_code=403, detail="Project access is no longer active")
    if capability not in project.allowed_capabilities:
        raise HTTPException(status_code=403, detail="Capability is not enabled for this project")
    allowed_ids = set(project.allowed_provider_ids)
    if allowed_ids:
        records = [record for record in records if record.id in allowed_ids]
    return records


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



@app.post(
    "/v1/projects",
    response_model=ProjectView,
    dependencies=[Depends(require_admin)],
)
async def create_project(
    payload: ProjectCreateRequest,
    session: Annotated[AsyncSession, Depends(get_session)],
):
    if await session.get(ProjectRecord, payload.id) is not None:
        raise HTTPException(status_code=409, detail="Project already exists")
    project = ProjectRecord(
        id=payload.id,
        name=payload.name,
        allowed_capabilities=payload.allowed_capabilities,
        allowed_provider_ids=payload.allowed_provider_ids,
    )
    session.add(project)
    audit_event(
        session,
        action="project.create",
        resource_type="project",
        resource_id=project.id,
        details={
            "allowed_capabilities": payload.allowed_capabilities,
            "allowed_provider_ids": payload.allowed_provider_ids,
        },
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


@app.put(
    "/v1/projects/{project_id}",
    response_model=ProjectView,
    dependencies=[Depends(require_admin)],
)
async def update_project(
    project_id: Annotated[str, Path(min_length=1, max_length=80, pattern=PROVIDER_ID_PATTERN.pattern)],
    payload: ProjectUpdateRequest,
    session: Annotated[AsyncSession, Depends(get_session)],
):
    project = await session.get(ProjectRecord, project_id)
    if project is None:
        raise HTTPException(status_code=404, detail="Project not found")
    project.name = payload.name
    project.allowed_capabilities = payload.allowed_capabilities
    project.allowed_provider_ids = payload.allowed_provider_ids
    audit_event(
        session,
        action="project.update",
        resource_type="project",
        resource_id=project_id,
        details={
            "allowed_capabilities": payload.allowed_capabilities,
            "allowed_provider_ids": payload.allowed_provider_ids,
        },
    )
    await session.commit()
    await session.refresh(project)
    return ProjectView.model_validate(project, from_attributes=True)


@app.delete(
    "/v1/projects/{project_id}",
    status_code=204,
    dependencies=[Depends(require_admin)],
)
async def delete_project(
    project_id: Annotated[str, Path(min_length=1, max_length=80, pattern=PROVIDER_ID_PATTERN.pattern)],
    session: Annotated[AsyncSession, Depends(get_session)],
):
    project = await session.get(ProjectRecord, project_id)
    if project is None:
        raise HTTPException(status_code=404, detail="Project not found")
    tokens = await session.scalars(
        select(ProjectTokenRecord).where(ProjectTokenRecord.project_id == project_id)
    )
    for token in tokens.all():
        await session.delete(token)
    audit_event(
        session,
        action="project.delete",
        resource_type="project",
        resource_id=project_id,
        details={"name": project.name},
    )
    await session.delete(project)
    await session.commit()


@app.post(
    "/v1/projects/{project_id}/tokens",
    response_model=ProjectTokenCreated,
)
async def create_project_token(
    project_id: Annotated[str, Path(min_length=1, max_length=80, pattern=PROVIDER_ID_PATTERN.pattern)],
    payload: ProjectTokenCreateRequest,
    session: Annotated[AsyncSession, Depends(get_session)],
    principal: Annotated[Principal, Depends(require_project_operator)],
):
    if not principal.is_admin and (
        principal.role != "owner" or principal.project_id != project_id
    ):
        raise HTTPException(status_code=403, detail="Project owner or administrator required")
    if not principal.is_admin and payload.role == "owner":
        raise HTTPException(status_code=403, detail="Only the administrator can issue owner tokens")
    project = await session.get(ProjectRecord, project_id)
    if project is None:
        raise HTTPException(status_code=404, detail="Project not found")
    raw_token = secrets.token_urlsafe(32)
    token_record = ProjectTokenRecord(
        project_id=project_id,
        name=payload.name,
        role=payload.role,
        token_hash=hashlib.sha256(raw_token.encode("utf-8")).hexdigest(),
    )
    session.add(token_record)
    audit_event(
        session,
        action="project.token.create",
        resource_type="project",
        resource_id=project_id,
        details={"token_name": payload.name, "role": payload.role},
        actor=principal.actor,
    )
    await session.commit()
    await session.refresh(token_record)
    return ProjectTokenCreated(
        id=token_record.id,
        project_id=project_id,
        name=token_record.name,
        role=token_record.role,
        token=raw_token,
    )


@app.get(
    "/v1/projects/{project_id}/tokens",
    response_model=list[ProjectTokenView],
)
async def list_project_tokens(
    project_id: Annotated[str, Path(min_length=1, max_length=80, pattern=PROVIDER_ID_PATTERN.pattern)],
    session: Annotated[AsyncSession, Depends(get_session)],
    principal: Annotated[Principal, Depends(require_project_operator)],
):
    if not principal.is_admin and (
        principal.role != "owner" or principal.project_id != project_id
    ):
        raise HTTPException(status_code=403, detail="Project owner or administrator required")
    if await session.get(ProjectRecord, project_id) is None:
        raise HTTPException(status_code=404, detail="Project not found")
    result = await session.scalars(
        select(ProjectTokenRecord)
        .where(ProjectTokenRecord.project_id == project_id)
        .order_by(ProjectTokenRecord.created_at.desc())
    )
    return [
        ProjectTokenView.model_validate(token, from_attributes=True)
        for token in result.all()
    ]


@app.delete(
    "/v1/projects/{project_id}/tokens/{token_id}",
    status_code=204,
)
async def revoke_project_token(
    project_id: Annotated[str, Path(min_length=1, max_length=80, pattern=PROVIDER_ID_PATTERN.pattern)],
    token_id: str,
    session: Annotated[AsyncSession, Depends(get_session)],
    principal: Annotated[Principal, Depends(require_project_operator)],
):
    if not principal.is_admin and (
        principal.role != "owner" or principal.project_id != project_id
    ):
        raise HTTPException(status_code=403, detail="Project owner or administrator required")
    token = await session.scalar(
        select(ProjectTokenRecord).where(
            ProjectTokenRecord.id == token_id,
            ProjectTokenRecord.project_id == project_id,
        )
    )
    if token is None:
        raise HTTPException(status_code=404, detail="Project token not found")
    if token.revoked_at is None:
        token.revoked_at = datetime.now(UTC)
        audit_event(
            session,
            action="project.token.revoke",
            resource_type="project",
            resource_id=project_id,
            details={"token_id": token_id, "token_name": token.name},
            actor=principal.actor,
        )
        await session.commit()


@app.get(
    "/v1/providers",
    response_model=list[ProviderView],
    dependencies=[Depends(require_admin)],
)
async def list_providers(session: Annotated[AsyncSession, Depends(get_session)]):
    result = await session.scalars(
        select(ProviderRecord).order_by(ProviderRecord.priority.desc(), ProviderRecord.id.asc())
    )
    return [view(record) for record in result.all()]


@app.get(
    "/v1/audit",
    response_model=list[AuditEventView],
    dependencies=[Depends(require_admin)],
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
    dependencies=[Depends(require_admin)],
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
    dependencies=[Depends(require_admin)],
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
    dependencies=[Depends(require_admin)],
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
)
async def select_provider(
    payload: ProviderSelectionRequest,
    session: Annotated[AsyncSession, Depends(get_session)],
    principal: Annotated[Principal, Depends(require_project_operator)],
):
    result = await session.scalars(
        select(ProviderRecord).where(ProviderRecord.enabled.is_(True))
    )
    scoped = await scope_project_providers(
        session, principal, payload.capability, result.all()
    )
    excluded = set(payload.exclude)
    candidates = [
        record
        for record in scoped
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
)
async def ai_chat(
    payload: AIChatRequest,
    session: Annotated[AsyncSession, Depends(get_session)],
    principal: Annotated[Principal, Depends(require_project_operator)],
):
    result = await session.scalars(
        select(ProviderRecord)
        .where(ProviderRecord.enabled.is_(True))
        .order_by(ProviderRecord.priority.desc(), ProviderRecord.id.asc())
    )
    scoped = await scope_project_providers(
        session, principal, payload.capability, result.all()
    )
    excluded = set(payload.exclude)
    candidates = [
        record
        for record in scoped
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
                actor=principal.actor,
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
            actor=principal.actor,
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
