# Security baseline

- All `/v1/*` control endpoints require an HTTP Bearer token. `USH_API_TOKEN` must contain at least 32 characters and must be stored in the deployment platform's secret manager.
- The token is compared with a constant-time comparison and is never returned by API responses.
- Startup fails in `USH_ENVIRONMENT=production` when the API token is missing.
- `/healthz` and `/readyz` remain public for platform health checks; they expose no provider credentials.
- This increment provides a single service token, not user identity, project tenancy or role-based access control. Do not treat it as a complete multi-tenant authorization system.
- Never commit provider API keys, OAuth refresh tokens, database passwords, private keys or production connection strings.
- Keep provider secret values in the deployment platform's secret manager. The Hub should store secret references and metadata only.
- Do not return credentials or raw provider error bodies from API endpoints.
- Require TLS for all external endpoints and restrict network access to trusted operators and project services.
- Apply least privilege and separate development, staging and production credentials.
- Provider create/update/delete and AI inference outcomes are written to a durable audit table. AI audit records contain provider/model, outcome, latency, attempt count and numeric token usage only; prompts, responses and credentials are excluded.
- The audit actor is currently the single service-token identity. Project identity, tenant-scoped audit access and cryptographically protected append-only storage remain future work.
- Outbound endpoints must use HTTPS. Local, private and link-local IP destinations are rejected, and DNS answers are checked before requests. This DNS check is defense in depth, not a substitute for a production egress firewall; enforce outbound network policy at the hosting platform to prevent DNS rebinding and metadata access.
- Validate webhook signatures and prevent replay when webhook adapters are introduced.
- Use bounded timeouts, retries with jitter, rate limits and circuit breakers.
- Pin dependencies, scan images and dependencies, and rotate credentials after suspected exposure.
- Do not allow autonomous agents to access production secrets or deploy production changes without a separate policy gate and verified rollback.
