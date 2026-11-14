# Provider API integration matrix

The Hub must model provider integrations as capability adapters. A vendor entry is not an active connection: each adapter is promoted only after credentials are supplied through the secret manager, official API behavior is checked, contract tests pass, and a non-production smoke test succeeds.

## User-selected infrastructure candidates

| Capability | Primary candidates | Additional alternatives | Target adapter contract |
|---|---|---|---|
| App hosting / edge | Northflank, Cloudflare, Netlify, Render | Railway, Fly.io, DigitalOcean | create/update deployment, status, logs, domains, rollback, usage |
| Backend compute / API | Northflank, Railway | Fly.io, DigitalOcean | deploy service, health, scale, logs, rollback |
| Relational database | Supabase | Neon, self-hosted PostgreSQL | connect, migrate, backup, restore, export, health |
| Object storage | Supabase Storage | Cloudflare R2, S3, Backblaze B2 | S3-compatible operations where supported, multipart, lifecycle, export |
| Queues / background jobs | Railway / Northflank Redis | managed Redis-compatible service | enqueue, ack, retry, visibility timeout, dead-letter, idempotency |

## Additional integration candidates

- Secrets: Infisical first; platform-native secret stores as fallbacks.
- AI inference: Starimg gateway, OpenRouter, direct model vendor APIs.
- Source control: GitHub App with installation-scoped permissions.
- Observability: Sentry plus OpenTelemetry-compatible metrics/logs/traces.
- Search: Tavily or Exa; crawling: Firecrawl.
- Media generation: fal.ai, Runway; speech: ElevenLabs.

## Current implementation status

The Rust control plane currently verifies read-only account access for Northflank, Railway, Cloudflare, Netlify, Render, Fly.io and DigitalOcean; Supabase/Neon management APIs; Supabase Storage, Cloudflare R2, Backblaze B2 and generic S3-compatible bucket access; PostgreSQL and Redis connectivity. This proves credentials and the narrow read operation only. It does not yet provide the full create/update/deploy/scale/rollback, migration/backup/restore, object lifecycle or queue orchestration contract shown above.

## Required per-provider metadata

Store provider id, capabilities, official API documentation URL, auth mechanism, regions, quotas/rate limits, pricing unit, data residency, SLA, status page, portability/export path, adapter version, last verified timestamp, health state and circuit-breaker state. Secret values must never be stored in provider metadata or returned by API endpoints.

## Failover rules

1. Route only to adapters that passed capability contract tests and are marked enabled.
2. Respect per-project allowlists, region/data-residency constraints and budget ceilings.
3. Apply timeouts, bounded retries with jitter, idempotency keys and circuit breakers.
4. Do not automatically fail over writes unless the adapter declares idempotent semantics or a reconciliation strategy.
5. Never silently migrate a database or move production traffic across regions.
6. A free tier is not a capacity guarantee; enforce quotas and stop safely at budget limits.

## User authorization boundary

GitHub OAuth authorizes access to repositories only within the granted GitHub scopes. It does not authorize Northflank, Cloudflare, Netlify, Render, Railway, Fly.io, DigitalOcean, Supabase, Neon, S3 or Redis accounts. Those integrations require their own OAuth/API credentials or service-account grants. Add them only when their official connection flow is available; never request or commit raw secrets in chat or repository files.
