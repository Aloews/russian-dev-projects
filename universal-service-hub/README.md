# Universal Service Hub

A provider-neutral control plane for the user's applications. The primary control-plane implementation is Rust (Axum); the original Python foundation is retained temporarily for compatibility and comparison, not as the target runtime.

## Rust service

```bash
cd universal-service-hub/rust
cargo run
```

Configuration is supplied through `USH_PROVIDERS_JSON`. Example:

```json
[
  {"id":"northflank","name":"Northflank","capabilities":["container-hosting"],"priority":100,"enabled":true,"health_url":"https://example-health-endpoint.invalid"},
  {"id":"railway","name":"Railway","capabilities":["container-hosting"],"priority":90,"enabled":true}
]
```

Endpoints:
- `GET /healthz`
- `GET /readyz`
- `GET /v1/providers`
- `POST /v1/providers/health-check` with optional `provider_id`
- `POST /v1/providers/select` with `capability` and optional `exclude`

The Rust implementation currently provides in-memory provider inventory, health probes and deterministic capability selection. It is not yet a production control plane: durable PostgreSQL state, authentication/RBAC, secret-manager integration, vendor-specific provisioning adapters, real traffic failover, quotas, audit logs and deployment orchestration remain gated roadmap items.

## Provider integration policy

See [API connection guide](./docs/API_CONNECTIONS.md) and [provider API matrix](./docs/PROVIDER_API_MATRIX.md) and [provider catalog](./docs/PROVIDER_CATALOG.md). The user's initial infrastructure shortlist is represented explicitly:
- Hosting/edge: Northflank, Cloudflare, Netlify, Render.
- Backend/API: Northflank, Railway, Fly.io, DigitalOcean.
- Database: Supabase, Neon, PostgreSQL on a dedicated server.
- Object storage: Supabase Storage, Cloudflare R2, S3, Backblaze B2.
- Queues: Railway/Northflank Redis and compatible managed Redis.

A GitHub OAuth connection grants only the repository permissions shown in GitHub's authorization screen. It does not connect the other infrastructure vendors. Each vendor requires its own official OAuth/API credential or service-account authorization. Credentials must be stored in a secret manager, never in Git.

## Operational guarantees

- Providers are not considered verified until their official API adapter and contract tests pass.
- Routing respects capability, enablement, health, project allowlists and budget policies.
- Automatic failover for non-idempotent writes and database migrations is prohibited.
- No production service is changed by this repository foundation.
- A free tier is not treated as unlimited capacity; quotas and spend ceilings are mandatory.

## Roadmap

1. Rust control-plane core and provider API matrix.
2. PostgreSQL registry, migrations, auth/RBAC, audit log and encrypted secret references.
3. Official provider adapters for hosting, databases, storage, queues and AI gateways.
4. Policy engine with circuit breakers, idempotency, quotas, cost and region constraints.
5. GitHub-driven project onboarding and isolated build/test workers.
6. Deployment controllers, staged rollout, synthetic monitoring and rollback.

See [ARCHITECTURE.md](./docs/ARCHITECTURE.md), [SECURITY.md](./docs/SECURITY.md), and [OPERATIONS.md](./docs/OPERATIONS.md).
