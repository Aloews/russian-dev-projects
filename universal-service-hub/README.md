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

Tasks can enter `verified` only after a separate, trusted attestation workflow proves that the source build/test run completed successfully. The Hub validates a signed GitHub OIDC token, independently fetches the source run from GitHub's Actions API, and stores an immutable tenant-scoped receipt bound to the task's exact repository and candidate SHA. A worker cannot certify its own task by supplying JSON.

The flow deliberately uses two workflow runs: the source CI workflow uploads a small image/test manifest; `.github/workflows/ush-post-ci-verification.yml` runs on GitHub's `workflow_run` event after source CI completes; it calls `.github/workflows/ush-trusted-verify.yml`, which downloads the source run's artifact, checks its SHA and run metadata, requests a short-lived OIDC token, and submits the source run ID/attempt and artifact digest. This avoids the impossible condition of trying to verify a workflow run while that same run is still in progress.

Configure server-side variables only after reviewing the exact workflow files and pinning the reusable workflow:

- `USH_GITHUB_OIDC_AUDIENCE`: exact audience requested by the reusable workflow.
- `USH_GITHUB_ALLOWED_WORKFLOW_REFS`: exact post-CI caller ref, e.g. `Aloews/russian-dev-projects/.github/workflows/ush-post-ci-verification.yml@refs/heads/main`.
- `USH_GITHUB_ALLOWED_JOB_WORKFLOW_REFS`: exact reusable workflow ref, e.g. `Aloews/russian-dev-projects/.github/workflows/ush-trusted-verify.yml@refs/heads/main`.
- `USH_GITHUB_TRUSTED_JOB_WORKFLOW_SHA`: exact 40-character SHA of the approved reusable workflow at the deployed default branch.
- `USH_GITHUB_ALLOWED_SOURCE_WORKFLOW_PATHS`: exact source workflow path, e.g. `.github/workflows/universal-service-hub-ci.yml`.
- `USH_GITHUB_ALLOWED_SOURCE_BRANCHES`: exact protected source branches allowed to produce evidence, e.g. `main`.
- `USH_GITHUB_API_TOKEN`: optional read-only Actions token for private repositories or increased API rate limits.

Only `push` and `workflow_dispatch` source runs on an explicitly allowlisted protected branch can be attested. `pull_request` source runs are deliberately rejected: a same-repository pull request can modify its own CI workflow and falsely report success. This keeps source evidence anchored to a protected branch rather than trusting a workflow file supplied by the candidate change.

The post-CI caller also requires GitHub repository variables `USH_API_BASE_URL`, `USH_GITHUB_OIDC_AUDIENCE`, `USH_PROJECT_ID`, and `USH_TASK_ID`. The current caller uses one configured task ID as an initial integration hook; dynamic task resolution for concurrent project tasks remains a separate acceptance item. The source workflow creates a manifest containing the exact commit, run identity and container image ID/layer list; the reusable workflow downloads that manifest from the completed source run and calculates its SHA-256 digest. The Hub stores the source run start/completion timestamps, source and attestation run identities, caller/reusable workflow claims, candidate SHA and artifact digest.

Do not enable this endpoint in production until the reusable workflow is merged, its immutable SHA/ref and source-path allowlist are configured, exact-head CI passes, and a real signed OIDC round-trip is tested. The example values remain blank.
