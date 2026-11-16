# Modular model routing

This document defines the model/provider/tool switching contract for Universal Service Hub. It is an architecture specification, not a claim that all listed capabilities are implemented.

## Core separation

- Provider: account, endpoint, credential reference, balance and policy.
- Model: exact provider model ID and verified capabilities.
- Protocol adapter: request/response contract (OpenAI Responses, OpenAI Chat Completions, Anthropic Messages, Gemini, or modality-specific API).
- Agent tool: Codex CLI, Claude Code CLI, direct API worker or local inference runtime.
- Task: required capability, quality floor, privacy class, context, latency and spend ceiling.

Codex CLI is an OpenAI tool and must use a Responses-compatible model. Claude Code CLI is an Anthropic tool and must use a Messages-compatible model. Fable 5.2 is a Claude-family model and must be routed through Claude Code/Anthropic Messages. Astra is eligible for Codex/Responses only after its exact live catalogue ID passes the Responses function-call probe.

## Selection lifecycle

1. Fetch the authenticated provider catalogue.
2. Normalize model IDs and metadata without guessing capabilities from names.
3. Probe required protocol and tool calling; mark the candidate eligible only after success.
4. Filter by project allowlist, task capability/modality, context, privacy/data residency, health and budget.
5. Choose the least expensive verified candidate meeting the task quality floor.
6. Escalate on objective quality failure, material independent-review finding, repeated failed check or high-risk ambiguity.
7. Require an approved spend ceiling and sufficient verified unreserved provider/user balance before premium dispatch.
8. Persist a route decision and usage reconciliation record; redact secrets and sensitive prompt content.

The planning phase has a separate higher reasoning budget. Codex CLI creates the task DAG and implementation contract; Claude Code independently challenges it before coding. Implementation and review follow only after the plan gate passes.

## Budget and quality invariants

- Provider credits and user credits are separate. Neither may be treated as the other's funds.
- A model coefficient is not an account balance or a budget approval.
- A missing/stale balance snapshot is not sufficient funds.
- Reserve estimated spend before dispatch and reconcile actual usage afterward.
- Never exceed the task ceiling or silently switch a billable task to another provider.
- Quality escalation must be tied to measurable acceptance/evaluation evidence, not branding or model confidence.
- Premium models are optional and task-specific, not universal defaults.

## Catalogue lifecycle

Model additions, changed coefficients, context changes and deprecations trigger metadata refresh and required re-probes. Unknown models remain quarantined from private project data and secrets until provider identity, protocol, modality, pricing, data handling and safety are verified.

Multimodal capabilities are separate: a model shown in a provider UI does not prove an accessible API endpoint. Image, video, audio, music and embeddings require documented request/response schemas and their own contract tests.

## Status

- CLI/model protocol correction: implemented in the workflow branch; exact-head CI and a real provider run are still required.
- Dynamic authenticated catalogue discovery: present in the workflow; classification beyond model IDs is incomplete.
- Automatic provider/user balance integration: not implemented.
- Persistent model capability registry and admin switching UI: not implemented.
- Objective quality-triggered escalation: policy specified; runtime integration not implemented.


## Task-specialized routing and adaptive collaboration

Read [MODEL_SELECTION_RESEARCH.md](./MODEL_SELECTION_RESEARCH.md) for the provider-by-provider task specialization review, multi-agent collaboration evidence, token economics, stop/re-plan triggers and AR-01—AR-11 implementation DAG.

The router must treat these as independent dimensions:
- execution tool: Codex CLI, Claude Code CLI, direct API worker or local runner;
- provider/account and protocol adapter;
- exact model ID/version and verified capabilities;
- task profile, reasoning effort and collaboration topology;
- spend ceiling, token budget, latency and stop policy.

### Collaboration default

Use one coordinator with a durable DAG, one writer per overlapping file set, bounded specialist tasks and deterministic verification. Parallelize only independent work. Use a separate model/provider for critical review when it adds meaningful independence. Avoid unbounded debate and duplicate full-repository reviews.

### Quality and progress control

Optimize cost per accepted task, not token price alone. Use a low-cost verified attempt where checks are deterministic; run tests/validators; escalate only failed criteria with a stronger model, higher effort or a changed strategy. A model change is not progress unless objective evidence improves.

Stop and re-plan after two bounded attempts with no acceptance-criterion progress, repeated identical failure, malformed/tool-incompatible outputs or spend that threatens verification/repair. Preserve the diff and failing evidence; change decomposition, context, prompt, model/effort, tool, tests or collaboration topology. Never just increase tokens and repeat.

### Usage accounting

Record input, output, cached-input, reasoning, tool and retry usage where provider telemetry exposes it. Store price snapshots, estimated/actual cost, elapsed time, attempts, task profile, model/provider/protocol, test results and acceptance outcome. Do not infer exact cost from token count when price or cache metadata is unknown. Keep provider and user balances separate and reserve spend before dispatch.

Initial engineering allocation bands are configurable: planning 20–25%, implementation 40–45%, verification/review 15–20%, repair/model-switch reserve 15–20%. They are policy defaults, not measured costs. The runtime must enforce a hard total ceiling and preserve repair reserve.

### Current evidence boundary

The research and architecture are documented. The workflow now includes explicit no-progress/token discipline and a Codex verification/recovery pass after Claude's independent review. The runtime still does not enforce persistent per-model token accounting, dynamic cost-per-accepted-task routing, balance reservations or automatic strategy switching. Those remain implementation work under AR-01—AR-11.
