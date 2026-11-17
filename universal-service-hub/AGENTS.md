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

## Preferred coding-agent workflow

- For changes to the Universal Service Hub itself, use the dedicated GitHub Actions workflow Universal Hub — Codex + Claude Code.
- Codex CLI through Starimg's OpenAI Responses API is the primary implementation writer.
- Claude Code CLI through Starimg's Anthropic Messages API is the sequential independent reviewer and repair writer. Planning through Claude Code must also use an Anthropic Messages-compatible model; do not silently pass an OpenAI-only model to that client.
- The workflow discovers exact model IDs from the live Starimg model catalogue and verifies forced tool calling on both protocols before starting either CLI.
- Keep the two coding agents sequential; never let them write overlapping files concurrently.
- Both agents may change only `universal-service-hub/`. They must not read or expose secrets, commit, push, create PRs, merge, deploy or mutate production data.
- GitHub CI validates the exact PR head. A passing model probe is not proof that code is correct; a passing build is not proof of production readiness.
- Cloud Codex through Starimg remains the preferred writer/reviewer path when available. If Starimg preflight, CLI installation, Codex or Claude fails, the workflow may invoke the bounded local Ollama/Qwen fallback in the same runner workspace.
- Local Qwen is a single local inference session with role-separated instructions, not four independent agents. It may edit only the explicitly allowlisted Hub source/test/docs paths; it cannot edit workflows, dependency manifests, credentials, deployment files or database migrations.
- The local model receives no cloud-provider secrets. It must preserve the existing candidate diff, produce a review artifact, pass path/secret/diff checks, and then pass the exact-head repository CI. A Qwen self-review is not independent QA; deterministic tests and the CI gate remain mandatory.
- The workflow can target a self-hosted runner through the repository variable `USH_DEV_RUNNER_LABELS` (JSON array of runner labels). Without that variable it uses `ubuntu-latest`; this default is local inference on an ephemeral GitHub runner, not proof that a dedicated owner-operated GPU host is connected.
- Never silently bypass a failed quality gate or automatically merge/deploy a local-model candidate.

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


## Adaptive multi-agent execution

- The Hub coding workflow enables Codex subagent orchestration and provides four narrow roles: codebase exploration, application security, provider/API contracts and QA/test analysis.
- Use two or three read-only specialists by default; activate a fourth only when it has a distinct, decision-relevant workstream. Maximum four concurrent subagents, nesting depth one.
- The primary Codex session is the sole writer and integrator. Subagents return evidence-backed findings only; do not permit overlapping writes or duplicate broad scans.
- Keep the selected, verified Starimg-compatible model as the default subagent model unless the live catalogue and protocol checks establish a cheaper compatible model with sufficient capability. Do not guess model quality or pricing from names.
- The primary agent must consolidate findings, implement one coherent vertical slice, and request a read-only diff review before the sequential Claude review. If subagent orchestration is unsupported by the selected model, continue safely with one writer and record the limitation.
- A checkpoint step captures the candidate patch and status after the Codex phase, including when that phase fails or times out. The checkpoint is uploaded as a short-lived Actions artifact only after a basic secret-pattern scan. It is recovery evidence, not a validated deliverable.

- **Isolation limitation:** the read-only boundary for Codex subagents is currently an instruction, not a separately enforced filesystem permission. Subagents inherit the parent session's sandbox. The primary must inspect the complete diff and reject unintended edits; never describe these roles as technically sandbox-isolated until a tested enforcement mechanism is added.


## Verified autonomous continuation skill

For PR status monitoring, CI completion, verification gates, and continuing planned engineering work without routine user confirmation, load and follow `.agents/skills/verified-autonomous-continuation/SKILL.md`. This skill authorizes continuation only within the existing repository/task scope after all required checks pass on the exact current head SHA. It does not authorize merge, deployment, production mutation, paid provider/resource use, credential changes, or other approval-gated actions.
