# Product development lifecycle

This is the shared product-development logic for Product Factory and all managed product repositories. It complements the agent execution contract; it does not replace product-specific discovery or owner approval.

## Governing metric

Optimize for **total cost per accepted, verified product increment**, with latency, escaped defects, rework, maintainability, privacy/security risk and user outcomes as guardrails. Do not optimize for lines of code, number of agents, number of PRs, raw tokens or speed of generation.

Count model input/output/cache/reasoning tokens, tool/API calls, retries, runner/CI compute, storage/network, review time, repair/rework and operational burden. Separate measured usage from estimates and provider invoices.

## Stage gates

| Gate | Required output | Exit condition |
|---|---|---|
| 0. Intake | Product brief and permissions | User outcome, owner, scope and non-goals are clear |
| 1. Discovery | Evidence inventory | Existing product, stack, users/evidence, constraints and unknowns inspected |
| 2. Product definition | Journeys and requirements | Testable acceptance criteria and success measures exist |
| 3. Architecture/UX | Architecture, data/API contracts, UX states, ADRs | Trust boundaries, failure semantics and alternatives are explicit |
| 4. Planning | Task DAG and budget | Dependencies, file ownership, roles, tests and stop conditions are defined |
| 5. Build | Working vertical slice | A real user journey works; no placeholder-only delivery |
| 6. Review | Independent findings | Actual diff reviewed; blockers resolved or explicitly retained |
| 7. Verification | Reproducible test evidence | Relevant deterministic checks pass on exact candidate SHA |
| 8. Preview/release readiness | Staging and operations evidence | Observability, rollback, privacy, support and approvals are ready |
| 9. Release/operate | Release record and monitoring | Authorized release completed and post-release signals observed |
| 10. Learn | Updated map and decision records | Defects, costs, outcomes and reusable learning captured |

Do not collapse all gates into one generic "agent completed" state. For a narrow low-risk change, gates may be lightweight, but the reason and omitted checks must be recorded.

## Product discovery and prioritization

Prioritize work using:
- user impact and frequency of the problem;
- strength and source of evidence;
- strategic fit and dependencies;
- implementation and lifecycle cost;
- uncertainty and reversibility;
- security, privacy, legal and operational risk;
- time to a complete, testable outcome.

Use estimates as ranges with confidence and assumptions. Recalibrate from the project's own accepted-task history; generic effort models and model estimates are priors, not facts.

## Task decomposition and collaboration

- Maintain one durable dependency DAG per increment.
- Split work by independently testable outcomes and non-overlapping file ownership.
- Use parallel agents only for independent discovery, isolated modules or independent test/review work.
- Keep one coordinator responsible for reconciliation and integration.
- Cap debate and repair cycles. After two no-progress attempts, change task shape, context, model/effort, runtime/tool or verification strategy.
- Preserve a reserve for review, verification and repair; do not spend the full budget on planning or implementation.

## Design and architecture quality

Before coding, establish:
- system context and component boundaries;
- data classification, ownership, lifecycle and tenancy;
- identity, authentication, authorization and audit;
- API/event contracts and compatibility;
- UX journeys, loading/empty/error/permission states and accessibility;
- resilience, idempotency, retries, rate limits and recovery;
- deployment topology, observability, backup and rollback;
- alternatives, trade-offs and irreversible decisions.

Record significant decisions as ADRs with context, decision, alternatives, consequences, owner and date.

## API documentation

For HTTP APIs:
1. Maintain one OpenAPI source specification alongside the API.
2. Generate Swagger UI and typed clients from that contract.
3. Validate schema and breaking changes in CI.
4. Test implementation-to-spec conformance where practical.
5. Keep auth scopes, errors, pagination, rate limits, idempotency and deprecation explicit.

Swagger UI is a presentation/test surface, not the API source of truth. Do not document endpoints that do not exist or claim undocumented behavior is implemented.

## Knowledge management

- Canonical product state: target repository's project map (usually `docs/MAP.md`).
- Architecture decisions: version-controlled ADRs.
- API contracts: OpenAPI files.
- Operational procedures: version-controlled runbooks.
- Agent instructions: root and nested `AGENTS.md`.
- Reusable procedures: versioned skills.
- Confluence may mirror/publish selected material only when ownership, sync direction, conflict resolution and freshness checks are explicit.

Avoid duplicating the same facts across multiple documents. Link to the canonical record and update it at the same time as the implementation.

## Quality and release evidence

Each acceptance criterion maps to one or more tests or observable checks. Choose checks by risk:
- unit/property tests for deterministic logic;
- contract tests for API/provider boundaries;
- integration tests for persistence, identity and queues;
- security tests for authorization, tenancy, injection, secrets and egress;
- browser E2E for critical user journeys;
- migration/backup/restore/rollback tests for data and operations;
- load/reliability tests where traffic or availability requirements justify them.

A release candidate requires exact commit identity, passing required checks, independent review, secret/dependency/security checks, staging evidence, monitoring and rollback readiness. Production release additionally requires the appropriate owner authorization.

## Project onboarding output

Every managed product should have:
- a local `AGENTS.md` created from the template and filled with verified facts;
- a project map and architecture overview;
- reproducible setup/build/test commands;
- API contract if HTTP APIs exist;
- dependency/provider manifest with secret references only;
- security and data-lifecycle notes;
- release and rollback runbook;
- acceptance test matrix;
- known limitations and owner-only blockers.

Do not mark a product managed until these artifacts and the minimum onboarding checks exist.
