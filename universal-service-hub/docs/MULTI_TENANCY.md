# Multi-tenancy architecture

## Decision

Universal Service Hub is multi-tenant from its first product release. Aloews Development Center is the initial tenant, not a privileged shared namespace for customer projects. External signup stays disabled until the security release gate passes.

## Ownership hierarchy

`platform → tenant → workspace → project → task/run/artifact`.

Users may have different memberships in multiple tenants. Project ownership is immutable within a tenant except through an explicit audited transfer workflow. Cross-tenant sharing is disabled by default.

## Identity and authorization

- Platform administrator credentials are platform-scoped and must never be issued to tenants.
- Human identities are represented by the external identity tuple `(identity_issuer, identity_subject)`; tenant memberships bind that identity to one tenant and a least-privilege role.
- Tenant service tokens are tenant-scoped, stored as SHA-256 token fingerprints, revocable and optionally expiring.
- Existing project tokens remain project-scoped. Authentication must derive tenant and project scope from the stored token record and project ownership; caller-supplied tenant identifiers are never authorization.
- Authorization is deny-by-default and checked at every API, queue claim, worker capability, event, artifact, search, preview and support path.
- A platform operator's support access requires time-bounded elevation, reason and audit evidence; no silent tenant impersonation.

## Data isolation

- Tenant-owned tables carry `tenant_id`; project references use composite tenant/project foreign keys. Task, event and checkpoint ownership is derived from the owning project/task by database triggers, not from request JSON.
- Tenant ID is immutable after resource creation. Resource IDs and idempotency constraints include tenant scope where appropriate.
- PostgreSQL RLS is enabled and forced on execution-plane and core tenant-owned tables: tasks, events, checkpoints, dependencies, attempts, worker tokens, workspaces, assignments, path locks, tenants, workspace records, memberships, provider connections, resource/budget policies, usage events, API rate limits, projects, audit events, tenant tokens and project tokens. The Rust API sets tenant context with transaction-local `set_config(..., true)`.
- Application queries are role-switched to the non-login `ush_runtime` role after privileged startup migrations. That role is explicitly `NOSUPERUSER NOBYPASSRLS`; startup fails closed if the runtime connection does not resolve to that role or its flags are unsafe. Do not run API queries directly as the migration login.
- Authentication performs exact token-hash lookup in short-lived platform-scoped transactions before a principal exists; subsequent rate-limit, project, workspace, token-management and task operations use tenant-scoped transactions. This boundary is security-sensitive and requires route-by-route adversarial testing.
- Exact-head CI tests tenant A/B visibility, forged cross-tenant write denial, missing-context fail-closed behavior, token/audit/usage/rate-limit isolation and transaction-local context reset across sequential transactions on one reused connection. These checks must pass before calling RLS verified.
- Provider credentials are references into a secret manager, never plaintext in configuration. Provider connections and model policy are tenant-scoped.
- Artifacts, workspaces, caches, logs, vector indexes, backups and object storage must be partitioned by tenant before those capabilities are exposed.
- Tenant audit and usage data are partitioned and attributable. Unknown usage prices remain NULL/unknown, never zero.

## Resource and spend isolation

Each tenant receives explicit limits for active tasks, concurrent workers, CPU, memory, storage and API rate. Budget policy separates soft and hard limits; actual usage is appended to an idempotent ledger. A hard limit must stop new billable work, not interrupt unrelated tenants. Fair queue scheduling and noisy-neighbor tests are required before public onboarding.

## Deletion and export

Tenant deletion is a state machine: suspend access and new work, drain/cancel tenant work under policy, export requested records, delete primary and derived data, expire backups under the documented retention window, and emit verifiable completion evidence. Legal holds override deletion only when explicitly recorded and authorized.

## Implementation status

- Schema foundation is in the bootstrap PR: tenants, workspaces, external-identity membership records, tenant service tokens, tenant-owned project/token/task/event/checkpoint records, tenant provider connections, quotas, budget policy and usage ledger.
- Existing projects are backfilled into tenant `aloews` / workspace `default`.
- Tenant service-token authentication and token lifecycle endpoints are wired into the Rust principal; project/worker tokens derive tenant ownership from stored records. Human OIDC membership authentication is not yet wired.
- Execution-plane and core/token RLS, forced RLS, a non-bypass runtime DB role and transaction-local tenant context are implemented in the candidate branch. Exact-head PostgreSQL RLS and pooled-connection tests are pending; this is not yet verified.
- Task/workspace leases, task/workspace assignment and overlapping path locks are implemented. Actual persistent Git worktree provisioning and execution sandboxing remain outstanding.
- Full tenant isolation across project/token/provider/usage/audit/artifact/cache/log surfaces, quotas/budgets, fair scheduling, export/deletion, cross-tenant penetration tests and public onboarding are not complete. Do not claim complete multi-tenancy or enable external signup until these gates pass.

## Release acceptance

1. Tenant A cannot read, mutate, claim, cancel or download Tenant B resources through any route.
2. Same idempotency key in different tenants is independent; duplicate within one tenant is idempotent.
3. Composite foreign keys reject cross-tenant references.
4. RLS isolation passes with application filters deliberately omitted; pooled connection context cannot leak.
5. Revocation blocks new requests and invalidates active worker capabilities within a bounded interval.
6. Per-tenant quotas, budgets and fair scheduling prevent noisy-neighbor exhaustion.
7. Secret references, logs, traces, metrics, caches, preview links and artifacts reveal no cross-tenant data.
8. Export/deletion includes derived indexes and backup-expiry evidence.
9. Support access is elevated, time-bounded, reason-coded and audited.
