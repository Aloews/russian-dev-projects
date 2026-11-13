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
