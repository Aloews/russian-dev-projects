from __future__ import annotations

import hashlib
import secrets
from dataclasses import dataclass
from datetime import UTC, datetime
from typing import Annotated

from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.config import get_settings
from app.database import get_session
from app.models import ApiKeyRecord, ProjectRecord

bearer_scheme = HTTPBearer(auto_error=False)


@dataclass(frozen=True)
class Actor:
    id: str
    role: str
    project_id: str | None
    scopes: frozenset[str]


async def require_api_token(
    credentials: Annotated[
        HTTPAuthorizationCredentials | None,
        Depends(bearer_scheme),
    ],
    session: Annotated[AsyncSession, Depends(get_session)],
) -> Actor:
    configured = get_settings().api_token
    if configured is None:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="API authentication is not configured",
        )
    if credentials is None or credentials.scheme.lower() != "bearer":
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Bearer token required",
            headers={"WWW-Authenticate": "Bearer"},
        )

    supplied = credentials.credentials
    if secrets.compare_digest(supplied, configured.get_secret_value()):
        return Actor(
            id="bootstrap",
            role="admin",
            project_id=None,
            scopes=frozenset({"*"}),
        )

    token_hash = hashlib.sha256(supplied.encode("utf-8")).hexdigest()
    key = await session.scalar(
        select(ApiKeyRecord).where(
            ApiKeyRecord.token_hash == token_hash,
            ApiKeyRecord.enabled.is_(True),
        )
    )
    if key is None or key.revoked_at is not None:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid bearer token",
            headers={"WWW-Authenticate": "Bearer"},
        )

    now = datetime.now(UTC)
    expires_at = key.expires_at
    if expires_at.tzinfo is None:
        expires_at = expires_at.replace(tzinfo=UTC)
    if expires_at <= now:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Bearer token has expired",
            headers={"WWW-Authenticate": "Bearer"},
        )

    if key.project_id is not None:
        project = await session.get(ProjectRecord, key.project_id)
        if project is None or not project.enabled:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Project access is disabled",
                headers={"WWW-Authenticate": "Bearer"},
            )

    key.last_used_at = now
    await session.commit()
    return Actor(
        id=f"api-key:{key.id}",
        role=key.role,
        project_id=key.project_id,
        scopes=frozenset(key.scopes),
    )


def require_scope(scope: str):
    async def dependency(actor: Annotated[Actor, Depends(require_api_token)]) -> Actor:
        if actor.role == "admin" or scope in actor.scopes:
            return actor
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Insufficient API key scope",
        )

    return dependency


async def require_admin(actor: Annotated[Actor, Depends(require_api_token)]) -> Actor:
    if actor.role != "admin":
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Administrator role required",
        )
    return actor
