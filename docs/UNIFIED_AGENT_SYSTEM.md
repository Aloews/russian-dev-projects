# Unified agent system — Universal Service Hub

This project uses the shared evidence-first agent system in the repository root `.claude/`. It supplements, and never replaces, `/AGENTS.md`, `/universal-service-hub/AGENTS.md`, `universal-service-hub/docs/MAP.md`, the security roadmap and provider contracts.

## Roles
- `engineering-lead`: task DAG, scope, integration and exact-HEAD evidence.
- `rust-backend`: Rust/Axum/SQLx implementation and persistence.
- `durable-agent-runtime`: checkpoints, recovery, tool authorization, leases and bounded execution.
- `provider-integration`: official APIs, protocol compatibility, model IDs, health and fallback.
- `security-reviewer`: identity, tenant isolation, secrets, SSRF/DNS rebinding and supply chain.
- `data-integrity`: provenance, audit, persistence, idempotency and replay.
- `qa-adversarial`: independent attempts to falsify agent/runtime claims.

## Mandatory project constraints
- Follow the existing sequential Codex writer → Claude reviewer/repair workflow. No overlapping writers.
- Keep model clients protocol-correct: Codex uses OpenAI Responses; Claude Code uses Anthropic Messages.
- Discover and verify exact model IDs from the live catalogue and perform protocol-specific forced-tool probes before any permitted model execution.
- Do not add paid requests, new credentials, provider pool membership, infrastructure or production mutations without explicit authorization and a budget ceiling.
- A model probe, health endpoint, green CI, simulated tool call or report-only output is not proof of autonomous product execution.
- Prove real tool execution, durable observations, checkpoint persistence, restart recovery, authorization, audit integrity and failure handling before claiming autonomy.

## Commands and gates
Use `/verify`, `/finish-task` and `/security-gate`. Run `python3 scripts/validate_agent_system.py` to validate the repository-local agent files. The CI workflow runs the same structural check. This check validates configuration structure only; it does not prove Claude Code loaded agents or that agents executed.

Dispatch command: `/dispatch` (launches named specialist subagents when the Claude Code runtime exposes the Task tool; otherwise requires sequential work).
