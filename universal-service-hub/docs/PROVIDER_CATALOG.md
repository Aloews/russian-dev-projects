# Provider catalog and discovery scope

This catalog defines categories to evaluate. Listing a vendor is not evidence of an active connection, supported feature, favorable price or tested failover.

## Compute and application hosting

- Northflank (containers, jobs, managed add-ons)
- Railway (services, workers, cron, volumes and Redis add-on)
- Fly.io (Machines, Apps, volumes and private networking)
- DigitalOcean App Platform and Droplets
- Render (web services, workers, cron jobs and managed PostgreSQL)
- Netlify (frontend hosting, functions and deploy previews)
- Cloudflare Workers / Pages (edge runtime and static hosting)
- Vercel (frontend and serverless hosting)
- AWS (ECS, App Runner, Lambda)
- Google Cloud Run
- Azure Container Apps
- Hetzner Cloud
- Self-hosted Coolify

## Databases and state

- Supabase (PostgreSQL and managed platform APIs)
- Neon (serverless PostgreSQL and branching)
- Crunchy Bridge
- AWS RDS / Aurora
- Google Cloud SQL
- Azure Database for PostgreSQL
- DigitalOcean Managed Databases
- PlanetScale (where workload and engine fit)
- Redis Cloud / Upstash
- PostgreSQL on a self-managed server (direct PostgreSQL protocol; owner-operated backups, patching and HA)
- Managed or self-hosted PostgreSQL and Redis

## Object and file storage

- Supabase Storage
- Cloudflare R2 (S3-compatible API)
- Amazon S3 and S3-compatible object stores
- Backblaze B2 (S3-compatible API)
- Google Cloud Storage
- Azure Blob Storage
- Supabase Storage
- DigitalOcean Spaces

## Queues, background jobs and cache

- Railway Redis
- Northflank Redis / managed Redis add-ons
- Upstash Redis / QStash
- Self-hosted Redis (only with managed backups, monitoring and failover)

## AI model gateways and inference

- Starimg
- OpenRouter
- OpenAI
- Anthropic
- Google Gemini
- Mistral
- Fireworks AI
- Together AI
- Groq
- Cerebras
- Self-hosted inference (vLLM / Ollama) when economically and operationally justified

## Provider execution and recovery order

1. **Starimg / Fable 5.2** — primary senior implementation and repair agent. Before launch, the workflow sends a real Anthropic Messages tool-call probe; a present key is not considered valid until a funded tool call succeeds.
2. **Starimg / Claude Opus 5.5** — bounded senior-engineer fallback if Fable is unavailable or fails.
3. **OpenRouter / Claude Sonnet 4.5** — optional provider fallback through the Anthropic Messages-compatible endpoint `https://openrouter.ai/api`, using `ANTHROPIC_AUTH_TOKEN` (Bearer) and `OPENROUTER_API_KEY`. It is attempted only when the OpenRouter tool-call probe succeeds. Budget ceiling: USD 10.
4. **Local Ollama / Qwen2.5-Coder 3B** — provider-independent implementation/recovery path. It runs a bounded multi-role engineering pass, requires real source/test changes, validates exact old-text edits, and fails closed on timeouts or documentation-only candidates.

Starimg senior-agent budgets are capped at USD 15 for Fable and USD 10 for Opus; OpenRouter is capped at USD 10. The combined external-agent ceiling is USD 35 per run, excluding the small compatibility probes. A 402 insufficient-balance response is recorded as exhausted provider quota, not misclassified as a tool-protocol defect. The workflow preserves redacted diagnostics as an artifact.

## Multi-agent engineering roles

| Role | Default executor | Responsibility | Permission boundary |
|---|---|---|---|
| Architecture analyst | Qwen2.5-Coder 3B local | Inspect current architecture, dependencies and failure modes | Read-only |
| Product analyst | Qwen2.5-Coder 3B local | Define user journeys and measurable acceptance criteria | Read-only |
| Security analyst | Qwen2.5-Coder 3B local | Challenge auth, tenancy, PII, secrets, SSRF and abuse controls | Read-only |
| QA analyst | Qwen2.5-Coder 3B local | Identify regression and test gaps | Read-only |
| Implementer | Qwen2.5-Coder 3B local | Make bounded structured source edits | Single writer, source-path allowlist |
| Independent QA | Qwen2.5-Coder 3B local | Review the actual candidate diff | Read-only |
| Senior engineer / fixer | Fable 5.2 via Starimg | Reconcile role findings, fix hard problems and own the final candidate | Single final writer |
| Independent architecture reviewer | Opus 5.5 | Challenge design and review final diff | Read-only |
| Independent security reviewer | Sonnet 5.5 | Security/privacy review | Read-only |
| Product/user-journey reviewer | Haiku 5.5 | Challenge product scope and usability | Read-only |

Qwen roles are a bounded, role-separated local engineering pass using one Qwen2.5-Coder 3B inference session, not six independent model instances or separate vendors. Fable is the escalation/fixer and must independently inspect the source and test evidence. These are configured roles; each provider/model combination remains unverified until its run produces attributable evidence.

## Source control and delivery

- GitHub
- GitLab
- Bitbucket
- Buildkite
- GitHub Actions
- GitLab CI

## Observability, security and edge

- Sentry
- Better Stack
- Grafana / Prometheus / Loki / Tempo
- Datadog
- New Relic
- Cloudflare
- Tailscale
- 1Password Secrets Automation / Doppler / platform-native secret managers

## Integration acceptance

For every candidate, capture: official docs URL, supported API/SDK, auth type, regions, data residency, pricing unit, free-tier limits, rate limits, service-level commitments, portability, export path, failure modes, status-page URL and last verification timestamp.

Only candidates with a working sandbox integration and automated contract tests may be promoted to "verified". A vendor appearing in this catalog is only a discovery candidate.


## Local inference operating boundary

- Ollama is a first-class local chat/model-discovery adapter in the Rust runtime branch `feat/ush-local-agent-failover`; this is not yet a verified live deployment.
- The Hub accepts only loopback or explicitly allowlisted private Ollama hosts via `USH_OLLAMA_ALLOWED_HOSTS`. Public hosts, metadata endpoints and DNS answers outside private/loopback ranges are rejected.
- Local model routing must retain project capability/provider allowlists, explicit model IDs, health status and audit attribution. Do not silently route a cloud-only or billable workload to local inference if required capabilities (tools, vision, structured output, context length) are missing.
- The Hub coding workflow now has a bounded Ollama/Qwen fallback path for Starimg/CLI/Codex/Claude unavailability. It reuses an existing local Ollama server/model when present, otherwise installs Ollama and pulls the configured model. The live failure-triggered path remains unverified until an actual fallback workflow run completes.
- This is a single local model execution with role-separated instructions, not a parallel multi-model agent network. The exact-head CI and review process remain mandatory.
