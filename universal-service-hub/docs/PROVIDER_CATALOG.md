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
