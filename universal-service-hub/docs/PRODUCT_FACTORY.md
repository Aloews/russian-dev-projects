# Autonomous Product Factory

## Purpose

Universal Service Hub is the shared engineering control plane for owned products. Product Factory turns a bounded product objective into reviewed code, automated verification evidence and a pull request. It must deliver working user journeys, not a proposal, demo scaffold or claim based on compilation alone.

The first target is AUREL / Elite Estate in `Aloews/full`. Do not start other product implementations until this target's release gates are satisfied.

## Execution architecture

Product Factory runs on a GitHub-hosted Ubuntu runner. Railway Cloud Agent Tasks are not used.

- **Orchestrator:** GitHub Actions workflow `.github/workflows/product-factory.yml`; manual `workflow_dispatch` only. PR updates do not trigger paid model runs.
- **Primary agent:** Claude Code running Claude Fable 5.2 through the Starimg Anthropic-compatible Messages gateway.
- **Specialists:** read-only architecture reviewer (Claude Opus 5.5), security/privacy reviewer (Claude Sonnet 5.5), product/user-journey analyst (Claude Haiku 5.5), and independent post-change reviewer (Claude Opus 5.5).
- **Single writer:** Fable is the only agent allowed to modify product files. Specialist agents receive read-only tools.
- **Tool boundary:** the model session receives repository file tools only; it has no shell, network, Git push, GitHub API, deployment or production-data tool. This keeps provider credentials and write tokens out of model-accessible shell processes.
- **Verification:** a separate workflow step runs Rust and frontend checks after the model session has ended. It does not receive the Starimg key.
- **Repair:** up to two bounded repair cycles receive the actual failed verification report; tests are rerun after each repair.
- **Publication:** only after a complete verification pass, a separate step commits and pushes the candidate branch and creates or updates a PR.
- **Release:** no automatic merge, production deployment, DNS change, paid infrastructure provisioning or production-data mutation.

The agent session uses a per-invocation Claude Code budget ceiling and turn limit. Starimg account-level spend limits remain the authoritative billing guard; gateway pricing and budget accounting must be checked against the actual Starimg account.

## Required GitHub configuration

Add these repository Actions secrets in `Aloews/russian-dev-projects`:

- `STARIMG_API_KEY`: the Starimg API key for the Anthropic-compatible Messages endpoint.
- `PRODUCT_FACTORY_GITHUB_TOKEN`: fine-grained token scoped only to the target product repositories, with **Contents: read/write** and **Pull requests: read/write**. It is used by checkout and the final publish step; it is not passed to the model session.

Optional repository Actions variables:

- `STARIMG_ANTHROPIC_BASE_URL`: gateway base URL. Defaults to `https://ai.starimg.ru`; verify the exact endpoint path and Anthropic Messages compatibility against the current Starimg account documentation.
- `STARIMG_FABLE_MODEL`: defaults to `claude-fable-5-2`.
- `STARIMG_OPUS_MODEL`: specialist profile default is `claude-opus-5-5`.
- `STARIMG_SONNET_MODEL`: specialist profile default is `claude-sonnet-5-5`.
- `STARIMG_HAIKU_MODEL`: specialist profile default is `claude-haiku-5-5`.

A model name in a catalog is not proof that the selected gateway endpoint supports Messages API tool calls. The first real workflow invocation is the compatibility and authentication test. A failed provider preflight must stop the run; do not silently substitute a model or provider.

## Standard run

1. Owner starts **Autonomous Product Factory** manually and supplies the target repository, working branch, base branch and objective.
2. The runner validates the repository/branch allowlist, checks the required credentials, fetches the latest target branch and records its starting SHA.
3. Fable inspects the real code, project map, delivery plan, tests and CI. Architecture, security and product specialists independently challenge scope and acceptance criteria.
4. Fable implements the bounded increment and requests read-only review of the actual changes.
5. The runner executes the verification script. For a Rust workspace it runs formatting, Clippy with warnings denied, workspace tests and release build. For an npm project with a lockfile it runs `npm ci`, dependency audit, configured lint/test scripts and production build.
6. If any gate fails, Fable receives the captured report and may repair. The complete gate set is rerun; tests are never weakened or skipped to manufacture a pass.
7. A verified candidate is committed to the configured working branch and a PR is created or updated against the base branch. Verification logs are retained as a 30-day workflow artifact.
8. If both repair cycles fail, the workflow stops with the failing report. It does not publish an unverified candidate.

## First product: AUREL / Elite Estate

- Repository: `Aloews/full`
- Default working branch: `factory/elite-estate`
- Default PR base: `foundation/luxury-estate-platform`

The current branch already contains a Rust/Axum API, PostgreSQL persistence and row-level tenant isolation, evidence-backed property review/publication and a Next.js presentation layer. These are a technical foundation, not a production release.

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

## Cost and runtime limits

GitHub-hosted runner usage for private repositories is subject to the account's included Actions compute allowance and any overage billing. Starimg model inference is billed separately by Starimg. Configure provider-side spend limits before repeated runs; do not interpret a workflow's successful start as proof of remaining account balance.

The workflow is bounded by a six-hour job timeout, a maximum of two repair cycles, Claude Code turn/budget limits and one active run per target branch.
