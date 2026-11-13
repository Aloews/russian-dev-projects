# Security baseline

- All `/v1/*` control endpoints require an HTTP Bearer token. `USH_API_TOKEN` is the bootstrap administrator token and must contain at least 32 characters; issued API keys are also accepted. Store all token values in the deployment platform's secret manager.
- The bootstrap token is compared in constant time. Issued API keys are SHA-256 hashed at rest and are shown only once. Token values are never returned by list or audit endpoints.
- Startup fails in `USH_ENVIRONMENT=production` when the API token is missing.
- `/healthz` and `/readyz` remain public for platform health checks; they expose no provider credentials.
- Never commit provider API keys, OAuth refresh tokens, database passwords, private keys or production connection strings.
- Keep provider secret values in the deployment platform's secret manager. The Hub should store secret references and metadata only.
- Do not return credentials or raw provider error bodies from API endpoints.
- Require TLS for all external endpoints and restrict network access to trusted operators and project services.
- Apply least privilege and separate development, staging and production credentials.
- Log actor, project, provider, operation, decision and correlation ID; redact secrets and request payloads. Current audit records are durable but not tamper-evident or exported to an immutable external store.
- Validate webhook signatures and prevent replay when webhook adapters are introduced.
- Use bounded timeouts, retries with jitter, rate limits and circuit breakers.
- Pin dependencies, scan images and dependencies, and rotate credentials after suspected exposure.
- Do not allow autonomous agents to access production secrets or deploy production changes without a separate policy gate and verified rollback.

- The bootstrap `USH_API_TOKEN` is the root administrator credential. Store it only in the deployment secret manager and rotate it deliberately.
- Issued API tokens are random, expire within at most 365 days, are shown only once, and are stored as SHA-256 hashes. Revocation disables a token immediately.
- Project and agent tokens are scoped to one project and cannot write provider configuration or manage projects/API keys. They can access global providers plus providers assigned to their project.
- Operator tokens have only the scopes explicitly granted at creation. Project and API-key administration remains bootstrap-admin-only.
- Control actions are recorded in the audit log without request prompts or secret values. Audit records are not yet tamper-evident or exported to an external immutable log store.
