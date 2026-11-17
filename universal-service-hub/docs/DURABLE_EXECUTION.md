# Durable Execution Core

## Delivery boundary

This increment adds the PostgreSQL persistence contract and the first authenticated queue-control API. It exposes enqueue, claim, heartbeat, metadata checkpoint, completion, bounded failure/retry and artifact-reference handoff. It does **not** yet include a continuously running worker daemon, model/provider invocation, task CRUD, object-store byte transfer, monitoring or production deployment. The schema is applied only through the existing Rust SQLx migration path.

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

## Implemented queue API (worker-runtime foundation)

All routes are project-scoped. A project `owner` or `operator` token is required; the authenticated project must match `{project_id}`. A worker identity is a bounded identifier supplied by the authenticated worker and is not a substitute for the project token.

- `POST /v1/projects/{project_id}/jobs`: enqueue an idempotent job with `job_type`, `agent_role`, optional project task, priority and bounded attempt count. The persisted payload contains only the specialist role, not a prompt or model transcript.
- `POST /v1/projects/{project_id}/workers/claim`: transactionally recovers expired leases, then claims one due job using `FOR UPDATE SKIP LOCKED`. A fresh UUID fencing token and attempt row are created in the same transaction. Lease duration is 10–300 seconds.
- `POST /v1/projects/{project_id}/jobs/{job_id}/heartbeat`: extends only the current unexpired lease for the same worker and fencing token.
- `PUT /v1/projects/{project_id}/jobs/{job_id}/checkpoint`: accepts a strict metadata allowlist (`phase`, `cursor`, `progress`, `completed_steps`, `artifact_ids`); arbitrary text/prompt fields are rejected.
- `POST /v1/projects/{project_id}/jobs/{job_id}/complete`: commits success and attempt outcome transactionally. An optional `next_agent_role` + `next_job_type` + `next_idempotency_key` creates a follow-up job in the same transaction; artifact references must belong to the same project and source job.
- `POST /v1/projects/{project_id}/jobs/{job_id}/fail`: records a sanitized error code and applies bounded exponential retry or terminal failure/dead-letter based on retry budget.

A worker that loses its lease must treat the job as cancelled locally: every mutation is rejected unless its worker ID, lease token, job state and unexpired lease all match. The queue is at-least-once; external side effects still require idempotency at the provider boundary.

## Security and operational invariants

- Tenant scope is enforced structurally by composite foreign keys and must also be enforced in every API query.
- Lease tokens are fencing tokens, not authentication credentials; workers still require a separate authenticated identity.
- JSON metadata has explicit type and size bounds. Application-level allowlists and redaction remain mandatory.
- The event stream is append-oriented by contract. No event update/delete routes are exposed.
- The current API is a durable queue protocol, not yet an autonomous execution daemon: no process currently polls this API continuously or invokes specialist models. That worker supervisor/agent adapter is the next increment.
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

- Schema migration is inherited from `feat/ush-durable-execution-schema`.
- Queue API and lease lifecycle handlers are implemented on `feat/ush-worker-runtime-core`; exact-head CI and PostgreSQL smoke are pending.
- Continuous worker supervisor, specialized-agent dispatch, durable artifact upload/download, metrics, alerts and dashboards remain outstanding.
- No production migration, deployment or provider invocation has occurred.
