# Agent execution contract

This directory contains the Universal Service Hub. Follow the repository-wide `/AGENTS.md` and the applicable `/.github/AGENTS.md` instructions in addition to this file.

Work autonomously within the permissions and safety boundaries below.

## Universal Service Hub project adapter

The Hub is a separate project adapter inside this monorepo, not permission to operate on every repository or every path.

- Canonical repository: `Aloews/russian-dev-projects`.
- Project root: `universal-service-hub/`.
- The executor's explicit USH scope must force this one repository even if a broader legacy repository list is configured.
- A task is eligible only when its issue identifies the Universal Service Hub in its title/body or carries the `project:ush` label.
- The executor must pin its working base to the branch named by `USH_BASE_BRANCH` and verify the branch resolves to a 40-character commit SHA before cloning.
- The scoped writer paths are limited to:
  - `universal-service-hub/src/`
  - `universal-service-hub/rust-migrations/`
  - `universal-service-hub/tests/`
  - `universal-service-hub/docs/`
  - `universal-service-hub/Cargo.toml`
  - `universal-service-hub/Dockerfile`
  - `universal-service-hub/README.md`
- This scope does not grant access to repository workflows, root files, `Cargo.lock`, environment files, secrets, credentials, production configuration or other projects. Do not expand the executor allowlist from a model-generated patch.
- Rust formatting and analysis use `universal-service-hub/Cargo.toml`, never another project's manifest:
  - `cargo fmt --manifest-path universal-service-hub/Cargo.toml -- --check`
  - `cargo clippy --manifest-path universal-service-hub/Cargo.toml --all-targets -- -D warnings`
  - `cargo test --manifest-path universal-service-hub/Cargo.toml --all-targets --no-run`
- PostgreSQL integration and migration tests require a disposable isolated database. Never point test commands at production or a shared live database.
- A candidate PR must target the pinned USH base branch. The existing parent PR remains the release path to the default branch; do not merge or deploy it automatically.

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
- Do not use OpenRouter or local Qwen as the default writer for Hub development. They remain separate fallback/legacy routes until a deliberate, tested migration changes that policy.

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
