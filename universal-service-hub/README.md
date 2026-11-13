# Universal Service Hub

A provider-neutral control plane for the user's applications. It is designed to discover, register, evaluate and route workloads across interchangeable infrastructure and AI providers.

## Current status

**Development control plane / not production-ready.** The Hub has Bearer-token-protected provider management, health checks, deterministic capability selection, SQLite development persistence and PostgreSQL production persistence with Alembic migrations. Full user identity, project tenancy/RBAC, secret-manager adapters, vendor API adapters and live request failover are still outstanding.

## Run locally

```bash
cd universal-service-hub
python -m venv .venv
. .venv/bin/activate
pip install -e ".[dev]"
export USH_API_TOKEN="$(python -c 'import secrets; print(secrets.token_urlsafe(32))')"
uvicorn app.main:app --reload
```

SQLite is used by default for local development and the provider table is initialized automatically. Set `USH_DATABASE_URL` to a PostgreSQL SQLAlchemy async URL to use PostgreSQL. Container startup runs `alembic upgrade head` before starting the API.

## API

Public:
- `GET /healthz`
- `GET /readyz`

All `/v1/*` endpoints require `Authorization: Bearer $USH_API_TOKEN`. The token must be at least 32 characters. Production startup fails if the token is missing or the database URL is not PostgreSQL.

Protected:
- `GET /v1/providers` — list providers
- `PUT /v1/providers/{provider_id}` — create or replace provider configuration
- `DELETE /v1/providers/{provider_id}` — remove a provider
- `POST /v1/providers/health-check` — probe one provider or all providers
- `POST /v1/providers/select` — choose the highest-priority eligible provider for a capability

Provider fields: `name`, `capabilities`, `priority`, `enabled`, optional `health_url`, optional `timeout_seconds`. Health state and timestamps are stored in the database. Environment configuration in `USH_PROVIDERS_JSON` seeds only missing provider IDs; it does not overwrite changes already made through the API.

Do not put API keys in provider records. Secrets belong in a deployment secret manager and are not implemented in this increment.

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
3. Durable PostgreSQL provider registry and migrations. (This increment)
4. Audit log, project tenancy and RBAC.
5. Secret-manager integration and provider adapters for compute, databases, storage, queues, observability and AI gateways.
6. Policy engine: cost/latency/region constraints, circuit breakers, quotas and fallback chains.
7. GitHub-driven project onboarding, isolated build/test workers and independent QA gates.
8. Deployment controllers, staged rollout, synthetic monitoring and automated rollback.
9. Multi-project dashboard, budget controls and operator notifications.

See [ARCHITECTURE.md](./docs/ARCHITECTURE.md), [SECURITY.md](./docs/SECURITY.md), and [OPERATIONS.md](./docs/OPERATIONS.md).
