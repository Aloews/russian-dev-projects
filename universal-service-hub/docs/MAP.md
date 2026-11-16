# Universal Service Hub — project map

> **Security-first is the primary product constraint.** No feature or integration may weaken identity, tenant isolation, privacy, secrets handling or user control to improve convenience. The detailed, sequenced release plan is [SECURITY_FIRST_ROADMAP.md](./SECURITY_FIRST_ROADMAP.md); the security programme is tracked in [GitHub issue #40](https://github.com/Aloews/russian-dev-projects/issues/40).

## Mission

Build a secure digital-life control plane and universal service aggregator. It connects useful internet services across personal communication, social identity, banking, crypto, entertainment, telecom, operating systems, infrastructure, AI generation, health, style, sport, commerce and work. It must route workloads safely, protect personal data, measure reliability and spend, and recover from provider failures without requiring the owner to process intermediate agent output.

**Security invariant:** the Hub should not collect or retain data merely because it can. Prefer federated access and provider-side actions; store the minimum metadata needed to operate safely. Sensitive connections are opt-in, read-only by default and individually revocable.

## Product security hierarchy

1. Identity and account recovery.
2. Tenant isolation and authorization.
3. Secret/key protection and cryptography.
4. Data minimization, consent, retention and deletion.
5. Secure integration/agent execution and network egress.
6. Audit, monitoring, incident response and resilience.
7. Broad catalog coverage and convenience.

A lower item must never be delivered by bypassing a higher item.

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

## Engineering planning protocol

Planning is a first-class engineering phase, not a short preamble to coding. The system deliberately spends more model capability, context and iteration budget on discovery, architecture, decomposition and acceptance design because a precise plan reduces rework, integration defects and total delivery time.

### Required phase gates

1. **Discovery and evidence inventory** — inspect the current commit, project map, architecture, runtime, data model, migrations, auth/tenant boundaries, provider registry, UI, tests, CI and deployment constraints. Record exact paths/symbols and distinguish verified facts from assumptions and unknowns.
2. **Product and system design** — convert the requested outcome into user journeys, functional/non-functional requirements, explicit non-goals, data/control flows, trust boundaries, failure states and operational constraints. Prefer existing abstractions; justify any new abstraction.
3. **Task decomposition** — create a dependency DAG of small tasks. Every task must have a stable ID, owner role, objective, exact expected modules/files, prerequisites, inputs/outputs, interface/data contracts, acceptance criteria, tests, risks and required evidence. Tasks must be independently reviewable and sized to fit one bounded implementation pass.
4. **Resource and model allocation** — reserve the strongest available reasoning model for architecture, security, ambiguity resolution and plan critique. Use a coding-specialist model for implementation and a separate strong model for adversarial review. Prefer Astra 6.1 for deep planning when its exact model ID is present in the live Starimg catalogue and it passes the Anthropic Messages tool-call probe; otherwise fall back to the strongest verified Claude model. Prefer Fable 5.2 for Codex implementation when its exact ID is present and it passes the Responses API function-call probe; otherwise use the verified Codex/GPT candidate. `STARIMG_ASTRA_MODEL`, `STARIMG_FABLE_MODEL` and `STARIMG_PLANNING_MODEL` may explicitly select exact catalogue IDs. Never claim either model was used unless the workflow log records its selected ID and successful protocol probe. Allocate more reasoning turns/time to planning than to routine code edits; preserve meaningful budget for implementation, tests, independent review and repair. Never infer capability from a model name alone: protocol probes and actual results are evidence.
5. **Execution and integration** — execute tasks in topological order, parallelizing only tasks without shared-file or data-contract conflicts. Integrate at explicit checkpoints. Re-plan when evidence invalidates an assumption; record the change and impact before continuing.
6. **Verification and closure** — map every acceptance criterion to a test or observable artifact. Track task state as `planned`, `in_progress`, `blocked` or `verified`; only use `verified` with exact commit/test/run evidence. A green unit suite does not substitute for API, database, authorization or user-journey checks.

### Mandatory task contract

Each execution plan must include: objective and user outcome; repository evidence; architecture decision and alternatives; task DAG/critical path; task IDs and role assignments; API/schema contracts; acceptance criteria; test matrix (success, malformed input, provider failure, authorization/tenant isolation, idempotency, concurrency and recovery where applicable); security and spend boundaries; effort/model allocation; stop conditions; and exact evidence required to close each task.

### Planning quality gate

Implementation must not begin until the planner has written the plan to this map and the coordinator has checked that:
- every requested outcome maps to one or more tasks;
- dependencies and integration order are explicit;
- each task has measurable acceptance criteria and tests;
- data ownership, auth and failure semantics are designed before handlers/UI are changed;
- tasks are small enough to implement and review without broad speculative scaffolding;
- model selection and effort allocation match task complexity;
- unknowns, blockers and non-goals are visible.

The planner may not claim implementation or test completion. The implementation agent must follow the plan, and the independent reviewer must challenge both plan quality and implementation coverage. This protocol applies to every future Hub run and to product projects orchestrated by the Hub.

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
- Product Factory now runs without Starimg/Fable. OpenRouter Claude Sonnet 4.5 is the primary implementation and verification-repair model after a live Anthropic tool-call probe; local Qwen2.5-Coder 3B is the fallback.
- GitHub Actions uses isolated agent, verification, repair and publication jobs. Only a candidate that passes the configured verification gates can be published as a PR. No automatic merge or production deployment.
- The Qwen editor now supplies bounded existing-file context on a rejected new-file edit, rejects symlink path escapes, and has regression coverage for both cases.
- The workflow prevents new runs from cancelling an active product build and allows 25 minutes for each local Qwen pass.
- Latest control-plane head: `d3f24c9e6931211044462feb175ca9b21cf61bf1`. Control-plane CI run #832 is in progress; Qwen safety tests, workflow/actionlint validation, composite action metadata and Rust format have passed so far.
- Product Factory output is not yet evidenced: no candidate artifact, isolated verification report or AUREL delivery PR has been confirmed. Do not mark the factory operational until these artifacts exist.
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
- Primary implementation and repair route: Anthropic-compatible Claude Sonnet 4.5 through OpenRouter. The workflow probes tool-call compatibility before primary invocation.
- Fallback route: local Qwen2.5-Coder 3B through Ollama on an isolated GitHub-hosted runner. Qwen can create the initial candidate and repair from verification evidence if OpenRouter is unavailable or fails.
- Starimg and Fable are disabled. `STARIMG_API_KEY` is not required or called by this workflow.
- Specialist profiles cover architecture, security, product and independent review. The model gets bounded file tools only; no shell, network, credential, Git push, GitHub API, deployment or production-data tools.
- Verification, repair and publication run in separate jobs. Verification receives neither model-provider keys nor a target-repository write token.
- Up to two repair cycles use the actual failed verification report. Publication rechecks target and base branch SHAs before creating/updating a PR. No target PR is auto-merged or deployed.
- Required Actions secret: target-scoped `PRODUCT_FACTORY_GITHUB_TOKEN` with Contents read/write and Pull requests read/write. `OPENROUTER_API_KEY` is optional: if absent or incompatible, the workflow attempts Qwen.
- The first acceptance target is `Aloews/full`, working branch `factory/elite-estate-rc3`, base `foundation/luxury-estate-platform`.
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
   - Use specialist agents for bounded work; keep one authoritative writer per branch. Escalate difficult or repeated failures to the OpenRouter primary model and independent reviewers.
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
- Do not rerun unchanged failing jobs indefinitely. After two unsuccessful attempts with the same failure signature, change the approach/provider/model or isolate a minimal reproduction and route the exact failure evidence to OpenRouter Sonnet or the Qwen fallback.
- If OpenRouter fails, capture sanitized provider request/response metadata and tool-call protocol errors, then use the bounded Qwen fallback. Never claim model compatibility from secret presence or HTTP reachability alone.
- Use Qwen 2.5 Coder 3B for bounded specialist analysis and structured edits; validate every edit and test the resulting code. OpenRouter Sonnet owns primary implementation and evidence-based repair. Independent architecture, security and product reviewers challenge behavior; their roles are not proof of successful execution.
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
- CI run #832 for the workflow code commit is in progress. Verified successful steps on this exact head: Qwen Python syntax, Qwen safety regression tests, Product Factory workflow/actionlint validation, composite action metadata and Rust formatting. Rust lint and the remaining suite have not yet completed.
- The workflow source contains no Starimg API probe, Fable/Opus model ID, or `STARIMG_API_KEY` dependency. OpenRouter Sonnet 4.5 is the primary; Qwen is the fallback.
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
- Trigger the Product Factory on this feature-branch push using the configured OpenRouter Sonnet 4.5 primary and Qwen fallback.
- Deliver a running Vercel Preview backed by the provisioned Neon staging database, with the complete authenticated property submission → independent review → public verified listing → enquiry journey and evidence from browser/API tests.
- Do not merge or deploy production. Update this section only with immutable run IDs, candidate SHA, actual preview URL and verified journey results; do not infer that a push or comment means the factory completed.


## Universal Hub coding-agent workflow

- Added dedicated workflow: .github/workflows/universal-hub-codex-claude.yml.
- Codex CLI is the primary writer through Starimg OpenAI Responses API; Claude Code CLI is the sequential independent reviewer and repair writer through Starimg Anthropic Messages API.
- The workflow discovers model IDs from the live Starimg catalogue and requires successful function/tool-call probes for both protocols before either coding agent runs.
- The two agents work sequentially on an isolated GitHub-hosted runner and are restricted to the universal-service-hub/ subtree. The GitHub write token is only exposed in the later publication step.
- The workflow opens a candidate PR; it does not merge, deploy, provision paid infrastructure, mutate production data or rotate credentials.
- The workflow is configured in this branch but has not yet been run against the live Starimg account. Model compatibility, billing availability and candidate quality remain unverified until the first run completes.
- CI now actionlints the new workflow file when it changes.


## Resumable execution and balanced media workloads

The phased implementation roadmap, task decomposition contract, interruption recovery protocol, fair model scheduling and image-generation workstream are specified in [docs/EXECUTION_PLAN.md](./EXECUTION_PLAN.md). This is planned work, not implemented functionality. Prioritize the first A1–A2 persistence slice, then outbox/leases/checkpoints before enabling long-running or billable media jobs. Do not mark service-stop recovery or image-model balancing operational until their failure-injection and live provider tests pass.


## Security-first implementation map — authoritative sequence

The step-by-step implementation and release plan lives in [docs/SECURITY_FIRST_ROADMAP.md](./SECURITY_FIRST_ROADMAP.md). Do not reorder milestones merely to expand the catalog faster.

| Milestone | Scope | Release gate |
|---|---|---|
| S0 | Asset/data inventory, data classification, threat model, trust boundaries, security invariants | Threat model reviewed; risks mapped to tests |
| S1 | User identity, passkeys/MFA, secure sessions, RBAC/scopes, rate limits, break-glass admin | Browser never uses root token; session/revocation tests pass |
| S2 | Tenant-aware persistence, PostgreSQL RLS, artifact/object isolation, privacy domains | Cross-tenant negative tests pass for every data path |
| S3 | OAuth vault, envelope encryption/KMS, key rotation, TLS and browser security headers | Tokens encrypted, scoped, revocable; key lifecycle tested |
| S4 | Official integrations, egress policy, agent/tool sandbox, consent, idempotency | SSRF/prompt-injection/tool-abuse and sensitive-action tests pass |
| S5 | Audit, monitoring, SBOM, scans, backups, restore, incident response | Security operations and recovery drills pass |
| S6 | Universal catalog and personal dashboard across digital-life categories | Every service has truthful capability, scope and privacy metadata |
| S7 | Independent security verification and controlled release | Exact-SHA CI, independent review, staging E2E, rollback and authorization |

### Current security baseline — verified from main before this roadmap

- Rust Axum/Tokio/SQLx runtime with root bearer token and project-scoped hashed tokens.
- Project roles are owner/operator/viewer; project capability/provider allowlists are enforced on current execution paths.
- Project tokens are revocable, but the current model does not yet provide per-user customer identity, MFA/passkeys, fine-grained per-operation grants or tenant-scoped audit views.
- Infisical machine identity is supported for provider credentials; live production identity verification remains outstanding.
- Request body size is bounded; outbound URL/IP checks exist, but production egress firewall and DNS-rebinding controls remain required.
- Existing audit records omit AI prompts/responses, but tamper-evident storage, tenant views, retention and deletion controls remain outstanding.
- No production deployment is verified. This is not yet a security-certified or production-ready customer platform.

### Mandatory execution rules

- Update this map and the security roadmap in the same PR as material security changes.
- Every milestone records status as NOT STARTED, IN PROGRESS, BLOCKED or VERIFIED; VERIFIED requires exact commit SHA, CI run URL, named test commands and results.
- One writer per branch; independent security reviewer and QA inspect the exact immutable PR head.
- No production deployment, paid resource creation, production-data migration or credential rotation without explicit authorization and rollback.
- A passing unit test does not prove a live provider, identity system or deployment is secure. State the evidence boundary.
- Stop catalog expansion if a new integration lacks a data classification, scope map, retention policy, revoke path and threat review.


## Cross-device product and public accountability objectives

Top-level initiative: [issue #45](https://github.com/Aloews/russian-dev-projects/issues/45).
- Cross-device clients: [issue #44](https://github.com/Aloews/russian-dev-projects/issues/44).
- Public evidence-backed development portal: [issue #43](https://github.com/Aloews/russian-dev-projects/issues/43).
- Security programme: [issue #40](https://github.com/Aloews/russian-dev-projects/issues/40).

These are open implementation objectives, not completed features. No public development portal or native mobile/desktop clients are currently evidenced by this roadmap PR.

## Evidence review — 2026-11-15

- Security roadmap and security baseline are documentation requirements; they do not implement S0–S7 controls.
- Source inspection was limited to the current main-branch Rust entry point and repository files. Source presence is not runtime or penetration-test evidence.
- CI evidence must be checked against the live PR head at review time. Earlier runs targeted superseded commits and were pending with no completed job evidence; none of them is a passing verification. Do not copy historical run IDs or SHAs forward as current proof.
- Overall security programme status remains **NOT STARTED as a complete programme**. No claim of security certification, production readiness, public portal availability or cross-platform app availability is made.


## Security implementation — first runtime increment

### S1 API credential abuse control
- **Status:** IN PROGRESS.
- **Implementation:** PostgreSQL-backed fixed-window rate limiting for authenticated root and project bearer tokens; default 120 requests/minute, configurable from 1–10000.
- **Response contract:** HTTP 429, stable `rate_limit_exceeded` error code and `Retry-After: 60`.
- **Storage:** SHA-256 token fingerprints only; no raw bearer tokens are persisted. Rows idle for 24 hours are pruned at startup and hourly.
- **Proof required:** exact-head Rust CI, migration execution, container smoke test with 100 allowed requests followed by a 101st rejected request, and header/body assertions.
- **Known limits:** unauthenticated IP throttling, identity/login/recovery controls and operational alerting are not included in this increment.
- **No production deployment or migration has been performed.**

## Adaptive agent and model escalation — outcome first

The objective is delivery of the intended, usable product. An agent, model, provider, workflow or successful CI run is an execution mechanism—not the product outcome and not a reason to wait indefinitely. The coordinator owns progress and must change the execution strategy when evidence shows that the current executor is too slow, stuck or under-capable.

### Progress measurement and escalation triggers

- Before each substantial task, record its acceptance criteria, expected duration, hard timeout, current executor/model/provider and the next verifiable checkpoint.
- For tasks expected to take more than 20 minutes, inspect progress at least every 10 minutes. Progress means a reviewable artifact or measurable engineering result: a scoped diff, a passing targeted test, a reproduced/fixed defect, a validated API contract or a completed build stage. Token activity, status messages and generated plans alone do not count.
- Escalate when either (a) two consecutive checkpoints show no meaningful, verifiable progress, or (b) elapsed time exceeds 1.5 times the task estimate without meeting its checkpoint. If no estimate was recorded, treat 20 minutes without a verifiable artifact as a stall.
- Escalate immediately after the same material failure signature occurs twice, or when the current model cannot reliably use the required tools/context, repeatedly misunderstands acceptance criteria, or produces changes that fail the same targeted checks.
- A long-running task is not automatically a stalled task: retain the current executor when it is producing measurable progress and remains within its agreed time budget. Do not interrupt an active run merely to rotate models.

### Executor/model selection

- Select the executor by task difficulty and demonstrated capability, not by habit, cheapest-token preference or agent brand. For complex architecture, security-sensitive changes, multi-file implementation, difficult debugging and repeated failed attempts, move to a more capable coding model/agent.
- Before switching, inspect the live provider/model catalogue and verify the candidate model's actual availability, tool-call protocol, context capacity and task compatibility. A model name, marketing tier, configured secret or successful HTTP response alone is not proof of capability.
- Prefer the strongest verified compatible model available within the already authorized spend ceiling. Compare expected completion quality and latency against cost; do not silently exceed a budget or introduce a new paid provider. If no authorized budget covers the required escalation, stop only at that spending boundary and request the specific approval.
- When possible, first escalate within the currently authorized provider to avoid unnecessary integration changes. Switch provider only when its API contract, credentials, privacy boundary, data handling and cost are verified and authorized. Never silently change providers for a billable generation or external action.
- Use a stronger model for implementation or repair when the task requires it; retain a separate independent reviewer/QA role. A reviewer must inspect the actual diff and evidence, not simply endorse the previous agent.

### Safe handoff and continuity

- Before replacing an executor, checkpoint its current workspace/diff, exact base and candidate SHAs, task brief, acceptance criteria, failing command and sanitized logs, completed tests, unresolved decisions and remaining work.
- Stop or suspend the previous writer before enabling the replacement. Never allow two agents to write to the same branch/worktree concurrently.
- Give the replacement the original product goal and acceptance criteria plus the checkpoint—not merely the previous agent's summary. Require it to inspect the actual repository state before editing.
- Preserve useful work. Do not discard a partial implementation just because the executor is being changed; retain, test and repair it unless evidence shows it is unsafe or incompatible.
- After handoff, rerun the failed targeted check first, then all impacted regression, security, integration and end-to-end checks on the resulting exact candidate SHA.

### Product-outcome priority and stop conditions

- Optimize for the user's intended product outcome: a coherent, usable, secure product with its critical journey working and persisted results—not for agent completion, number of agents, number of PRs, code volume, generated documentation or green lint alone.
- Prefer a narrow, fully working vertical slice over broad incomplete scaffolding. Re-plan scope when necessary, but preserve the user's core intended outcome and disclose any deferred capability in the project map.
- Do not mark a task complete because an agent timed out, changed models, opened a PR or reported success. Completion requires the product-level definition of done above and direct evidence from the running candidate.
- If no compatible executor can make measurable progress within the authorized budget, record the concrete technical or authorization blocker and the evidence needed to resume. Do not loop indefinitely or manufacture progress.



## Cross-project agent governance and product lifecycle — 2026-11-16

**Status: VERIFIED IN MAIN.** Shared standards were merged in PR #57 (merge commit `4232690152ce8e6551ecf10599532918ec51678b`); exact-PR CI and post-merge main CI both passed.

- Repository-wide agent contract: `/AGENTS.md`.
- GitHub automation and credential boundaries: `.github/AGENTS.md`.
- Product Factory orchestration contract: `.github/product-factory/AGENTS.md`.
- Reusable target-product instruction template: `.github/product-factory/templates/AGENTS.md`.
- Product development lifecycle and stage gates: `.github/product-factory/PRODUCT_DEVELOPMENT_LIFECYCLE.md`.
- CI structural validation: `.github/scripts/validate_agent_docs.py`; the CI workflow checks required sections and verifies that the shared contract/template are injected into hosted and local agent runtimes.
- Universal Service Hub local agent contract: `universal-service-hub/AGENTS.md`.
- Product Factory runtime description: this map's [Product Factory architecture](./PRODUCT_FACTORY.md).

The intended hierarchy is repository-wide policy → automation-specific policy → Product Factory policy → target product's generated local `AGENTS.md` → more specific nested instructions. Local instructions may add constraints but may not weaken parent security, privacy, evidence or release rules.

### API and knowledge management

- OpenAPI is the machine-readable source contract for implemented HTTP APIs; Swagger UI and generated clients must derive from the same specification.
- Do not publish specifications for routes or behavior that do not exist.
- GitHub project documentation remains canonical for this repository. Confluence is not currently configured as a synchronized source of truth.
- Product repositories should keep project maps, ADRs, API specifications, runbooks and local agent instructions version-controlled and linked from their map.

### Provider routing correction

- PR #56 was merged as `b0157e6bbdec6f64a2c2c24e11e70969e1aeb1aa`.
- CI runs `43838620761` and `43838636167` both passed on exact PR head `25255dc3cc8ff875eebbc14efa89501852bac2d4`.
- The change separates Codex/OpenAI Responses from Claude Code/Anthropic Messages, fixes bounded retry handling and retains bounded, redacted upstream error diagnostics.
- **Live Starimg compatibility has not yet been re-proven after the fix.** The successful CI runs are not evidence that both live provider tool-call probes now pass.


## Runtime bootstrap — local launch path

- **Status:** implementation candidate on `feat/ush-local-runtime-bootstrap`; not yet merged or claimed operational.
- Production binary: Rust Axum/SQLx in `universal-service-hub/Dockerfile`; PostgreSQL 17 is mandatory; SQLx migrations run at process startup. Python/FastAPI is reference/compatibility only.
- Added a Docker Compose stack, a one-command local startup script that creates strong local-only credentials, waits for database readiness and tests authenticated API access, and a safe stop script that retains the local database volume.
- Added an exact-CI-head Compose startup smoke test in addition to the existing extensive Docker RBAC/rate-limit smoke test.
- **API-only:** the current product has no browser management UI; only health/readiness endpoints are directly viewable in a browser, and administrative workflows use the authenticated HTTP API.
- **Not a public deployment:** no persistent host is currently provisioned. The existing Railway projects contain other product services and were not modified. The Hub's Neon Free project exists but is not connected to a deployed runtime. No paid resource was created.
- Acceptance: exact-head GitHub CI passes; Compose starts Rust against PostgreSQL 17; migrations complete; /healthz and /readyz return success; unauthenticated API returns 401; authenticated provider-list endpoint returns success. Public deployment is a separate gate requiring an isolated host and configured production secrets.


## Adaptive specialist team and recoverable execution

### Execution model

- **Status:** CONFIGURED IN WORKFLOW; live subagent execution is NOT YET VERIFIED.
- Workflow: `.github/workflows/universal-hub-codex-claude.yml`.
- Codex subagents are enabled with a maximum of four concurrent child agents and nesting depth one. The primary selected Codex model is the default child model; this avoids silently introducing another provider or unverified model.
- Four bounded roles are provisioned: `codebase_explorer`, `security_reviewer`, `provider_analyst`, and `test_analyst`. Start with two or three; use the fourth only when it has a distinct task. The main Codex session remains the sole writer and integrator.
- Claude Code remains the sequential independent reviewer/repair writer after Codex. The main writer must reconcile specialist findings before implementation and ask a read-only specialist to inspect a substantial final diff.
- Subagents increase token consumption. Do not maximize their count by default; add concurrency only when independent work materially reduces elapsed time or risk. No price/cost advantage is assumed for an unverified model.

### Recovery and evidence

- After the Codex phase, the workflow records a binary Git patch and working-tree status, including on failure/timeout, and uploads them as an Actions artifact with a seven-day retention period.
- The patch is withheld when the basic secret-pattern scan detects a likely credential. This scan is a guardrail, not a comprehensive secret scanner; the normal candidate secret checks remain mandatory.
- The checkpoint is not proof of correctness and must not be merged or published without the existing Claude review, candidate validation, exact-SHA CI and independent QA.
- The Starimg model/tool-call compatibility probe is still not proof that the selected model can delegate subagents. The first completed run must confirm actual spawned-agent activity and collected results. If delegation is unsupported, use the single-writer fallback and record it.
- The existing long-running run #7 was started before this workflow change and will not inherit the new orchestration or checkpoint steps. Its workspace cannot be recovered through the GitHub API while its runner remains active. Do not claim this change retroactively fixes that run.

### First-run acceptance

1. Codex logs show at least two distinct subagents spawned and their outputs collected, or explicitly record the safe single-writer fallback.
2. No subagent edits shared files; only the primary Codex process writes during the implementation phase.
3. The checkpoint artifact exists after Codex and contains a patch/status matching the runner's final pre-review working tree, with no detected secret patterns.
4. Claude reviews the actual combined diff; candidate scope checks pass; CI runs against the exact candidate SHA.
5. Any timed-out/failed run retains its checkpoint artifact so a replacement agent can resume from evidence instead of restarting blindly.

- **Isolation limitation:** current custom-role instructions ask subagents not to edit, but they inherit the primary Codex session's sandbox. This is not a hard read-only filesystem boundary. The main agent must inspect the full diff and reject unauthorized or out-of-scope edits. Do not claim technical write isolation until it is separately enforced and tested.

## Continuous technology radar and delivery-acceleration system

### Objective

Continuously discover and evaluate technologies that can materially improve product quality, security, delivery speed, reliability or total cost across the Universal Service Hub and its Product Factory. This is an evidence-driven engineering capability, not a news feed and not permission to adopt every new tool.

### Monitoring cadence and sources

- **Status:** SPECIFIED; automated scheduled monitoring is NOT YET IMPLEMENTED or VERIFIED.
- Run a lightweight discovery scan weekly. Run a focused review immediately when a critical security advisory affects a used dependency/provider, a material API/model deprecation is announced, or a current delivery workflow repeatedly misses its acceptance/time budget.
- Prefer primary sources: official release notes and changelogs, vendor documentation and status pages, GitHub releases/security advisories, standards bodies, benchmark datasets and reproducible research. Community discussion may reveal candidates but is not sufficient evidence for adoption.
- Track relevant areas: coding and reasoning models; agent orchestration and tool protocols; inference gateways and local models; testing/browser automation; build/CI and caching; databases, queues and storage; deployment/runtime platforms; observability/security tooling; image, video, music and voice generation; cross-platform clients.
- Record source URL, publication/release date, checked date, version/model identifier and the exact capability being evaluated. Detect stale, duplicate and superseded entries.

### Evaluation and prioritization

For each candidate, compare it with the current production or workflow baseline using a small, relevant benchmark:
- measurable task completion time and end-to-end lead time;
- first-pass acceptance rate, defect/regression rate and rework;
- quality against explicit product acceptance criteria;
- total cost per accepted outcome, including tokens, retries, CI/runtime and human review;
- reliability, rate limits, context/tool compatibility and failure recovery;
- security posture, data residency/retention, permissions, supply-chain risk and vendor lock-in;
- integration effort, operational burden and rollback path.

Classify each candidate as **ADOPT**, **TRIAL**, **WATCH**, or **REJECT**. Every recommendation must state the baseline, evidence, uncertainty, estimated cost, risks, compatibility, expected measurable gain and a falsifiable acceptance threshold. Do not compare models using marketing claims or raw tokens/second alone; measure accepted product outcomes on representative tasks.

### Safe experiment and adoption workflow

1. Discover a candidate and capture its authoritative sources and version.
2. Check licensing, terms, privacy/data handling, security advisories, API/tool protocol, account availability, regional limits and actual pricing.
3. Define a bounded benchmark and acceptance threshold before the trial.
4. Run in an isolated branch, sandbox or staging environment with synthetic/non-sensitive data and a strict spend/time ceiling.
5. Compare against the current baseline; retain raw test outputs, exact commit/configuration, run IDs and reproducible commands.
6. Have an independent reviewer inspect the results and security implications.
7. If the threshold is met, create a scoped implementation PR and update the relevant architecture/integration map. Keep the previous path available for rollback.
8. If evidence is weak or the threshold is missed, reject or keep watching; record why and when to reassess.

- No automatic production changes, credential sharing, new paid subscriptions, provider switching, dependency upgrades or data transfers based only on a discovery result.
- Never send private source code, customer data, secrets or production prompts to a candidate provider until its data handling and authorization are verified.
- A scheduled scan, generated report, model catalogue entry or passing microbenchmark is not proof of faster product delivery. Only end-to-end results on representative accepted tasks count.
- The radar must prioritize technologies that remove a measured bottleneck in the current roadmap. Avoid adding tools, agents or services that increase coordination/cost without improving the delivered product.

### Required record and success criteria

Maintain a dated technology register with: candidate, category, source/version, discovery date, evaluation owner/agent, current baseline, benchmark, result, cost, risk, decision, follow-up date and evidence links. Add accepted changes and deferred opportunities to this project map; do not silently overwrite previous decisions.

The system is operational only when:
- scheduled scans execute reliably and their run history is inspectable;
- source freshness and duplicate detection are tested;
- candidate scoring is reproducible from recorded evidence;
- at least one isolated benchmark compares a candidate against the existing baseline;
- recommendations link to reproducible test evidence and a reviewable PR/issue;
- false positives, stale recommendations and failed scans are surfaced;
- no scan can directly mutate production configuration or spend.

Initial implementation sequence:
1. Add a scheduled GitHub Actions discovery workflow and a versioned technology register.
2. Add source adapters for official GitHub releases/security advisories and selected vendor/model changelogs; keep source allowlists explicit.
3. Add relevance filters and candidate scoring tied to the current Hub/Product Factory bottlenecks.
4. Add isolated benchmark templates for coding-agent throughput, accepted-task quality, CI duration and cost per accepted change.
5. Publish findings as an issue or artifact; require human/reviewer approval before any adoption PR can change runtime dependencies or provider configuration.

## Technology radar — first automated implementation increment

- **Status:** IMPLEMENTED IN BRANCH; CI and first scheduled/live scan are NOT YET VERIFIED.
- Scheduled workflow: `.github/workflows/technology-radar.yml`, weekly on Monday at 09:23 UTC and manually dispatchable.
- Scanner: `.github/scripts/technology_radar.py`. It reads public GitHub release metadata from an explicit allowlist of ten repositories covering OpenAI SDK/agents, MCP, Transformers, Ollama, vLLM, LangGraph, Vercel AI SDK, Supabase and Neon.
- The scanner stores release tags, publication dates, prerelease flags and official release URLs; compares against the prior snapshot; separates newly observed tags from the initial baseline; reports partial source failures; and fails if every configured source is unavailable.
- Output: Markdown summary plus JSON evidence artifact, retained for 90 days. A GitHub Actions cache provides the prior comparison snapshot; cache eviction can reset the baseline, in which case the next scan explicitly marks itself as an initial baseline rather than falsely reporting new releases.
- Regression tests cover initial-baseline semantics, new-tag detection, source failures and the WATCH-only decision boundary. The main Hub CI now runs these tests and actionlint on the scheduled workflow.
- The scanner is discovery-only. It does not install packages, invoke discovered code, alter providers/models, open adoption PRs or touch production settings. All findings remain WATCH-only until a separately reviewed benchmark proves value.
- **Current scope limitation:** this first increment monitors public GitHub release feeds only. It does not yet ingest vendor RSS/changelogs, CVE/security advisories, research papers, pricing changes or API deprecations; it does not yet benchmark latency/quality/cost or rank candidates for adoption.
- **Acceptance still required:** CI must pass on the exact latest PR head; the workflow must execute once successfully against GitHub's API; the artifact must contain a valid snapshot; a subsequent run must prove comparison against the prior baseline. Until then, the scheduled radar is configured but not operationally verified.



## Local model control and development failover — first runtime increment

- **Status:** IN PROGRESS; source changes are in branch `feat/ush-local-agent-failover`. Exact-head CI and live Ollama execution are pending.
- The Rust provider registry now recognizes an `ollama` AI adapter for chat inference and model discovery through Ollama's documented local `/api/chat` and `/api/tags` endpoints.
- Local endpoints are intentionally separate from public cloud endpoints. They must use loopback or a hostname explicitly listed in `USH_OLLAMA_ALLOWED_HOSTS`; each hostname is resolved once per request, every DNS answer is validated, and the HTTP client pins the selected validated socket address with reqwest `ClientBuilder::resolve`. The pinned client disables environment proxies and redirects while retaining the original hostname for HTTP Host and TLS SNI. No API key or Infisical secret is required for the local adapter.
- The health-check path probes Ollama's local model-tags endpoint. The provider model catalogue normalizes Ollama model names and sizes.
- Project capability and provider allowlists continue to apply. Local inference is not automatically enabled for every project; an administrator must register and enable the local provider and explicitly include its ID in project allowlists where those are configured.
- Regression tests cover adapter recognition, empty/mixed/public/metadata DNS answers, loopback/RFC1918/ULA policy, IPv4-mapped IPv6, pinned HTTP connection and Host preservation, and refusal to follow redirects. Exact-head GitHub Actions validation is pending; these deterministic tests do not prove that an Ollama server or model is installed or reachable.
- **Coding workflow fallback:** `.github/workflows/universal-hub-codex-claude.yml` now invokes local Ollama/Qwen when Starimg preflight, CLI installation, Codex or Claude fails. The fallback reuses the same checked-out workspace and any retained Codex changes/checkpoint, so it does not discard partial work before attempting recovery.
- The local Qwen script is project-scoped through explicit `ALLOWED_PREFIXES` and context-file allowlists. It cannot edit workflow files, credentials, dependency manifests, deployment configuration or database migrations. Its role-separated engineering prompt is one model inference session, not four independent local agents.
- The workflow can use a self-hosted runner selected by repository variable `USH_DEV_RUNNER_LABELS` (JSON array); without it, the workflow defaults to `ubuntu-latest`. It reuses a running Ollama server and configured model when present; otherwise it installs Ollama and pulls the selected model (default `qwen2.5-coder:3b`). A self-hosted runner with preloaded weights is the path to persistent owner-operated local inference.
- **Not yet implemented:** automatic cross-runner resume after the runner itself is lost; a separate independent local-model reviewer; multiple local models selected by task class; resource-aware GPU/model sizing; full management UI for local-model pool and task routing.
- **Verification boundary:** Rust format, lint, tests and Qwen safety tests passed on intermediate commit `720849e`; Docker build and container smoke test were still running at the time of this map update. The final branch head and the newly added local fallback workflow have not yet passed exact-head CI. No real cloud-outage-to-Ollama workflow run has been observed. Do not call local failover operational until that simulation produces a candidate and the exact candidate SHA passes CI.
- **Acceptance required:** exact-head CI including actionlint on the updated workflow; live Ollama health/model-discovery/chat smoke tests; a controlled cloud-agent failure that demonstrably invokes Qwen; successful bounded patch generation; exact-head CI on the generated candidate; proof that a defective candidate is blocked.
- No production deployment, provider credential change, paid resource creation or production data mutation is included.
