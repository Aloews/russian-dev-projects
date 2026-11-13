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
- Log actor, project, provider, operation, decision and correlation ID; redact secrets and request payloads.
- Validate webhook signatures and prevent replay when webhook adapters are introduced.
- Use bounded timeouts, retries with jitter, rate limits and circuit breakers.
- Pin dependencies, scan images and dependencies, and rotate credentials after suspected exposure.
- Do not allow autonomous agents to access production secrets or deploy production changes without a separate policy gate and verified rollback.

- AI provider API keys are resolved from environment-variable references at request time. The database stores only the reference name, never the credential value.
- AI provider base URLs must use HTTPS, cannot contain URL credentials/query strings/fragments, and redirects are disabled. These checks are not a substitute for an outbound egress allowlist; deploy privately until network-level SSRF controls are in place.
- Do not enable provider idempotent failover unless the provider's current documentation and a sandbox test confirm support. Gemini is not configured for idempotent failover.
- The Hub does not currently enforce per-user or per-project quotas, spend ceilings or rate limits.
