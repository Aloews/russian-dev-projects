# External API and service integration plan

This is a capability catalog, not a claim that integrations are already connected. Only explicitly configured credentials and passing provider contract tests count as active integrations.

## Core accounts to connect first

| Priority | Service/API | What the Hub will use it for | Credential to create |
|---|---|---|---|
| P0 | GitHub App | Read/write project repositories, branches, PRs, checks, Actions and release automation | GitHub App private key + installation ID; narrowly scoped repository permissions |
| P0 | Infisical | Central secret storage, short-lived machine identity tokens and per-project secret access | Machine Identity Client ID/Secret with read-only access to the Hub's secret paths |
| P0 | Starimg API | Existing primary AI gateway; model discovery and generation | Starimg API key; verify account model IDs and endpoint protocol before activation |
| P0 | OpenRouter API | Independent model gateway and alternate routes across model providers | OpenRouter API key and explicit spend limit |
| P0 | Anthropic API | Direct Claude Messages API, including coding/reasoning models when available to the account | Anthropic API key; workspace budget/limits |
| P0 | OpenAI API | Direct Responses API, tool calling, image/vision and transcription where enabled | OpenAI API key with project-level budget |
| P1 | Google Gemini API | Independent text, multimodal and image-capable model route | Google AI API key / project quota |
| P0 | Northflank API | Compute, services, jobs, add-ons and deployments | Organization-scoped token with least-privilege Team Read for account verification |
| P0 | Railway Public API | Deploy and manage services, variables, environments, logs and rollbacks | Account token or OAuth token for the current-user check; workspace/project tokens need their own scoped query and header adapter |
| P1 | Render API | Services, workers, cron, deploys and logs | Render API key |
| P1 | Netlify API | Sites, deploys, functions and domains | Personal access token |
| P1 | Fly.io API | Apps, Machines, volumes and deployments | Fly API token |
| P1 | DigitalOcean API | App Platform, Droplets and managed resources | Scoped DigitalOcean API token |
| P1 | Vercel API | Deployments, domains, environment variables, logs and rollback for frontend/serverless projects | Scoped Vercel access token |
| P0 | Supabase Management API | Manage projects and database operations when authorized | Scoped Supabase PAT; avoid broad legacy tokens |
| P1 | Neon API | Manage serverless PostgreSQL projects, branches and compute | Personal or organization-scoped Neon API key |
| P1 | Cloudflare API + R2 S3 API | DNS, TLS, CDN, object storage and edge services | Scoped API token + R2 access key/secret |
| P1 | Backblaze B2 API | Object storage and S3-compatible access | Restricted application key ID + application key |
| P1 | Upstash Redis / QStash | Distributed queues, job scheduling, locks, cache, rate limits and retries | Redis REST URL/token and/or QStash token |
| P1 | Sentry | Error tracking, releases and alert integration | Project/org token with minimum required scopes |

## Specialized generation and research providers

| Capability | Primary recommendation | Independent alternative |
|---|---|---|
| Image generation/editing | fal.ai model API | OpenAI Images API or Gemini image generation |
| Video generation | Runway API (including supported partner models) | fal.ai video model endpoints |
| Speech, voice and transcription | ElevenLabs API | OpenAI audio APIs |
| Web search for agents | Tavily or Exa | A second search API selected after quality and price tests |
| Website crawling/extraction | Firecrawl | Browserless or a self-hosted browser worker |
| Long-running coding agents | Claude API/Managed Agents where enabled | OpenAI Responses API tools + isolated code workers |

## Rust read-only account verification (implemented; live credentials pending)

The Rust control plane exposes `POST /v1/integrations/{provider_id}/verify`. Provider records use the vendor name as `adapter_type` and `api_key_env` as an Infisical secret reference. The endpoint uses fixed HTTPS API destinations, bounded requests, no redirects, normalized non-secret output and redacted audit events. It never creates or modifies vendor resources.

Implemented account checks:
- Northflank: `GET /v1/teams?per_page=1` (organization-scoped token with Team Read permission).
- Railway: GraphQL `query { me { id name email } }`; this identity query requires an account token or OAuth access token, not a workspace/project token.
- Cloudflare: token verification endpoint; requires an active token. This does not prove Workers/Pages permissions; R2 permissions are tested separately.
- Netlify: current-user endpoint (authentication check only; site/deploy permissions require a separate resource check).
- Render: list owners endpoint (authentication check only; service/deploy permissions require a separate resource check).
- DigitalOcean: account-read endpoint (authentication check only; App Platform/Droplet permissions require a separate resource check).
- Fly.io: GraphQL viewer identity query (authentication check only; Machines app permissions require a separate resource check).
- Supabase: Management API list-projects endpoint; requires project-read scope.
- Neon: list-projects endpoint; bearer API key.
- Cloudflare R2: list buckets endpoint; requires Workers R2 Storage Read permission. Store a JSON secret referenced by `CLOUDFLARE_R2_CREDENTIALS_JSON` with `account_id` and `api_token`.
- Backblaze B2: Native API account authorization endpoint. Store a JSON secret referenced by `BACKBLAZE_B2_CREDENTIALS_JSON` with `key_id` and `application_key`; prefer a restricted application key.

Supabase Storage:
- Adapter type `supabase_storage`. Store a JSON secret referenced by `SUPABASE_STORAGE_CREDENTIALS_JSON` with `project_url` and `service_role_key`. The verifier calls the read-only bucket-list endpoint and returns only bucket count/name. The service-role key is highly privileged; use a dedicated Supabase project and keep it in Infisical only.

S3-compatible object storage:
- Adapter type `s3_compatible`. Store `S3_COMPATIBLE_CREDENTIALS_JSON` in Infisical with `endpoint_url`, `region`, `access_key_id`, `secret_access_key`, optional `session_token`, and optionally a bucket name. The verifier uses AWS Signature Version 4 and performs a read-only `HeadBucket` when a bucket is specified; otherwise it performs `ListBuckets`. Prefer bucket-scoped credentials and `HeadBucket` to avoid account-wide listing permissions. Supports AWS S3, Cloudflare R2, Supabase Storage S3 protocol and Backblaze B2 S3 API endpoints.

PostgreSQL data-plane connectivity:
- Self-managed PostgreSQL: adapter type `postgresql`, with a secret reference such as `POSTGRESQL_DATABASE_URL`. The verifier requires a PostgreSQL URL, rejects private/local destinations, forces TLS with certificate and hostname verification, opens a single-connection pool, executes `SELECT 1`, then closes the pool. It does not run migrations or modify data.

Private-network access:
- Public destinations are the default. For self-managed PostgreSQL or Railway/Northflank Redis reachable only over a private service network, configure `USH_ALLOWED_PRIVATE_HOSTS` as a comma-separated list of exact DNS hostnames (no wildcards, no IP literals). The verifier permits only RFC1918/ULA addresses for those exact names; loopback, link-local and cloud metadata addresses remain blocked. Keep the allowlist minimal and deploy the Hub inside the same private network.

Redis data-plane connectivity:
- Railway Redis, Northflank Redis and other Redis-compatible services: adapter type `redis`, with a secret reference such as `REDIS_URL`. The verifier requires `rediss://` for public endpoints. Plain `redis://` is permitted only for a hostname in the exact `USH_ALLOWED_PRIVATE_HOSTS` allowlist when DNS resolves exclusively to RFC1918/ULA addresses. It opens one bounded async connection and sends `PING`; it never writes keys or data.

These are implemented API checks, not active connections. Each provider remains unconnected until credentials are stored in Infisical and live verification succeeds. PostgreSQL and Redis have read-only connection probes only; they do not manage schema, replication, queues or locks. R2/B2 object transfer and generic S3 object operations remain future increments. GitHub OAuth sign-in does not automatically grant the Hub a provider's management API token; each vendor credential must be separately created and stored in Infisical.

## Architecture decisions

1. **Do not hardwire projects to one vendor.** Every connector implements a Rust trait with capability metadata, timeouts, typed errors, health state, quotas and contract tests.
2. **Keep direct providers and gateways as separate routes.** Starimg and OpenRouter are gateways; Anthropic, OpenAI and Gemini direct APIs are independent fallbacks. Never assume model IDs or protocol compatibility without a live contract test.
3. **Keep media jobs asynchronous.** Image/video/audio generation returns a durable job ID. Store state, provider task ID, attempt count, idempotency key and output object references. Poll or consume webhooks; do not hold HTTP requests open for long renders.
4. **Use one central secret manager.** Infisical is the proposed default. The Hub stores secret references, not API key values. Project identities can read only their own secret paths.
5. **Separate compute from control plane.** The Hub plans and dispatches work; isolated build/generation workers execute untrusted code and large jobs. Workers have short-lived credentials and no access to other projects' secrets.
6. **Make failover capability-aware.** Switch only to a provider supporting the same input/output features, data policy, region and quality requirements. Preserve structured outputs, tool calls, image references and model-specific parameters.
7. **Control spending before enabling automation.** Set per-project and per-provider monthly budgets, per-task ceilings, concurrency limits, queue limits and kill switches. Do not auto-upgrade paid plans.
8. **No blind retries for billable generation.** Retry only with a documented idempotency guarantee or after proving the request was not accepted. Persist request state before dispatch.

## Minimal connection order

1. GitHub App.
2. Infisical machine identity.
3. Starimg + OpenRouter + one direct model provider (Anthropic or OpenAI).
4. Railway + Vercel + Supabase.
5. Cloudflare R2 + Upstash.
6. Sentry.
7. Add Runway/fal.ai/ElevenLabs/Tavily/Firecrawl only when the relevant project needs those modalities.

Do not paste API keys into chat, GitHub issues, PR comments or project files. Create credentials in the vendor console and inject them through the secret manager. Keep each project's keys separately scoped and revocable.

## Limits and fallback reality

No external API provides literally unlimited execution. Limits include account quotas, concurrent jobs, rate limits, context/output caps, model availability, geographic restrictions and billing. The Hub can reduce interruptions through provider diversity, queueing, rate-aware scheduling, budget controls and graceful degradation; it cannot bypass vendor terms, account limits or payment requirements.

## Verification gates per connector

- Official API contract and authentication method recorded.
- Current models/capabilities fetched dynamically where possible.
- Secret stored in the secret manager and never returned by API.
- Provider-specific timeout, retry and idempotency semantics recorded.
- Contract tests cover success, malformed response, auth failure, rate limit, timeout and unavailable service.
- One explicitly authorized live smoke test passes.
- Pricing, quota and budget ceiling are configured before the connector is enabled for production.


## Credential inventory (names only)

Create these secrets in Infisical, not in GitHub or source files. The Rust runtime will consume secret references after the Infisical adapter is implemented.

### Core
- `GITHUB_APP_ID`
- `GITHUB_APP_PRIVATE_KEY`
- `GITHUB_INSTALLATION_ID`
- `INFISICAL_CLIENT_ID`
- `INFISICAL_CLIENT_SECRET`
- `INFISICAL_PROJECT_ID`
- `INFISICAL_ENVIRONMENT`
- `STARIMG_API_KEY`
- `OPENROUTER_API_KEY`
- `ANTHROPIC_API_KEY` (or `OPENAI_API_KEY` as the first direct model provider)
- `RAILWAY_TOKEN`
- `VERCEL_TOKEN`
- `SUPABASE_ACCESS_TOKEN`

### Platform operations
- `CLOUDFLARE_API_TOKEN`
- `R2_ACCOUNT_ID`
- `R2_ACCESS_KEY_ID`
- `R2_SECRET_ACCESS_KEY`
- `UPSTASH_REDIS_REST_URL`
- `UPSTASH_REDIS_REST_TOKEN`
- `QSTASH_TOKEN`
- `SENTRY_AUTH_TOKEN`

### Optional generation/research capabilities
- `GEMINI_API_KEY`
- `OPENAI_API_KEY`
- `FAL_KEY`
- `RUNWAY_API_KEY`
- `ELEVENLABS_API_KEY`
- `TAVILY_API_KEY` or `EXA_API_KEY`
- `FIRECRAWL_API_KEY`

Do not create duplicate secrets for the same provider/account. The Hub must support multiple credential versions per provider, rotation without downtime, per-project access policies and revocation. Never place the root Infisical identity in project runtime containers.


## User-selected infrastructure providers

The default provider pool for new project onboarding is now:

| Layer | Primary candidates | Alternate / self-hosted route | Control-plane integration |
|---|---|---|---|
| Hosting / frontend | Northflank, Cloudflare, Netlify, Render | Vercel | Provider-specific management API + GitHub repository connection |
| Backend / API / workers | Northflank, Railway | Fly.io, DigitalOcean App Platform / Droplets | Deployment, service, environment, logs, metrics and rollback adapters |
| Database | Supabase | Neon; PostgreSQL on an owner-managed server | Supabase Management API; Neon API; PostgreSQL wire protocol for database operations |
| File/object storage | Supabase Storage, Cloudflare R2 | Amazon S3-compatible storage, Backblaze B2 | Supabase Storage API; S3 API; B2 S3-compatible API |
| Queues / background work | Railway Redis, Northflank Redis | Upstash Redis/QStash; self-hosted Redis | Redis protocol or provider control-plane API; durable queue semantics in Hub |

Provider selection is per workload, not a single global winner. Keep production databases, file stores and queues independent from application hosting where practical so a host outage does not take down the data plane.

### GitHub sign-in versus provider authorization

Signing in to a hosting platform with GitHub normally grants that platform permission to see selected repositories and trigger builds/deployments. It does **not** automatically grant the Hub access to the platform's management API, cloud account, billing, database, storage, Redis or secrets.

The Hub must treat these as separate credentials and scopes:

- **GitHub App installation:** repository contents, pull requests, checks and workflow permissions explicitly granted by the user.
- **Platform GitHub integration:** repository access used by the platform to build/deploy source code.
- **Platform management API token / OAuth grant:** required for the Hub to create or manage services, environments, deployments, domains and variables.
- **Data-plane credentials:** database connection string, storage access key, Redis URL/password. These are separate from management API tokens and should be restricted to the individual project/resource.

If a vendor supports GitHub OAuth but does not issue an API token or management OAuth grant with the required scopes, the Hub can still use GitHub-triggered deployments, but it must not claim direct account-management access.

### Credential names for the selected provider pool

Store credentials in Infisical under project-specific paths. These are names only; do not paste secret values into GitHub or chat.

- Northflank: `NORTHFLANK_API_TOKEN`
- Cloudflare: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`; for R2 also `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`
- Netlify: `NETLIFY_AUTH_TOKEN`
- Render: `RENDER_API_KEY`
- Railway: `RAILWAY_TOKEN`
- Fly.io: `FLY_API_TOKEN`
- DigitalOcean: `DIGITALOCEAN_ACCESS_TOKEN`
- Supabase management: `SUPABASE_ACCESS_TOKEN`; per-project database credentials stored separately
- Neon: `NEON_API_KEY`; per-project PostgreSQL connection string stored separately
- PostgreSQL self-hosted: `DATABASE_URL` (TLS required; least-privilege role)
- Supabase Storage: per-project URL and restricted service credentials
- Amazon S3: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`, bucket name
- Backblaze B2: `B2_APPLICATION_KEY_ID`, `B2_APPLICATION_KEY`; or S3-compatible endpoint/access keys
- Redis: `REDIS_URL` / provider-specific API token, with distinct credentials per project and environment

A credential is not considered connected merely because its name is documented. Mark it active only after the owner authorizes access, the secret is stored, and a read-only identity/health check plus the relevant API contract test pass.


## Minimum permission profiles

- **GitHub App:** repository metadata read; contents read/write; pull requests read/write; checks and commit statuses read; Actions read, with write only if the Hub must dispatch workflows; issues read/write only for task intake. Do not grant organization-owner permissions.
- **Infisical machine identity:** read-only access to the exact provider-secret paths needed by the Hub. No secret write/delete permission for the runtime identity.
- **Railway:** project-scoped token where possible; deployments, service/environment metadata and logs only. Separate a break-glass workspace token from normal runtime.
- **Vercel:** team-scoped token; deployments and project/environment metadata required by the connected project only.
- **Supabase:** Management API PAT only for project administration. Runtime database access uses each project's own database credential or restricted service identity; do not share a Supabase service-role key across unrelated projects.
- **Cloudflare:** scoped API token for the required zone and R2 bucket. R2 credentials should be bucket-scoped and object-only unless bucket administration is explicitly needed.
- **Upstash:** per-database Redis token and separate QStash signing/token credentials; do not reuse one token across projects.
- **Sentry:** project-scoped token for issue/release/alert operations. Read-only access is the default unless automated release creation is required.

A provider connector must declare its requested permission profile in code and refuse to run if its credential scope is broader than the configured policy where the vendor exposes scope introspection.
