# Universal Service Hub — project map

## Mission

One provider-neutral control plane for all owned projects. It must route workloads to interchangeable providers, protect credentials, measure reliability and spend, and recover from provider failures without requiring the owner to process intermediate agent output.

## Operating contract

- The authoritative Reality-First Business skill lives at `.github/product-factory/skills/reality-first-business/SKILL.md` and is injected into every implementation and specialist session. Every product project map must reference this standard; agents must separate verified facts, assumptions and unknowns and never infer completion without evidence.

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
- Isolated Neon Free PostgreSQL project created for Hub: `wandering-haze-16638539` (`aws-eu-central-1`, PostgreSQL 17, 0.25–1 CU autoscaling). It is not yet connected to a deployed Hub service; no connection string is committed or exposed.
- Railway project creation attempt: blocked by the workspace Free-plan resource limit; no existing Sherlock or Novel Forge project was modified.
- Live provider inference: not yet verified.
- No paid cloud resources were provisioned by these changes.

## Implemented capabilities

- Rust Axum/Tokio/SQLx is the production container runtime; FastAPI remains a temporary compatibility/reference implementation.
- Rust bearer-token authentication for root administrator and scoped project tokens.
- Durable provider registry on PostgreSQL, Alembic migrations and local SQLite development.
- Provider CRUD, capability/priority selection and bounded health checks.
- OpenAI-compatible, Anthropic Messages and Gemini generateContent adapters.
- Normalized chat request/response and usage metadata.
- Environment-variable credential references; API keys are not stored in provider records.
- Conservative failover policy.
- HTTPS/local/private destination validation and DNS resolution checks before outbound requests.
- Durable audit records for provider mutations and AI inference outcomes.
- Project registry with scoped, hashed, revocable API tokens and owner/operator/viewer roles.
- Project-level capability and provider allowlists for AI inference and provider selection.
- Rust implements project/token management, provider CRUD, health probes, model discovery, selection, audit reads, AI chat routing and read-only infrastructure account verification.
- Infisical Universal Auth machine-identity login, short-lived token cache and project-scoped provider secret lookup (live verification pending).

## Infrastructure API adapter increment

Branch: `feat/ush-provider-api-adapters-final-v2`, layered on the latest observed Rust tenancy head.
- Read-only account checks: Northflank, Railway, Cloudflare, Netlify, Render, DigitalOcean, Fly.io, Supabase Management API and Neon.
- Storage management checks: Supabase Storage bucket inventory, Cloudflare R2 bucket inventory and Backblaze B2 account authorization.
- Authenticated `POST /v1/integrations/{provider_id}/verify` resolves credentials through Infisical and uses fixed HTTPS endpoints, bounded timeouts, no redirects, normalized responses and redacted audit events.
- These checks do not provision hosting resources or perform object upload/download. S3-compatible SigV4 bucket verification now covers AWS S3, Cloudflare R2, Supabase Storage S3 protocol and Backblaze B2. Object upload/download remains outstanding. PostgreSQL and Redis probes are read-only: TLS-enforced SELECT 1 and rediss:// PING; they do not manage schema, replication, queues or locks.
- PR and exact-SHA CI validation are required before merge.

## Work in progress

- PR #21: provider-neutral Universal Service Hub + Railway-independent Product Factory, branch `feat/ush-autonomous-product-factory`.
- Exact CI head `f3a6f673f485d65388f5fa24cf54948c8da06b42` passed Rust format/Clippy/tests, Python Ruff/migrations/pytest, Docker build and container smoke test. Later bootstrap-trigger and agent-permission commits are undergoing CI on their new exact head.
- Product Factory uses GitHub-hosted runners, Starimg Anthropic Messages via Claude Code/Fable 5.2, isolated verification and a separate PR publication job. No Railway Cloud Agent Tasks are required.
- The 2026-11-14 bootstrap run `43599416331` passed credential preflight and target-repository checkout using the configured Actions secrets. Fable 5.2 action step completed successfully; the local Qwen fallback step was subsequently running and its outcome is not yet verified. This does not prove Starimg tool-call compatibility or successful candidate generation.
- PR #20 is superseded by PR #21 and must not be merged independently.
- No AUREL product code has yet been generated or published by Product Factory.

Remaining platform work after the Product Factory activation:
- Provider-specific resource management (provision/deploy/scale/logs/rollback) behind explicit approval and dry-run plans.
- Durable asynchronous jobs, queue adapters, retry/dead-letter policies and idempotent task execution.
- Per-project/provider budgets, spend ceilings, quotas, usage metering and kill switches.
- GitHub App installation and project onboarding automation.
- Production hosting, managed PostgreSQL, Infisical machine identity, backups, alerts and rollback rehearsal.

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

1. Security: token expiry and fine-grained scopes, SSO/per-user identity, audit retention/access, rate limits, egress firewall, managed secret storage, dependency/image scanning.
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


## Autonomous Product Factory increment

Branch: `feat/ush-autonomous-product-factory`, PR #21 (draft, retargeted to `main`; includes the provider-adapter work previously proposed in PR #20).
- Railway Cloud Agent Tasks have been removed from the Product Factory runtime. Product Factory uses GitHub Actions runners and Starimg; Railway remains only an optional hosting-provider adapter in the Universal Service Hub.
- Workflow: `.github/workflows/product-factory.yml`. Supports owner-triggered `workflow_dispatch`, a narrowly scoped feature-branch bootstrap push, and owner-only `/factory run`. Private target repositories are allowed by the current workflow; the control repository and its Actions artifacts are private.
- Primary model configured: Claude Fable 5.2 through the Starimg Anthropic Messages endpoint. Live endpoint/tool-call compatibility has not yet been proven.
- Read-only specialists: Opus 5.5 architecture/code review, Sonnet 5.5 security/privacy, Haiku 5.5 product/user-journey review.
- Agent, verification and publication run on separate ephemeral jobs. The model gets file tools only; it cannot run shell, network, Git push, GitHub API, deployment or production-data tools. It cannot modify `.github/workflows/**` or `.github/actions/**`.
- Verification jobs execute Rust formatting/Clippy/tests/release build, npm install/audit/lint/tests/build, required Playwright browser E2E and production API Docker build. They receive neither the Starimg key nor a write token in the test process environment.
- Up to two repair cycles use the actual failed verification report. A fresh publication job receives only the verified patch artifact, rechecks the target and base branch SHAs, then opens/updates a PR. No target PR is auto-merged or deployed.
- Required Actions secrets: `STARIMG_API_KEY` and target-scoped `PRODUCT_FACTORY_GITHUB_TOKEN` with Contents read/write and Pull requests read/write. The latest run passed the secret-presence and GitHub target write-permission preflight. Optional variables: `STARIMG_ANTHROPIC_BASE_URL`, `STARIMG_FABLE_MODEL`.
- CI validates the workflow, composite action metadata and verification script. The current exact head is `4e2d9e8577e4fd640f3595479dca57be4640c45d`; run `43599420761` is still in progress, with Rust format and Product Factory shell/workflow/action validation passed and Clippy running at last inspection.
- A real agent workflow has started. A candidate patch, independent verification report and target PR have not yet been evidenced.
- PR #20 is the earlier provider-adapter proposal; verify its current status before closing or superseding it.
- First target: `Aloews/full`, working branch `factory/elite-estate`, PR base `foundation/luxury-estate-platform`.
- AUREL is not production-ready until its dedicated database, real identity/JWT issuance, public enquiry abuse controls and PII retention, licensed inventory/partners, jurisdiction review, staging E2E and rollback gates pass.
- No merge of the target product PR, production deployment, paid hosting provisioning, billing change, DNS change or production-data mutation has been performed.

### Remaining acceptance gates
- Pass CI and actionlint on the exact final PR #21 head SHA; resolve any workflow or composite-action validation errors.
- Integrate PR #21 into `main`; PR #20 is superseded by the consolidated branch.
- Confirm the successful credential and target write-access preflight in run `43599416331`; never expose secret values.
- Execute one controlled run to prove Starimg authentication, Messages API tool calling, Fable model ID, GitHub branch write and PR creation.
- Inspect the resulting verification artifact and target PR; do not mark the factory operational until all gates are directly evidenced.
- Complete AUREL's owner-only release gates before any production deployment.

## Agent runtime implementation — 2026-11-14

- Primary implementation agent: Claude Code with Starimg model `claude-fable-5-2`.
- Caveman plugin 4.1.0 is installed per session from the signed, verified release commit `1e91f1306d602d4ca23618701b1185575ae1869b`. The workflow clones the exact tagged commit, adds its local marketplace and installs `caveman@caveman`; it does not install a moving `main` branch.
- Local Qwen2.5-Coder 3B runs a sequential multi-role team (architecture, product, security, QA, implementation and independent QA) on GitHub-hosted runners. Its structured edits use exact old-text matches and source-path validation; the evidence report is reviewed by Fable 5.2.
- Fable 5.2 acts as the senior engineer and problem fixer: it inspects the Qwen review report and actual diff, independently challenges the findings, repairs defects and owns the final candidate. If Qwen fails, Fable can implement directly; if Fable fails, the bounded Qwen fallback is available.
- Verification now detects Rust, Python, Go, Gradle/Maven and Node projects; browser E2E is required for detected browser apps and not imposed on non-browser Node services. Additional stacks still need explicit verifier adapters before they can be considered covered.
- The Qwen team, Fable senior engineer and subsequent repair cycles feed the same isolated verification, evidence-driven repair and PR publication pipeline. No merge or production deploy is automatic.
- The model is installed per ephemeral runner, not as a persistent Railway process. Current run must prove fallback execution before marking Qwen active.
- codebase-memory-mcp and Spotify Portal are not installed: this runtime has no persistent supported MCP host or authenticated Portal/AiKA session. Do not claim otherwise.
