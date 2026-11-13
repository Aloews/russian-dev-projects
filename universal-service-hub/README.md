# Universal Service Hub

A provider-neutral control plane for the user's applications. It is designed to discover, register, evaluate and route workloads across interchangeable infrastructure and AI providers.

## Current status

**Development control plane / not production-ready.** The Hub has authenticated provider management, PostgreSQL persistence, health checks, deterministic capability selection and real HTTP adapters for OpenAI-compatible, Anthropic-compatible and Gemini chat APIs. Project-scoped API keys, role/scope authorization and durable control-action audit logs are implemented. A managed secret-store connector, non-AI vendor adapters, quotas and production deployment are still outstanding.

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

All `/v1/*` endpoints require a Bearer token. The bootstrap `USH_API_TOKEN` is the root administrator credential (minimum 32 characters); additional scoped keys are created through the admin API. Production startup fails if the bootstrap token is missing or the database URL is not PostgreSQL.

Administration:
- `POST /v1/projects` and `GET /v1/projects` — create/list projects (bootstrap admin only)
- `POST /v1/api-keys` — issue an operator, agent or project key (bootstrap admin only; plaintext shown once)
- `GET /v1/api-keys` — list key metadata without token values
- `DELETE /v1/api-keys/{key_id}` — revoke a key
- `GET /v1/audit-logs` — inspect durable control-action audit events (bootstrap admin only)

Project and agent keys require a project ID, expire within at most 365 days and carry explicit scopes. Project-scoped keys can access global providers plus providers assigned to their project; they cannot modify provider configuration or administer projects/keys. Operator keys are global and receive only the scopes granted when created.

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

Example for a standard Anthropic Messages API endpoint:

```json
{
  "name": "Anthropic",
  "capabilities": ["ai.chat"],
  "priority": 100,
  "enabled": true,
  "adapter_type": "anthropic",
  "model": "provider-model-id",
  "base_url": "https://api.anthropic.com",
  "api_key_env": "ANTHROPIC_API_KEY",
  "timeout_seconds": 30,
  "supports_idempotency": false
}
```

Send this object to `PUT /v1/providers/anthropic`. Set `ANTHROPIC_API_KEY` in the runtime environment. For Starimg, use this adapter only after its current account documentation confirms the Anthropic Messages endpoint and use the model identifier returned by that provider. The API response reveals only whether the referenced credential is configured; it never returns the environment-variable name or secret.

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
- Keep the bootstrap token private; distribute only short-lived, least-privilege project/operator keys to agents and services.
- Database failover is not automatic; this Hub does not replicate or move project data between providers.

## Roadmap

1. Foundation: registry, health checks, selection API, tests. (Completed)
2. Control API authentication. (Completed)
3. Durable PostgreSQL provider registry and migrations. (Completed)
4. AI provider adapters and normalized chat routing. (This increment)
5. Audit log, project tenancy and RBAC. (Completed)
6. Managed secret-store integration and non-AI adapters for compute, databases, storage, queues and observability.
7. Policy engine: cost/latency/region constraints, circuit breakers, quotas and fallback chains.
8. GitHub-driven project onboarding, isolated build/test workers and independent QA gates.
9. Deployment controllers, staged rollout, synthetic monitoring and automated rollback.
10. Multi-project dashboard, budget controls and operator notifications.

See [ARCHITECTURE.md](./docs/ARCHITECTURE.md), [SECURITY.md](./docs/SECURITY.md), and [OPERATIONS.md](./docs/OPERATIONS.md).
