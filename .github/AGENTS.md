# GitHub automation instructions

These instructions apply to workflows, composite actions, scripts and agent profiles under `.github/`. Follow the repository-root `AGENTS.md` as well.

## Control-plane safety

- Treat GitHub Actions, tokens, workflow permissions, artifacts and publication steps as security boundaries.
- Pin newly added or changed third-party actions by immutable commit SHA. Treat existing mutable action references as legacy debt and migrate them in a separately verified dependency-update change.
- Grant the minimum permissions per job; do not expose write tokens to model sessions, build/test commands or untrusted project code.
- Keep implementation, verification and publication in separate jobs/runners where practical.
- Never execute candidate code in the publication job.
- Validate target repository, branch, PR base and exact SHAs before applying or publishing a candidate.
- Serialize writers by target repository and branch. Do not cancel useful in-progress work by default.
- Keep secrets in GitHub Secrets or an approved secret manager; never echo them or upload them in artifacts.
- Artifacts must be scoped, redacted, size-bounded and retained only as long as operationally necessary.

## Workflow changes

1. Inspect every caller, trigger, permissions block, concurrency rule, job dependency, artifact boundary and downstream consumer.
2. Preserve existing workflow inputs and behavior unless the task explicitly changes them.
3. Validate YAML and expressions, shell syntax, Python syntax, action metadata and workflow semantics.
4. Test both success and failure paths: missing credentials, invalid repository/branch, rejected patch, failed tests, timeout, cancellation, retry, stale SHA and publication denial.
5. Never treat a successful workflow parse or model call as evidence of product delivery.
6. Do not add paid provider calls to ordinary CI unless an explicit budget and safe test policy authorize them.
7. Do not enable experimental CLI modes unless the exact CLI version, runtime, provider endpoint and account configuration pass a live compatibility probe.

## Product Factory boundaries

The Product Factory must be product-agnostic. It may discover a target stack and invoke the appropriate verifier, but it must not impose AUREL-specific architecture, credentials, database policy or deployment assumptions on other products.

The canonical shared engineering contract is `.github/product-factory/AGENTS.md`, supported by the Reality-First Business skill at `.github/product-factory/skills/reality-first-business/SKILL.md` and the target template at `.github/product-factory/templates/AGENTS.md`. Composite actions must inject these into isolated agent sessions; prompts must not assume that the target repository already contains them.

Only the isolated publication job may receive the scoped GitHub write token. Model agents and verification jobs must not receive it. No automatic merge or production deployment is allowed.

## Evidence

For each workflow change, preserve exact commit/run IDs, completed job conclusions, relevant step results and redacted diagnostics. A skipped or `continue-on-error` step is not a successful capability. Explicitly distinguish provider probe success, candidate creation, verification, PR publication and product release.


## Hub self-hosting and bootstrap boundary

- The dedicated `universal-hub-codex-claude.yml` workflow is an **external bootstrap runner**, not the Hub's self-hosting execution path. It may be used only to implement missing bootstrap capabilities until the Hub API passes its dogfooding acceptance gate.
- Do not describe a direct Codex/Claude CLI run, GitHub API file edit, or successful PR as a task executed by the Hub.
- Once the Hub engineering-task API and sandboxed worker are verified, route Hub implementation requests through that API. The external workflow may then only perform source checkout, invoke the Hub API, collect redacted evidence, run independent GitHub CI and publish the already-verified patch.
- Keep provider probes separate: direct Starimg CLI compatibility, Hub provider-adapter compatibility, and full task execution are three different tests. A pass in one does not prove the others.
- Never make the Starimg coding workflow depend on unrelated fal.ai/video credentials. Each provider secret must be required only by a task that actually calls that provider.
