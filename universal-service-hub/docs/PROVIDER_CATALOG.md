# Provider catalog and discovery scope

This catalog defines categories to evaluate. Listing a vendor is not evidence of an active connection, supported feature, favorable price or tested failover.

## Compute and application hosting

- Northflank
- Railway
- Fly.io
- Render
- DigitalOcean App Platform
- Vercel
- Cloudflare Workers / Pages
- AWS (ECS, App Runner, Lambda)
- Google Cloud Run
- Azure Container Apps
- Hetzner Cloud
- Self-hosted Coolify

## Databases and state

- Supabase
- Neon
- Crunchy Bridge
- AWS RDS / Aurora
- Google Cloud SQL
- Azure Database for PostgreSQL
- DigitalOcean Managed Databases
- PlanetScale (where workload and engine fit)
- Redis Cloud / Upstash
- Managed or self-hosted PostgreSQL and Redis

## Object and file storage

- Cloudflare R2
- Amazon S3
- Backblaze B2
- Google Cloud Storage
- Azure Blob Storage
- Supabase Storage
- DigitalOcean Spaces

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
