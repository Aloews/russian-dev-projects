# Russian Dev Projects — repository map

Last reviewed: 2026-11-14. This map describes the default branch only; inspect open PRs and other branches before assuming a project is present or production-ready.

## Repository scope
- Root README and `pages/`: public catalogue of independent developer projects.
- `universal-service-hub/`: separate backend/control-plane project. Its canonical engineering map is [universal-service-hub/docs/MAP.md](../universal-service-hub/docs/MAP.md).
- Product Factory / AUREL implementation was not identifiable as a current project directory in the inspected default-branch tree. Do not treat this catalogue or Universal Service Hub as proof that Product Factory/AUREL code is present here; inspect its PR, branch and repository before routing implementation work.

## Universal Service Hub
- Rust Axum/Tokio/SQLx control-plane migration with FastAPI retained as a temporary reference implementation.
- PostgreSQL migrations and tests are run by `.github/workflows/universal-service-hub-ci.yml`.
- No production deployment or live provider inference was verified in the current project map snapshot.
- Read the subproject map and `universal-service-hub/AGENTS.md` before editing.

## AI execution contract
- Keep each task on an isolated branch and PR. One writer per overlapping file set; reviewer and QA inspect the immutable PR head.
- Do not merge/deploy production, provision paid services, or alter secrets without explicit authorization.
- GitHub Actions CI is a test workflow, not an autonomous coding agent. Do not label it an agent fallback until trigger, model invocation, patch permissions and PR output are verified.
- Keep intermediate agent output in workflow logs/artifacts/PRs; user receives only verified outcomes or a concrete owner-only blocker.


## AI engineering toolchain — required project standard

This section is shared across the active project maps. A repository entry is not proof that a tool is installed or active.

### Caveman — concise agent output
- Upstream: https://github.com/JuliusBrussee/caveman
- Claude Code: `claude plugin marketplace add JuliusBrussee/caveman && claude plugin install caveman@caveman`
- Codex CLI: `npx -y github:JuliusBrussee/caveman -- --only codex` (or `npx skills add JuliusBrussee/caveman -a codex -g`).
- Use the correct project/user scope; restart/reload the host and verify discovery. Never compress code, identifiers, error messages, assertions, contracts, negations or acceptance criteria.

### codebase-memory-mcp — persistent code graph
- Upstream: https://github.com/DeusData/codebase-memory-mcp
- Supported host install (macOS/Linux): `curl -fsSL https://raw.githubusercontent.com/DeusData/codebase-memory-mcp/main/install.sh | bash`
- Restart the coding agent, verify the MCP server is visible, then explicitly index this repository.
- Keep its graph on a persistent developer/agent host. GitHub Actions runners and ephemeral Railway cron containers are not persistent memory hosts. Pin and verify release/checksum before production use.

### Spotify Portal AI Plugins — AiKA delegation
- Upstream: https://github.com/spotify/portal-ai-plugins
- Claude Code: `claude plugin marketplace add spotify/portal-ai-plugins`, `claude plugin install portal@portal`, `claude plugin install shunt@portal`.
- Codex: `codex plugin marketplace add spotify/portal-ai-plugins`, then install Spotify Portal from `/plugins`. Cursor uses its team marketplace.
- Start a new supported agent session and run `/portal:setup`; authenticate Portal CLI and run readiness diagnostics.
- Shunt is Claude Code-only. Verify AiKA modes `bulk-reader` and `code-writer` before relying on delegation. Never commit Portal credentials or local MCP config.

### Runtime compatibility and evidence
- These are coding-agent integrations, not generic Python/Node packages. A worker that only calls Starimg HTTP cannot load Claude/Codex plugins or call a local MCP server without that runtime and MCP client.
- Do not use ephemeral CI or cron containers as persistent MCP/Portal hosts.
- The Railway `stream-service` is a Python/Starimg worker; its current path has no verified Claude/Codex plugin host or MCP client.
- Track each independently as `not installed`, `installed`, `connected`, `indexed`, `verified`. Advance only after observing the exact runtime check.
- Last Fable/Starimg task request returned HTTP 402 before model execution. Do not retry billable generation until an authenticated test request succeeds.

### GitHub Actions fallback
- Use GitHub Actions as a fallback only when a repository-owned agent workflow, trigger, permissions, model invocation and draft-PR output are verified.
- Sherlock Scholes has `.github/workflows/local-autonomous-agent.yml`: owner-authored issue comments beginning `/ai ` trigger a GitHub-hosted runner with local Qwen2.5-Coder 3B. It is not Fable 5.2 or Starimg; it has a restricted patch allowlist and must pass repository checks before opening a PR.
- Do not assume other repositories have an autonomous fallback; inspect their workflows first.
- No automatic merge or production deploy. Review the exact PR head and verify deployment separately.

### Installation status — 2026-11-14
| Integration | Documentation | Active runtime |
|---|---|---|
| Caveman | Recorded here | Not verified |
| codebase-memory-mcp | Recorded here | Not installed/indexed on a persistent supported host |
| Spotify Portal / shunt | Recorded here | Not connected; Portal auth and AiKA readiness not established |


## Autonomous Product Factory (in progress; not merged)
- Implementation currently exists on branch `feat/ush-autonomous-product-factory`, draft PR #21: https://github.com/Aloews/russian-dev-projects/pull/21.
- Workflow: `.github/workflows/product-factory.yml`; bounded agent, separate verification/repair/publication jobs and target AUREL repository `Aloews/full`.
- Primary implementation model is Fable 5.2 through the Starimg Anthropic Messages endpoint. This is distinct from the Sherlock local Qwen GitHub Actions fallback.
- Latest inspected run #43594647745: configuration/credential validation and target checkout succeeded; `Run Fable 5.2 agent team` failed; verification, repair and publication jobs were skipped; no candidate artifact was published. The connector could not retrieve the full job log, so the exact denied tool/cause is not established here. Do not repeat the run until the failure is diagnosed.
- No merge or deployment has occurred. Treat the Product Factory as a draft workflow, not a verified autonomous production service.
