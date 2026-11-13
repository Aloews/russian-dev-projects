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
| P0 | Railway Public API | Deploy and manage services, variables, environments, logs and rollbacks | Workspace token or project token; prefer project scope |
| P1 | Vercel API | Deployments, domains, environment variables, logs and rollback for frontend/serverless projects | Scoped Vercel access token |
| P0 | Supabase Management API | Manage projects and database operations when authorized | Scoped Supabase PAT; avoid broad legacy tokens |
| P1 | Cloudflare API + R2 S3 API | DNS, TLS, CDN, object storage and edge services | Scoped API token + R2 access key/secret |
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
