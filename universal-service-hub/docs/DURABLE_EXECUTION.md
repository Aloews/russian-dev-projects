# Durable Execution Core

## Delivery boundary

This increment adds the PostgreSQL persistence contract for project plans and task execution. It does **not** yet expose task CRUD, enqueue jobs, claim leases, run workers, invoke agents/providers, or enable automatic schedule mutation. The schema is intentionally deployed only through the existing Rust SQLx migration path.

## Data model

| Table | Responsibility |
|---|---|
| `project_plans` | Immutable plan versions and one active plan per project |
| `project_tasks` | Tenant-scoped tasks, status, priority, role and three-point estimates |
| `task_dependencies` | Tenant-safe finish-to-start task edges |
| `project_events` | Append-oriented project/task lifecycle metadata |
| `execution_jobs` | Durable queue state, idempotency key, bounded payload/checkpoint, retry budget and lease |
| `execution_attempts` | Per-attempt worker, fencing token, outcome and duration |
| `execution_artifacts` | Object-store references and content hashes; no file bytes |
| `execution_approvals` | Explicit approval records for gated actions |

Every project-owned row carries `project_id`. Composite foreign keys include that tenant key so a task, plan, job, attempt or artifact cannot be attached across projects. Project deletion cascades through project-owned data. References from events/artifacts to optional task/plan/job rows null only the nullable reference column, preserving the tenant key.

## Queue contract for the worker increment

1. Claim only `queued` or due `retry_wait` rows in a short transaction using `FOR UPDATE SKIP LOCKED`.
2. Transition to `leased`, assign a fresh random `lease_token`, worker identity and expiry, and increment `attempt_count` atomically.
3. Every heartbeat, checkpoint write and terminal transition must compare both job ID and current lease token. A stale worker must not mutate a re-leased job.
4. Expired leases become retryable only through a bounded recovery sweep. Exhausted jobs transition to `dead_letter`; no unbounded retry loop.
5. The unique `(project_id, idempotency_key)` prevents duplicate logical enqueue operations. It does not make an external provider call exactly-once. Provider calls require provider-supported idempotency and a stable request key.
6. Checkpoints and job payloads contain only bounded, non-sensitive execution metadata. Never persist prompts, generated content, credentials, raw model transcripts or customer payloads.
7. Artifact bytes remain in a separately authorized object store. PostgreSQL stores only opaque storage references, SHA-256 and size.
8. Approval records are required before any future paid-resource, deployment, secret, production-data or other policy-gated action.

## Security and operational invariants

- Tenant scope is enforced structurally by composite foreign keys and must also be enforced in every API query.
- Lease tokens are fencing tokens, not authentication credentials; workers still require a separate authenticated identity.
- JSON metadata has explicit type and size bounds. Application-level allowlists and redaction remain mandatory.
- The event stream is append-oriented by contract. The next API increment must not expose event update/delete routes.
- Database backups, point-in-time recovery, retention, restore rehearsal, worker concurrency limits, metrics and alerting are separate release gates.
- The existing runtime runs SQLx migrations at startup. This migration must pass the repository's PostgreSQL-backed container smoke test before the schema is treated as verified.

## Acceptance tests for the next execution increments

- Fresh PostgreSQL database applies all Rust migrations; restart is idempotent.
- Cross-project plan/task/dependency/job/attempt/artifact references are rejected by constraints.
- A single active plan per project is enforced under concurrent activation.
- Concurrent queue claimers never lease the same job.
- A stale lease token cannot heartbeat, checkpoint or complete a job after re-lease.
- Expired leases recover once; attempt limits route exhausted work to dead letter.
- Duplicate enqueue with the same project idempotency key does not create a second job.
- A process restart preserves queued work, attempts and checkpoints.
- Oversized or non-object JSON metadata is rejected.
- No prompt, generated content, secret or raw provider response is written to database events/checkpoints/logs.

## Current status

- Schema migration authored on branch `feat/ush-durable-execution-schema`.
- CI and PostgreSQL migration execution are pending for the exact branch head.
- Task/job API, worker claim/heartbeat/checkpoint handlers, recovery sweeper, metrics and dashboards are not implemented in this increment.
