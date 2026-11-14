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

- PR #21: provider-neutral Universal Service Hub + Railway-independent Product Factory, branch `feat/ush-autonomous-product-factory`, draft and not merged.
- Product Factory now probes the configured Starimg Anthropic Messages endpoint and uses Starimg `claude-opus-5` as the primary implementation and repair model. Local Qwen2.5-Coder 3B is invoked only when the Starimg tool-call probe or Opus agent fails.
- GitHub Actions uses isolated agent, verification, repair and publication jobs. Only a candidate that passes the configured verification gates can be published as a PR. No automatic merge or production deployment.
- The Qwen editor now supplies bounded existing-file context on a rejected new-file edit, rejects symlink path escapes, and has regression coverage for both cases.
- The workflow prevents new runs from cancelling an active product build; the initial local Qwen fallback has a bounded 40-minute step and a 20-minute inference timeout.
- PR #31 (`fix/factory-starimg-primary`) contains the Starimg-first routing, bounded Qwen fallback and regression fixes; it remains open and unmerged.
- First AUREL acceptance run #75 (`43639707964`) validated target-repository write access, but Starimg returned HTTP 402 `insufficient_balance_error`; Qwen fallback then rejected an existing-file edit incorrectly marked as a new file. No candidate patch was produced.
- The Qwen edit validator now accepts only sufficiently complete, substantially similar whole-file replacements when a small model emits an empty old snippet for an existing path; unrelated/short replacements remain rejected. Regression tests were added, but the revised tests have not yet passed on a GitHub runner.
- Latest exact-head CI run #900 (`43641015883`) failed before any job step with `runner_id=0` and no runner/logs. This is a runner/account availability blocker; the reason (for example, Actions quota versus transient GitHub runner failure) is not verified. Do not merge until a real runner completes the full checks.
- Product Factory has not produced a candidate artifact, isolated verification report, AUREL delivery PR or preview URL. No product has been delivered or deployed.
- PR #20 is superseded by PR #21 and must not be merged independently.

Remaining platform work after the first verified product delivery:
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

Branch: `feat/ush-autonomous-product-factory`, PR #21 (draft, base `main`).
- Railway Cloud Agent Tasks are not used for agent orchestration. Product Factory runs on GitHub Actions; Railway remains an optional hosting-provider adapter.
- Workflow: `.github/workflows/product-factory.yml`. Supports workflow dispatch, feature-branch bootstrap push and owner-only `/factory run`. Private target repositories are supported with a scoped `PRODUCT_FACTORY_GITHUB_TOKEN`.
- Primary implementation and repair route: Starimg `claude-opus-5` through the Anthropic Messages API. The workflow performs a real forced-tool-call compatibility probe before invoking the agent.
- Fallback route: local Qwen2.5-Coder 3B through Ollama on an isolated GitHub-hosted runner. Qwen creates or repairs a candidate only when the Starimg probe or Opus agent fails.
- Required model gateway secret: `STARIMG_API_KEY`. Optional Actions variable `STARIMG_MODEL` defaults to `claude-opus-5`; the model is not considered compatible until the live tool-call probe succeeds.
- Specialist profiles cover architecture, security, product and independent review. The model gets bounded file tools only; no shell, network, credential, Git push, GitHub API, deployment or production-data tools.
- Verification, repair and publication run in separate jobs. Verification receives neither model-provider keys nor a target-repository write token.
- Up to two repair cycles use the actual failed verification report. Publication rechecks target and base branch SHAs before creating/updating a PR. No target PR is auto-merged or deployed.
- Required Actions secrets: `STARIMG_API_KEY` and target-scoped `PRODUCT_FACTORY_GITHUB_TOKEN` with Contents read/write and Pull requests read/write. The Qwen fallback runs if Starimg is unavailable or incompatible.
- The current acceptance target is `Aloews/full`, working branch `factory/elite-estate-rc4`, base `foundation/luxury-estate-platform`.
- AUREL is not production-ready until real identity/JWT verification, public enquiry abuse controls and PII retention, licensed inventory/partners, jurisdiction review, staging E2E and rollback gates pass.
- No merge of the target product PR, production deployment, paid hosting provisioning, billing change, DNS change or production-data mutation is permitted by this workflow.

## Mandatory end-to-end product delivery contract

This is the primary operating objective of the Universal Service Hub and Product Factory. The factory exists to deliver working products, not merely connect providers, configure agents, generate plans, open PRs or produce status reports.

**Required lifecycle — every product request must pass all six stages in order:**

1. **Receive and normalize the task**
   - Capture the user's goal, intended users, problem, constraints and expected outcome.
   - Resolve routine product, architecture, stack, provider and implementation decisions autonomously.
   - Ask the owner only for genuinely owner-exclusive decisions: legal identity/authorization, credentials that cannot be accessed through approved integrations, irreversible production actions, or a material spending approval.
   - Convert the request into a product brief, explicit scope, non-goals, acceptance criteria and a testable definition of done.

2. **Design the product**
   - Inspect the target repository, existing services, project map, connected integrations, current deployment and constraints before changing anything.
   - Produce the smallest coherent product architecture and implementation plan that can satisfy the user outcome.
   - Select the stack and infrastructure from verified, interchangeable providers. Check actual account access, limits, pricing and regional constraints; a catalog entry or configured secret is not proof of a working integration.
   - Design data model, authentication/authorization, privacy, security, observability, failure handling and migration/rollback before implementation where relevant.
   - Record decisions and acceptance criteria in the target project's map. Do not ask the owner to approve ordinary engineering choices.

3. **Implement the product**
   - Build the actual user-facing product and required backend, data, integrations and operational paths—not only scaffolding, placeholder screens, mock data, agent configuration or documentation.
   - Use specialist agents for bounded work; keep one authoritative writer per branch. Escalate difficult or repeated failures to Starimg `claude-opus-5` and independent reviewers.
   - Use real provider integrations in the implementation. Mocks are permitted for isolated unit tests only and must never be presented as evidence of live integration.
   - Commit coherent increments to an isolated branch. Preserve existing production data and services.

4. **Build and launch an executable candidate**
   - Install dependencies, compile/build all required components, apply migrations to an isolated test/staging database and start the complete application in a clean environment.
   - Create and configure the required preview/staging resources using already authorized integrations. Prefer free/low-cost alternatives where fit and verified limits allow.
   - Resolve build, startup, configuration, secrets, networking and migration failures autonomously.
   - A PR, successful compilation, generated source files or a green lint job alone is not a launched product.

5. **Verify the running product**
   - Test the actual running candidate by URL/API, not only source inspection.
   - Run unit, integration, migration, API contract, browser/E2E, authentication/authorization, security and regression checks appropriate to the product.
   - Exercise the critical user journey from entry point through the core value-producing action to persisted/retrievable result.
   - Verify real external dependencies with authorized live smoke tests; record which providers/models were actually exercised and which remain unverified.
   - Check logs, readiness/health, error paths, privacy/secret leakage, performance basics and rollback/recovery.
   - Review the exact immutable candidate commit and test evidence independently. If any required gate fails, fix and rerun; do not relabel a failure as a warning or bypass required tests.

6. **Hand over a usable product**
   - Deliver a working preview/staging URL or other directly runnable artifact, the exact source repository and commit/PR, and the verified core user journey.
   - Supply only the minimum operating instructions the owner needs: how to open/use it, test access where safely provisioned, known limitations, operational status and how to report an issue.
   - State precisely what is live, what is staging, what is verified, what is not verified and any remaining owner-only blocker.
   - Do not claim "ready", "complete", "deployed" or "working" without direct evidence from the running candidate.
   - The handover is incomplete if the only deliverables are a PR, code diff, design, plan, CI result, agent report or list of configured services.

### Definition of done — product-level, not agent-level

A product is complete for handover only when all applicable conditions below are evidenced on the same candidate revision:

- [ ] The original user task is mapped to a concrete product brief and measurable acceptance criteria.
- [ ] The core product experience is implemented; no critical flow is a placeholder or mock.
- [ ] Required backend, database, authentication and external integrations are configured and actually connected.
- [ ] A clean build succeeds and the application starts in an isolated preview/staging environment.
- [ ] A reachable URL or directly runnable deliverable is provided.
- [ ] The critical end-to-end user journey succeeds against the running application and persists/returns real data as intended.
- [ ] Required automated tests, browser E2E and security checks pass on the exact candidate SHA.
- [ ] Independent QA/review examines the exact candidate diff and runtime evidence.
- [ ] Secrets, personal data and production data are protected; no credentials appear in source, logs or handover.
- [ ] Known limitations, operational dependencies and recovery/rollback steps are recorded.
- [ ] The owner receives the product URL/artifact, source/PR reference, verification summary and minimal usage instructions.

If a product has no applicable authentication, persistence, payment or external integration, explicitly mark that criterion "not applicable" with a reason; never silently skip it.

### Autonomous recovery and escalation

- A failed stage returns the task to the responsible agent with the exact error, logs and failed acceptance criterion. The coordinator must diagnose, change the implementation and rerun the failed stage plus impacted regression checks.
- Do not rerun unchanged failing jobs indefinitely. After two unsuccessful attempts with the same failure signature, change the approach/provider/model or isolate a minimal reproduction and route the exact failure evidence to Starimg Opus 5 or the Qwen fallback.
- If Starimg Opus fails, capture sanitized provider request/response metadata and tool-call protocol errors, then use the bounded Qwen fallback. Never claim model compatibility from secret presence or HTTP reachability alone.
- Use Qwen 2.5 Coder 3B for bounded specialist analysis and structured edits; validate every edit and test the resulting code. Starimg Opus 5 owns primary implementation and evidence-based repair. Independent architecture, security and product reviewers challenge behavior; their roles are not proof of successful execution.
- Preserve artifacts for each run: task brief, decision record, candidate commit, diff, build output, test results, live smoke-test evidence, provider/model identity (without secrets), runtime URL and handover record.
- Report to the owner only a verified handover or a specific owner-exclusive blocker. Agent chatter, intermediate decisions and routine failure details stay inside the factory and project artifacts.

### Safety and release boundary

- Autonomous implementation, isolated builds, tests and preview/staging deployment are expected within already authorized accounts and budgets.
- Production deployment, paid resource creation beyond an explicitly approved budget, DNS/domain changes, billing changes, irreversible migrations and production-data mutation remain gated by explicit owner authorization and a rollback plan.
- Never reuse credentials or production data from Sherlock Scholes, Novel Forge or another project. Use project-scoped identities and secrets.
- The factory must prefer delivering a verified working preview over leaving the task as an untested PR, while preserving the production safety boundary.


## Current verified state — 2026-11-14

### Product Factory control plane
- Latest workflow code commit: `5bf142c7d5710b9b94a1a9aa6cfc0cd56650dc34`.
- The earlier acceptance run #74 (`43637640350`) used the obsolete OpenRouter-first workflow and its diagnostic artifact proved `OPENROUTER_API_KEY` was empty; it entered the slow Qwen path. It is not evidence of product delivery.
- The updated workflow contains a Starimg Anthropic Messages tool-call probe and routes implementation plus both repair cycles through `claude-opus-5`; OpenRouter is not used by Product Factory.
- A complete Product Factory run, candidate artifact, verification report and target product PR have not yet been confirmed. Do not claim delivery.

### AUREL staging prerequisites
- Neon project `aurel-production` (`quiet-band-32908169`) exists in `aws-eu-central-1`; default branch `main` (`br-dry-lake-b91cdjc5`) and database `aurel` are ready.
- Neon Managed Better Auth is active with mandatory email OTP and the organization plugin. The auth base URL and JWKS endpoint are configured for staging.
- `aurel_app` login was verified against database `aurel`. It is neither SUPERUSER nor BYPASSRLS. RLS is enabled and forced on `organizations`, `properties`, `property_evidence`, `property_audit_events` and `property_enquiries`. The app role has no DELETE grants on these tables.
- A pooled TLS-required Neon connection string for `aurel_app` and a separately generated 48-byte cookie-signing secret were stored as sensitive Vercel environment variables `DATABASE_URL` and `NEON_AUTH_COOKIE_SECRET` for Preview/Development only. No secret values are committed or displayed.
- Vercel project `aurel-private-property` has a successful preview deployment for AUREL branch commit `d977c1438a4418ee16572fc75f11898ed4340e2d`. This proves the preview build completed, not that database-backed user journeys work. SSO protection remains enabled.
- AUREL Rust CI run `43607323635` for that commit completed successfully, including the production container build.
- AUREL target branch: `foundation/luxury-estate-platform`. Product Factory must create its working branch from the latest base SHA and refuse publication if the base moves during the run.

### Remaining AUREL release gates
- Implement Next.js route handlers and connect them to the real Neon database.
- Verify Neon Auth JWTs through JWKS and enforce tenant isolation at the API boundary.
- Add public enquiry rate limiting, abuse controls, email verification and a retention policy.
- Run authenticated staging E2E from sign-up through property submission, independent review, public listing and enquiry persistence.
- Confirm lawful property supply, partner agreements and jurisdiction-specific legal review.
- Rehearse rollback. Production deployment remains blocked until all release gates pass.


### Execution request — 2026-11-14
- Owner requested completion of the first usable product, AUREL Elite Estate, without Fable.
- Trigger the Product Factory on this feature-branch push using Starimg `claude-opus-5` as primary and Qwen2.5-Coder 3B as fallback.
- Deliver a running Vercel Preview backed by the provisioned Neon staging database, with the complete authenticated property submission → independent review → public verified listing → enquiry journey and evidence from browser/API tests.
- Do not merge or deploy production. Update this section only with immutable run IDs, candidate SHA, actual preview URL and verified journey results; do not infer that a push or comment means the factory completed.
