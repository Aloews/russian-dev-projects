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
    project_id: str | None = None


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
    project_id: str | None = Field(default=None, max_length=80, pattern=r"^[a-zA-Z0-9._-]+$")

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


class ProjectCreateRequest(BaseModel):
    id: str = Field(min_length=2, max_length=80, pattern=r"^[a-zA-Z0-9._-]+$")
    name: str = Field(min_length=1, max_length=160)


class ProjectView(BaseModel):
    id: str
    name: str
    enabled: bool
    created_at: datetime


ALLOWED_SCOPES = {
    "providers:read",
    "providers:write",
    "health:probe",
    "routing:select",
    "ai:chat",
}


class ApiKeyCreateRequest(BaseModel):
    name: str = Field(min_length=1, max_length=120)
    role: Literal["operator", "agent", "project"]
    project_id: str | None = Field(default=None, max_length=80)
    scopes: list[str] = Field(min_length=1, max_length=10)
    expires_in_days: int = Field(default=90, ge=1, le=365)

    @model_validator(mode="after")
    def validate_key_scope(self):
        if self.role == "operator" and self.project_id is not None:
            raise ValueError("Operator keys cannot be project-scoped")
        if self.role in {"agent", "project"} and self.project_id is None:
            raise ValueError("Project and agent keys require project_id")
        if not set(self.scopes).issubset(ALLOWED_SCOPES):
            raise ValueError("API key contains an unsupported scope")
        if self.role != "operator" and "providers:write" in self.scopes:
            raise ValueError("Project-scoped keys cannot write provider configuration")
        return self


class ApiKeyCreatedResponse(BaseModel):
    id: str
    name: str
    role: str
    project_id: str | None
    scopes: list[str]
    token: str
    expires_at: datetime


class ApiKeyView(BaseModel):
    id: str
    name: str
    token_prefix: str
    role: str
    project_id: str | None
    scopes: list[str]
    enabled: bool
    created_at: datetime
    expires_at: datetime
    last_used_at: datetime | None = None
    revoked_at: datetime | None = None


class AuditLogView(BaseModel):
    id: str
    actor_id: str
    project_id: str | None
    action: str
    resource_type: str
    resource_id: str | None
    outcome: str
    correlation_id: str
    event_metadata: dict[str, Any] = Field(default_factory=dict)
    created_at: datetime
