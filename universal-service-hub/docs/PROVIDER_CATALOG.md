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

## Model and agent execution policy

A model, provider and CLI are separate registry entities. Model availability in a catalogue does not establish protocol compatibility, task quality, price, account balance or a working integration.

- **Codex CLI (OpenAI tool):** use only with a model that passed the OpenAI Responses/function-call probe. Fable/Claude models must never be assigned to this slot.
- **Claude Code CLI (Anthropic tool):** use only with a model that passed the Anthropic Messages/tool-use probe. Fable 5.2 is a Claude-family model and belongs here.
- **Astra:** treat as an OpenAI-family model only after its exact live ID is discovered and it passes the Responses probe. Use it for high-compute planning/escalation when budget policy authorizes it.
- **Fable 5.2:** use through Claude Code only after its exact live ID passes the Anthropic Messages probe and premium escalation is authorized.
- Ordinary tasks use a verified baseline model. Premium models are not mandatory defaults; select them when objective quality gates fail, the task is high-risk/ambiguous, or an approved budget/balance policy allows the higher-cost route.
- Both CLIs participate before source changes: Codex CLI drafts the plan; Claude Code independently challenges it. Then Codex implements and Claude Code reviews the implementation.
- The current workflow's premium approval flag is an administrative gate, not evidence of a provider/user balance. Balance-based automatic routing remains unimplemented until an authoritative balance API or trusted ledger is integrated.

## Model discovery and modular routing roadmap

1. Discover exact IDs from the authenticated live catalogue; never rely on a permanent hardcoded list.
2. Classify each model by family, modality, context, protocol, tool use, structured output, reasoning controls, price/coefficient and deprecation status.
3. Verify each protocol with a real tool-call probe before making a model eligible.
4. Quarantine new/unknown models until metadata, privacy policy, safety and capability probes pass.
5. Route by task requirements, objective quality floor, context, protocol, privacy, health and project allowlists.
6. Use verified price and separate provider/user balance reservations; stop on stale balance, insufficient funds or exceeded task ceiling.
7. Keep per-attempt routing evidence and fallback lineage; never expose credentials or sensitive prompts.
8. Treat text/code, image, video, audio, music and embeddings as separate modalities requiring documented endpoints and contract tests.

See [docs/MAP.md](./MAP.md), section **Modular model and agent routing**, for the implementation DAG and release gates.

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
