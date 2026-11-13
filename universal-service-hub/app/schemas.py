from __future__ import annotations

from datetime import datetime
from typing import Literal

from pydantic import BaseModel, Field


class ProviderView(BaseModel):
    id: str
    name: str
    capabilities: list[str]
    priority: int
    enabled: bool
    health_status: Literal["unknown", "healthy", "unhealthy"]
    checked_at: datetime | None = None


class HealthCheckResult(BaseModel):
    provider_id: str
    status: Literal["healthy", "unhealthy", "not_configured"]
    latency_ms: float | None = None
    checked_at: datetime
    detail: str | None = None


class ProviderSelectionRequest(BaseModel):
    capability: str = Field(min_length=1, max_length=100)
    exclude: list[str] = Field(default_factory=list)


class ProviderSelectionResponse(BaseModel):
    capability: str
    provider: ProviderView | None
    reason: str


class ProviderHealthProbeRequest(BaseModel):
    provider_id: str | None = None
