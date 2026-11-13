# Universal Service Hub

A provider-neutral control plane for the user's applications. It is designed to discover, register, evaluate and route workloads across interchangeable infrastructure and AI providers.

## Current status

**Development foundation / not production-ready.** The API provides provider inventory, health probes, deterministic capability selection and Bearer-token protection for control endpoints. Provider configuration is still loaded from environment configuration; durable storage, vendor adapters and live failover are not yet implemented.

## Run locally

```bash
cd universal-service-hub
python -m venv .venv
. .venv/bin/activate
pip install -e ".[dev]"
export USH_API_TOKEN="$(python -c 'import secrets; print(secrets.token_urlsafe(32))')"
uvicorn app.main:app --reload
```

- Public health: `GET /healthz`
- Public readiness: `GET /readyz`
- Protected provider inventory: `GET /v1/providers`
- Protected health probes: `POST /v1/providers/health-check`
- Protected provider selection: `POST /v1/providers/select`

All `/v1/*` endpoints require `Authorization: Bearer $USH_API_TOKEN`. The token must be at least 32 characters. Production startup fails if `USH_ENVIRONMENT=production` and no token is configured.

Configure providers with `USH_PROVIDERS_JSON`. The value is a JSON array; each item supports `id`, `name`, `capabilities`, `priority`, `enabled`, optional `health_url`, and optional `timeout_seconds`. Do not put API keys in this registry. Secrets belong in the deployment secret manager and will be handled by dedicated credential adapters in a later increment.

Example:

```json
[
  {"id":"northflank","name":"Northflank","capabilities":["container-hosting"],"priority":100,"enabled":true},
  {"id":"flyio","name":"Fly.io","capabilities":["container-hosting"],"priority":80,"enabled":true}
]
```

## Operating principles

- Provider adapters isolate vendor-specific APIs from application logic.
- Selection is based on declared capability, enabled state, health and explicit priority.
- No credentials are returned by API responses or logs.
- A failed health check excludes a provider from selection until a successful check.
- Production changes require an explicit rollout plan, smoke tests and rollback path.
- A provider is not a verified fallback until its integration and failure modes have been exercised.
- This release uses one service token; do not expose it as a multi-user or multi-project authorization model.

## Roadmap

1. Foundation: registry, health checks, selection API, tests. (Completed)
2. Control API authentication. (This increment)
3. Durable PostgreSQL registry, audit log, project tenancy and RBAC.
4. Provider adapters: cloud hosting, databases, object storage, queues, observability and AI gateways.
5. Policy engine: cost/latency/region constraints, circuit breakers, quotas and fallback chains.
6. GitHub-driven project onboarding, isolated build/test workers and independent QA gates.
7. Deployment controllers, staged rollout, synthetic monitoring and automated rollback.
8. Multi-project dashboard, budget controls and operator notifications.

See [ARCHITECTURE.md](./docs/ARCHITECTURE.md), [SECURITY.md](./docs/SECURITY.md), and [OPERATIONS.md](./docs/OPERATIONS.md).
