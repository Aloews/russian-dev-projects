# AR-05 — Resumable tool execution integration contract

Status: implementation contract and test gate; **not evidence of live-loop integration**.

Tracked implementation issue: [#111](https://github.com/Aloews/russian-dev-projects/issues/111)

## Verified building blocks (as of 2026-11-27)

- PR #107: lease-scoped worker artifact download transport; exact-head CI verified according to the project map.
- PR #109: checkpoint observation-reference validation and artifact restore helper; exact-head CI run 45990946500 passed.
- PR #110: extends the PostgreSQL restart/fencing test to persist observation-reference metadata; its exact-head CI was still running when this contract was written.
- The worker loop has not yet been shown to invoke these pieces as one durable lifecycle.

Do not infer integration, production readiness, or exactly-once side effects from these components.

## Required durable action lifecycle

An action identity is immutable and scope-bound: project, job, task, action UUID, tool ID/version, canonical action digest. Use the existing ToolReceiptIdentity / derived idempotency key contract. Do not invent a second key scheme.

1. **Lease fence** — revalidate active worker lease and scope before each action; fail closed if absent, expired, or replaced.
2. **Reserve receipt** — reserve the action key durably. A completed receipt returns its recorded artifact/result metadata; an uncertain receipt stops automatic replay; a conflicting identity is rejected.
3. **Invoke tool** — only the reserved action may invoke. External writes must pass the same idempotency key to the target if that target explicitly supports it. Otherwise a crash after the external write but before receipt completion is ambiguous and must become uncertain.
4. **Bound and validate output** — cap output to MAX_TOOL_OUTPUT_BYTES; serialize to JSON; reject oversize, invalid, or unsupported payloads without logging sensitive content.
5. **Persist observation artifact** — upload using lease-scoped worker artifact transport. Capture artifact ID, SHA-256, byte length, JSON content type. Recheck lease and project/job/task scope before accepting the upload result.
6. **Complete receipt** — mark the reserved action complete with artifact reference and digest. A transition that cannot be confirmed must not be treated as completed.
7. **Checkpoint CAS** — save the updated state, completed-step list, artifact IDs, observation refs and sequence using the current lease token and expected sequence. If CAS loses, discard local authority and reload under a newly validated lease.
8. **Continue** — only after completion and checkpoint success proceed to the next action.

## Restart / recovery rules

- Load the checkpoint through CheckpointStore::load (which enforces lease fencing and digest).
- Fetch only the exact observation artifact IDs referenced by the checkpoint, within the current lease and scope.
- Restore using restore_tool_observations; it verifies exact-reference set, JSON content type in metadata, size, SHA-256 and JSON decoding.
- Never synthesize missing observations, silently skip corrupt artifacts, or load artifacts from another tenant/project/job/task.
- A completed receipt is read, not replayed.
- An uncertain external side effect is surfaced for reconciliation and never blindly repeated.
- If artifact restore or receipt state conflicts with the checkpoint, stop the job with a typed recoverable error and preserve evidence metadata only (no raw tool output in logs).

## Crash-boundary test matrix (required)

| Crash/failure boundary | Expected recovery behavior |
| --- | --- |
| Before reservation | Action may be planned again; no tool invocation occurred. |
| After reservation, before invocation | Receipt must be distinguishable from ambiguous in-flight; safe recovery requires proving invocation never began or mark uncertain. |
| During/after external invocation, before output capture | Mark uncertain; do not blindly repeat side effect. |
| Output captured, artifact upload failed | No completed receipt; retry only if invocation is provably read-only/idempotent; otherwise uncertain. |
| Artifact uploaded, receipt completion failed | Reconcile receipt/artifact under lease; do not re-invoke tool. |
| Receipt completed, checkpoint CAS failed | Re-read completed receipt and artifact; rebuild checkpoint without tool replay. |
| Checkpoint persisted, worker crashes before ack | Reload fenced checkpoint and referenced artifacts; do not repeat completed action. |
| Lease expired/replaced at any boundary | Old worker is fenced from writes; new lease holder performs reconciliation. |

## Required adversarial tests

- Tenant/project/job/task scope mismatch and cross-tenant artifact IDs.
- Expired/replaced lease for receipt reserve, artifact upload/download, receipt completion and checkpoint CAS.
- Concurrent duplicate action ID/key, idempotency-key collision with differing action digest.
- Crash-injection at every lifecycle transition.
- Completed, reserved/in-flight and uncertain receipt states; verify no blind replay.
- Missing artifact, unexpected extra artifact, corrupt hash, invalid JSON, wrong content type, size mismatch and over-limit output.
- Artifact upload success with receipt failure; receipt completion success with checkpoint CAS failure.
- Worker restarts and lease reassignment while recovery is running.
- Sensitive output redaction: logs contain identifiers/status/error codes, never tool payloads, prompt text, tokens or secrets.

## Definition of done

This document alone is **not** completion. The issue #111 implementation must:
- integrate the actual worker and agent-loop path, with regression tests that fail if integration is removed;
- use existing durable receipt/checkpoint/artifact APIs, or document and review any contract change;
- pass format, lint, unit, DB-backed, S3-compatible, worker, compose/container and crash-recovery checks on the exact immutable PR head;
- include test names and actual CI job/run evidence in the MAP;
- remain unmerged until exact-head review and all required checks are green.

No production migration, deploy, paid provisioning, or external side-effect retry is authorized by this task.
