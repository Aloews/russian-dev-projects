# Agent execution contract

This directory contains the Universal Service Hub. Work autonomously within the permissions and safety boundaries below.

## Required workflow

1. Inspect repository state, open PRs, project map, tests and deployment configuration before editing.
2. Convert each task into explicit acceptance criteria and a bounded file scope.
3. Assign one writer per overlapping file set. Independent reviewer and QA must inspect the immutable candidate commit.
4. Run formatting, static checks, unit tests, integration tests and container build.
5. Fix failures and rerun all affected checks on the exact final commit SHA.
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

## Safety boundaries

- Do not change existing production applications, databases, DNS, billing, or secrets as part of Hub development.
- No autonomous paid resource creation. Require explicit budget ceilings and an approved account/project scope.
- No production deploy without staging verification, independent QA, health checks and a tested rollback.
- If a credential, permission, quota or external approval is missing, continue all independent work and record the exact blocker; do not ask the user to perform routine debugging.
- Never claim a task, test, deployment, failover or provider connection succeeded without direct evidence.


## Preferred coding-agent workflow

- For changes to the Universal Service Hub itself, use the dedicated GitHub Actions workflow Universal Hub — Codex + Claude Code.
- Codex CLI through Starimg's OpenAI Responses API is the primary implementation writer.
- Claude Code CLI through Starimg's Anthropic Messages API is the sequential independent reviewer and repair writer. It must inspect the actual diff, not accept the Codex summary.
- The workflow discovers exact model IDs from the live Starimg model catalogue and verifies forced tool calling on both protocols before starting either CLI.
- Keep the two coding agents sequential; never let them write overlapping files concurrently.
- Both agents may change only universal-service-hub/. They must not read or expose secrets, commit, push, create PRs, merge, deploy or mutate production data.
- GitHub CI validates the exact PR head. A passing model probe is not proof that code is correct; a passing build is not proof of production readiness.
- Do not use OpenRouter or local Qwen as the default writer for Hub development. They remain separate fallback/legacy routes until a deliberate, tested migration changes that policy.
