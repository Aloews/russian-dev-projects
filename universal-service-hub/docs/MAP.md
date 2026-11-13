# Universal Service Hub — project map

## Mission

One provider-neutral control plane for all owned projects. It must route workloads to interchangeable providers, protect credentials, measure reliability and spend, and recover from provider failures without requiring the owner to process intermediate agent output.

## Operating contract

- The owner receives only a verified completion report or a concrete blocker requiring owner-only access or a spending decision.
- Agents and CI output are internal evidence. The coordinator evaluates failures, edits code, reruns checks and makes implementation decisions autonomously.
- Never report an action as complete without a commit, successful test/run, or live system evidence appropriate to the claim.
- Inspect current repository state before every implementation cycle.
- Each implementation increment uses an isolated branch and PR. No concurrent writers to the same branch.
- Reviewer and QA inspect the exact immutable PR head SHA.
- Do not merge a failing or unreviewed change.
- Do not deploy, provision paid infrastructure, migrate a live database or expose credentials without explicit authorization and a rollback plan.
- Never move secrets from another project into this Hub without explicit authorization.
- Never retry a billable generation request unless idempotency is explicitly supported by that provider and a request key is present.
- Do not store prompts, generated content or credentials in logs/audit events.

## Current verified state

Base repository: `Aloews/russian-dev-projects`

- PR #2: foundation — merged earlier.
- PR #4: durable PostgreSQL provider registry and bounded health probes — merged earlier; CI and PostgreSQL migration smoke test passed.
- PR #5: AI adapters and normalized chat routing — merged 2026-11-13; exact final head `f8a2b7de53647aeaf3aa134868935eaf62a6536e`; GitHub Actions run #107 passed Ruff, PostgreSQL 16 migration smoke test, pytest, Docker build and container smoke test.
- PR #8: outbound destination checks and durable audit trail — merged 2026-11-13; final head `482e4f4542f8db4900024d12350cf92cb280d0d9`; GitHub Actions run #143 passed Ruff, PostgreSQL migration, pytest, Docker build and container smoke test.
- Current production deployment: none.
- Live provider inference: not yet verified.
- No paid cloud resources were provisioned by these changes.

## Implemented capabilities

- Rust Axum/Tokio/SQLx is the production container runtime; FastAPI is retained temporarily as a compatibility/reference implementation.
- Root administrator bearer authentication and scoped project tokens with owner/operator/viewer roles.
- Durable PostgreSQL provider, project, token and audit tables; SQLx migrations run at startup.
- Provider CRUD, capability/priority selection and bounded health checks.
- OpenAI-compatible, Anthropic Messages and Gemini generateContent AI adapters.
- Read-only account verification adapters for Northflank, Railway, Cloudflare, Netlify, Render, DigitalOcean, Fly.io, Supabase Management API and Neon.
- Cloudflare R2 bucket inventory and Backblaze B2 credential authorization checks.
- Normalized chat request/response and usage metadata.
- Provider API credentials are resolved through Infisical secret references; raw credentials are not stored in provider records.
- Conservative AI failover policy with idempotency safeguards.
- HTTPS destination validation, DNS resolution checks, redirect blocking and bounded outbound requests.
- Durable audit records exclude prompts, responses and secret values.
- Project-level capability and provider allowlists for AI inference and provider selection.
- Infisical Universal Auth machine-identity login, short-lived token cache and project-scoped secret lookup; live account verification is pending.

## Infrastructure API adapter increment

Branches: `feat/ush-infra-api-verification-v2` and `feat/ush-database-and-fly-apis`, layered on the Rust tenancy branch.
- Read-only account verification adapters: Northflank, Railway, Cloudflare, Netlify, Render, DigitalOcean, Fly.io, Supabase Management API and Neon.
- Object-storage management checks: Cloudflare R2 bucket inventory and Backblaze B2 account authorization.
- Authenticated `POST /v1/integrations/{provider_id}/verify` uses fixed vendor HTTPS endpoints, Infisical secret references, bounded timeouts, redirect blocking, normalized responses and redacted audit events.
- These endpoints verify access only. They do not yet provision hosting resources or implement general object upload/download, Redis or PostgreSQL data-plane operations.
- Both increments remain unmerged pending exact-SHA CI and review.

## Work in progress

Project tenancy increment on branch `feat/ush-project-tenancy`:
- Add project and project-token tables with PostgreSQL migration.
- Issue one-time raw tokens; store SHA-256 hashes only; support revocation and last-used timestamps.
- Separate root-only administration from project-scoped AI/provider selection.
- Enforce owner/operator/viewer roles and per-project capability/provider allowlists.
- Permit viewer tokens read-only provider selection; restrict AI inference to owner/operator.
- Allow project owners to manage operator/viewer tokens without granting root administration.
- Add project update/delete and lifecycle tests.
- Require passing CI, review exact head SHA, then merge.

## Owner-selected provider pool

Preferred infrastructure categories recorded in `docs/INTEGRATION_PLAN.md`:
- Hosting/frontend: Northflank, Cloudflare, Netlify, Render.
- Backend/API/workers: Northflank, Railway, Fly.io, DigitalOcean.
- Databases: Supabase, Neon, self-managed PostgreSQL.
- Object storage: Supabase Storage, Cloudflare R2, S3, Backblaze B2.
- Queues/background work: Railway Redis, Northflank Redis, with Redis-compatible alternatives.

Current ChatGPT-connected management integrations available to the agent: GitHub, Railway, Supabase and Vercel. Direct connector tools for Northflank, Cloudflare, Netlify, Render, Fly.io, DigitalOcean, Neon and Backblaze are not available in this workspace at present. These vendors are included in the Hub's provider catalog and require their own API credentials/adapter implementation for direct account management.

GitHub OAuth sign-in to a hosting vendor is not equivalent to granting the Hub that vendor's management API access. Repository linking can enable GitHub-triggered deploys; direct service creation, configuration, database/storage administration and rollback require separately scoped vendor API credentials. Never mark a vendor connected until a live API identity check succeeds.

## Production readiness gates

1. Security: SSO/per-user identity, fine-grained scoped RBAC, audit retention/access, rate limits, egress firewall, managed secret storage, dependency/image scanning.
2. Persistence: managed PostgreSQL, backups, restore rehearsal, migration/rollback runbook.
3. Provider reliability: live protocol verification for each vendor, provider-specific limits, health probes, circuit breakers, quota and spend controls.
4. Routing: compute/storage/database/queue/observability adapters, deterministic policies, safe fallback semantics.
5. Operations: metrics, traces, alerts, synthetic checks, deployment health, staged rollout and rollback.
6. Project onboarding: per-project service identity, adapter configuration, secret references and verified test suite.
7. Deployment: dedicated isolated hosting environment, TLS, domain, network egress restrictions, secrets and live smoke tests.

## Acceptance criteria for a production release

- All required CI checks pass on the exact release commit.
- Migration is applied and verified against PostgreSQL.
- Container starts and readiness succeeds in a clean environment.
- Authenticated routes reject missing/invalid credentials.
- No secret values, prompts or generated content appear in API output, logs or audit data.
- Outbound network policy blocks loopback, private, link-local and cloud metadata destinations.
- Provider contract tests and at least one explicitly authorized live smoke test per configured adapter pass.
- Backup/restore and rollback procedures are tested.
- Budget limits and alerting are active before paid services are enabled.
- No existing project production database or service is modified by Hub onboarding without an isolated, reviewed change.
