# Autonomous Product Factory

## Purpose and scope

Universal Service Hub is the shared engineering control plane for owned products. Product Factory turns a bounded product objective into an implemented candidate, independent review, automated verification evidence and a pull request. It must deliver working user journeys, not a proposal, demo scaffold or claim based on compilation alone.

The first validation target is AUREL / Elite Estate in `Aloews/full`. The factory architecture must remain product-agnostic: it discovers each target repository's real stack, domain, security boundaries and deployment constraints. It must not impose AUREL assumptions on other products.

## Current workflow configuration

The canonical implementation is `.github/workflows/product-factory.yml`; this section describes the checked-in workflow configuration, not a claim that the latest run delivered a product.

- **Runner/orchestrator:** GitHub Actions on ephemeral Ubuntu runners. Railway Cloud Agent Tasks are not used.
- **Initial local team:** Qwen2.5-Coder 3B through Ollama runs a bounded multi-role pass and writes evidence-backed findings for the senior engineer. The workflow allows this step to fail without immediately aborting the run.
- **Primary hosted route:** OpenRouter Claude Sonnet 4.5, model ID `anthropic/claude-sonnet-4.5`, after a live Anthropic Messages tool-call compatibility check. The workflow sets a per-invocation budget ceiling of USD 15 for the primary implementation action.
- **Fallback:** local Qwen2.5-Coder 3B is attempted when the OpenRouter implementation action does not succeed. This fallback is not proof that the local model can complete every product task; candidate verification remains mandatory.
- **Specialists:** architecture, product, security and independent review roles are injected from `.github/product-factory/agents/`. Their tool access is read-only; the primary writer owns product-file edits.
- **Skill injection:** the canonical Reality-First Business skill is stored at `.github/product-factory/skills/reality-first-business/SKILL.md` and copied into the isolated agent workspace by the composite action. Do not assume the target repository already contains this skill.
- **Isolation:** model sessions receive repository file tools, not shell, network, Git push, GitHub API, deployment or production-data tools. Target `.claude` and `.mcp.json` configuration is temporarily removed and restored byte-for-byte. The model cannot modify `.github/workflows/**` or `.github/actions/**`.
- **Verification:** a fresh runner applies the candidate patch and runs stack-aware checks for supported Rust, Python, Go, Gradle/Maven and Node projects, with applicable migration, container and browser E2E checks. Verification does not receive model-provider or publication credentials.
- **Repair:** up to two bounded repair cycles receive the actual verification report. The complete relevant gate set is rerun; tests must not be weakened or deleted to obtain a pass.
- **Publication:** a separate fresh job receives only the verified patch artifact and scoped GitHub write token. It checks base and target branch SHAs and publishes a PR. It does not execute candidate code.
- **Release:** no automatic merge, production deployment, DNS change, paid infrastructure provisioning or production-data mutation.

## Provider and model truth

The Product Factory workflow currently uses OpenRouter and local Qwen; Fable/Starimg is not the primary Product Factory route in the checked-in workflow. Do not describe Fable as the current primary implementation model unless a later merged workflow and live run prove that change.

A catalogue listing, configured key, HTTP 200 or successful CLI process exit is not sufficient proof of tool compatibility. The live compatibility check must produce the expected Anthropic tool-use result. Record the exact model ID, endpoint/protocol, tool-call evidence and redacted failure category.

The workflow's `max_budget_usd` is a Claude Code invocation ceiling. It is not proof of actual OpenRouter invoice cost, remaining provider balance or account-level spend. Reconcile provider billing separately. GitHub runner compute is also a separate cost bucket.

**Routing limitation:** this is not yet a general task-aware adaptive model router. The checked-in factory workflow has a fixed OpenRouter Claude Sonnet 4.5 primary route and a local Qwen fallback. Per-task capability routing, provider-balance-aware selection and a durable cross-provider token/cost ledger are not yet implemented. The shared policy defines how those capabilities must be added; it is not evidence that they already exist.

## Agent roles and coordination

- **Coordinator:** owns task state, acceptance criteria, budget, dependencies and evidence reconciliation.
- **Product analyst:** validates user problem, journeys and scope; does not invent demand or market evidence.
- **Architect:** read-only inspection of existing architecture, data and interfaces.
- **Security reviewer:** read-only inspection of trust boundaries, authorization, privacy and abuse cases.
- **Implementation writer:** one writer for each overlapping file set.
- **Independent reviewer:** inspects the actual diff and surrounding code, not the writer's summary.
- **Verification runner:** deterministic checks in an isolated job.
- **Publisher:** receives only a verified patch and narrowly scoped write access.

Do not spawn agents merely to increase headcount. Parallelize only independent work. Preserve one accountable coordinator and a durable task DAG.

## Required GitHub configuration

The current workflow references these secrets:
- `PRODUCT_FACTORY_GITHUB_TOKEN`: fine-grained token scoped to target product repositories, with Contents read/write and Pull requests read/write. It is used for checkout/publication only, never passed to model agents or verification commands.
- `OPENROUTER_API_KEY`: used for the hosted Claude Sonnet route.

The workflow's local Qwen fallback downloads/runs Ollama and Qwen2.5-Coder 3B on the ephemeral runner. No Starimg secret is required by the current Product Factory workflow.

Never paste or print secret values. A secret's presence is not proof that the associated provider works.

## Standard run

1. Validate target repository and branch names, token permissions and the selected target/base branches.
2. Record exact base and working-branch SHAs.
3. Run the bounded Qwen multi-role review/implementation pass.
4. Probe OpenRouter Anthropic Messages tool use.
5. Run Claude Sonnet 4.5 as the primary hosted implementation writer if the probe succeeds.
6. Use local Qwen fallback if the primary implementation action fails.
7. Apply the candidate on a fresh verification runner and execute the complete applicable gate set.
8. If checks fail and the candidate is repairable, run a bounded repair with actual failure evidence; rerun the complete gate set.
9. Publish only a candidate that passes verification and SHA checks. Open or update a PR; do not merge or deploy.
10. Preserve failed logs and evidence. A failed or skipped candidate is not a product delivery.

## First validation product: AUREL / Elite Estate

- Repository: `Aloews/full`
- Current default working branch input: `factory/elite-estate-rc3`
- Current default PR base input: `foundation/luxury-estate-platform`

The target objective is a functional staging MVP, not a production release. The factory must inspect the current target repository and its own map before making product-specific assumptions. The target still requires evidence for identity/token issuance, public enquiry abuse controls, PII retention, licensed property supply/provenance, launch jurisdiction and legal/partner approvals, staging E2E and rollback. Never fabricate listings, partner relationships or approvals.

## Product-agnostic verification

The workflow must infer the target stack from source and manifests. It must not impose AUREL's Rust/PostgreSQL architecture on another project. Current verifier coverage is limited to explicitly implemented adapters; an unsupported ecosystem requires a verifier adapter before the factory may claim coverage. Browser products require critical-journey browser E2E before release readiness.

## Durable project evidence

Every target product must maintain:
- one authoritative project map (normally `docs/MAP.md`);
- local `AGENTS.md` generated from `.github/product-factory/templates/AGENTS.md` and filled with verified facts;
- architecture and significant decision records;
- API contract (OpenAPI) when HTTP APIs exist;
- setup/build/test instructions;
- security, privacy and data-lifecycle notes;
- operational and rollback runbooks;
- acceptance test matrix and known blockers.

The control repository's workflow artifacts are temporary evidence. Product-specific durable decisions belong in the target repository.

## Cost and stop policy

Track model input/output/cache/reasoning usage, tool calls, retries, elapsed time, runner/CI compute, review, repair and provider invoices separately. Optimize cost per accepted, verified increment rather than raw token price or agent count.

Use bounded attempts. After two consecutive attempts without measurable acceptance progress, change the task decomposition, context, model/effort, tool/runtime or verification strategy. Preserve the partial diff and failure evidence. Never infer provider balance or exceed an approved spend ceiling.

## Completion and release rules

A workflow run is not complete merely because an agent action or workflow job reports success. A candidate is publishable only when:
- requested acceptance criteria map to tests or observable evidence;
- required checks pass on the exact candidate SHA;
- independent review has no unresolved blocker;
- secret/path checks pass;
- project documentation is updated;
- limitations and owner-only blockers are explicit.

A PR is a verified candidate, not proof of production readiness. No automatic merge, production deployment, paid provisioning, production-data mutation, DNS change, billing change or credential rotation is permitted.
