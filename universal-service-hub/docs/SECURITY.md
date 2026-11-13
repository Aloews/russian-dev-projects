# Security baseline

- Never commit provider API keys, OAuth refresh tokens, database passwords, private keys or production connection strings.
- Keep secret values in the deployment platform's secret manager. The Hub should store secret references and metadata only.
- Do not return credentials or raw provider error bodies from API endpoints.
- Enforce authentication and role-based authorization before exposing the API outside a private network. Authentication is not implemented in this foundation.
- Require TLS for all external endpoints.
- Apply least privilege and separate development, staging and production credentials.
- Log actor, project, provider, operation, decision and correlation ID; redact payloads and secrets.
- Validate webhook signatures and prevent replay when webhook adapters are introduced.
- Use bounded timeouts, retries with jitter, rate limits and circuit breakers.
- Pin dependencies, scan images and dependencies, and rotate credentials after suspected exposure.
- Do not allow autonomous agents to access production secrets or deploy production changes without a separate policy gate and verified rollback.
