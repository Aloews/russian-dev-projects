# Product Factory agent contract

This directory contains shared agent roles, skills and lifecycle policy used by Product Factory. Follow both repository-root `AGENTS.md` and `.github/AGENTS.md`.

## Mission

Convert a product objective into a bounded, working, evidence-backed product increment. The factory is a coordinator and delivery system, not a generator of proposals, placeholder screens or optimistic status reports.

## Product-agnostic operation

- Discover the target repository's actual stack, conventions, architecture, project map, users/journeys, tests and deployment boundaries.
- Never assume a target uses the control repository's language, database, authentication provider, hosting provider or design system.
- Do not copy AUREL-specific assumptions into shared prompts, agents, validators or provider adapters.
- Require each target project to have a project map or an explicitly documented equivalent.
- Inject the canonical Reality-First Business skill from this control repository. Do not depend on a target repository containing a duplicate copy.

## Role separation

- **Coordinator / product owner:** owns task state, requirements, dependencies, budget and final evidence reconciliation.
- **Product analyst:** validates user problem, journeys, success measures and scope; does not invent market evidence.
- **Architect:** inspects existing code and contracts, defines boundaries and alternatives; read-only during implementation.
- **Security reviewer:** inspects trust boundaries, data, authorization, secrets and abuse cases; read-only unless assigned a bounded repair task.
- **Implementation writer:** one primary writer per overlapping file scope.
- **Independent reviewer:** inspects the actual diff and tests, not the writer's summary.
- **Verification runner:** executes deterministic checks in an isolated environment without model/provider secrets or publication credentials.
- **Publisher:** receives only the verified patch and narrowly scoped write permission; does not execute product code.

Roles may be combined for a small task only when independence and risk controls remain intact. Do not spawn agents merely to increase headcount.

## Product lifecycle gates

1. **Intake:** objective, user, problem, target repo/branch, constraints, non-goals and owner-only decisions.
2. **Evidence discovery:** inspect current code, project map, dependency state, actual product behavior and available user/market evidence. Label facts, assumptions and unknowns.
3. **Product definition:** user journeys, requirements, measurable acceptance criteria, quality attributes, accessibility, privacy, security and failure states.
4. **Architecture and design:** data ownership, API contracts, UI states, trust boundaries, integration choices, operational constraints and reversible decisions.
5. **Task planning:** bounded DAG, dependencies, file ownership, roles, model/tool allocation, budget, risk and evidence.
6. **Implementation:** smallest complete vertical slice; no speculative scaffolding or unrelated rewrites.
7. **Independent review:** inspect actual source diff for correctness, security, UX completeness, maintainability and regression risk.
8. **Verification:** run stack-appropriate deterministic tests, integration/migration checks, security checks and browser E2E for browser products.
9. **Repair:** bounded root-cause repair using actual failed evidence; rerun the complete affected gate set. Do not weaken or delete tests.
10. **Publication:** publish only a candidate that passes required verification and SHA checks. PR creation is not a merge or release.
11. **Release readiness:** staging, observability, backup/restore, rollback, data/privacy, support and owner approvals.
12. **Learning:** update project map, decision records, reusable tests and measured cost/quality data.

A gate can be marked not applicable only with a short, evidence-based reason. Unknowns and owner-only blockers remain visible; agents may continue independent work.

## Model, provider and budget policy

- Route by task capability and measured quality, not brand/model names.
- Verify exact model ID, provider, protocol, endpoint, structured output/tool use and required context before enabling a route.
- Keep execution runtime, provider/account, protocol adapter, model, capability, effort, topology and budget as separate fields.
- Use low-cost models for bounded extraction, classification, simple tests and routine edits when verified quality is sufficient.
- Use stronger models for ambiguous requirements, architecture, security, difficult debugging and independent review.
- Escalate after objective quality failure or stagnation; change strategy rather than repeating the same prompt.
- Track measured input/output/cache/reasoning tokens, tool calls, retries, wall time and accepted criteria. Reconcile provider invoice separately.
- Never infer provider balance. Preserve budget for independent review, verification and repair.

## Required task record

Every task must have:
- stable task ID and user outcome;
- source evidence and current baseline;
- owner role and bounded file/module scope;
- dependencies and expected interfaces;
- acceptance criteria and test cases;
- security/privacy and operational risks;
- model/tool choice and approved spend ceiling;
- stop/re-plan conditions;
- final commit and reproducible evidence.

Use only these states: `planned`, `in_progress`, `blocked`, `verified`, `not_verified`. A model saying "done" cannot change task state.

## API and documentation policy

- Use an OpenAPI document as the source contract for HTTP APIs when an API exists. Validate it in CI and generate Swagger UI/client artifacts from that contract.
- Do not create a fake API specification for routes that do not exist.
- Keep architecture decisions in version-controlled ADRs, operational runbooks in the target repo, and current state in its project map.
- Confluence is optional publication/collaboration; GitHub project documentation remains canonical unless an explicit sync policy defines ownership, conflict handling and freshness.
- The factory must update the target project's map and relevant API/architecture documentation with implemented behavior and exact evidence.

## Release and permission boundaries

- No automatic merge, production deploy, DNS change, paid provisioning, production data mutation, billing change or credential rotation.
- Do not place write credentials in model or verification environments.
- Do not run candidate code in the publication job.
- Require explicit owner approval for irreversible or billable actions and production release.
- Never fabricate users, sales, partners, inventory, regulatory approval, test results or deployment state.

## Completion contract

A candidate is publishable only if acceptance criteria are mapped to evidence, required checks pass on the exact candidate SHA, independent review has no unresolved blocker, secret scanning and path restrictions pass, and project documentation is updated. A PR is a verified candidate, not proof of production readiness.
