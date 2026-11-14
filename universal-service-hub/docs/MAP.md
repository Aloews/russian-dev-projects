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
- Product Factory uses GitHub-hosted runners, a bounded local Qwen2.5-Coder 3B multi-role pass, Starimg Anthropic Messages (Fable 5.2 primary, Opus 5.5 fallback), optional OpenRouter Anthropic Messages fallback, isolated verification and a separate PR publication job. No Railway Cloud Agent Tasks are required.
- A previous run exposed two false-success defects: Qwen could produce a report without source edits, and Fable attempted to clone Caveman into a directory already created by Qwen. Both were fixed. Qwen candidates now require non-document source/test changes and exact old-text edits; the Fable action reuses the verified Caveman checkout.
- Starimg's current key returned HTTP 402 `insufficient_balance_error` for both Fable 5.2 and Opus 5.5. This is exhausted key allowance, not an API compatibility failure. OpenRouter fallback now probes `OPENROUTER_API_KEY` using the Anthropic Messages API and uses `ANTHROPIC_AUTH_TOKEN` when available.
- Run `43602951049` produced no candidate: Qwen's source edit was rejected because it treated an existing file as new; Starimg Fable/Opus were skipped after the 402 probes. No PR or product was published. Latest isolated attempt: run `43604172878`, branch `factory/elite-estate-rc3`, currently pending/in progress; its Qwen and OpenRouter outcomes must be checked before any success claim.
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
   - Use specialist agents for bounded work; keep one authoritative writer per branch. Escalate difficult or repeated failures to Fable 5.2 and independent reviewers.
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
- Do not rerun unchanged failing jobs indefinitely. After two unsuccessful attempts with the same failure signature, change the approach/provider/model or isolate a minimal reproduction and escalate to Fable 5.2.
- If Fable fails, capture the sanitized provider request/response metadata and tool-call protocol error, then use a verified compatible Starimg model or another authorized provider. Never claim model compatibility from secret presence or HTTP reachability alone.
- Use Qwen 2.5 Coder 3B for bounded specialist analysis and structured edits; validate every edit and test the resulting code. Fable 5.2 owns difficult repairs and final engineering reconciliation. Opus/Sonnet/Haiku or other independent reviewers challenge architecture, security and product behavior; their roles are not proof of successful execution.
- Preserve artifacts for each run: task brief, decision record, candidate commit, diff, build output, test results, live smoke-test evidence, provider/model identity (without secrets), runtime URL and handover record.
- Report to the owner only a verified handover or a specific owner-exclusive blocker. Agent chatter, intermediate decisions and routine failure details stay inside the factory and project artifacts.

### Safety and release boundary

- Autonomous implementation, isolated builds, tests and preview/staging deployment are expected within already authorized accounts and budgets.
- Production deployment, paid resource creation beyond an explicitly approved budget, DNS/domain changes, billing changes, irreversible migrations and production-data mutation remain gated by explicit owner authorization and a rollback plan.
- Never reuse credentials or production data from Sherlock Scholes, Novel Forge or another project. Use project-scoped identities and secrets.
- The factory must prefer delivering a verified working preview over leaving the task as an untested PR, while preserving the production safety boundary.


## Agent routing update — 2026-11-14 (no Fable)

- The Product Factory workflow no longer calls Starimg or Fable and no longer requires `STARIMG_API_KEY`.
- Primary implementation and up to two verification-repair passes use Anthropic-compatible Claude Sonnet 4.5 through OpenRouter, only after a live tool-call compatibility probe succeeds.
- Local Qwen2.5-Coder 3B is the fallback for initial candidate generation and verification repair when OpenRouter is absent or fails.
- Missing OpenRouter credentials must not stop the pipeline from attempting the local Qwen route. A missing provider key is a capability gate, not a fabricated success.
- Workflow head implementing this routing: `a17c9023464f770ac28e46dd84f4169105aa2c3b`.
- Required verification: control-plane CI on this exact head, then a complete factory run that produces candidate artifacts, passes isolated verification and publishes a PR for `Aloews/full`. Until those artifacts exist, the factory is not accepted as operational.

## Execution recovery update — 2026-11-14

- Latest confirmed control-plane CI before this recovery: run `43604324413` (workflow run #810) passed the complete Rust, Python, migration, Docker and container-smoke suite on its exact commit.
- Product Factory execution is still not proven. Run `43602951049` reached the Qwen implementation pass, which failed because the model marked an existing source file as a new file (`new-file edit targets an existing file`). The subsequent Starimg probe returned HTTP 402 `insufficient_balance_error` for both Fable 5.2 and Opus 5.5. No candidate artifact or product PR was published.
- Run `43604172878` was cancelled while Qwen was installing its local model; the run's implementation, verification and publication jobs did not complete. This is consistent with the workflow's previous `cancel-in-progress: true` concurrency policy, which could cancel a long-running agent job when another run entered the same target group.
- Recovery changes on this branch:
  - Set Product Factory concurrency to `cancel-in-progress: false` so a newer run queues rather than cancelling the active product build.
  - Increase both Qwen team and Qwen fallback job-step timeouts from 10 to 25 minutes. The local 3B model runs CPU-only on GitHub-hosted runners and the previous 10-minute ceiling was too tight for model startup plus inference/retry.
  - When Qwen incorrectly marks an existing path as a new file, include bounded current-file evidence in the corrective retry instead of only repeating the generic validation error.
  - Reject edits whose resolved filesystem path escapes the target repository through a symlinked parent directory.
  - Add regression tests for the existing-file retry context and symlink escape rejection.
- The latest PR head contains these recovery changes. CI and a new end-to-end Product Factory run on the exact recovery head must pass before this platform is considered operational. Starimg balance remains an external blocker unless a funded Starimg key or a separately verified OpenRouter fallback is available.
- Do not merge PR #21 or claim a completed product from these code changes alone. The first product acceptance target remains `Aloews/full`, branch `factory/elite-estate-rc3`, base `foundation/luxury-estate-platform`.
- Recovery implementation commit sequence is complete at control-branch head `2d620a96d28d6cb557198c03f4a09e66f9ef9a43` (workflow concurrency/timeout, Qwen retry/path-safety logic and regression tests). The exact recovery head has not yet returned CI or product-run evidence; keep PR #21 in draft until both are verified.
