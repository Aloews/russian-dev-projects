# UHS Rust Agent Runtime — Architecture and Delivery Plan

Status: **DESIGN BASELINE — implementation not yet verified**  
Owner: UHS coordinator  
Scope: replace the current `system_noop`-only worker capability with a secure, provider-neutral Rust agent runtime. This document describes intended behavior; it is not evidence that these capabilities exist.

## 1. Product outcome

A project-scoped task can be executed by one or more specialized agents. Each agent operates through a bounded loop:

1. Receive a typed task and a minimal, authorized context bundle.
2. Ask a configured model for the next structured action or final result.
3. Validate the model output against a versioned action schema.
4. Ask the policy engine whether the exact action is permitted.
5. Execute only through a registered tool adapter.
6. Store a bounded checkpoint and artifact references.
7. Continue, delegate to an authorized specialist, or finish.
8. Complete the durable job only while its lease fencing token remains valid.

The model never receives ambient database, filesystem, shell, network, object-store, provider-secret or Hub-admin credentials. The Rust runtime is the policy enforcement point; model output is untrusted input.

## 2. Architectural components

### 2.1 Runtime supervisor
- Rust/Tokio process with bounded worker concurrency, graceful shutdown, health/readiness and lease renewal.
- Claims only job types enabled by a local capability allowlist.
- Maintains a per-job cancellation token and execution deadline.
- Stops work immediately on lease loss, policy-engine failure or shutdown.
- Uses PostgreSQL as the durable queue/checkpoint authority; no in-memory-only job ownership.

### 2.2 Agent loop
- Explicit states: `queued → leased → planning → awaiting_model → validating_action → authorizing → executing_tool → checkpointing → delegating | completed`; terminal/error states include `retry_wait`, `dead_letter`, `cancelled`.
- Every iteration has limits for turns, wall time, input/output tokens, tool calls, artifact bytes and estimated spend.
- Model responses are parsed as typed envelopes; malformed, unknown or oversized actions fail closed.
- Tool results are treated as untrusted data and cannot modify policy or tool registry.
- No autonomous loop may extend its own budget or grant itself permissions.

### 2.3 Model gateway
- Provider-neutral Rust trait for Anthropic Messages, OpenAI-compatible and Gemini adapters already present in the Hub.
- Resolves provider credentials server-side by secret reference; secrets never enter task payloads, checkpoints, artifacts or model-visible tool output.
- Enforces provider/model allowlists, request deadlines, response-size limits, token/spend ceilings and explicit idempotency policy.
- Retries only safe/idempotent requests. A billable generation is not repeated after an ambiguous timeout unless provider idempotency is verified.
- Records model/provider IDs, latency, token counts and outcome metadata; never records prompts, completions or raw provider bodies in telemetry.

### 2.4 Tool registry and executor
- Tools are registered with a stable name/version, JSON input schema, output schema, required capabilities, risk class, timeout, byte limits and idempotency semantics.
- Tool execution happens in Rust adapters, not arbitrary model-generated shell commands.
- Initial tool families: read-only project metadata; bounded artifact read/write; Git read operations; isolated worktree operations; test execution in a sandbox; provider inference.
- Shell and filesystem tools are disabled until a sandbox with mount, process, syscall, network-egress and resource controls is implemented and tested.
- Every invocation is checked against tenant, project, task, agent identity and current lease.

### 2.5 Permission and policy engine
- Default deny. Evaluate exact action + resource + project + task + actor + requested data classification.
- Separate capabilities: `project.read`, `artifact.read`, `artifact.write`, `git.read`, `git.branch.write`, `test.run`, `provider.infer`, `external.publish`, `data.delete`, `production.deploy`.
- Risk classes: read-only, reversible project mutation, external side effect, destructive/high-impact.
- High-impact actions require explicit user approval bound to a canonical action digest, target and expiry. Approval cannot be reused for a different payload.
- Agents cannot grant permissions, edit policies, approve their own actions, or promote their own trust level.
- Policy/audit failure is fail-closed.

### 2.6 Context, memory and checkpoints
- Context assembler fetches only task-authorized, project-scoped facts; every item carries source, classification, provenance and freshness.
- Separate task working context from durable user-approved project memory.
- Checkpoints store state-machine position, bounded structured variables, tool invocation IDs and artifact IDs—not transcripts, credentials or raw prompt history.
- Memory writes require a typed fact, provenance, retention class and policy approval; untrusted content cannot silently become trusted memory.
- User export, retention and deletion semantics must be defined before persistent conversational memory ships.

### 2.7 Specialist delegation
- Planner creates a dependency DAG of small typed subtasks with owner role, expected output schema, budgets and acceptance criteria.
- Dispatcher enqueues specialist jobs transactionally and uses idempotency keys.
- Each specialist gets a distinct service identity and narrower capabilities than the coordinator.
- Delegation passes references to authorized artifacts, not copied unrestricted context.
- Coordinator validates specialist output against contract and evidence before integrating it.
- No concurrent writers to the same Git worktree/branch; use one-writer leases and fencing.

### 2.8 Prompt-injection and untrusted-data boundary
- User instructions, project policy, model output, tool output and external content are distinct typed trust classes.
- Documents, web pages, repository files, issue text, tool output and pasted content are data, never executable instructions by default.
- Strip or visibly flag invisible Unicode/bidi controls in text entering instruction-sensitive paths; preserve original bytes only as quarantined evidence when necessary.
- Tool output is length-bounded and wrapped as untrusted data in subsequent model requests.
- Security tests cover indirect prompt injection, instruction smuggling, path traversal, schema confusion and tool-output spoofing.

### 2.9 Privacy, telemetry and audit
- No IDE auto-connect, selection capture, shell history scan, environment-variable inventory, hostname collection or repository-content telemetry.
- Operational telemetry is opt-in/minimized and limited to opaque IDs, action/tool names, model/provider identifiers, durations, token/cost counters and normalized error codes.
- Never log prompts, completions, file contents, secrets, authorization headers, raw provider responses or sensitive URLs.
- Audit events are append-only metadata with tenant scope and integrity controls; user-facing export/deletion rules are explicit.

## 3. Trust boundaries and invariants

1. Model output is untrusted; it cannot invoke tools without schema validation and policy authorization.
2. Tools never inherit ambient credentials; adapters receive short-lived, narrowly scoped capabilities.
3. Project/tenant identity comes from authenticated server context, never from model output.
4. Lease token is a fencing capability; every state mutation checks current owner, token and unexpired lease.
5. Every side-effecting action has idempotency semantics or requires explicit non-retry handling.
6. All budgets are enforced by the runtime and cannot be raised by an agent.
7. No raw prompts, completions, credentials or file contents in logs/audit.
8. External content cannot alter system policy, tool definitions or permissions.
9. Unknown tool, malformed action, missing approval, stale lease or failed audit means deny.
10. Production deploys, live migrations, external publication, payments, destructive changes and credential/permission changes remain approval-gated.

## 4. Execution data contracts (target)

### Agent task
- `task_id`, `project_id`, `job_id`, `agent_role`, `objective_ref`
- `input_artifact_ids[]`, `allowed_tool_ids[]`, `model_policy_id`
- `turn_budget`, `tool_call_budget`, `token_budget`, `wall_clock_deadline`, `spend_ceiling`
- `acceptance_criteria[]`, `idempotency_key`

### Proposed action envelope
- `schema_version`, `action_id`, `kind`, `tool_name`, `tool_version`
- `arguments` (validated against tool schema)
- `expected_output_schema`, `reason_code` (short, non-sensitive)
- No free-form policy overrides, credentials or approval claims.

### Checkpoint
- `schema_version`, `state`, `iteration`, `budget_remaining`
- `pending_action_digest`, `tool_invocation_ids[]`, `artifact_ids[]`
- `updated_at`, `lease_generation`
- No raw transcript, prompt, completion or secret.

### Audit event
- `event_id`, `tenant/project/task/job/agent IDs`, `actor_kind`
- `action_kind`, `tool_id`, `resource_digest`, `decision`, `policy_version`
- `approval_id` when applicable, `duration_ms`, normalized `outcome_code`
- No content payloads or secret-bearing headers.

These are target contracts; migration/API compatibility must be designed and reviewed before implementation.

## 5. Task dependency graph and acceptance plan

| ID | Task | Dependencies | Acceptance evidence |
|---|---|---|---|
| AR-00 | Inventory current Rust runtime, adapters, queue, artifact API, auth and schema | none | exact path/symbol map; no guessed interfaces |
| AR-01 | Versioned task/action/tool/checkpoint contracts | AR-00 | serde schema tests; unknown-version rejection; size bounds |
| AR-02 | Tool registry + default-deny permission engine | AR-01 | allow/deny matrix; tenant/project/lease negative tests |
| AR-03 | Agent-loop state machine and budget ledger | AR-01, AR-02 | deterministic fake-model tests; turn/tool/time/token limits |
| AR-04 | Provider gateway interface and structured model adapters | AR-03 | protocol contract tests; malformed/oversized response tests; no secret leakage |
| AR-05 | Artifact-backed context and checkpoint persistence | AR-01, existing artifact API | restart/resume, hash integrity, tenant isolation, lease fencing |
| AR-06 | Prompt-injection and untrusted-data wrappers | AR-02, AR-03 | adversarial fixture suite; no policy/tool escalation |
| AR-07 | Specialist DAG planner and transactional dispatcher | AR-03, AR-05 | idempotent fan-out, bounded parallelism, dependency ordering |
| AR-08 | Sandboxed tool execution + Git worktree manager | AR-02, AR-05 | process/resource/network isolation, one-writer fencing, crash quarantine |
| AR-09 | Independent security review and operational hardening | AR-02..AR-08 | exact-head CI, threat-case tests, restart and chaos tests |
| AR-10 | UI/control plane for approvals, budgets and audit | AR-01..AR-09 | end-to-end approval digest binding, revocation and audit UX |

### Critical path
AR-00 → AR-01 → AR-02 → AR-03 → AR-04/AR-05 → AR-06 → AR-07 → AR-08 → AR-09 → AR-10.

Parallel work is allowed only for tasks with no shared file or contract conflicts. No implementation begins until AR-00 inventory and AR-01 contracts are reviewed and recorded.

## 6. Verification matrix

For every component, CI must cover:
- valid request and successful completion;
- malformed/unknown schema version and oversized payload;
- unauthorized project/tenant and revoked capability;
- stale/expired lease and concurrent duplicate invocation;
- provider timeout, malformed response, rate limit and ambiguous billable timeout;
- artifact corruption, unavailable object store and partial-write cleanup;
- worker process kill, restart, checkpoint resume and duplicate delivery;
- prompt injection in user-provided and retrieved external content;
- log/trace/audit canary scan for prompts, secrets and file contents;
- budget exhaustion, cancellation, deadline and kill-switch behavior.

A unit test or green compilation alone does not close a task. Each task needs exact commit SHA, CI run, relevant test names and observable integration evidence.

## 7. Resource and model allocation

- Strongest verified reasoning model: architecture, threat model, ambiguity resolution and independent red-team review.
- Coding model: bounded implementation tasks only after contract acceptance.
- Independent reviewer: security and acceptance coverage; must not be the implementation agent.
- Local Qwen fallback: non-sensitive code/test tasks only; no secrets or production data.
- Model selection is based on live protocol probes and observed workflow logs, not advertised names.
- Every task has token, wall-time and spend ceilings. No provider call without a configured budget.
- Provider spend is not authorized by this architecture document.

## 8. Explicit non-goals for the first runtime milestone

- No unrestricted autonomous shell or host filesystem access.
- No background scraping of user devices, IDEs, environment variables or git history.
- No hidden memory consolidation or telemetry.
- No self-approval or self-granted capabilities.
- No automatic production deployment, data migration, payments, external publication or destructive action.
- No claim of production readiness until the complete verification matrix passes.

## 9. Stop conditions

Stop and mark the task blocked if:
- tenant identity or authorization cannot be established;
- required policy/audit service is unavailable;
- model/provider capability is not protocol-probed;
- idempotency for a billable side effect is unknown;
- exact-head CI evidence is unavailable or stale;
- a tool requires ambient credentials or unrestricted host access;
- a task exceeds its approved budget;
- acceptance evidence cannot be produced without production access or owner-only approval.

