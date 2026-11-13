from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from time import perf_counter

import httpx

from app.config import ProviderConfig


@dataclass
class ProviderState:
    config: ProviderConfig
    health_status: str = "unknown"
    checked_at: datetime | None = None
    latency_ms: float | None = None
    detail: str | None = None


class ProviderRegistry:
    def __init__(self, providers: list[ProviderConfig]) -> None:
        ids = [provider.id for provider in providers]
        if len(ids) != len(set(ids)):
            raise ValueError("Provider IDs must be unique")
        self._states = {provider.id: ProviderState(config=provider) for provider in providers}

    def list(self) -> list[ProviderState]:
        return sorted(self._states.values(), key=lambda state: (-state.config.priority, state.config.id))

    def get(self, provider_id: str) -> ProviderState | None:
        return self._states.get(provider_id)

    async def probe(self, provider_id: str) -> ProviderState:
        state = self._states.get(provider_id)
        if state is None:
            raise KeyError(provider_id)
        checked_at = datetime.now(timezone.utc)
        if not state.config.enabled:
            state.health_status = "unhealthy"
            state.detail = "Provider is disabled"
            state.latency_ms = None
        elif not state.config.health_url:
            state.health_status = "unknown"
            state.detail = "No health endpoint configured"
            state.latency_ms = None
        else:
            started = perf_counter()
            try:
                async with httpx.AsyncClient(timeout=state.config.timeout_seconds) as client:
                    response = await client.get(state.config.health_url)
                    response.raise_for_status()
                state.health_status = "healthy"
                state.detail = None
                state.latency_ms = round((perf_counter() - started) * 1000, 2)
            except (httpx.HTTPError, ValueError) as exc:
                state.health_status = "unhealthy"
                state.detail = type(exc).__name__
                state.latency_ms = round((perf_counter() - started) * 1000, 2)
        state.checked_at = checked_at
        return state

    async def probe_all(self) -> list[ProviderState]:
        results = []
        for provider_id in self._states:
            results.append(await self.probe(provider_id))
        return results

    def select(self, capability: str, exclude: set[str] | None = None) -> ProviderState | None:
        excluded = exclude or set()
        candidates = [
            state for state in self._states.values()
            if state.config.enabled
            and capability in state.config.capabilities
            and state.config.id not in excluded
            and state.health_status != "unhealthy"
        ]
        if not candidates:
            return None
        # Unknown providers remain eligible only before a health endpoint has been configured.
        # Once a probe is possible, unhealthy providers are excluded.
        return sorted(candidates, key=lambda state: (-state.config.priority, state.config.id))[0]
