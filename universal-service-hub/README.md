# Universal Service Hub

A provider-neutral control plane for the user's applications. It is designed to discover, register, evaluate and route workloads across interchangeable infrastructure and AI providers.

## Current status

**Development control plane / not production-ready.** The Hub has authenticated provider management, PostgreSQL persistence, health checks, deterministic capability selection and real HTTP adapters for OpenAI-compatible, Anthropic-compatible and Gemini chat APIs. Full user identity, project tenancy/RBAC, a managed secret-store connector, audit log, non-AI vendor adapters and production deployment are still outstanding.

## Run locally

```bash
cd universal-service-hub
python -m venv .venv
. .venv/bin/activate
pip install -e ".[dev]"
export USH_API_TOKEN="$(python -c 'import secrets; print(secrets.token_urlsafe(32))')"
uvicorn app.main:app --reload
```

SQLite is used by default for local development and the provider table is initialized automatically. Set `USH_DATABASE_URL` to a PostgreSQL URL to use PostgreSQL. Container startup runs Alembic migrations before starting the API.

## API

Public:
- `GET /healthz`
- `GET /readyz`

All `/v1/*` endpoints require `Authorization: Bearer $USH_API_TOKEN`. The token must be at least 32 characters. Production startup fails if the token is missing or the database URL is not PostgreSQL.

Provider management:
- `GET /v1/providers` — list providers
- `PUT /v1/providers/{provider_id}` — create or replace provider configuration
- `DELETE /v1/providers/{provider_id}` — remove a provider
- `POST /v1/providers/health-check` — probe one provider or up to 50 providers by default (maximum 100 per call; 10 probes run concurrently)
- `POST /v1/providers/select` — choose the highest-priority eligible provider for a capability

AI:
- `POST /v1/ai/chat` — route a chat request to a configured AI provider

## AI provider configuration

The provider record stores the environment-variable name that references the key, never the key value. Put the actual key in the deployment secret/environment settings.

Example for an Anthropic-compatible Starimg endpoint:

```json
{
  "name": "Starimg",
  "capabilities": ["ai.chat"],
  "priority": 100,
  "enabled": true,
  "adapter_type": "anthropic",
  "model": "claude-fable-5-2",
  "base_url": "https://ai.starimg.ru",
  "api_key_env": "STARIMG_API_KEY",
  "timeout_seconds": 30,
  "supports_idempotency": false
}
```

Send this object to `PUT /v1/providers/starimg`. Set `STARIMG_API_KEY` in the runtime environment. The API response reveals only whether the referenced credential is configured; it never returns the environment-variable name or secret.

Supported adapter types:
- `openai_compatible` — OpenAI-compatible `/chat/completions` endpoints
- `anthropic` — Anthropic Messages API
- `gemini` — Gemini `generateContent` API

All configured API base URLs must use HTTPS. Query strings, URL userinfo and embedded credentials are rejected.

Example chat request:

```json
{
  "messages": [
    {"role": "system", "content": "Answer precisely."},
    {"role": "user", "content": "Explain the task."}
  ],
  "temperature": 0.2,
  "max_tokens": 1200
}
```

## Failover and billing safety

The Hub does not blindly retry paid generation requests. Cross-provider failover is attempted only when the caller supplies an `idempotency_key`, the attempted provider is explicitly configured to support idempotency, and the failure is a rate-limit rejection or a connection failure before request transmission. Ambiguous 5xx/read-timeout failures are not retried because the upstream may already have processed and billed the request. Gemini is not marked idempotent by this adapter.

## Operating principles

- Provider adapters isolate vendor-specific APIs from application logic.
- Selection is based on declared capability, enabled state, health and explicit priority.
- Health probes do not follow redirects and do not return raw provider response bodies.
- No credentials are returned by API responses or logs.
- A provider is not a verified fallback until its integration and failure modes have been exercised.
- This release uses one service token; do not expose it as a multi-user or multi-project authorization model.
- Database failover is not automatic; this Hub does not replicate or move project data between providers.

## Roadmap

1. Foundation: registry, health checks, selection API, tests. (Completed)
2. Control API authentication. (Completed)
3. Durable PostgreSQL provider registry and migrations. (Completed)
4. AI provider adapters and normalized chat routing. (This increment)
5. Audit log, project tenancy and RBAC.
6. Managed secret-store integration and non-AI adapters for compute, databases, storage, queues and observability.
7. Policy engine: cost/latency/region constraints, circuit breakers, quotas and fallback chains.
8. GitHub-driven project onboarding, isolated build/test workers and independent QA gates.
9. Deployment controllers, staged rollout, synthetic monitoring and automated rollback.
10. Multi-project dashboard, budget controls and operator notifications.

See [ARCHITECTURE.md](./docs/ARCHITECTURE.md), [SECURITY.md](./docs/SECURITY.md), and [OPERATIONS.md](./docs/OPERATIONS.md).
