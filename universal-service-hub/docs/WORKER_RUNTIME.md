# Worker Supervisor v1

## Delivered boundary

This increment adds a separately runnable Tokio worker supervisor to the Rust container image. It is deliberately limited to the non-billable `system_noop` job type while provider authorization, project budgets, artifact transfer and specialist dispatch are implemented and reviewed.

The worker process:
- runs a bounded number of independent polling loops;
- claims only explicitly allowlisted job types using PostgreSQL row locks and fencing tokens;
- recovers expired leases transactionally and closes the matching attempt ledger row;
- records successful non-billable jobs through the durable queue completion transaction;
- backs off on database errors and exits on SIGINT/SIGTERM after stopping its loops;
- requires a dedicated worker database URL; it never falls back to the Hub administrator token or API token;
- requires verified TLS (`verify-full`) when `USH_ENVIRONMENT=production`;
- does not run schema migrations and therefore does not require DDL privileges.

This is a supervisor/lease-recovery foundation, **not yet an AI agent runtime**. It does not call Starimg, Anthropic, OpenAI, Ollama or any other model provider. It does not process architecture, development, QA, security-review or integration jobs.

## Worker database role

Provision a dedicated PostgreSQL login outside the application process. Do not reuse the Hub migration owner, root database role or API token. The worker needs access only to queue tables:

```sql
GRANT CONNECT ON DATABASE ush TO ush_worker;
GRANT USAGE ON SCHEMA public TO ush_worker;
GRANT SELECT (id, project_id, task_id, job_type, status, priority, attempt_count, max_attempts, available_at, created_at, lease_token, lease_expires_at)
  ON execution_jobs TO ush_worker;
GRANT UPDATE (
  status, lease_owner, lease_token, lease_expires_at, heartbeat_at,
  attempt_count, available_at, last_error_code, finished_at, updated_at
) ON execution_jobs TO ush_worker;
GRANT SELECT (project_id, job_id, attempt_number, lease_token, status, started_at)
  ON execution_attempts TO ush_worker;
GRANT INSERT ON execution_attempts TO ush_worker;
GRANT UPDATE (status, finished_at, duration_ms, error_code, result_metadata)
  ON execution_attempts TO ush_worker;
```

Use a unique strong password, TLS and a connection string stored in the deployment secret manager. The example assumes the database name is `ush`; adapt it to the isolated environment. The worker must not receive project-token, provider-secret or administrator credentials. The current queue-only role can observe bounded queue metadata across projects; it must remain restricted to the `system_noop` capability. Do not enable customer task types until project-scoped worker authorization or PostgreSQL row-level security is implemented and independently tested. The worker query does not select job payloads, checkpoints, prompts or generated content.

## Configuration

- `USH_WORKER_DATABASE_URL`: required, dedicated role only.
- `USH_WORKER_ID`: optional bounded worker identity prefix.
- `USH_WORKER_CONCURRENCY`: 1–32, default 2.
- `USH_WORKER_POLL_MS`: 100–30000, default 1000.
- `USH_WORKER_LEASE_SECONDS`: 10–300, default 60.
- `USH_WORKER_TEST_PAUSE_AFTER_CLAIM_MS`: test-only crash-injection delay; nonzero values are rejected in production.
- `USH_WORKER_JOB_TYPES`: currently only `system_noop`; default `system_noop`.
- `USH_ENVIRONMENT=production`: enforces PostgreSQL TLS certificate and hostname verification.

A non-billable smoke job can be enqueued through the existing project-scoped API with `job_type: "system_noop"`, `agent_role: "integrator"` and a unique `idempotency_key`. The worker's allowlist matches the API's lowercase/underscore job-type validation; it does not accept dotted job types.

The Compose file defines an opt-in `worker` profile. Start it with `USH_WORKER_DATABASE_URL=... docker compose --profile worker up -d worker`. This service receives only the worker database URL and worker settings; it does not inherit the API token or application database URL. Keep the API and worker as separate processes so their restart policies, resource limits and scaling can be controlled independently. Do not start the worker until the dedicated role and secret are provisioned.

## Acceptance gates

1. Cargo builds both binaries and unit tests validate the worker capability allowlist.
2. PostgreSQL integration tests prove concurrent claims are unique, expired leases update both job and attempt rows, exhausted attempts enter dead-letter, and stale fencing tokens cannot complete.
3. Container smoke confirms the API image contains both executables.
4. The PostgreSQL/container smoke starts a worker with a least-privilege role, kills it with SIGKILL after claim, restarts it, and verifies one expired attempt plus one successful retry.
5. A production worker test proves TLS verification and database grants reject DDL and unrelated table access.

Until gates 2–5 have exact-head evidence, report the worker binary as implemented but the end-to-end worker service as unverified. Specialist provider execution remains a separate gated milestone.
