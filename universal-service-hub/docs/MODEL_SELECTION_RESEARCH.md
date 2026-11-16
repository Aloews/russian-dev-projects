# Model specialization, multi-agent engineering and adaptive token economics

**Research snapshot:** 2026-11-16  
**Status:** architecture and implementation requirements; not proof that every vendor model is exposed by Starimg or enabled on this account.

## Executive decision

Universal Service Hub must be a task-aware, evidence-driven model router and bounded engineering orchestrator, not a fixed list of favorite model names and not a swarm of agents that repeat the same work.

The system chooses independently:
1. execution tool/agent runtime (Codex CLI, Claude Code CLI, direct API worker, local runtime);
2. provider/account and protocol adapter;
3. exact model ID and version;
4. capability profile and reasoning/effort level;
5. task strategy and collaboration topology;
6. budget, timeout, retry and stop policy.

A model name is not evidence that it supports a protocol, tool calling, structured output, modality or endpoint. Live account catalogue discovery, official provider documentation and exact protocol/capability probes are separate evidence requirements.

## 1. Model specialization by task

### OpenAI

OpenAI's official model-selection guide separates high-end ambiguous/deep-analysis work, cost/performance complex work, visual reasoning/computer use and efficient scoped tasks. It recommends comparing models on identical inputs, holding reasoning effort constant, and evaluating code changes with regression tests and rendered desktop/mobile interfaces. Availability, tools and limits vary by product and version, so the current API catalogue is authoritative.

Hub profiles:
- Frontier planning and ambiguous architecture: strongest verified reasoning model; high reasoning effort only when plan quality requires it.
- Complex implementation/refactor: coding-capable model with verified CLI/API tool use, sufficient context and strong repository-task evaluation.
- Visual UI/computer use: explicit vision/screen interaction support; text-only coding ability is insufficient.
- Triage/extraction/small edits: efficient low-latency model with deterministic verification.
- Embeddings, moderation, audio, image generation: dedicated task model/API, not a general chat model unless its official contract supports the operation.

### Anthropic

Anthropic's current official guidance differentiates:
- Claude Fable 5.2: demanding reasoning and long-horizon agentic work; escalation when evaluations on Opus at higher effort still fall short.
- Claude Opus 5.5: long-running agentic coding and knowledge work.
- Claude Sonnet 5.5: speed/intelligence balance for everyday coding and agent work.
- Claude Haiku 5.5: high-volume, latency-sensitive classification, extraction and routing.

Effort tuning is often a better first lever than changing models. Anthropic recommends efficiency-first for high-volume/checkable tasks and capability-first for unusually complex work. Vendor benchmarks are directional, not a guarantee for our codebase.

### Google Gemini

The Gemini API catalogue separates general reasoning/coding from specialized modality/task APIs:
- Pro family: complex reasoning, coding and difficult problem solving.
- Flash family: high-throughput, responsive multimodal and agentic work; the current catalogue describes a Flash generation for long-horizon software engineering and autonomous agents.
- Flash-Lite: low-cost, high-volume, simple/latency-sensitive tasks.
- Image generation/editing: dedicated image models (Nano Banana family in current docs).
- Video generation: Veo family.
- Speech/realtime voice: Live, TTS and transcription endpoints.
- Embeddings: text/multimodal embedding models.
- Deep Research, computer use and robotics: specialized agent/domain endpoints with separate access and API constraints.

A model shown in a consumer Gemini app is not automatically available through the Gemini API. The API catalogue, exact endpoint, account access and request/response contract are authoritative.

### Cross-vendor implication

The registry must distinguish vendor family, model identity, protocol and task capability. Names such as GPT, Claude, Gemini, DeepSeek, Kimi, Qwen, MiniMax or Grok are not task profiles. The same family exposed through first-party API, cloud marketplace or aggregator may have different IDs, quotas, prices and feature parity.

## 2. Collaboration patterns

Use a hierarchical, centralized coordinator with explicit durable state and bounded specialist workers as the default for product engineering.

| Pattern | Appropriate use | Main failure/cost | Hub policy |
|---|---|---|---|
| Single specialist | Small sequential well-scoped change | Context limits or blind spots | Default for small work |
| Planner → implementer → verifier | Most feature work | Handoff/context loss | Default engineering workflow |
| Independent parallel investigators | Separate failure hypotheses, modules, sources or tests | Duplicate work and synthesis overhead | Only when DAG proves independence |
| Independent red-team review | Security, architecture and critical criteria | Reviewer may share model bias | Use a different model/provider when justified |
| Debate / consensus | Ambiguous high-impact decision with evidence rubric | Repeated opinions and token explosion | Rare; fixed round limit |
| Manager + subagents | Large codebase exploration or disjoint modules | Coordination/context overhead | Isolated scopes; one writer per file |
| Local model worker | Routine offline/low-sensitivity work or degraded mode | Capability/quality limits | Only after task-specific evaluation |

OpenAI's multi-agent documentation explicitly warns that subagents increase token use and are not beneficial for sequential tasks, shared mutable state or work dominated by one slow external operation. Strong use cases are independent codebase exploration, research, comparisons, test suites and separate failure hypotheses.

The 2026 orchestration survey emphasizes explicit state, bounded control flow, failure recovery and task-aligned evaluation. Coordination can cause duplicated work, contradictory outputs and inconsistent state. Agent count is not a quality metric.

### Hub collaboration contract

- One accountable orchestrator owns the task DAG and durable state.
- One writer owns each overlapping file set.
- Specialists receive a bounded task, relevant files and explicit output schema; do not send the whole repository to every worker.
- Parallel workers must have independent inputs/outputs and no shared mutable writes.
- A verifier checks artifacts and tests, not the worker's confidence.
- The coordinator reconciles conflicting findings against repository evidence and acceptance criteria.
- Every handoff contains task ID, findings, changed paths, evidence, unresolved risks and next action.
- Debate has a maximum number of rounds; stop when no new evidence or decision-relevant information is added.

## 3. Adaptive routing and token economics

### Optimize cost of a verified result

Do not optimize only for cost per input token. Optimize:
- cost per accepted task and passed acceptance criterion;
- total input, output, cached-input and reasoning tokens;
- tokens and dollars spent on tools, retries and agent handoffs;
- time to accepted result;
- defect escape rate and rework;
- probability of completion under budget;
- failed-attempt and recovery cost.

A more expensive model can be cheaper overall if it reduces tool rounds, failed attempts or rework. A frontier model can also cost several times more without measurable quality gain on a task class. The router needs task-specific evaluation data, not static vendor rankings.

### Two-stage quality cascade

1. Cheap verified attempt: least expensive model that passed the task profile's evaluation floor; use low/default effort for objectively checkable outputs.
2. Deterministic verification: formatter, compiler, static analysis, unit/integration/e2e tests, schema validators, visual checks or domain graders.
3. Targeted escalation: re-run only failed criteria with a stronger model, higher effort or different strategy. Pass exact failing evidence and minimal relevant context.
4. Independent review: use a separate model/provider for high-risk architecture/security changes. Do not require an expensive reviewer for every trivial edit.
5. Reconcile and close: re-run failed checks on the exact final candidate SHA. Close only with evidence.

Anthropic's published cost guidance reports an internal SWE-bench subset where low-effort attempts followed by high-effort retries on failures achieved approximately the same or slightly higher pass rate for about 40% less than running everything at high effort. This is workload-specific; the transferable principle is verify cheaply and escalate failures selectively.

### Cost levers, in priority order

1. Trim context: include only relevant source, diffs, test output and task contracts.
2. Cache stable prefixes: system instructions, tool definitions and repository policy should be cache-friendly.
3. Avoid repeated discovery: persist verified repo map, dependency graph, decisions and test results with commit hashes.
4. Use structured outputs to reduce ambiguity and rework.
5. Filter tool output: summarize large logs and keep only failing evidence in model context.
6. Use batch processing for non-interactive catalogue classification and offline evaluations when latency/data policies permit.
7. Tune effort before upgrading; change model/provider when a capability gap remains.
8. Route by modality to dedicated embedding/image/video/audio/speech/moderation APIs.
9. Bound agent count, turns, retries, debate rounds, tool calls and output tokens.
10. Use local inference only when it passes task tests and its privacy/latency/cost advantage is real.

### Initial budget envelope

For an engineering increment, start with a configurable envelope:
- Planning/discovery: 20–25%
- Implementation: 40–45%
- Verification/independent review: 15–20%
- Repair/model-switch reserve: 15–20%

These are initial allocation bands, not universal ratios or observed costs. The allocator may move unused budget forward while preserving a non-spendable repair reserve. Never exhaust the budget on planning, debate or repeated failed attempts.

A token budget is not a financial balance. Provider credits and user credits are separate. A price coefficient is not proof of available funds. If balance cannot be queried authoritatively, use an approved spend ceiling and reservation ledger; otherwise do not make a paid call.

### Stop and re-plan triggers

Stop the current attempt and preserve state when:
- two consecutive bounded attempts close no new acceptance criterion, pass no previously failing check and materially reduce no measured defect;
- the same failure recurs after a purported fix;
- actual token/cost burn exceeds phase budget or leaves insufficient funds for verification/repair;
- the model repeatedly emits malformed structured output or cannot use required tools;
- output remains poor despite higher effort;
- workers duplicate tasks, conflict on files or produce no new evidence;
- repository/test evidence invalidates architecture assumptions.

Then change one or more of task decomposition, context, prompt/contract, model family, reasoning effort, protocol adapter, tool, test strategy or collaboration topology. Do not retry the same prompt with more tokens. Re-plan the DAG and acceptance criteria before resuming.

## 4. Capability taxonomy

Each model entry should have explicit independently verified capability tags:
- Text: generate, classify, extract, translate, summarize.
- Reasoning: plan, math, long-horizon.
- Research: web search and synthesis.
- Code: generate, edit, review, debug, refactor.
- Tools: function calling, MCP, computer use, structured JSON.
- Vision/image: image input, image generation, image editing.
- Video: video input, generation, editing.
- Audio: audio input, transcription, generation, realtime, music generation.
- Embeddings: text and multimodal.
- Safety: moderation.

A tag becomes eligible only after endpoint-specific probe or documented first-party evidence plus account smoke test. A model-list response alone does not establish capability.

## 5. Router decision sequence

1. Parse task contract: work type, modality, quality floor, risk, privacy class, context, latency, dependencies, testability and spend ceiling.
2. Build candidates from live catalogue snapshots and project allowlists.
3. Reject candidates lacking protocol, tools, structured output, region/data policy, context, health or fresh capability probe.
4. Estimate input/output/reasoning/tool tokens; use provider token-count APIs when available, not character estimates for multimodal input.
5. Estimate cost using current price snapshot, cache rates, batch/priority tier and expected tool rounds; retain uncertainty range.
6. Check separate provider and user budget ledgers; reserve before dispatch and reconcile actual usage afterward.
7. Select lowest expected cost-per-accepted-task candidate above quality floor. Exploration is isolated to evaluation budget.
8. Record route reason and candidate set without private prompts or credentials.
9. Verify objectively; escalate only failed criteria while preserving correlation/idempotency lineage.
10. Update task-class performance only from verified outcomes, separating model quality from prompt, tool, dataset and environment effects.

Start with deterministic rules and accumulated evidence. Do not train a contextual-bandit router on sparse/biased observations. Once sufficient task/outcome/cost history exists, evaluate contextual-bandit or constrained optimization offline, then shadow mode, then limited canary. Online exploration must never consume unapproved production budget.

## 6. Evaluation system

Maintain a versioned evaluation set per task class:
- representative, edge and failure cases;
- immutable input/context hash and repository commit;
- acceptance rubric and deterministic tests;
- model/provider/protocol/version and reasoning effort;
- token usage by category, price snapshot and total cost;
- elapsed time, tool calls, retries, switches and compaction;
- accepted/rejected outcome and failure category;
- evaluator version, sample count and uncertainty.

Use paired comparisons on identical inputs and the same environment; repeat stochastic tasks. Do not infer a winner from one task/run.

Metrics: pass rate, criteria completion, cost per accepted task, cost per defect fixed, median/p95 time, tokens per accepted task, retries/tool calls, regression rate, escalation success lift, reviewer disagreement resolution and budget overrun (target zero).

## 7. Privacy and governance

- Never send secrets, production customer data or restricted source to an unapproved model/provider.
- Quarantine unknown catalogue models.
- Provider retention, training use, region and contractual restrictions are eligibility requirements.
- Use least-privilege task-scoped tools. Planning agents cannot deploy or mutate production.
- Audit model IDs, protocols, policy decisions, budgets, usage and results; redact secrets and sensitive prompts.
- Fallback must preserve data policy and idempotency; never silently switch a paid/sensitive task to another vendor.
- Price/model deprecation triggers re-probe and re-evaluation; keep a known-good rollback route.

## 8. Implementation DAG

| ID | Work package | Dependencies | Acceptance evidence |
|---|---|---|---|
| AR-01 | Task taxonomy and quality rubric registry | Provider registry audit | Versioned schemas and validation tests |
| AR-02 | Model catalogue snapshots and capability evidence | AR-01 | Live sync, metadata diff, stale/quarantine/deprecation tests |
| AR-03 | Protocol adapters and capability probes | AR-02 | Responses, Chat Completions, Anthropic Messages, Gemini and modality contract tests |
| AR-04 | Token/price estimator and usage normalizer | AR-02/03 | Input/output/cache/reasoning/tool usage tests; unknown pricing handled safely |
| AR-05 | Budget ledger, reservations and reconciliation | AR-04, tenancy/auth | Concurrent reservation, separate provider/user balance, no-overrun tests |
| AR-06 | Deterministic task-aware routing and quality cascade | AR-01–05 | Golden routing matrix; cheap-pass/targeted-escalation tests |
| AR-07 | Progress/stagnation controller and strategy switching | AR-06 | Two-no-progress stop, repeated failure, budget exhaustion, re-plan and model-switch tests |
| AR-08 | Multi-agent DAG scheduler and bounded collaboration | Durable executor + AR-06/07 | One-writer policy, isolated parallelism, max-round and resume tests |
| AR-09 | Evaluation dataset, paired model benchmarks and scorecards | AR-01–08 | Reproducible paired runs, cost-per-accepted-task and confidence reporting |
| AR-10 | Multimodal catalogue and routing | AR-02–05 | Endpoint-specific image/video/audio/music/embedding probes |
| AR-11 | Admin policy and project-level model controls | AR-01–10 | Tenant authorization, audit, budget controls, rollback and safe defaults |

Critical path: AR-01 → AR-02 → AR-03/AR-04 → AR-05 → AR-06 → AR-07. AR-08/09 depend on durable executor and verified routing. AR-10 is capability-by-capability and must not block text/code vertical slice.

## 9. Sources and evidence hierarchy

### Official provider documentation
- OpenAI, Model selection: https://developers.openai.com/api/docs/guides/model-selection
- OpenAI, Reasoning best practices: https://developers.openai.com/api/docs/guides/reasoning-best-practices
- OpenAI, Multi-agent: https://developers.openai.com/api/docs/guides/responses-multi-agent
- OpenAI, Token counting: https://developers.openai.com/api/docs/guides/token-counting
- OpenAI, Compaction: https://developers.openai.com/api/docs/guides/compaction
- Anthropic, Models overview: https://platform.claude.com/docs/en/models/overview
- Anthropic, Choosing a model: https://platform.claude.com/docs/en/about-claude/models/choosing-a-model
- Anthropic, Optimizing for cost and intelligence: https://platform.claude.com/docs/en/about-claude/models/optimizing-for-cost-and-intelligence
- Google, Gemini API models: https://ai.google.dev/gemini-api/docs/models
- Google, Gemini API pricing: https://ai.google.dev/gemini-api/docs/pricing

### Research literature
- Panda et al., Adaptive LLM Routing Under Budget Constraints, Findings of EMNLP 2025: https://aclanthology.org/2025.findings-emnlp.1301/
- Zhu et al., LLM-Based Multi-Agent Orchestration: A Survey of Frameworks, Communication Protocols, and Emerging Patterns, Future Internet 2026, 18(6), 326: https://doi.org/10.3390/fi18060326
- Orogat et al., Understanding Multi-Agent LLM Frameworks: A Unified Benchmark and Experimental Analysis, arXiv:2602.03128: https://arxiv.org/abs/2602.03128

### Evidence caveats
- Vendor benchmarks are vendor-reported; useful for hypotheses, not certification of Hub performance.
- Research benchmarks differ in tasks, environments, versions, graders and prices. Do not compare absolute scores across papers.
- Hub's paired, repeatable repository-task evaluations are final authority.
- Starimg may expose a subset of first-party models with different aliases, protocol behavior, prices, quotas and feature support. Only authenticated live catalogue plus endpoint probes establish eligibility.
