# Universal Service Hub

A provider-neutral control plane for the user's applications. It is designed to discover, register, evaluate and route workloads across interchangeable infrastructure and AI providers.

## Current status

**Foundation / not production-ready.** This first increment provides a runnable API, a provider registry, health probes, deterministic provider selection and tests. It does not yet provision cloud resources, store credentials, perform automatic failover for live traffic, or deploy itself.

## Run locally

```bash
cd universal-service-hub
python -m venv .venv
. .venv/bin/activate
pip install -e ".[dev]"
uvicorn app.main:app --reload
```

- Health: `GET /healthz`
- Readiness: `GET /readyz`
- Provider inventory: `GET /v1/providers`
- Probe configured providers: `POST /v1/providers/health-check`
- Select a provider: `POST /v1/providers/select`

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
- A provider is not considered a verified fallback until its integration and failover path have been exercised.

## Roadmap

1. Foundation: registry, health checks, selection API, tests. (This increment)
2. Durable PostgreSQL registry, encrypted secret references, audit log and RBAC.
3. Provider adapters: cloud hosting, databases, object storage, queues, observability and AI gateways.
4. Policy engine: cost/latency/region constraints, circuit breakers, quotas and fallback chains.
5. GitHub-driven project onboarding, isolated build/test workers and independent QA gates.
6. Deployment controllers, staged rollout, synthetic monitoring and automated rollback.
7. Multi-project dashboard, budget controls and operator notifications.

See [ARCHITECTURE.md](./docs/ARCHITECTURE.md), [SECURITY.md](./docs/SECURITY.md), and [OPERATIONS.md](./docs/OPERATIONS.md).
