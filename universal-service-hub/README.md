# Universal Service Hub

A provider-neutral control plane for the user's applications. It is designed to discover, register, evaluate and route workloads across interchangeable infrastructure and AI providers.

## Current status

**Rust runtime migration in progress / not production-ready.** The production container has been switched to an Axum/Tokio/SQLx Rust binary. Rust currently implements health/readiness, PostgreSQL-backed project and token management, provider registry/selection, safe health probes, audit reads and initial OpenAI-compatible/Anthropic/Gemini chat routing. The legacy FastAPI implementation remains as a temporary compatibility reference while endpoint parity and security tests are completed. Production deployment, non-AI adapters, quota/queue controls, full project lifecycle parity and live provider verification remain outstanding. Infisical machine-identity secret retrieval is implemented but has not yet been live-tested.

## Run locally

The Rust runtime requires PostgreSQL.

```bash
cd universal-service-hub
export USH_DATABASE_URL="postgres://ush:password@localhost:5432/ush"
export USH_API_TOKEN="$(openssl rand -hex 32)"
cargo run --release
```

The Rust binary applies its SQLx migrations on startup. The production Docker image runs the Rust binary. Python/FastAPI files and tests are retained temporarily as a reference while route parity is verified; they are not the production container entrypoint.

## API

Public:
- `GET /healthz`
- `GET /readyz`

Administrative endpoints require `Authorization: Bearer $USH_API_TOKEN`. This root token must be at least 32 characters. Project runtime endpoints accept project tokens issued by the administrator. Production startup fails if the root token is missing or the database URL is not PostgreSQL.

Provider management:
- `GET /v1/providers` — list providers
- `GET /v1/providers/{provider_id}/models` — discover current model IDs from the provider API
- `GET /v1/audit?limit=50` — list recent audit events (maximum 100)
- `POST /v1/projects` — register a project and its allowed capabilities/providers
- `GET /v1/projects` — list registered projects
- `PUT /v1/projects/{project_id}` — update project capabilities/provider allowlist
- `DELETE /v1/projects/{project_id}` — remove a project and revoke its tokens
- `POST /v1/projects/{project_id}/tokens` — issue a project token (shown once)
- `GET /v1/projects/{project_id}/tokens` — list token metadata (root administrator or project owner for its own project)
- `DELETE /v1/projects/{project_id}/tokens/{token_id}` — revoke a token (root administrator or project owner for its own project)
- `PUT /v1/providers/{provider_id}` — create or replace provider configuration
- `DELETE /v1/providers/{provider_id}` — remove a provider
- `POST /v1/providers/health-check` — probe one provider or up to 50 providers by default (maximum 100 per call; probes run sequentially with per-provider timeouts)
- `POST /v1/providers/select` — choose the highest-priority eligible provider for a capability (admin or any project role; viewer is read-only)

AI:
- `POST /v1/ai/chat` — route a chat request to a configured AI provider (admin or project owner/operator token)

Project tokens have `owner`, `operator` or `viewer` roles. The Rust runtime enforces project capability/provider allowlists and role boundaries: owner/operator tokens can invoke AI; all project roles can perform read-only provider selection. Project owners can list, issue and revoke operator/viewer tokens only within their own project; only the root administrator can issue owner tokens or change/delete project policy. Tokens are generated once, stored as SHA-256 hashes and revocable.

## Secret manager

Infisical Universal Auth is supported for provider credentials. Configure:

- `USH_SECRET_BACKEND=infisical`
- `INFISICAL_URL` (optional; defaults to `https://app.infisical.com`)
- `INFISICAL_CLIENT_ID`
- `INFISICAL_CLIENT_SECRET`
- `INFISICAL_PROJECT_ID`
- `INFISICAL_ENVIRONMENT`
- `USH_INFISICAL_SECRET_PATH_PREFIX` (optional; defaults to `/projects`)

Store each provider key in Infisical under `/{project_id}/providers` with the same name as the provider's `api_key_env` reference (for example `OPENAI_API_KEY`). Root-administrator requests use `/shared/providers`. The Hub exchanges the machine identity credentials for a short-lived token, caches it until shortly before expiry, and retrieves the key only when dispatching a request. Secret values are not written to the provider table, audit events or logs. In development only, when Infisical is not configured and `USH_SECRET_BACKEND` is not set to `infisical`, provider credentials can be read from environment variables.

## AI provider configuration

The provider record stores a secret reference name, never the key value. In production, the preferred source is Infisical; environment-variable lookup is retained only for development or temporary compatibility.

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
- Health probes and AI adapters do not follow redirects, require HTTPS, reject local/private IP destinations, and perform DNS checks before outbound requests.
- Provider mutations and AI usage metadata are recorded without storing prompts, responses or credential values.
- No credentials are returned by API responses or logs.
- A provider is not a verified fallback until its integration and failure modes have been exercised.
- The root service token is administrator-only. Runtime projects use scoped tokens with `owner`, `operator` or `viewer` roles; provider configuration, audit reads and project/token administration remain root-only.
- Database failover is not automatic; this Hub does not replicate or move project data between providers.

## Roadmap

1. Foundation: registry, health checks, selection API, tests. (Completed)
2. Control API authentication. (Completed)
3. Durable PostgreSQL provider registry and migrations. (Completed)
4. AI provider adapters and normalized chat routing. (This increment)
5. Project tenancy, scoped tokens, roles and audit event table/read API. (Completed)
6. Managed secret-store integration and non-AI adapters for compute, databases, storage, queues and observability.
7. Policy engine: cost/latency/region constraints, circuit breakers, quotas and fallback chains.
8. GitHub-driven project onboarding, isolated build/test workers and independent QA gates.
9. Deployment controllers, staged rollout, synthetic monitoring and automated rollback.
10. Multi-project dashboard, budget controls and operator notifications.

See [ARCHITECTURE.md](./docs/ARCHITECTURE.md), [SECURITY.md](./docs/SECURITY.md), [OPERATIONS.md](./docs/OPERATIONS.md), and [INTEGRATION_PLAN.md](./docs/INTEGRATION_PLAN.md).
