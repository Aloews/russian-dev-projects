from __future__ import annotations

import json
from functools import lru_cache
from typing import Any

from pydantic import BaseModel, Field, SecretStr, ValidationError
from pydantic_settings import BaseSettings, SettingsConfigDict


class ProviderConfig(BaseModel):
    id: str = Field(min_length=1, max_length=80, pattern=r"^[a-zA-Z0-9._-]+$")
    name: str = Field(min_length=1, max_length=160)
    capabilities: list[str] = Field(default_factory=list)
    priority: int = 100
    enabled: bool = True
    health_url: str | None = None
    timeout_seconds: float = Field(default=3.0, gt=0, le=30)


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="USH_", env_file=".env", extra="ignore")
    app_name: str = "Universal Service Hub"
    environment: str = "development"
    api_token: SecretStr | None = Field(default=None, min_length=32)
    providers_json: str = "[]"

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
