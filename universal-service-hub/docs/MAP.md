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

- Rust Axum/Tokio/SQLx control-plane runtime is under active migration; FastAPI remains a temporary reference implementation.
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
- Rust implementation of project/token management, provider upsert/delete, health probes, model discovery, selection, audit reads and initial AI chat adapters.
- Infisical Universal Auth machine-identity login, short-lived token cache and project-scoped provider secret lookup (live verification pending).

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


## AI engineering toolchain — required project standard

This section is shared across the active project maps. It records installation and runtime requirements; a repository entry is **not** proof that a tool is installed or active.

### 1. Caveman — concise agent output

- Upstream: https://github.com/JuliusBrussee/caveman
- Claude Code: `claude plugin marketplace add JuliusBrussee/caveman && claude plugin install caveman@caveman`
- Codex CLI: `npx -y github:JuliusBrussee/caveman -- --only codex` (or install its skill with `npx skills add JuliusBrussee/caveman -a codex -g`).
- Use the project/user scope matching the actual coding-agent host. Restart/reload the host and verify the skill is discoverable before counting it as installed.
- Do not apply compression to source code, identifiers, error messages, test assertions, API contracts, security rules, or acceptance criteria. Preserve negations and exact technical facts.

### 2. codebase-memory-mcp — persistent code graph

- Upstream: https://github.com/DeusData/codebase-memory-mcp
- Supported host install (macOS/Linux): `curl -fsSL https://raw.githubusercontent.com/DeusData/codebase-memory-mcp/main/install.sh | bash`
- The installer detects supported coding-agent clients and registers MCP/instructions where available. Restart the coding agent, verify the MCP server is visible, then explicitly index this repository.
- Keep the graph/index on a persistent developer or agent host. A disposable GitHub Actions runner or ephemeral Railway cron container is not a persistent memory installation.
- Before production rollout, pin and verify the release/checksum; do not silently install unpinned `main` into a production runtime.

### 3. Spotify Portal AI Plugins — AiKA delegation

- Upstream: https://github.com/spotify/portal-ai-plugins
- Claude Code:
  `claude plugin marketplace add spotify/portal-ai-plugins`
  `claude plugin install portal@portal`
  `claude plugin install shunt@portal`
- Codex: `codex plugin marketplace add spotify/portal-ai-plugins`, then install Spotify Portal from `/plugins`. Cursor is installed through its team marketplace.
- Start a new supported agent session and run `/portal:setup`; authenticate the Portal CLI. Run its read-only doctor/readiness checks.
- The shunt plugin is Claude Code-only. It requires Portal CLI authentication and AiKA availability; verify `bulk-reader` and `code-writer` modes before relying on delegation. Do not create modes or change Portal settings without explicit authorization.
- Never put Portal credentials or local MCP configuration in this repository.

### 4. Runtime compatibility and installation evidence

- These are coding-agent integrations, not generic Python/Node runtime packages. A worker that only sends HTTP requests to the Starimg API cannot load Claude/Codex plugins or call a local MCP server unless it is explicitly given that agent runtime and MCP client.
- GitHub Actions runners are ephemeral: use them for bounded jobs, CI checks and a temporary fallback agent, not as the persistent host for a local code graph or authenticated Spotify Portal session.
- The Railway `stream-service` worker is a Python/Starimg HTTP worker; its current code path does not expose a Claude Code/Codex plugin host or MCP client. Do not claim these integrations are installed there.
- Record each tool independently as `not installed`, `installed`, `connected`, `indexed`, and `verified`. Only advance a state after observing its exact runtime check. Never store tokens or secret values in this map.
- Current blocker: the last Starimg Fable task request returned HTTP 402 before model execution. Do not retry billable generation until the provider returns a successful, authenticated test request.

### 5. GitHub Actions fallback

- When the Railway worker is unavailable, use a repository-owned GitHub Actions agent workflow only if that workflow exists, its trigger/permissions are verified, and the model/provider can be invoked successfully.
- For Sherlock Scholes, `.github/workflows/local-autonomous-agent.yml` listens for an owner-authored issue comment beginning with `/ai ` and runs the local Qwen2.5-Coder 3B model on a GitHub-hosted runner. It is a separate fallback from Starimg/Fable, has a restricted patch allowlist, and must produce a draft PR with CI evidence. It is not Fable 5.2.
- Do not assume another repository has this fallback: inspect its workflow and trigger before requesting work there.
- No automatic merge or production deployment. Review the exact PR head, run required checks, and verify deployment separately.

### Installation status (2026-11-14)

| Integration | Repository documentation | Active runtime installation |
|---|---|---|
| Caveman | Project skill at `.agents/skills/caveman/SKILL.md` | Included in this PR; runtime discovery/activation not yet verified |
| codebase-memory-mcp | Recorded here | Not verified; no persistent supported host indexed |
| Spotify Portal / shunt | Recorded here | Not verified; Portal authentication and AiKA readiness not established |

