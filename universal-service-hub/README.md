# Universal Service Hub

A provider-neutral control plane for the user's applications. It is designed to discover, register, evaluate and route workloads across interchangeable infrastructure and AI providers.

## Current status

USH has a working Rust API foundation, PostgreSQL migrations, tenant-scoped service tokens, project-scoped credentials, provider registry, AI request adapters, durable engineering task records, worker identities, workspace leases, checkpoints, task attempts and recovery primitives.

The execution-plane API now includes:

- PostgreSQL-backed task queue with legal state transitions, dependency checks, idempotency keys and immutable event history.
- Role-specific worker identities and queues for architecture, implementation, build, test, review and recovery.
- Atomic task claims, expiring leases, heartbeats and recovery of expired task leases.
- Workspace leases, task-to-workspace assignments, base/candidate SHA pinning and exclusive path-prefix locks.
- Read-only reviewer assignments: candidate SHA required, writer scope empty, and path-lock acquisition denied.
- Checkpoint and model-attempt history, including strategy/model changes.
- Transaction-local tenant context and fail-closed PostgreSQL RLS across tenant-owned core data, tokens, rate limits and engineering execution tables.
- Runtime startup rejects database roles with superuser or BYPASSRLS privileges. Local Compose and CI use `ush_app` as the restricted migration login; after migrations, every API pool connection switches to the non-login `ush_runtime` role (`NOLOGIN`, `NOSUPERUSER`, `NOBYPASSRLS).

**Not yet production-ready:** the Railway worker is not yet consuming the USH task queue. The API primitives and database controls do not by themselves create a working autonomous runtime. The remaining work is to connect the Railway project adapter, create real Git worktrees in leased workspaces, separate the role-specific executors, and prove the complete task-to-PR-to-independent-review-and-recovery flow in a sandbox.

No production execution has been enabled and no broad repository permissions have been granted. Railway's existing `distinguished-transformation` project remains the intended execution plane; its current workers are disabled and their live cron configuration still needs reconciliation with the documented role cadence.

## Security-first product roadmap

Security is the primary product requirement. The complete milestone map, threat model, data classification, controls and release gates are documented in docs/SECURITY_FIRST_ROADMAP.md, with current verified state in docs/MAP.md and control requirements in docs/SECURITY.md. Tracking: https://github.com/Aloews/russian-dev-projects/issues/40.

The Hub is not production-ready or security-certified. Customer-facing identity, MFA/passkeys, full tenant RLS, encrypted OAuth vault, managed KMS key lifecycle, security operations and independent verification remain gated roadmap work.

## Provider integration matrix

See [`docs/PROVIDER_API_MATRIX.md`](docs/PROVIDER_API_MATRIX.md) for the requested hosting, backend, database, object-storage and queue provider matrix, current verification coverage and remaining resource-management work.

## Run locally

The production container runs the Rust Axum/SQLx runtime. PostgreSQL is required. With Docker Compose v2 installed:

~~~bash
cd universal-service-hub
bash scripts/dev-up.sh
~~~

This creates local-only credentials in .env.local, starts PostgreSQL 17 and the Hub, applies SQLx migrations, and checks database readiness plus authenticated API access. The API is available at http://localhost:8000. See docs/OPERATIONS.md for shutdown, direct Rust development and deployment configuration.

The Python/FastAPI files remain a compatibility/reference implementation; they are not the production container entrypoint. The current Hub is an authenticated HTTP API, not a browser-based management UI; the health/readiness URLs are directly viewable in a browser, while administrative operations use the API token.

## API

Public:
- `GET /healthz`
- `GET /readyz`

Administrative endpoints require `Authorization: Bearer $USH_API_TOKEN`. This root token must be at least 32 characters. Project runtime endpoints accept project tokens issued by the root administrator or, within their own project, an owner token. Production startup fails if the root token is missing or the database URL is not PostgreSQL. Set `USH_ENVIRONMENT=production`; startup then requires PostgreSQL and a complete Infisical machine identity configuration. Environment-variable secret fallback is development-only.

Provider management:
- `GET /v1/providers` — list providers
- `GET /v1/providers/{provider_id}/models` — discover current model IDs from the provider API
- `POST /v1/integrations/{provider_id}/verify` — verify read-only account/API access for Northflank, Railway, Cloudflare, Netlify, Render, DigitalOcean, Fly.io, Supabase Management API and Storage, Neon, Cloudflare R2, Backblaze B2, generic S3-compatible storage, PostgreSQL and Redis
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

## Trusted GitHub Actions verification

The execution API now has a dedicated evidence-ingestion route:

`POST /v1/projects/{project_id}/tasks/{task_id}/verification/github-actions`

Send the GitHub Actions OIDC JWT as a Bearer token. The request body is not accepted as proof. The Hub:

- Verifies the JWT signature using GitHub's published JWKS, and validates issuer, configured audience, expiry and subject.
- Requires the caller repository to match the task's `metadata.source_repository`.
- Requires an allowlisted reusable workflow reference (`job_workflow_ref`) and its exact pinned `job_workflow_sha`.
- Rejects pull-request refs and accepts only `push` or `workflow_dispatch` events on branch refs.
- Fetches the run directly from GitHub's Actions API and checks repository, workflow path, branch, event, run ID, run attempt, completion status, successful conclusion and exact head SHA.
- Persists an append-only, tenant-scoped receipt bound to the task's current candidate SHA. Duplicate delivery of the same attestation is idempotent; conflicting reuse is rejected.
- Allows a separate read-only reviewer to mark the task verified only when a matching successful receipt exists and the reviewer differs from the latest implementation/build/test worker.

Configure these variables before enabling the integration:

- `USH_GITHUB_OIDC_AUDIENCE` — exact audience requested by the trusted workflow.
- `USH_GITHUB_ALLOWED_JOB_WORKFLOW_REFS` — comma-separated exact reusable workflow refs, including repository, file path and ref.
- `USH_GITHUB_TRUSTED_JOB_WORKFLOW_SHA` — 40-character commit SHA of the approved reusable workflow implementation.
- `USH_GITHUB_API_TOKEN` — optional GitHub API token for higher API rate limits; required if the target repository's Actions API is not publicly readable.

The trusted caller workflow must request `id-token: write`, invoke the allowlisted reusable workflow pinned to the approved ref, and run the evidence submission only after its test/build dependencies succeed. The task must already be in `review`, have a pinned candidate SHA, and identify its repository in `metadata.source_repository`.

The receipt stores the OIDC token hash, not the raw token. Artifact digest capture and full adversarial end-to-end tests remain outstanding; this endpoint is not a production activation signal by itself. Do not enable worker completion or production verification until the exact-head CI, trusted workflow wiring, API credentials/rate limits and operational acceptance gates have passed.

## Secret manager

Infisical Universal Auth is supported for provider credentials. Configure:

- `USH_SECRET_BACKEND=infisical`
- `INFISICAL_URL` (optional; defaults to `https://app.infisical.com`)
- `INFISICAL_CLIENT_ID`
- `INFISICAL_CLIENT_SECRET`
- `INFISICAL_PROJECT_ID`
- `INFISICAL_ENVIRONMENT`
- `USH_INFISICAL_SECRET_PATH_PREFIX` (optional; defaults to `/projects`)

Store each project-scoped provider key in Infisical under `/projects/{project_id}/providers` with the same name as the provider's `api_key_env` reference (for example `OPENAI_API_KEY`). Root-administrator integrations and shared infrastructure accounts use `/projects/shared/providers`; the integration verification endpoint resolves its credential reference from this shared path. The Hub exchanges the machine identity credentials for a short-lived token, caches it until shortly before expiry, and retrieves the key only when dispatching a request. Secret values are not written to the provider table, audit events or logs. In development only, when Infisical is not configured and `USH_SECRET_BACKEND` is not set to `infisical`, provider credentials can be read from environment variables.

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

Send this object to `PUT /v1/providers/anthropic`. In production, store `ANTHROPIC_API_KEY` in Infisical at the project's `/providers` path; environment variables are development-only. For Starimg, use this adapter only after its current account documentation confirms the Anthropic Messages endpoint and use the model identifier returned by that provider. The API response reveals only whether the referenced credential is configured; it never returns the environment-variable name or secret.

Supported adapter types:
- Infrastructure verification adapters: `northflank`, `railway`, `cloudflare`, `netlify`, `render`, `digitalocean`, `flyio`, `supabase`, `supabase_storage`, `neon`, `cloudflare_r2`, `backblaze_b2`, `s3_compatible`, `postgresql`, `redis` (read-only checks only; PostgreSQL requires verified TLS; public Redis requires TLS)
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
6. Managed secret-store integration and read-only infrastructure account verification. (Implemented; live credential checks pending.)
7. Provider resource-management adapters for compute, deployments, databases, storage lifecycle, queues and observability.
8. Policy engine: per-project budgets, quotas, cost/latency/region constraints, circuit breakers and fallback chains.
9. GitHub-driven project onboarding, isolated build/test workers and independent QA gates.
10. Deployment controllers, staged rollout, synthetic monitoring, rollback and operator dashboard.

See [ARCHITECTURE.md](./docs/ARCHITECTURE.md), [SECURITY.md](./docs/SECURITY.md), [OPERATIONS.md](./docs/OPERATIONS.md), and [INTEGRATION_PLAN.md](./docs/INTEGRATION_PLAN.md).


## Coding the Universal Service Hub

The Hub has a dedicated Codex CLI + Claude Code CLI workflow through Starimg. Codex is the primary writer using the OpenAI Responses API; Claude Code independently reviews and repairs the resulting diff using the Anthropic Messages API. The workflow discovers exact model IDs, verifies tool calling on both protocols, runs the agents sequentially, restricts changes to this Hub subtree and opens an isolated PR. GitHub CI validates the exact PR head. No automatic merge or production deployment is permitted. See [CODING_AGENTS.md](./docs/CODING_AGENTS.md).

## Product Factory for other projects

The separate Autonomous Product Factory workflow orchestrates work in target product repositories. Its current provider/model route is distinct from the Hub self-development workflow; do not infer that Starimg Codex/Claude configuration has migrated the target-product pipeline. See [PRODUCT_FACTORY.md](./docs/PRODUCT_FACTORY.md) for its current execution and safety contract.


## API request limits

The Rust runtime enforces a PostgreSQL-backed fixed-window rate limit per authenticated bearer token. Default: 120 requests per minute. Configure `USH_RATE_LIMIT_PER_MINUTE` between 1 and 10000. Exceeded limits return HTTP 429 and `Retry-After: 60`. This does not replace unauthenticated IP-level throttling at the edge.

Rate-limit fingerprint rows idle for 24 hours are pruned at startup and hourly. This keeps the limiter's metadata table from retaining inactive token fingerprints indefinitely.

## Trusted GitHub Actions verification

Tasks can enter `verified` only after the Hub accepts a signed GitHub Actions OIDC token and independently confirms the referenced workflow run through the GitHub Actions API. The runtime stores an immutable, tenant-scoped evidence receipt tied to the task's exact repository and candidate SHA. Worker-supplied JSON alone cannot certify a task.

Configure these server-side variables only after reviewing and pinning the reusable workflow:

- `USH_GITHUB_OIDC_AUDIENCE`: exact audience requested by the reusable workflow.
- `USH_GITHUB_ALLOWED_WORKFLOW_REFS`: comma-separated exact caller `workflow_ref` values; no wildcard matching.
- `USH_GITHUB_ALLOWED_JOB_WORKFLOW_REFS`: comma-separated exact reusable `job_workflow_ref` values; no wildcard matching.
- `USH_GITHUB_TRUSTED_JOB_WORKFLOW_SHA`: exact 40-character `job_workflow_sha` of the approved reusable workflow.
- `USH_GITHUB_API_TOKEN`: optional read-only Actions token for private repositories or increased API rate limits.

The reusable workflow requires an artifact digest in `sha256:<64 hex characters>` form. The calling build/test workflow must calculate it from the exact artifact it wants reviewed (for example, `sha256sum build-artifact.tar.gz`) and pass it as `artifact_digest`. The Hub persists that digest with the GitHub run start/completion timestamps and signed identity claims.

Projects call `.github/workflows/ush-trusted-verify.yml` only after their own build/test workflow succeeds, passing the Hub URL, audience, project ID, task ID and exact candidate SHA. The reusable workflow requests a short-lived GitHub OIDC token and posts it directly to the Hub; it does not expose a long-lived Hub credential. The API additionally verifies the completed run's repository, caller workflow path, branch, event, run ID/attempt, conclusion and SHA.

Do not enable the endpoint in production until the reusable workflow is merged and its immutable SHA/ref are configured, exact-head CI passes, and a real OIDC round-trip is tested. The example values above are intentionally blank.
