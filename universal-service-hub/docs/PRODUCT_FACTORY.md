# Autonomous Product Factory

## Purpose

Universal Service Hub is the shared engineering control plane for every owned product. A project is onboarded once with its repository, branch policy, acceptance criteria, provider permissions, budget limits and deployment targets. Product work then runs through the same evidence-driven multi-agent pipeline.

The first product run is AUREL / Elite Estate in `Aloews/full`. Do not run multiple product implementations concurrently until the first complete release has passed its acceptance gates.

## Agent team

| Role | Railway Cloud Agent model | Responsibility |
|---|---|---|
| Chief architect / lead implementer | `claude-fable-5-1` | Decompose the objective, make decisions, implement approved work, reconcile reviews and repair failures |
| Independent architecture reviewer | `claude-opus-5-5` | Challenge domain boundaries, data design, scalability, failure modes and maintainability |
| Security / privacy reviewer | `claude-sonnet-5-5` | Review authorization, tenant isolation, secrets, PII, threat model, legal/privacy boundaries |
| Research analyst | `claude-haiku-5-5` | Verify vendor/API facts from primary sources, identify constraints and provide evidence |
| QA / release reviewer | `claude-sonnet-5-5` | Test the exact candidate commit, inspect CI, identify missing regression and acceptance coverage |

Model IDs are explicit. A missing model must stop that role; no silent model substitution is permitted.

## Standard run

1. **Discovery — Fable:** inspect the repository, project map, issues, PRs, CI, deployment and current production state. Produce a bounded plan and acceptance criteria. Do not edit files yet.
2. **Independent design review — Opus:** inspect the same repository state and challenge the plan. Return concrete risks, alternatives and tests; do not edit files.
3. **Security review — Sonnet:** review trust boundaries, authorization, privacy, data retention, secrets, abuse cases and jurisdictional constraints; do not edit files.
4. **Evidence research — Haiku:** verify claims against primary documentation and report source URLs, dates and uncertainty; do not edit files.
5. **Implementation — Fable:** reconcile all findings, record decisions in the project map, implement one bounded workstream at a time, run checks and open/update a PR. Never merge or deploy production.
6. **Independent review — Opus + Sonnet:** inspect the exact immutable PR head SHA and its diff. Reviewers must not rely on the implementation agent's summary.
7. **QA — Sonnet:** run the project's documented checks against the exact candidate SHA; inspect logs and artifacts. Missing checks are not passes.
8. **Repair — Fable:** resolve every blocking finding, add regression tests, rerun all affected checks and request fresh reviews on the new SHA.
9. Repeat review/repair for at most two automated cycles per invocation. If still failing, leave the PR open with exact blockers; do not merge/deploy.
10. **Release:** a separate, explicit release policy requires green checks, independent approvals, staging smoke tests, verified rollback and authorized production access.

Review agents are read-only by instruction. Only the implementation agent writes. Work is serialized to avoid concurrent branch writers.

## Run evidence and durability

- Every product has one authoritative `docs/MAP.md` (or an explicitly configured equivalent).
- The current workflow stores each role's Railway task response, model, task ID and result in a 14-day GitHub Actions artifact; the final project map and PR must retain the durable decisions and delivery evidence.
- This initial workflow does not yet persist agent-run state in the Hub database. A database-backed run registry, policy engine and public orchestration API remain a later implementation milestone.
- Prompts and generated content are not copied into general logs. Secrets never enter prompts, source files, PRs or artifacts.
- Agents use stable Railway conversation keys per project and role so the workspace can be resumed; each run must fetch the latest remote branch before acting.
- Railway task webhooks are hints, not the source of truth. The runner polls the task API and verifies terminal results.
- Every task dispatch uses an idempotency key. A timeout must not create duplicate billable tasks.
- The first invocation starts at most four persistent specialist agents. Review/repair cycles are capped at two. Railway bills new agent creation and normal agent compute; configure a project-specific spend limit before repeated runs.

## Safety and autonomy

Agents may inspect and modify only allowlisted repositories and branches. They may create branches, commits and pull requests. They may not merge, alter production data, change DNS, rotate credentials, provision paid application infrastructure, change billing plans or deploy to production unless a separate project-specific release policy explicitly grants that operation.

When a credential, permission or external approval is missing, agents must continue independent work, record the precise blocker and never ask the owner to perform routine debugging. The owner receives only verified completion or a genuinely owner-only blocker.

## First product: Elite Estate

Target repository: `Aloews/full`

Target branch: `foundation/luxury-estate-platform`

Initial scope:
- Complete the Elite Estate vertical slice before expanding the other nine domains.
- Build real Rust API, PostgreSQL persistence/migrations, identity/authorization, evidence-backed property ingestion, review/publication, search and enquiry flows.
- Keep Next.js presentation-only.
- Do not fabricate properties, owners, agents, prices, valuations, sources, partners or transaction records.
- Do not claim brokerage, legal, tax, immigration, AML/KYC or investment advisory authority.
- Use a dedicated AUREL database and isolated development environment; do not reuse Sherlock Scholes or Novel Forge production data.
- Record the first launch jurisdiction and any licensing-dependent decisions as explicit blockers; implement jurisdiction-neutral foundations while those decisions are pending.

Elite Estate acceptance requires real end-to-end journeys, cross-tenant isolation tests, evidence/provenance validation, private/public listing controls, audit trails, migrations, complete Rust CI, security review, staging smoke tests and a tested rollback. A compiling scaffold is not a completed product.

## Execution entrypoint

The GitHub Actions workflow `.github/workflows/product-factory.yml` invokes the Railway Cloud Agent Tasks API. Required repository secrets:
- `RAILWAY_API_TOKEN`: account/workspace token authorized to dispatch Cloud Agent Tasks.
- `RAILWAY_PROJECT_ID` and `RAILWAY_ENVIRONMENT_ID`: isolated agent environment IDs.
- Optional `PRODUCT_FACTORY_WEBHOOK_SECRET` is reserved for a future authenticated webhook receiver.

The token is used only by the workflow and is never written to repository files. The workflow refuses to start if required credentials or target values are missing. Railway documents that new agents incur normal agent costs; keep concurrency and repair cycles bounded.

This workflow is the first execution layer. The Hub's database-backed agent-run registry, policy engine and long-term orchestration API are not implemented yet. A run is not considered complete until its final evidence is recorded and the product's release gates pass.
