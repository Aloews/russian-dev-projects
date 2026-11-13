# Operations and release gates

## Current foundation

Run tests with:

```bash
pip install -e ".[dev]"
pytest -q
ruff check app tests
```

Build the container with:

```bash
docker build -t universal-service-hub:local .
```

## Production readiness gates

A deployment is blocked until all are true:

1. Authentication, RBAC and project-level tenancy are implemented and tested.
2. Provider configuration and audit events are durable.
3. Secret references resolve through the target secret manager; no secrets are stored in the database or repository.
4. Each enabled adapter has contract tests and a sandbox integration test.
5. Fallback behavior is tested against timeouts, 4xx/5xx responses, rate limits and duplicate requests.
6. Metrics, structured logs, tracing and alerting are configured.
7. Backup, restore and disaster-recovery procedures are tested.
8. Staging smoke tests pass on the exact release commit.
9. Rollback is automated and has been exercised.
10. Budget limits and maximum retry/agent-spend limits are enforced.

## Deployment policy

No automatic production deployment is enabled in this foundation. First deploy to an isolated development environment. Promote only after an independent review and verified staging checks. Do not modify existing project deployments or production databases as part of Hub bootstrap.
