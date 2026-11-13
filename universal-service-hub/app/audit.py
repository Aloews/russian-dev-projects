from __future__ import annotations

from uuid import uuid4

from fastapi import Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.auth import Actor
from app.models import AuditLogRecord


def add_audit_event(
    session: AsyncSession,
    *,
    actor: Actor,
    request: Request,
    action: str,
    resource_type: str,
    resource_id: str | None,
    outcome: str = "success",
    project_id: str | None = None,
    event_metadata: dict | None = None,
) -> None:
    correlation_id = request.headers.get("x-request-id", "")
    if not correlation_id or len(correlation_id) > 80 or not correlation_id.isprintable():
        correlation_id = str(uuid4())
    session.add(
        AuditLogRecord(
            id=str(uuid4()),
            actor_id=actor.id,
            project_id=project_id or actor.project_id,
            action=action,
            resource_type=resource_type,
            resource_id=resource_id,
            outcome=outcome,
            correlation_id=correlation_id,
            event_metadata=event_metadata or {},
        )
    )
