# Operations and release gates

## Local development

```bash
pip install -e ".[dev]"
pytest -q
ruff check app tests
```

The local default is SQLite. The application creates the SQLite schema during startup for development and tests.

## PostgreSQL deployment

Set these environment variables in the deployment platform's secret/configuration manager:

- `USH_ENVIRONMENT=production`
- `USH_DATABASE_URL=postgresql+asyncpg://...`
- `USH_API_TOKEN` — at least 32 random characters
- Optional `USH_PROVIDERS_JSON` — seeds provider records that do not already exist

The container entrypoint runs `alembic upgrade head` before starting Uvicorn. Do not use SQLite in production. The database must be provisioned separately; the Hub does not create paid cloud resources.

## Container

```bash
docker build -t universal-service-hub:local .
```

The container runs as a non-root user. The health check uses `/healthz`. PostgreSQL migrations run at container startup.

## Production readiness gates

A deployment is blocked until all are true:

1. User identity, project tenancy and RBAC are implemented and tested; the current single-token auth is only an operator boundary.
2. Audit events are durable and include actor, project, provider, action and correlation ID.
3. Secret references resolve through the target secret manager; no secret values are stored in the database or repository.
4. Each enabled adapter has contract tests and a sandbox integration test.
5. Fallback behavior is tested against timeouts, 4xx/5xx responses, rate limits and duplicate requests.
6. Metrics, structured logs, tracing and alerting are configured.
7. Backup, restore and disaster-recovery procedures are tested.
8. Staging smoke tests pass on the exact release commit.
9. Rollback is automated and has been exercised.
10. Budget limits and maximum retry/agent-spend limits are enforced.

## Deployment policy

No production deployment is enabled by this increment. First deploy to an isolated development environment. Promote only after an independent review and verified staging checks. Do not modify existing project deployments or production databases as part of Hub bootstrap.

## Project access and API keys

- Keep `USH_API_TOKEN` as a bootstrap administrator secret; do not distribute it to project agents.
- Create a project with `POST /v1/projects`, then issue a scoped key with `POST /v1/api-keys`.
- Copy the returned token at creation time; it is never returned again. Store it in the target project's secret manager.
- Set the narrowest scopes required: `providers:read`, `health:probe`, `routing:select`, `ai:chat`; reserve `providers:write` for operator keys.
- Revoke compromised or retired keys with `DELETE /v1/api-keys/{key_id}`.
- Review control actions through `GET /v1/audit-logs`.
- Keys expire automatically; current maximum lifetime is 365 days. Automated rotation and external immutable audit export are not yet implemented.
