# Security baseline

- All `/v1/*` control endpoints require an HTTP Bearer token. `USH_API_TOKEN` must contain at least 32 characters and must be stored in the deployment platform's secret manager.
- The token is compared with a constant-time comparison and is never returned by API responses.
- Startup fails in `USH_ENVIRONMENT=production` when the API token is missing.
- `/healthz` and `/readyz` remain public for platform health checks; they expose no provider credentials.
- The root service token is an administrator credential. Project runtime tokens are stored as SHA-256 hashes, scoped to one project, role-checked, revocable and returned only once at issuance.
- `owner` and `operator` project roles can invoke AI chat; all project roles can use read-only provider selection. Project owners can issue/revoke operator and viewer tokens for their own project, but cannot issue another owner token or change project policy. Only the root administrator can issue owner tokens, change/delete projects, configure providers or read global audit data. Project capabilities and optional provider allowlists are enforced on execution routes.
- Provider administration, audit reads and project/token administration require the root administrator token.
- Project tokens do not yet provide per-user identity, SSO, fine-grained per-operation grants, or tenant-scoped audit views.
- Never commit provider API keys, OAuth refresh tokens, database passwords, private keys or production connection strings.
- Keep provider secret values in the deployment platform's secret manager. The Hub should store secret references and metadata only.
- Do not return credentials or raw provider error bodies from API endpoints.
- Require TLS for all external endpoints and restrict network access to trusted operators and project services.
- Apply least privilege and separate development, staging and production credentials.
- Provider create/update/delete and AI inference outcomes are written to a durable audit table. AI audit records contain provider/model, outcome, latency, attempt count and numeric token usage only; prompts, responses and credentials are excluded.
- Audit events distinguish the root service token from project token identities. Tenant-scoped audit views and cryptographically protected append-only storage remain future work.
- Outbound endpoints must use HTTPS. Local, private and link-local IP destinations are rejected, and DNS answers are checked before requests. This DNS check is defense in depth, not a substitute for a production egress firewall; enforce outbound network policy at the hosting platform to prevent DNS rebinding and metadata access.
- Validate webhook signatures and prevent replay when webhook adapters are introduced.
- Use bounded timeouts, retries with jitter, rate limits and circuit breakers.
- Pin dependencies, scan images and dependencies, and rotate credentials after suspected exposure.
- Do not allow autonomous agents to access production secrets or deploy production changes without a separate policy gate and verified rollback.


## API token rate limiting — implementation in progress

The Rust runtime applies a PostgreSQL-backed fixed-window limit per successfully authenticated bearer token. The default is 120 requests per token per UTC minute; operators may set `USH_RATE_LIMIT_PER_MINUTE` to an integer from 1 to 10000. The database stores only the SHA-256 token fingerprint, window start, request count and update timestamp. Fingerprint rows idle for 24 hours are pruned at startup and hourly. A rejected request returns HTTP 429 with `Retry-After: 60`.

This control limits abuse of valid credentials and is shared across runtime instances through PostgreSQL. It does **not** replace edge/IP-based throttling for unauthenticated traffic, sign-in/recovery abuse controls, or a distributed abuse-detection system. Operational dashboards remain outstanding. Do not describe S1 as complete.
