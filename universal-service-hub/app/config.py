from __future__ import annotations

import json
import re
from functools import lru_cache
from typing import Any
from app.outbound import parse_public_https_url

from pydantic import BaseModel, Field, SecretStr, ValidationError, field_validator, model_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

ADAPTER_TYPES = {"none", "openai_compatible", "anthropic", "gemini"}


class ProviderConfig(BaseModel):
    id: str = Field(min_length=1, max_length=80, pattern=r"^[a-zA-Z0-9._-]+$")
    name: str = Field(min_length=1, max_length=160)
    capabilities: list[str] = Field(default_factory=list)
    priority: int = 100
    enabled: bool = True
    health_url: str | None = Field(default=None, max_length=2048)
    timeout_seconds: float = Field(default=3.0, gt=0, le=30)
    adapter_type: str = "none"
    model: str | None = Field(default=None, max_length=160)
    base_url: str | None = Field(default=None, max_length=2048)
    api_key_env: str | None = Field(default=None, max_length=120)
    supports_idempotency: bool = False

    @field_validator("health_url", "base_url")
    @classmethod
    def validate_external_urls(cls, value: str | None) -> str | None:
        if value is not None:
            parse_public_https_url(value)
        return value

    @model_validator(mode="after")
    def validate_adapter_configuration(self):
        if self.adapter_type not in ADAPTER_TYPES:
            raise ValueError("Unsupported provider adapter type")
        if self.adapter_type == "none":
            if self.model or self.base_url or self.api_key_env or self.supports_idempotency:
                raise ValueError("Adapter settings require a configured adapter type")
            return self
        if not self.model or not self.base_url or not self.api_key_env:
            raise ValueError("AI adapters require model, base_url and api_key_env")
        if not re.fullmatch(r"[A-Z][A-Z0-9_]{1,119}", self.api_key_env):
            raise ValueError("api_key_env must be an uppercase environment variable name")
        if self.adapter_type == "gemini" and self.supports_idempotency:
            raise ValueError("Gemini adapter does not support idempotent failover")
        return self


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="USH_", env_file=".env", extra="ignore")
    app_name: str = "Universal Service Hub"
    environment: str = "development"
    api_token: SecretStr | None = Field(default=None, min_length=32)
    database_url: str = "sqlite+aiosqlite:///./ush.db"
    providers_json: str = "[]"

    @field_validator("database_url", mode="before")
    @classmethod
    def normalize_database_url(cls, value: Any) -> Any:
        if isinstance(value, str):
            if value.startswith("postgres://"):
                return value.replace("postgres://", "postgresql+asyncpg://", 1)
            if value.startswith("postgresql://"):
                return value.replace("postgresql://", "postgresql+asyncpg://", 1)
        return value

    def providers(self) -> list[ProviderConfig]:
        try:
            raw: Any = json.loads(self.providers_json)
            if not isinstance(raw, list):
                raise TypeError("USH_PROVIDERS_JSON must be a JSON array")
            return [ProviderConfig.model_validate(item) for item in raw]
        except (json.JSONDecodeError, ValidationError) as exc:
            raise ValueError(f"Invalid provider configuration: {exc}") from exc


@lru_cache
def get_settings() -> Settings:
    return Settings()
