# Universal Service Hub — phased execution and resumable workload plan

## Objective

Turn the Hub into a reliable control plane for products and text, code, image, video, audio and infrastructure workloads. Large objectives are decomposed into bounded, independently verifiable work units. A service may stop at any time without losing acknowledged progress or causing duplicate billable generation.

This is a roadmap, not a claim that queues, checkpointing, media adapters or production deployment already exist.

## 1. Product roadmap: small delivery increments

Each increment is a separate PR-sized unit. Keep one writer per overlapping file set, define acceptance criteria before implementation, and merge only after exact-head CI and independent review.

### Track A — execution foundation

| ID | Increment | Deliverable | Acceptance evidence |
|---|---|---|---|
| A1 | Domain contracts | Typed task, step, attempt, checkpoint, artifact, provider capability and failure contracts | Serialization/schema and compatibility tests |
| A2 | Durable job persistence | PostgreSQL tables for jobs, steps, attempts, checkpoints, artifacts, leases, outbox and idempotency keys | Migration and PostgreSQL integration tests |
| A3 | Job API | Submit, inspect, list, cancel, pause, resume and retry endpoints with project-scoped authorization | API contract and cross-tenant denial tests |
| A4 | Queue/outbox | Transactional outbox, queue publisher, claim/ack/reject semantics and bounded queue depth | Crash-between-commit-and-publish and duplicate-delivery tests |
| A5 | Worker leases | Lease expiry, heartbeat, fencing token and stale-worker rejection | Kill-worker/reclaim and concurrent-claim tests |
| A6 | Checkpoint engine | Persist completed step outputs and resume from last verified checkpoint | Restart-at-every-boundary tests; no repeated completed steps |
| A7 | Retry and dead-letter policy | Typed transient/permanent/billing-ambiguous failures, backoff with jitter, max attempts and operator replay | Fake-clock and DLQ replay tests |
| A8 | Operations | Queue depth, oldest job age, retries, lease expiry, provider latency, failure rate and spend metrics | Metrics and alert tests |

### Track B — provider and model routing

| ID | Increment | Deliverable | Acceptance evidence |
|---|---|---|---|
| B1 | Live model catalogue | Discover model IDs and capabilities per provider; record observation time and source | Authorized live discovery and stale-catalogue tests |
| B2 | Capability registry | Normalize text, tool calling, vision, image generation/editing, video, audio, structured output, formats and limits | Contract tests; reject unsupported capability before dispatch |
| B3 | Model health and quotas | Track rate limits, concurrency, context/output limits, availability and latency | Synthetic tests and bounded live probes |
| B4 | Routing policy | Select by capability, project allowlist, quality tier, latency, region, privacy and cost ceiling | Deterministic routing and tenant-isolation tests |
| B5 | Fair scheduler | Weighted fair queues with per-project and per-model concurrency limits and starvation protection | Load simulation showing bounded wait and fair share |
| B6 | Failover | Capability-equivalent alternate providers; preserve parameters and artifacts | Provider-outage tests; no silent quality/format downgrade |
| B7 | Spend controls | Per-request ceiling, project/provider budgets, reservations, metering and kill switch | Budget-exhaustion tests; no dispatch after kill switch |

### Track C — image generation as a first-class workload

| ID | Increment | Deliverable | Acceptance evidence |
|---|---|---|---|
| C1 | Image provider adapters | Separate adapters for discovered Starimg image models, fal.ai, OpenAI Images and Gemini image endpoints when enabled | Per-provider live minimal generation and normalized errors |
| C2 | Image capability matrix | Text-to-image, image-to-image, editing, inpainting, aspect ratios, resolution, references, batch size, safety and formats | Timestamped catalogue and feature contract tests |
| C3 | Durable render jobs | Persist provider job ID, request fingerprint, idempotency support, render state and output references | Process-death simulation during submit, poll and download |
| C4 | Balanced model allocation | Fairly distribute independent image jobs across eligible healthy models using measured capacity and weights | Load test: no eligible model starves; limits respected |
| C5 | Image batch planner | Split brief into independently reproducible scenes/variants/sizes; preserve dependencies | Resume after arbitrary completed-image subset |
| C6 | Quality/output validation | Validate dimensions, MIME, decodeability, policy, constraints and duplicate/corrupt outputs | Golden fixtures and provider contract tests |
| C7 | Storage and delivery | Store originals and variants; persist checksums, metadata and access-controlled references | Upload/download integrity and signed-URL tests |
| C8 | Image evaluation | Optional blind ranking/quality checks and human selection; retain model/provider attribution | Reproducible evaluation set; no unsupported quality claims |

#### Image model balancing rules

1. Discover actual image-capable model IDs from each configured provider. Do not assume every model in a gateway catalogue supports generation, editing or a given resolution.
2. Build each job's eligible-model set from required features, project allowlists, region/data policy, format, safety policy, health, quota and budget.
3. Schedule with weighted deficit round-robin or an equivalent starvation-resistant fair queue. Weights represent configured share and are constrained by measured available concurrency and rate limits—not raw catalogue size.
4. Enforce per-model/provider concurrency, request/image quotas and project shares. A slow or rate-limited model must not block the entire image queue.
5. Among equally eligible models, rotate by least-recently-used or lowest normalized in-flight load. Record routing reason without logging prompts or credentials.
6. Reserve a configurable exploration share for quality benchmarking. Do not route everything to the cheapest model until blind quality and failure-rate evidence supports it.
7. Never silently substitute a model lacking a required editing mode, aspect ratio, reference-image behavior or resolution. Fail over only to a capability-equivalent model.
8. Reconcile provider-side jobs before retrying a billable request. If acceptance is ambiguous, mark the attempt submission_unknown and query by idempotency key/provider request ID. Never blindly submit a second render.
9. Split batches into independently checkpointed outputs (normally one output per work item or the smallest provider-supported atomic batch). A crash must not discard completed images.
10. Preserve reference-image object IDs, normalized parameters, seed when supported, model/version, provider job ID, checksum and output metadata. Keep prompts and generated bytes out of logs and audit events.

### Track D — media and specialist workloads

| ID | Increment | Deliverable | Acceptance evidence |
|---|---|---|---|
| D1 | Video adapter/jobs | Async submit, poll/webhook, checkpoint, storage and provider reconciliation | Long-render and worker-restart simulation |
| D2 | Audio adapter/jobs | Speech, voice and transcription tasks with streaming/async normalization | Format and interruption tests |
| D3 | Research/browser workers | Sandboxed bounded research/crawl tasks with source provenance | Timeout, SSRF and untrusted-content tests |
| D4 | Coding-agent jobs | Task graph, isolated workspace, run artifacts, verification/repair loops and PR boundary | End-to-end candidate PR with CI evidence |
| D5 | Infrastructure jobs | Dry-run plans, explicit approval gates, execution, health checks and rollback records | Contract tests plus separately authorized live smoke test |

### Track E — production operations and onboarding

| ID | Increment | Deliverable | Acceptance evidence |
|---|---|---|---|
| E1 | Project onboarding | Scoped identity, provider allowlists, secret references, quotas and acceptance profile | Isolated project onboarding test |
| E2 | Runtime deployment | Dedicated API, workers and scheduler; readiness, TLS and network policy | Clean staging deployment and live smoke test |
| E3 | Persistence resilience | Automated backups, restore rehearsal, migration strategy and retention | Restore into isolated database with checksums |
| E4 | Security hardening | Rate limits, SSO/per-user identity, audit access, egress controls, dependency/image scanning | Security regression suite |
| E5 | Reliability rehearsal | Kill API, scheduler, queue and workers during test jobs | Recovery drill without lost acknowledged work or duplicate billable submission |
| E6 | Release and rollback | Staged rollout, health gates, rollback automation and incident runbook | Rehearsed staging rollback |
| E7 | Product delivery | Product brief → design → implementation → build → running preview → E2E → handover | Product-level definition of done in MAP.md |


### Track F — adaptive delivery and schedule recovery

| ID | Increment | Deliverable | Acceptance evidence |
|---|---|---|---|
| F1 | Recovery preview | Tenant-scoped three-point critical-path forecast, bounded DAG validation and delay tiers | Exact-head Rust tests, three-point estimate tests, API authorization tests, malformed-DAG tests and independent review |
| F2 | Durable schedule state | Versioned project plans, task DAGs, estimates and immutable progress events in PostgreSQL | Migration, tenant-isolation, concurrency and history-integrity tests |
| F3 | Forecast history | Timestamped snapshots, drift trends and three-point estimates with confidence ranges | Deterministic replay against historical task events; forecast calibration report |
| F4 | Delivery visibility | Project dashboard, critical-path view, optimistic/realistic/pessimistic finish forecasts and alert preferences | API/UI E2E, accessibility, alert deduplication and notification opt-out tests |
| F5 | Recovery executor | Policy-gated creation of isolated assignments for ready independent tasks | Dry-run/approval tests, concurrency limits, no shared-writer conflicts, spend ceilings and rollback |
| F6 | Recovery calibration | Failure-injection and delivery simulations; calibrate thresholds from accepted work | Reproducible worker-loss, CI-delay, provider-outage and stale-estimate simulations |

F1 is implemented on branch `feat/ush-adaptive-delivery-recovery` (PR #70) and remains unverified until exact-head CI and independent review pass. F1 is advisory only; it does not mutate schedules or dispatch work. Do not enable F5 until F2–F4 provide durable, tenant-scoped state and auditable approval policy.


## 2. Bounded task decomposition contract

The coordinator converts every large request into a persisted DAG (directed acyclic graph) of tasks.

- A task should normally fit within 5–20 minutes of worker execution. Split by independently testable deliverable, not arbitrary token count.
- Each task has a stable ID, parent/project IDs, objective, file/resource scope, dependencies, acceptance criteria, capability/model pool, budget ceiling, timeout, retry policy and output schema.
- Claim only when dependencies are complete and project/provider budgets permit execution.
- Each task produces a structured result and evidence references. "Agent finished" is not an acceptance criterion.
- Independent tasks may run concurrently. Tasks sharing files, migrations, mutable resources or a branch are serialized or isolated in separate workspaces.
- The coordinator aggregates child results, detects missing deliverables and creates follow-up tasks. A parent is not complete until all required children and final integration checks pass.
- Keep prompts and generated artifacts out of operational logs. Store them in access-controlled project storage with retention controls.

## 3. Durable execution and interruption recovery

### Job lifecycle

queued → leased → running → checkpointed → succeeded

Additional states: paused, cancel_requested, cancelled, retry_wait, submission_unknown, dead_letter and failed_permanent.

A running task can checkpoint repeatedly. Only a committed checkpoint is durable progress.

### Required persistence records

- jobs: tenant/project, parent, type, state, priority, input reference, idempotency key, budget reservation and timestamps.
- job_steps: ordered graph, dependencies, state, capability, input/output references and acceptance status.
- job_attempts: attempt number, worker, provider/model, lease/fencing token, times, sanitized error class, provider request ID and billing ambiguity.
- job_checkpoints: step ID, version, durable output reference, checksum, schema version and timestamp.
- job_artifacts: object key, MIME, size, checksum, access policy, source step and retention.
- job_leases: owner, expiry, heartbeat and monotonically increasing fencing token.
- outbox_events: transactional events awaiting queue publication.
- provider_usage: billable units, cost estimate/actual where available, provider/model and request reference; no prompt or secret.

### Safe worker protocol

1. API validates and stores job plus initial steps in one database transaction; enqueue an outbox event in that transaction.
2. Dispatcher publishes outbox events and marks them delivered only after broker acknowledgement. Duplicate publication is expected and safe.
3. Worker atomically claims a ready job with a lease and increasing fencing token. A stale worker cannot commit after a newer worker reclaims the task.
4. Worker runs one bounded step, writes output/artifact to durable storage, verifies checksum/schema, then commits checkpoint and state transition transactionally.
5. Worker heartbeats during long operations. Lease duration exceeds heartbeat interval with a safety margin.
6. On graceful shutdown, worker stops claiming work, finishes or checkpoints the current safe boundary, releases lease and exits.
7. On abrupt termination, lease expires. A replacement worker loads the last committed checkpoint and resumes at the next incomplete step.
8. Every state mutation checks the current fencing token. A delayed response from an old worker cannot overwrite newer state.
9. On restart, a reconciler scans expired leases, stale running records, undelivered outbox events and submission_unknown attempts.
10. Parent completes only after all required children and final integration/acceptance step pass.

### Exactly-once reality and billing safety

Queues normally provide at-least-once delivery. The Hub makes its state transitions idempotent; it must not claim universal exactly-once execution.

- Use a unique idempotency key for every logical billable operation and persist it before external dispatch.
- Use provider idempotency keys when documented and supported.
- Persist provider request ID as soon as received. On restart, reconcile the existing provider task before considering resubmission.
- If acceptance is ambiguous and the provider has no reconciliation mechanism, stop in submission_unknown; do not automatically spend again.
- Retry transient network errors, 429 and selected 5xx only according to provider semantics, Retry-After, budget and bounded exponential backoff with jitter.
- Do not retry invalid input, policy rejection, unsupported capability, authentication failure or budget exhaustion without a corrective change.
- Dead-letter after the configured attempt limit; preserve sanitized evidence and support explicit replay after correction.

### Service stop and restart behavior

- API stops: committed jobs remain queryable; new submissions fail fast unless database/outbox remain available.
- Scheduler stops: queued work remains durable; replacement scheduler resumes dispatch.
- Worker stops: lease expires or is released; another worker resumes from last committed checkpoint.
- Queue outage: outbox accumulates durably; dispatch resumes when broker returns.
- Provider outage: circuit breaker pauses that provider queue; eligible jobs fail over only to capability-equivalent providers allowed by policy.
- Database outage: workers do not acknowledge work or report uncommitted progress; they stop safely and reconnect.
- Object storage outage: do not checkpoint an artifact until upload and checksum verification succeed.
- Deploy/restart: graceful drain, readiness gates, backward-compatible migrations and rolling worker replacement. Never rely on in-memory task state.

## 4. Fair scheduling across projects, modalities and models

Use hierarchical scheduling:

1. Global capacity and spend limits.
2. Per-project weighted share and concurrency cap.
3. Modality queues: coding, text/reasoning, image, video, audio, research and infrastructure.
4. Provider/model subqueues with individual concurrency, RPM/TPM/image quotas and health.
5. Priority within each queue, with aging to prevent starvation.

Determine eligibility before comparing models. Compute normalized load from in-flight jobs/configured concurrency, recent latency, rate-limit headroom and failure rate. Configured weights are policy, not inferred quality scores.

Reserve capacity for interactive/urgent tasks; cap batch workloads so image/video generation cannot starve coding and control-plane operations. Provide per-project fairness ceilings and a global emergency stop.

## 5. Image generation workload planning

For a large visual brief, create a manifest of requested outputs and split it into work units:

- Art direction and reference analysis.
- Prompt/spec normalization per scene.
- Composition/shot variants.
- Separate output jobs per eligible model pool.
- Aspect-ratio and resolution variants only where requested.
- Image edit/inpainting jobs dependent on a specific source artifact.
- Output validation, metadata extraction and checksum.
- Optional blind comparison and human selection.
- Final asset packaging and manifest.

Each output is independently checkpointed. A failed model affects only its assigned jobs. Completed outputs are not regenerated because another output failed. Allocation is balanced relative to eligible capacity, not blindly equal where quotas, speed or cost differ.

## 6. Delivery sequence and release gates

1. A1–A3: task contracts, PostgreSQL persistence and authorized job API.
2. A4–A7: transactional queue, leases, checkpoints and safe retry/DLQ.
3. B1–B4: live capability catalogue and deterministic routing.
4. C1–C3: image adapters and durable asynchronous render lifecycle.
5. B5–B7 + C4: fair scheduling, model quotas and image distribution.
6. C5–C8 + D1–D3: batch planning, media, validation and evaluation.
7. D4 + E1: coding-agent orchestration and project onboarding.
8. D5 + E2–E4: infrastructure execution gates, staging runtime and security.
9. E5–E7: interruption drills, rollback and verified product delivery.

Each stage requires implementation, tests, exact-commit CI and relevant live sandbox evidence. Paid provider calls stay disabled until per-task/monthly budgets and kill switches are enforced.

## 7. First implementation slice

The first engineering PR implements only A1 and the minimum schema portion of A2:

- Typed task/step/attempt/checkpoint contracts.
- PostgreSQL migration for jobs, steps, attempts, checkpoints, leases and outbox.
- Unique idempotency constraints and tenant/project foreign keys.
- State-transition validation and unit tests.
- Migration smoke test against PostgreSQL.
- No public worker execution, billable provider calls or production deployment in this slice.

The next PR adds durable outbox and claim/lease protocol. Checkpoint execution follows after persistence and concurrency semantics are tested.

## 8. Definition of done for resumable work

- [ ] Kill a worker at every state boundary; acknowledged checkpoints survive.
- [ ] Restart worker and scheduler; task resumes at first incomplete step.
- [ ] Duplicate queue delivery does not duplicate committed state or artifacts.
- [ ] Stale worker cannot commit after lease fencing token changes.
- [ ] Provider acceptance ambiguity does not trigger a second billable generation.
- [ ] Pause, cancel, retry and dead-letter are durable and authorized.
- [ ] Parent waits for required children and final acceptance.
- [ ] Projects cannot read, cancel or resume each other's tasks/artifacts.
- [ ] Model quotas, project shares and global kill switch are enforced.
- [ ] Image jobs are fairly allocated across eligible healthy models and independently recoverable.
- [ ] Logs/audit contain no credentials, prompts or generated content.
- [ ] Recovery drill evidence and exact commit SHA are recorded in MAP.md.
