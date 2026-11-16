# Agent execution contract

This directory contains the Universal Service Hub. Follow the repository-wide `/AGENTS.md` and the applicable `/.github/AGENTS.md` instructions in addition to this file.

Work autonomously within the permissions and safety boundaries below.

## Required workflow

1. Inspect repository state, open PRs, project map, tests, architecture and deployment configuration before editing.
2. Convert each task into explicit user outcomes, acceptance criteria and a bounded file scope.
3. Assign one writer per overlapping file set. Independent reviewer and QA must inspect the immutable candidate commit.
4. Run formatting, static checks, unit tests, integration tests, migration tests and container build as applicable.
5. Fix root causes and rerun all affected checks on the exact final commit SHA.
6. Record provider/API evidence, versions, limitations and reproducible commands.
7. Open or update a PR with test evidence. Do not merge or deploy production merely because CI is green.
8. Keep intermediate reports, test output, agent findings and proposed fixes inside the engineering workflow (PR checks, review notes, artifacts or project docs). The coordinator must inspect, reconcile and act on them. Do not send routine intermediate reports or decision requests to the user. Contact the user only for a genuinely unavoidable permission, credential, budget or product decision; continue independent work meanwhile.

## Provider integration rules

- Prefer official APIs and documented SDKs.
- Verify current model names, API compatibility, quotas, pricing and regional availability from primary sources.
- Never infer successful authentication from a configured key or a successful models-list request.
- Validate with a real, minimal sandbox request where permitted; redact all secrets and request payloads.
- Do not add a provider to the healthy/fallback pool until contract and failure-mode tests pass.
- Keep vendor-specific code behind an adapter interface.
- No plaintext secrets in source, issues, logs, fixtures or generated reports.
- Codex CLI uses the OpenAI Responses protocol; Claude Code uses Anthropic Messages. Never route a model across protocol families based only on a marketing name.
- Exact model IDs must be discovered or explicitly configured and validated against the live catalogue. A successful tool-call probe proves only protocol compatibility, not engineering quality or provider reliability.

## Safety boundaries

- Do not change existing production applications, databases, DNS, billing, or secrets as part of Hub development.
- No autonomous paid resource creation. Require explicit budget ceilings and an approved account/project scope.
- No production deploy without staging verification, independent QA, health checks and a tested rollback.
- If a credential, permission, quota or external approval is missing, continue all independent work and record the exact blocker; do not ask the user to perform routine debugging.
- Never claim a task, test, deployment, failover or provider connection succeeded without direct evidence.

## Temporary external bootstrap workflow (not Hub self-hosting)

- Until the Hub API passes its self-hosting acceptance gate, the dedicated GitHub Actions workflow Universal Hub — Codex + Claude Code may be used only as a bounded bootstrap runner to implement missing API/worker capabilities. It is not the permanent engineering path.
- Codex CLI through Starimg's OpenAI Responses API is the primary implementation writer.
- Claude Code CLI through Starimg's Anthropic Messages API is the sequential independent reviewer and repair writer. Planning through Claude Code must also use an Anthropic Messages-compatible model; do not silently pass an OpenAI-only model to that client.
- The workflow discovers exact model IDs from the live Starimg model catalogue and verifies forced tool calling on both protocols before starting either CLI.
- Keep the two coding agents sequential; never let them write overlapping files concurrently.
- Both agents may change only `universal-service-hub/`. They must not read or expose secrets, commit, push, create PRs, merge, deploy or mutate production data.
- GitHub CI validates the exact PR head. A passing model probe is not proof that code is correct; a passing build is not proof of production readiness.
- Prefer the configured Starimg route only after a fresh live probe. Do not silently switch provider or model. Local Qwen is a separately labelled fallback and may run only after its sandbox, path, secret-isolation and task-quality gates pass.

## API and documentation contract

- Maintain OpenAPI as the source contract for implemented HTTP endpoints when the API surface is ready to be specified.
- Swagger UI must be generated from the same OpenAPI document; never maintain a second, contradictory endpoint description.
- Do not document routes, schemas or capabilities that are not implemented and verified.
- Keep architecture decisions, operational runbooks, security controls and current state in version-controlled docs. `docs/MAP.md` is the current project-state index.
- Confluence is not currently the canonical source of truth for this repository. Do not assume it is synchronized.

## Verification and Definition of Done

- Map each acceptance criterion to a reproducible test or observable check.
- Record exact repository, branch, commit SHA, CI run, test command, result and known limitations.
- Distinguish provider probe, code correctness, integration readiness and production readiness.
- Mark a task `VERIFIED` only with evidence from the exact candidate commit. Otherwise use `IN PROGRESS`, `BLOCKED` or `NOT VERIFIED`.
- A task is complete only when the intended user outcome is delivered, relevant checks pass, the diff is reviewed, documentation reflects reality and all applicable security/release gates are satisfied.


## Self-hosting development contract

### Current boundary

The Rust API currently exposes project/token management, provider registry/model discovery, health checks, integration verification, provider selection, `/v1/ai/chat` and audit reads. `/v1/ai/chat` accepts text messages and returns text; it does not expose tool calls, structured tool execution, repository workspaces, durable engineering tasks, checkpoints, review gates or patch publication. The current OpenAI-compatible adapter calls `/chat/completions`, not OpenAI `/responses`; the Anthropic adapter parses text blocks and does not execute `tool_use`. Therefore the current Hub cannot yet build itself through its API.

### Bootstrap phase — temporary exception

Until the self-hosting acceptance gate is met, a bounded external bootstrap runner may implement the missing API/worker foundations. It must use the existing repository rules, a dedicated branch, no production secrets, no paid provisioning and exact-SHA CI. Label the work as external bootstrap; never record it as a Hub-executed task. Direct GitHub file editing is allowed only for this bootstrap/documentation/repair boundary.

### Required self-hosting architecture

1. **Provider protocol layer:** support OpenAI Responses (including function-call input/output and continuation), Anthropic Messages (including `tool_use` / `tool_result`) and compatible text-only chat as distinct adapters. Preserve exact provider/model IDs and raw protocol family; never translate a Responses-only model into Chat Completions by name.
2. **Durable task API:** authenticated, project-scoped create/read/approve/pause/resume/cancel/retry endpoints; idempotency keys; immutable task input; explicit budget ceiling; durable state transitions, attempt records, leases, heartbeats and checkpoints in PostgreSQL.
3. **Worker and workspace:** isolated per-task worktree/container, pinned base SHA, allowlisted filesystem and typed tools, bounded CPU/memory/time/output, no ambient secrets, controlled network egress, cancellation and restart recovery. Do not run arbitrary model-generated shell in the API process.
4. **Evidence pipeline:** candidate diff and artifact hashes, exact commands and exit codes, test results, provider/model/usage/latency, independent review, security scan and exact-SHA CI. Agent-authored summaries are not verification evidence.
5. **Publication boundary:** only a separate publisher with narrowly scoped GitHub write permission may create a branch/PR after policy and verification gates pass. It cannot execute candidate code. No auto-merge or production deploy.
6. **Dogfooding transition:** first execute one non-destructive documentation/test task against the Hub itself via its running API; then a bounded source-code task with a passing test; then prove worker restart/resume, duplicate-request idempotency, cross-project denial, provider failure handling and exact-SHA publication. Only after all pass may external bootstrap editing be disabled for ordinary work.

### Provider diagnosis rules

- The failure before PR #56 was a Hub workflow routing defect: a Fable/Claude model was selected for the OpenAI Responses endpoint, causing HTTP 400. PR #56 corrected model-family selection in the external CLI workflow; that change does not add Responses support to the Rust Hub API.
- A direct Starimg `/v1/responses` or `/v1/messages` probe is not proof that the Rust Hub adapter works. Verify each adapter through the running Hub API with a forced tool-call request and inspect the normalized result.
- Never label Starimg unavailable solely because the Hub's current `/chat/completions` adapter cannot use a Responses-only model. Diagnose endpoint/protocol/model/credential/quota independently.
- Keep Starimg as the preferred cloud route when the exact model ID and protocol pass live probes. Use the local Qwen runner only as a separately identified, bounded fallback after its sandbox and quality gates pass. No silent provider switching.

### Required workflow after transition

Every Hub source change starts as a durable task submitted to the Hub API. The worker uses registered provider adapters and typed tools; no agent receives GitHub publication credentials. The coordinator reviews task evidence and approves publication. If the API is unavailable, preserve the task and stop; do not fall back to direct GitHub file writes without a recorded break-glass bootstrap decision.


## Local inference fallback

- Codex CLI through Starimg Responses and Claude Code through Starimg Anthropic Messages remain the preferred sequential cloud path when both protocol probes pass.
- If cloud preflight, CLI installation, Codex execution or Claude review fails, the Hub workflow may invoke the bounded local Ollama/Qwen fallback in the same runner workspace. A fallback run must be explicitly labelled as local; never report it as Codex/Claude work.
- Local Qwen is one local inference session with role-separated instructions, not a multi-agent team. It may edit only allowlisted Hub source, tests and documentation. It receives no Starimg, fal.ai, GitHub publication or other provider secrets.
- Local model output is a candidate only. It must pass path/symlink and secret-pattern checks, `git diff --check`, exact-head CI and independent review. Never auto-merge or deploy it.
- The ephemeral GitHub-hosted runner is a temporary fallback, not proof that a persistent owner-operated GPU runner is connected. A self-hosted runner may be selected only through an explicitly configured runner-label variable and must pass the same controls.
- Ollama endpoints are restricted to loopback or explicitly allowlisted private hosts after DNS resolution. Never expose the local inference endpoint publicly.
