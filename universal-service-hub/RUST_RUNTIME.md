# Universal Service Hub — Rust runtime

The production control-plane container is Rust (Axum + Tokio + SQLx + PostgreSQL). The legacy FastAPI code remains temporarily for compatibility and reference tests; it is not the production container entrypoint.

## Runtime

- Rust stable, edition 2024
- Axum HTTP API, Tokio async runtime
- SQLx PostgreSQL with embedded idempotent SQL migrations
- Reqwest + rustls for outbound provider calls
- Structured tracing and bounded request bodies
- Root administrator and scoped project bearer tokens

## Required environment

- `USH_DATABASE_URL`: PostgreSQL URL
- `USH_API_TOKEN`: root administrator token (minimum 32 characters)
- `USH_ENVIRONMENT`: `development` or `production`
- `USH_SECRET_BACKEND`: `env` in development; production requires `infisical`
- `PORT`: optional, defaults to 8000
- `RUST_LOG`: optional tracing filter

When Infisical is enabled:
- `INFISICAL_CLIENT_ID`
- `INFISICAL_CLIENT_SECRET`
- `INFISICAL_PROJECT_ID`
- `INFISICAL_ENVIRONMENT`
- `INFISICAL_URL` (optional; defaults to Infisical Cloud)
- `USH_INFISICAL_SECRET_PATH_PREFIX` (optional; defaults to `/projects`)

## Implemented Rust routes

- `GET /healthz`, `GET /readyz`
- `POST/GET /v1/projects`; `PUT/DELETE /v1/projects/{project_id}`
- `POST/GET /v1/projects/{project_id}/tokens`; `DELETE /v1/projects/{project_id}/tokens/{token_id}`
- `GET /v1/providers`; `PUT/DELETE /v1/providers/{provider_id}`
- `GET /v1/providers/{provider_id}/models`
- `POST /v1/providers/health-check`; `POST /v1/providers/select`
- `POST /v1/ai/chat`
- `GET /v1/audit`

Provider adapters support OpenAI-compatible chat, Anthropic Messages, Gemini generateContent and local Ollama chat. Model discovery uses each provider's model-list endpoint; Ollama uses `/api/tags` and `/api/chat`. Configure the local adapter with an explicit model ID, `adapter_type: "ollama"`, `api_key_env: null`, `health_url: null`, `supports_idempotency: false` and a loopback or allowlisted private `base_url`. Add the private service hostname to `USH_OLLAMA_ALLOWED_HOSTS` (comma-separated exact hostnames); DNS results must all resolve to private/loopback addresses. For a Docker service named `ollama`, for example, configure `USH_OLLAMA_ALLOWED_HOSTS=ollama` and `base_url=http://ollama:11434`. Do not expose the Ollama API publicly. Starimg/OpenRouter should be configured through the compatible adapter only after their current account protocol and model IDs have been verified.

## Security boundaries

- Project tokens are generated randomly, stored only as SHA-256 hashes and shown once.
- Project owner/operator roles can invoke AI; viewer tokens can only perform read-only provider selection.
- Project capability and provider allowlists are checked server-side.
- Provider keys are resolved from Infisical per project path; development-only environment-variable fallback is available.
- Outbound provider URLs require HTTPS and reject local/private DNS results. This is defense in depth, not a substitute for a hosting-platform egress firewall against DNS rebinding.
- Audit events exclude prompts, generated responses and secret values.

## Not yet production-complete

- Durable asynchronous job queue and isolated worker fleet.
- Budget/quota accounting, distributed rate limiting and circuit breakers.
- Non-AI service adapters (GitHub, Railway, Vercel, Supabase, Cloudflare, object storage and media APIs).
- Tool calling, streaming, multimodal input/output and provider-specific structured-output parity.
- Full live contract tests for every provider, Infisical credentials and production deployment.
- Network egress firewall, backup/restore rehearsal, staged rollout and rollback.

Production cutover remains blocked until the exact release SHA passes CI, migration compatibility is tested against the target database, provider live smoke tests pass, budgets are configured, and rollback is rehearsed.
