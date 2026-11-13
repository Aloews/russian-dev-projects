from __future__ import annotations

import hashlib
import secrets
from dataclasses import dataclass
from datetime import UTC, datetime
from typing import Annotated

from fastapi import Depends, HTTPException, Request, status
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from sqlalchemy import select

from app.config import get_settings
from app.models import ProjectTokenRecord

bearer_scheme = HTTPBearer(auto_error=False)


@dataclass(frozen=True)
class Principal:
    actor: str
    role: str
    project_id: str | None
    is_admin: bool


async def require_api_token(
    request: Request,
    credentials: Annotated[
        HTTPAuthorizationCredentials | None,
        Depends(bearer_scheme),
    ],
) -> Principal:
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
    expected = configured.get_secret_value()
    if secrets.compare_digest(supplied, expected):
        return Principal(actor="service-token", role="admin", project_id=None, is_admin=True)

    session_factory = request.app.state.session_factory
    token_hash = hashlib.sha256(supplied.encode("utf-8")).hexdigest()
    async with session_factory() as session:
        token = await session.scalar(
            select(ProjectTokenRecord).where(
                ProjectTokenRecord.token_hash == token_hash,
                ProjectTokenRecord.revoked_at.is_(None),
            )
        )
        if token is None:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Invalid bearer token",
                headers={"WWW-Authenticate": "Bearer"},
            )
        token.last_used_at = datetime.now(UTC)
        principal = Principal(
            actor=f"project:{token.project_id}:token:{token.id}",
            role=token.role,
            project_id=token.project_id,
            is_admin=False,
        )
        await session.commit()
        return principal


async def require_admin(
    principal: Annotated[Principal, Depends(require_api_token)],
) -> Principal:
    if not principal.is_admin:
        raise HTTPException(status_code=403, detail="Administrator token required")
    return principal


async def require_project_operator(
    principal: Annotated[Principal, Depends(require_api_token)],
) -> Principal:
    if not principal.is_admin and principal.role not in {"owner", "operator"}:
        raise HTTPException(status_code=403, detail="Project operator role required")
    return principal


async def require_project_access(
    principal: Annotated[Principal, Depends(require_api_token)],
) -> Principal:
    if not principal.is_admin and principal.role not in {"owner", "operator", "viewer"}:
        raise HTTPException(status_code=403, detail="Project access required")
    return principal
