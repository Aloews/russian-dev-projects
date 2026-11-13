from __future__ import annotations

from datetime import datetime
from typing import Any, Literal

from pydantic import BaseModel, Field, model_validator

from app.config import ProviderConfig


class ProviderView(BaseModel):
    id: str
    name: str
    capabilities: list[str]
    priority: int
    enabled: bool
    health_status: Literal["unknown", "healthy", "unhealthy"]
    checked_at: datetime | None = None
    latency_ms: float | None = None
    adapter_type: Literal["none", "openai_compatible", "anthropic", "gemini"] = "none"
    model: str | None = None
    credentials_configured: bool = False
    supports_idempotency: bool = False


class ProviderUpsertRequest(BaseModel):
    name: str = Field(min_length=1, max_length=160)
    capabilities: list[str] = Field(default_factory=list, max_length=100)
    priority: int = Field(default=100, ge=-10000, le=10000)
    enabled: bool = True
    health_url: str | None = Field(default=None, max_length=2048)
    timeout_seconds: float = Field(default=3.0, gt=0, le=30)
    adapter_type: Literal["none", "openai_compatible", "anthropic", "gemini"] = "none"
    model: str | None = Field(default=None, max_length=160)
    base_url: str | None = Field(default=None, max_length=2048)
    api_key_env: str | None = Field(default=None, max_length=120)
    supports_idempotency: bool = False

    @model_validator(mode="after")
    def validate_adapter_configuration(self):
        ProviderConfig(
            id="schema-validation",
            name=self.name,
            capabilities=self.capabilities,
            priority=self.priority,
            enabled=self.enabled,
            health_url=self.health_url,
            timeout_seconds=self.timeout_seconds,
            adapter_type=self.adapter_type,
            model=self.model,
            base_url=self.base_url,
            api_key_env=self.api_key_env,
            supports_idempotency=self.supports_idempotency,
        )
        return self


class HealthCheckResult(BaseModel):
    provider_id: str
    status: Literal["healthy", "unhealthy", "not_configured"]
    latency_ms: float | None = None
    checked_at: datetime
    detail: str | None = None


class ProviderSelectionRequest(BaseModel):
    capability: str = Field(min_length=1, max_length=100)
    exclude: list[str] = Field(default_factory=list, max_length=100)


class ProviderSelectionResponse(BaseModel):
    capability: str
    provider: ProviderView | None
    reason: str


class ProviderHealthProbeRequest(BaseModel):
    provider_id: str | None = None
    limit: int = Field(default=50, ge=1, le=100)


class ChatMessage(BaseModel):
    role: Literal["system", "user", "assistant"]
    content: str = Field(min_length=1, max_length=20000)


class AIChatRequest(BaseModel):
    messages: list[ChatMessage] = Field(min_length=1, max_length=100)
    capability: str = Field(default="ai.chat", min_length=1, max_length=100)
    temperature: float = Field(default=0.2, ge=0, le=1)
    max_tokens: int = Field(default=1024, ge=1, le=8192)
    exclude: list[str] = Field(default_factory=list, max_length=100)
    idempotency_key: str | None = Field(default=None, min_length=8, max_length=128)


class AIChatResponse(BaseModel):
    provider_id: str
    model: str
    content: str
    usage: dict[str, Any] = Field(default_factory=dict)
    latency_ms: float
    attempts: int


class AuditEventView(BaseModel):
    id: str
    occurred_at: datetime
    actor: str
    action: str
    resource_type: str
    resource_id: str
    details: dict[str, Any] = Field(default_factory=dict)


class ProjectCreateRequest(BaseModel):
    id: str = Field(min_length=1, max_length=80, pattern=r"^[a-zA-Z0-9._-]+$")
    name: str = Field(min_length=1, max_length=160)
    allowed_capabilities: list[str] = Field(min_length=1, max_length=100)
    allowed_provider_ids: list[str] = Field(default_factory=list, max_length=100)


class ProjectView(BaseModel):
    id: str
    name: str
    allowed_capabilities: list[str]
    allowed_provider_ids: list[str]
    created_at: datetime


class ProjectTokenCreateRequest(BaseModel):
    name: str = Field(min_length=1, max_length=120)
    role: Literal["owner", "operator", "viewer"] = "operator"


class ProjectTokenCreated(BaseModel):
    id: str
    project_id: str
    name: str
    role: Literal["owner", "operator", "viewer"]
    token: str


class ProjectTokenView(BaseModel):
    id: str
    project_id: str
    name: str
    role: Literal["owner", "operator", "viewer"]
    created_at: datetime
    last_used_at: datetime | None = None
    revoked_at: datetime | None = None
