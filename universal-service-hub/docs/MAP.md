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

## Durable engineering task dispatcher and multi-tenant foundation

Tracked by [Issue #63](https://github.com/Aloews/russian-dev-projects/issues/63). PR #64 contains the first durable task-control slice and the first tenant-owned schema foundation. This is not yet a complete autonomous executor, full tenant authorization layer or self-hosting system.

### Implemented in the candidate branch

- Tenant and workspace tables, tenant membership identities and revocable tenant service-token records.
- Existing projects are backfilled into tenant `aloews` and workspace `default`.
- Projects and project tokens carry tenant ownership; project creation compatibility trigger assigns the initial Aloews tenant when the bootstrap API omits explicit ownership.
- Tenant-scoped provider connection references, resource-limit policies, budget policies and idempotent usage ledger schema. Credential values are not stored in these tables.
- Task, event and checkpoint records carry tenant ownership. Database triggers derive ownership from the owning project/task rather than trusting request payloads.
- Composite foreign keys prevent a task, event or checkpoint from pairing a tenant with another tenant's project/task.
- Project-scoped PostgreSQL task intake/listing, append-only user context events, version-checked state transitions, worker claims, hashed leases, heartbeat, checkpoint metadata and lease-expiry recovery.
- Role-bound workers for architect, implementer, builder, tester, reviewer and recovery queues; worker tokens are bound to a tenant/project and a single queue.
- Multiple workspace records with independent leases, heartbeats, immutable base SHAs, worktree metadata and expired-lease quarantine.
- Task-to-workspace assignments require both task and workspace leases. Overlapping writer path scopes are protected by transactional PostgreSQL locks; an independent task cannot acquire a conflicting prefix while the first lease is active.
- Model attempt records retain provider/model, strategy, outcome, error, cost fields and acceptance progress. These are the control-plane records; model selection and retry orchestration remain in the executor adapter.
- Transaction-local tenant context is applied to task/workspace/attempt and project/token/provider-selection APIs. Migration 0006 enables and forces RLS on execution-plane tables; migration 0007 provisions the non-login `ush_runtime` role; migration 0008 extends RLS to tenants, workspaces, memberships, provider connections, resource/budget policies, usage events, API rate limits, projects and audit events; migration 0009 enables RLS on tenant and project token tables.
- The Rust API runs application queries via `SET ROLE ush_runtime` after privileged migrations and verifies that the active role is neither `SUPERUSER` nor `BYPASSRLS`. Authentication uses short-lived platform-scoped transactions to resolve token hashes before establishing a tenant principal; management/read routes then use tenant-scoped transactions.
- Exact-head CI includes restricted-role RLS tests for tenant A/B visibility, forged cross-tenant writes, core/token/audit/rate-limit isolation and transaction-context reset on one reused connection.
- Migration version regression test and PostgreSQL tenant schema smoke test remain part of CI.

### API surface currently implemented

- POST/GET /v1/projects/{project_id}/tasks
- POST /v1/projects/{project_id}/tasks/{task_id}/events
- POST /v1/projects/{project_id}/tasks/{task_id}/transition
- POST /v1/tasks/claim
- POST /v1/tasks/{task_id}/heartbeat
- POST /v1/tasks/{task_id}/checkpoints
- POST /v1/tasks/{task_id}/attempts and GET /v1/projects/{project_id}/tasks/{task_id}/attempts
- POST/GET /v1/projects/{project_id}/workspaces; POST lease-next, heartbeat and release
- POST/DELETE /v1/tasks/{task_id}/workspace-assignment (requires valid task and workspace leases; automatically acquires/releases path locks)
- POST/DELETE /v1/projects/{project_id}/tasks/{task_id}/path-locks (requires an active task/workspace assignment and a path inside the assigned writer scope)
- Tenant service-token authentication and token lifecycle endpoints are present; human OIDC membership authentication is not.

### Explicit gaps — do not call this full multi-tenancy yet

- RLS policies now cover execution-plane and core tenant-owned tables, including projects, tenants, token records, usage, audit, memberships, provider connections, quotas/budgets and API rate limits. Token authentication is deliberately platform-scoped only for exact hash lookup; every token-management route must retain tenant/project ownership checks. A final route-by-route authorization review and adversarial API tests remain required.
- The runtime pool now verifies it is operating as `ush_runtime`, a non-login role with neither `SUPERUSER` nor `BYPASSRLS`. The migration connection remains privileged by design; the application connection is role-switched after migrations.
- Tenant membership rows are not yet bound to a verified human OIDC identity. Tenant service tokens work, but user session identity, invitation/SCIM lifecycle and support elevation are outstanding.
- Tenant resource limits, budget policies, usage ledger and provider-connection records are schema only; runtime enforcement and billing attribution remain outstanding.
- Workspace records and path locks are implemented, but actual isolated Git worktree provisioning, persistent workspace storage, shell/network sandboxing, artifacts, caches and per-tenant logs are not yet connected to the Railway execution plane.
- The existing GitHub/Starimg worker has a USH-only repository/path scope gate and validated model fallback/strategy rotation. It is **not yet connected to the USH PostgreSQL task API**: intake is still owner-authored GitHub `/ai` comments, execution remains bounded to one issue per invocation, and the Railway workers remain disabled. Do not describe this as an end-to-end USH adapter.
- Independent queue roles and persisted attempts exist in the API. The existing Railway worker is not yet a durable API-backed multi-role scheduler; unrelated tasks are not yet proven to continue concurrently while another waits on CI/review.
- Fair tenant scheduling, tenant export/deletion and cross-tenant penetration testing remain outstanding.
- No public signup/onboarding, production deployment or self-hosting acceptance is enabled.

### Next implementation order

1. Pass exact-head Rust/Clippy/migration/Compose/API/RLS tests; repair any failures without weakening gates.
2. Convert the USH-only worker scope into a real project adapter: poll/claim PostgreSQL tasks with worker leases, lease/assign workspaces, use persistent Git worktrees, heartbeat and checkpoint accepted units, and publish evidence through the API.
3. Split architect/implementer/builder/tester/reviewer/recovery into independent queue consumers; keep independent DAG branches moving while CI/review is pending.
4. Route retries through validated Starimg model candidates and strategy changes, recording every attempt and cost; prove a real provider call in a bounded sandbox.
5. Complete route-by-route tenant isolation for project/token/provider/usage/audit/artifact surfaces, then OIDC memberships, quotas/budgets, export/deletion and penetration tests.
6. Activate only the existing Railway runtime with USH-only repo/path scope after sandbox end-to-end acceptance; never enable the other six repositories as a side effect.
7. Continue Hub self-hosting only after API recovery, multi-workspace execution and publication acceptance passes.

### Verification evidence

- Schema foundation passed exact-head CI run [#43870849099](https://github.com/Aloews/russian-dev-projects/actions/runs/43870849099) on `1dbb093c56975f07eee47d3bbd3f26bc802610d6`; this evidence predates the execution-role/RLS/workspace-assignment increment.
- Current candidate includes RLS/runtime-role migration, transaction context, assignment/path-lock endpoints and expanded CI smoke tests. Exact-head run is required; do not infer success from earlier CI.
- Current runtime role test is designed to prove non-superuser/non-bypass access, tenant A/B row isolation, denied forged writes and no context leakage across reused transactions. Until the exact run passes, RLS is **implemented but not verified**.
- PR #64 remains open and unmerged. Railway execution, production deployment and external tenant onboarding remain disabled.


## CI repair checkpoint — 2026-11-18

- PR #64 exact candidate `429791aa89ca68a661df3fc1b76dee4ee6b25ad1` failed exact-head CI run [#44132283616](https://github.com/Aloews/russian-dev-projects/actions/runs/44132283616) at the container smoke assertion for a tester attempting to heartbeat an implementer-owned workspace lease.
- The runtime handler intentionally returns HTTP 403 (`workspace_lease_owner_forbidden`) for an authenticated worker that owns a different worker's live lease; HTTP 409 is reserved for an expired/invalid lease. The smoke test expected 409, contradicting the handler's authorization contract.
- Corrected the smoke assertion to expect 403 in commit `93c8f15d66e5811c1dcc3b2f5fe5edd5b269e763`.
- **Status: awaiting exact-head CI.** No verification is inferred from the code change; PR #64 remains draft/unmerged until the complete workflow passes.

### Follow-up CI diagnosis — 2026-11-18

- The next exact-head Container smoke failure was not in the workspace lease authorization check. The log identified the failing assertion as the tester claim not returning the second task.
- Root cause: the smoke fixture attempted to move a task directly from `planned` to `queued` while setting `queue="test"`. The state machine permits `planned -> queued`, but the test consumer only claims `queued_test`; queue and status were inconsistent, and the fixture skipped the implementation-to-test handoff.
- Corrected the fixture to use a valid lifecycle: `received -> classified -> planned -> queued`, claim from the implementation queue, transition to `queued_test`, then claim from the test queue. The state machine and queue semantics were not weakened.
- New workflow fixture commit: `54f1b9a895e9c8c2246f7ff92bb0187baba9c018`. Exact-head CI is still required.


### Exact-head verification — 2026-11-18

- PR #64 head `38562a96c4c48006c97fe5a74cd3e1daac60ee71` passed workflow run [#44352404767](https://github.com/Aloews/russian-dev-projects/actions/runs/44352404767) (`universal-service-hub-ci`).
- The single CI job completed all 47 steps successfully, including Product Factory syntax/safety/contract checks, Rust format/Clippy/tests, restricted-role and migration checks, Python checks, Docker build, Compose startup and container smoke test.
- The container smoke test passed after correcting the fixture to follow the real implementation-to-test lifecycle. This is exact-head CI evidence only; it does not prove persistent Git worktree execution, a live Railway worker, production deployment, external tenant onboarding or self-hosting.
- PR #64 remains draft and unmerged. Next: implement the real isolated workspace/worktree executor and connect the existing task API to a resumable worker adapter.


### Verification trust boundary — 2026-11-18

- Security review found that `worker_completion_evidence_valid` accepted worker-supplied `build_run_id`, `test_run_id` and reviewer approval JSON as proof of a passed check. Those values are self-reported and do not prove that GitHub Actions executed against the claimed commit.
- The latest code change removes positive completion rights from builder, tester and reviewer worker tokens. They can report `blocked` or `recovery`, but cannot advance a task to a passed-build/test state or `verified`. The ordinary user transition API already rejects `verified`.
- This is an intentional fail-closed gate, not a completed evidence pipeline. The next implementation must add a separate trusted GitHub Actions identity path (OIDC issuer/audience/repository/workflow validation), bind evidence to the exact candidate SHA and run attempt, persist the workflow conclusion and artifact digest, and only then allow an independent reviewer decision to mark the task verified.
- Until that trusted evidence path exists, the Hub must not advertise a task as verified based only on agent metadata. Exact-head CI for this hardening change is pending.


### Reviewer self-verification smoke correction — 2026-11-18

- Exact-head CI `44360949183` failed only in the container smoke test because the fixture still expected a reviewer worker to transition a task to `verified` using self-reported `decision=approved` JSON.
- The API correctly returned HTTP 403 with `trusted_verification_evidence_required`; this confirms the new fail-closed control is working. The application was healthy and the failure was an obsolete test expectation, not a runtime startup or database error.
- Updated the container smoke fixture to assert that exact 403/error response and then verify the task remains in `review` with its candidate SHA unchanged. This explicitly tests that an agent cannot self-certify verification.
- New workflow fixture commit: `95cda9909247c6c4168a8ac4e041c8238b7a026c`. Exact-head CI has been triggered; its result is pending.
- Positive verification remains intentionally unavailable until Issue #83 implements trusted GitHub Actions OIDC evidence ingestion plus independent review.


### Workspace cleanup smoke correction — 2026-11-18

- Exact-head run `44364315855` confirmed the reviewer self-verification rejection assertion passed; the container smoke then failed during cleanup because it tried to release a workspace while its still-live task assignment remained active. The API correctly returned `active_task_workspace_assignment_must_be_released` (HTTP 409).
- Corrected the smoke cleanup to call the worker-scoped task workspace-assignment DELETE endpoint with both live lease tokens before releasing each workspace. This preserves the runtime's lease/assignment safety invariant; no application behavior was weakened.
- Workflow fixture commit: `88b22195f05b1711f6113d914d4426b741825770`. Exact-head run pending.


### Workspace cleanup follow-up — 2026-11-18

- Exact-head run `44366947192` passed Rust format, lint and unit tests, then failed in container smoke cleanup. The implementation and tester completion paths already release their workspace assignments transactionally; the smoke fixture redundantly tried to DELETE those assignments using task lease tokens that had correctly been cleared at completion, producing HTTP 409.
- Updated cleanup to issue the explicit assignment DELETE only for the reviewer, whose attempted verification is intentionally rejected and therefore leaves its read-only assignment active. Implementer/tester assignments continue to be released by task completion; all workspaces are then released with their own lease tokens.
- Workflow fixture commit: `c914e2c51109b921ac9f5e581d3630907102157b`. Exact-head CI pending.


### Workspace cleanup smoke correction 2 — 2026-11-18

- Exact-head CI #44366947192 passed Rust format/lint/tests, migration and Python checks, Docker build and Compose startup, then failed in container smoke cleanup. The fixture released implementer and tester workspaces earlier, and their task completion had already released those assignments; the final cleanup loop then tried to delete the same assignment/release the same workspace again, receiving HTTP 409.
- Updated cleanup so only the still-active reviewer assignment and reviewer workspace are released in the final loop. Implementer/tester workspaces are not released a second time. Runtime authorization and lease invariants remain unchanged.
- Workflow correction commit: `6b6ec0a64084f7b45b197477e25ee7a504c2f49b`. Exact-head CI is pending.


### Exact-head workspace cleanup verification — 2026-11-18

- Current PR #64 head `46d9758b4b8a94d63480722096ffa719372a2ac6` passed exact-head CI run [#44369359083](https://github.com/Aloews/russian-dev-projects/actions/runs/44369359083).
- All 27 workflow steps passed: Product Factory checks, Rust format/Clippy/tests, restricted-role and migration checks, Python/Ruff tests, Docker build, Compose runtime startup and container smoke.
- The final smoke now releases only the still-active reviewer assignment/workspace. Implementer and tester assignments are released by their successful task completion paths; no duplicate release is attempted.
- This verifies the durable task API and current lease/cleanup smoke only. It does not verify actual Git worktree provisioning, long-running external worker execution, trusted OIDC evidence ingestion, production deployment, or multi-tenant live operation.


### Trusted GitHub Actions verification — implementation in progress

- New feature branch `feat/ush-trusted-github-verification-20261118` adds SQL migration `0011_trusted_github_verification.sql`, a dedicated OIDC ingestion endpoint, and a trusted-run verifier.
- OIDC checks include GitHub JWKS signature, fixed issuer, configured audience, expiry/nbf, subject, repository, branch ref, event, allowlisted `job_workflow_ref` and exact `job_workflow_sha` pin.
- The Hub independently fetches the Actions run from GitHub's API and requires exact repository, caller workflow path, branch, event, run ID/attempt, completed status, success conclusion and candidate SHA.
- Evidence receipts are tenant-scoped, append-only for runtime users, unique by repository/run/attempt/task, and bound by a database trigger to the task's repository and candidate SHA. Runtime role has SELECT/INSERT only; RLS is forced.
- Reviewer completion is re-enabled only behind the persisted trusted evidence lookup, exact candidate pin, empty writer scope and distinct implementation/reviewer identity.
- Added unit tests for repository/workflow allowlisting, reusable-workflow SHA pinning, exact run metadata, failure/replay mismatch and numeric/string run claims; migration contract tests; container smoke checks for RLS and restricted grants.
- Required configuration: `USH_GITHUB_OIDC_AUDIENCE`, `USH_GITHUB_ALLOWED_JOB_WORKFLOW_REFS`, `USH_GITHUB_TRUSTED_JOB_WORKFLOW_SHA`; optional `USH_GITHUB_API_TOKEN` for API rate limits/private run lookup.
- Not yet complete: trusted reusable workflow wiring, real signed-token end-to-end test, artifact digest capture, exact-head CI and adversarial disposable-Postgres tests. Existing fail-closed behavior remains until those gates pass. No production config or workers changed.
