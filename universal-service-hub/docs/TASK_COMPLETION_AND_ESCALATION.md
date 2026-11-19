# Task Completion Engine and Human Escalation

Status: approved engineering design; implementation not yet complete.
Owner: USH execution plane.
Scope: durable task lifecycle, acceptance evidence, bounded recovery, escalation and human handoff.

## Product invariant

A task is `completed` only when every required acceptance criterion has machine-verifiable evidence attached to the exact candidate commit. Agent messages, PR creation, code generation, reviewer agreement, and a green subset of checks are not completion evidence.

The system must prefer an explicit blocked/escalated state over a false success. It must never silently spend beyond a task's configured budget or grant a human contractor production credentials.

## Lifecycle

Use explicit persisted states and legal transitions:

`queued -> planning -> executing -> validating -> reviewing -> completed`

Recoverable failures transition to `repairing`; exhausted automation transitions to `escalated`; human work transitions to `human_in_progress -> validating`; unrecoverable or cancelled work transitions to `blocked` or `cancelled`.

Every transition is an append-only task event with actor, timestamp, prior/new state, reason, attempt ID, candidate SHA (when available), and evidence references. State changes must use compare-and-swap/version checks and be idempotent. Expired leases are recoverable; duplicate delivery must not duplicate work or evidence.

## Acceptance contract

Every task must have a versioned acceptance contract before execution:
- objective and explicit non-goals;
- independently checkable criteria with required evidence types;
- affected project/repository and allowed path/capability scope;
- required test suites and runtime/integration scenarios;
- security/data classification and rollback requirements;
- wall-clock, model/provider and monetary budgets;
- definition of done and escalation policy.

Criteria are individually tracked as pending, passed, failed, or waived. A waiver requires an authorized human decision, a reason and an immutable audit event. Agents cannot waive criteria or mark their own task complete.

## Evidence and completion gate

Evidence records bind criterion ID, candidate commit SHA, command/check identity, environment image/version, start/end time, exit status, bounded logs/artifact digest, and verifier identity. Evidence must be reproducible where practical. A changed candidate SHA invalidates evidence for criteria affected by that change; policy may require a full suite rerun.

Completion requires:
1. all mandatory criteria passed against the same candidate SHA;
2. required build, test, migration, integration and runtime smoke checks passed;
3. independent reviewer is not the implementing worker and has read-only workspace permissions;
4. no unresolved blocking findings;
5. evidence and final artifact are durably stored;
6. final state transition succeeds transactionally.

A green CI job alone is not proof of a production deployment or a live user workflow. Those require separately defined runtime/deployment evidence.

## Bounded recovery and strategy changes

Persist every attempt, model/provider, strategy fingerprint, prompt/input digest, output artifact, failure category, duration and cost. Classify failures (environment, provider/quota, context, implementation, test, architecture, security, external dependency).

Policy defaults (configurable per project/task):
- after 3 failures with the same strategy fingerprint, prohibit repeating that strategy;
- after 3 materially different strategies without acceptance progress, escalate to an independent provider/model;
- after 2 independent expert attempts fail, stop autonomous retries and create a human handoff;
- if there is no measurable acceptance-criterion progress for 24 hours, trigger reassessment/escalation;
- stop immediately on critical security findings, budget exhaustion, unsafe scope or missing required secrets;
- enforce maximum wall-clock, token/provider spend, retry count and concurrent worker limits.

Progress is measured by criterion/evidence state changes and validated defect resolution, not commit count, PR count, agent messages or documentation volume. A retry must change at least one justified dimension (diagnosis, context, implementation strategy, provider, or test environment); otherwise reject it as a duplicate attempt.

## Escalation ladder

- L0: assigned executor implements and runs its planned checks.
- L1: new executor/strategy with failure evidence; no same-strategy retry loop.
- L2: isolated architecture/debug review with bounded source context and reproducible failure.
- L3: independent provider/model not used for the failed implementation, with access to source, diffs and evidence.
- L4: human engineer handoff when configured thresholds are exhausted.

Escalation is a persisted task, not a chat message. It contains objective, acceptance contract, current state, repository/ref/SHA, minimal reproducible steps, exact logs and artifacts, attempted strategies and why each failed, unresolved risks, allowed scope, required skills, budget estimate and secure workspace instructions.

## Human engineer controls

UHS may prepare and route a handoff, but must not autonomously hire, contact, contract, disclose private source, grant broad repository access, approve payment, or expose production secrets. Those actions require explicit owner authorization and the organization's access/payment process.

Human engineers receive least-privilege, time-bounded, isolated workspaces and a task-specific branch. Production credentials/data are excluded by default. Deliverables are commits/PRs, test evidence, explanation of remaining risks and reproducible steps. Human work passes the same acceptance gate and independent review as AI work.

## Observability and operator experience

Expose per task: lifecycle state, current owner/role, lease, attempt number, strategy fingerprint, criteria progress, last measurable progress, spend/budget, blocker category, escalation level, evidence links and next automatic action. Provide project-level queues for active, stalled, escalated, blocked and completed tasks.

Alerts are actionable and deduplicated. Stalled tasks are not silently left in "running". Every automatic action and human decision is auditable.

## Implementation milestones

1. Contract and schema: acceptance criteria, attempt/strategy records, evidence, escalation cases, policy budgets and state transitions.
2. Durable orchestrator: claim/lease/heartbeat/checkpoint, idempotent transitions, retry classification and recovery.
3. Completion Gate: criterion-to-evidence binding, SHA invalidation, independent review and transactional completion.
4. Strategy controller: strategy fingerprinting, no-progress detection, retry budgets, provider escalation and circuit breakers.
5. Human handoff: secure package, approval workflow, isolated workspace, time-bounded access and human result verification.
6. Operator UI/API: task board, evidence view, stalled/escalated queues, budget and audit.
7. Sandbox acceptance: deliberately inject broken code, failing tests, provider 402/timeouts, worker crashes, lease expiry, duplicate events, changed SHA, missing evidence, budget exhaustion and human patch. Prove correct recovery or safe escalation for each.
8. Pilot: connect Sherlock only after sandbox acceptance; begin with non-production tasks and keep merge/deploy permissions disabled until separately authorized.

## Release gates

- Unit/contract tests cover every legal and illegal state transition.
- Integration tests prove concurrent workers cannot claim/complete the same task twice.
- Crash/restart tests prove lease recovery and checkpoint resume.
- Security tests prove tenant isolation, least privilege, read-only reviewer and no production secret exposure in handoffs.
- Failure-injection tests prove finite retries, strategy changes, budget stops and human escalation.
- End-to-end sandbox task reaches a verified PR and acceptance receipt; a second task deliberately fails and reaches a complete human handoff.
- No production enablement until all gates pass and owner approves the rollout.

## Current verified status

This document records the target contract and implementation plan only. The existing USH API has task, attempt, worker, lease, workspace, checkpoint and trusted-CI evidence primitives, but this design is not yet implemented end-to-end. Railway workers remain disabled; no production execution, hiring, payment or deployment is authorized by this document.
