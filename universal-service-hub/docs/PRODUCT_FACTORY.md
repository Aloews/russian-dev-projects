# Autonomous Product Factory

## Purpose

Universal Service Hub is the shared engineering control plane for owned products. Product Factory turns a bounded product objective into implemented code, independent review, automated verification evidence and a pull request. It must deliver working user journeys, not a proposal, demo scaffold or claim based on compilation alone.

The first target is AUREL / Elite Estate in `Aloews/full`. The control repository `Aloews/russian-dev-projects` is private. The current workflow supports private target repositories using a fine-grained GitHub token and stores candidate/verification artifacts in the private Actions run. The first validation target is AUREL, but the platform architecture must remain product-agnostic; do not encode AUREL assumptions into shared orchestration, provider routing or verification.

## Execution architecture

Product Factory runs on GitHub-hosted Ubuntu runners. Railway Cloud Agent Tasks are not used.

- **Orchestrator:** `.github/workflows/product-factory.yml`. While the control workflow is still on its feature branch, a narrowly scoped push to that workflow file bootstraps a run without merging. After the workflow is merged into the default branch, the owner can also comment exactly `/factory run` on an issue or PR. The comment trigger is restricted to the repository owner and starts the default AUREL objective. `workflow_dispatch` is available after the workflow exists on the default branch.
- **Senior implementation authority:** Claude Code running Starimg `claude-opus-5` through the Anthropic Messages endpoint. A forced tool-call smoke test runs before the agent session; Opus owns implementation and verification repairs. Fable is not used in this workflow.
- **Local Qwen fallback:** Qwen2.5-Coder 3B through Ollama is invoked only when the Starimg tool-call probe or Opus implementation/repair fails. It produces the same bounded candidate artifact consumed by isolated verification.
- **Independent specialists:** read-only architecture reviewer (Claude Opus 5.5), security/privacy reviewer (Claude Sonnet 5.5), product/user-journey analyst (Claude Haiku 5.5), and independent post-change reviewer (Claude Opus 5.5).
- **Single writer:** Starimg Claude Opus 5 is the only agent allowed to modify product files. Specialist agents receive read-only tools.
- **Agent isolation:** implementation and repair sessions run on separate ephemeral jobs. The model receives repository file tools only; it has no shell, network, Git push, GitHub API, deployment or production-data tool. The shared Reality-First Business skill is injected into the agent session and every specialist profile. Project-level `.claude` and `.mcp.json` configuration is moved out of the agent workspace for the session and restored byte-for-byte afterward; untrusted project hooks/MCP configuration is not loaded. The model cannot modify `.github/workflows/**` or `.github/actions/**`; those controls are maintained separately.
- **Verification isolation:** each candidate is applied on a fresh runner. stack-aware checks for Rust, Python, Go, Gradle/Maven and Node, plus browser Playwright E2E for detected browser applications and applicable container builds run without the Starimg key or a write token in the test process environment.
- **Repair:** up to two bounded repair cycles run on new isolated agent jobs and receive the actual failed verification report. The complete gate set is rerun after each repair; tests are never weakened or skipped to manufacture a pass.
- **Publication isolation:** a separate fresh job receives only the verified patch artifact and the write token. It does not execute project code, package scripts or tests. It checks both the target branch SHA and PR base SHA before applying the patch, disables Git hooks, then creates or updates a PR. An existing working branch must already contain the current PR base; stale branches are rejected rather than silently rebased.
- **Release:** no automatic merge, production deployment, DNS change, paid infrastructure provisioning or production-data mutation.

The agent sessions use per-invocation Claude Code budget ceilings and turn limits: up to $35 for implementation and $20 for each repair cycle. These are local CLI ceilings, not a guarantee of Starimg billing equivalence. Starimg account-level spend limits remain authoritative; verify gateway pricing and budget accounting against the actual account.

## Required GitHub configuration

The latest workflow run passed the checks for both required repository Actions secrets and the target repository write permission. Keep the values in GitHub Secrets; never paste or print them. The required names are:

- `STARIMG_API_KEY`: the first Starimg API key for the Anthropic-compatible Messages endpoint.
- `PRODUCT_FACTORY_GITHUB_TOKEN`: fine-grained token scoped only to the target product repositories, with **Contents: read/write** and **Pull requests: read/write**. The workflow preflights target repository write access. The token is used by checkout and the isolated publication job; it is never passed to the model or verification commands.

Optional additional Starimg keys: `STARIMG_API_KEY_2` through `STARIMG_API_KEY_10`. The compatibility probe tries configured keys in order and selects the first one that successfully returns a valid forced `tool_use` response. Only the selected slot number and redacted error metadata are written to the run artifact; key values are never logged. Add these as GitHub Actions Secrets only if separate keys already exist in the Starimg account.

Optional repository Actions variables:

- `STARIMG_ANTHROPIC_BASE_URL`: gateway base URL. Defaults to `https://ai.starimg.ru`; verify the exact endpoint path and Anthropic Messages compatibility against the current Starimg account documentation.
- `STARIMG_MODEL`: defaults to `claude-opus-5`, matching the model identifier already used by Sherlock Scholes.

The Starimg key's presence is not proof of tool-call compatibility. The workflow now sends a real forced-tool-call probe to the configured Anthropic Messages endpoint, saves redacted diagnostics, and invokes Opus only if the probe returns a valid `tool_use` block. Inspect the run artifact, candidate patch and verification report before declaring compatibility. The Qwen fallback is bounded and does not silently substitute another hosted provider.

## Current activation blocker

The earlier OpenRouter diagnostic for run `43637640350` proved `OPENROUTER_API_KEY` was empty. That workflow therefore entered a slow local Qwen pass before trying its primary provider. The routing correction moves Starimg Opus ahead of Qwen and isolates the new acceptance run on `factory/elite-estate-rc4`. Its provider compatibility and delivery remain unverified until the new run produces diagnostics, a candidate artifact and passing verification.

## Standard run

1. Before merge, a change to `.github/workflows/product-factory.yml` on `feat/ush-autonomous-product-factory` starts the bootstrap run. After merge, the owner can use **Autonomous Product Factory** (`workflow_dispatch`) or comment `/factory run` to start the default AUREL task.
2. The runner validates the repository/branch allowlist and required credentials, checks target write access, fetches the selected branches and records both starting SHAs.
3. Opus inspects the real code, project map, delivery plan, tests and CI. Architecture, security and product specialists independently challenge scope and acceptance criteria.
4. Opus implements the bounded increment and requests read-only review of the actual changes. The runner exports a patch artifact; it does not commit or push from the agent job.
5. A fresh verification job applies that patch at the recorded SHA and executes the complete gate set. The report is retained as a 30-day workflow artifact.
6. If any gate fails, Opus receives the captured report in a new isolated job and may repair. The complete gate set is rerun; at most two repair cycles are allowed.
7. Only a candidate with all gates green reaches the isolated publication job. It rechecks both branch SHAs, pushes the candidate branch and creates or updates a PR. The PR is not merged or deployed.
8. If no candidate passes, the workflow fails and retains the available verification evidence. It does not publish an unverified candidate.

## First product: AUREL / Elite Estate

- Repository: `Aloews/full`
- Default working branch: `factory/elite-estate`
- Default PR base: `foundation/luxury-estate-platform`

The current branch already contains a Rust/Axum API, PostgreSQL persistence and row-level tenant isolation, evidence-backed property review/publication and a Next.js presentation layer. These are a technical foundation, not a production release. The current target does not yet have a Playwright configuration; the factory must add browser-level tests for the critical user journeys before a candidate can pass.

The factory must close the remaining blockers with evidence:

- Dedicated AUREL PostgreSQL instance and isolated credentials; never reuse Sherlock Scholes or Novel Forge production data.
- Real identity provider and JWT issuance; the current API's JWT validation contract is not a token-issuing identity system.
- Rate limits and abuse controls for public enquiries, email verification and a documented PII retention policy.
- Real licensed property supply, provenance and partner authorization; never fabricate inventory or relationships.
- Launch jurisdiction, legal entity and licensing/partner model.
- Staging deployment, browser end-to-end journeys and rollback rehearsal.

The factory may implement jurisdiction-neutral foundations while owner-only legal, partner, credential or infrastructure approvals remain pending. It must record those blockers rather than pretending to complete them.

## Durable evidence and operating rules

- Every product has one authoritative `docs/MAP.md` or explicitly configured equivalent.
- The project map records implemented behavior, exact checks, verified external connections and remaining blockers separately.
- Workflow artifacts are temporary evidence; durable architecture and delivery decisions belong in the product repository.
- Never include API keys, JWT secrets, database URLs, private keys, raw personal data or model credentials in prompts, logs, commits, PRs or artifacts.
- Review agents are read-only. One writer per run. Concurrency is serialized by target repository and branch.
- Agents may work only in allowlisted `Aloews/*` repositories and the selected working branch.
- No agent may merge, deploy production, alter production data, rotate credentials, change DNS, provision paid infrastructure or change billing.
- A run is not complete until the final PR head has green verification evidence and all product-specific release gates are satisfied.

## Product-agnostic verification

The target repository and objective are selected through workflow inputs. Agents infer the stack and domain from the actual project map and source; they must not impose AUREL's Rust/PostgreSQL architecture on other products. Current automated verification covers Rust, Python, Go, Gradle/Maven and Node projects. Playwright is required when a browser application is detected; non-browser Node services are not forced to install a browser stack. Other ecosystems require an explicit verifier adapter before the factory may claim coverage. The first acceptance target remains AUREL, but the orchestration and provider registry are shared.

## Cost and runtime limits

GitHub-hosted runner usage for private repositories is subject to the account's included Actions compute allowance and any overage billing. Starimg model inference is billed separately by Starimg. Configure provider-side spend limits before repeated runs; do not interpret a workflow's successful start as proof of remaining account balance.

The workflow is bounded by a six-hour agent-job timeout, a maximum of two repair cycles, Claude Code turn/budget limits and one active run per target repository/branch. Starimg Opus runs first after the compatibility probe; Qwen is a bounded fallback only when the probe or Opus agent fails.
