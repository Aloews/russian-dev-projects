# Durable Execution Core

## Delivery boundary

This increment adds the PostgreSQL persistence contract for project plans and task execution. The follow-on queue API now exposes tenant-scoped enqueue/list/claim/heartbeat/checkpoint/complete/fail operations and a bounded expired-lease recovery sweep. It does **not** yet run agent/provider workers, expose task CRUD, or enable automatic schedule mutation. The schema is intentionally deployed only through the existing Rust SQLx migration path.

## Data model

| Table | Responsibility |
|---|---|
| `project_plans` | Versioned plan records and one active plan per project |
| `project_tasks` | Tenant-scoped tasks, status, priority, role and three-point estimates |
| `task_dependencies` | Tenant-safe finish-to-start edges constrained to one plan version |
| `project_events` | Append-oriented project/task lifecycle metadata |
| `execution_jobs` | Durable queue state, idempotency key, bounded payload/checkpoint, retry budget and lease |
| `execution_attempts` | Per-attempt worker, fencing token, outcome and duration |
| `execution_artifacts` | Object-store references and content hashes; no file bytes |
| `execution_approvals` | Explicit approval records for gated actions |

Every project-owned row carries `project_id`. Composite foreign keys include that tenant key so a task, plan, job, attempt or artifact cannot be attached across projects. Dependency edges also carry the plan ID, preventing accidental links between tasks in different plan versions. Project deletion cascades through project-owned data. References from events/artifacts to optional task/plan/job rows null only the nullable reference column, preserving the tenant key.

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
- Queue API handlers are implemented on the follow-on branch `feat/ush-durable-queue-api`; exact-head CI and PostgreSQL-backed API tests are pending.
- Routes:
  - `POST /v1/projects/{project_id}/jobs` — enqueue with project-scoped idempotency.
  - `GET /v1/projects/{project_id}/jobs?limit=50` — list bounded job metadata.
  - `POST /v1/projects/{project_id}/jobs/claim` — claim one due job; returns a lease token.
  - `POST /v1/projects/{project_id}/jobs/{job_id}/heartbeat` — renew a live lease.
  - `POST /v1/projects/{project_id}/jobs/{job_id}/checkpoint` — persist bounded checkpoint metadata.
  - `POST /v1/projects/{project_id}/jobs/{job_id}/complete` — complete the current attempt.
  - `POST /v1/projects/{project_id}/jobs/{job_id}/fail` — retry with bounded backoff or dead-letter.
- Every API operation is authenticated and project-scoped. Job mutation additionally requires owner/operator access; every lease mutation compares project ID, job ID and the current random fencing token.
- A runtime sweep runs every 15 seconds and recovers at most 100 expired leases per pass. Exhausted attempts move to `dead_letter`; retryable attempts return to `retry_wait`.
- Payload/checkpoint/result metadata is object-only, size-bounded and rejects common sensitive field names. This is a defense-in-depth filter, not a guarantee that arbitrary strings contain no personal data.
- Agent/provider worker execution, external provider idempotency, artifact upload, task CRUD, metrics, dashboards and restore rehearsal remain unimplemented release gates.
