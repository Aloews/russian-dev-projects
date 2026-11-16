# Operations and release gates

## Local development — Rust production runtime

The production container is the Rust Axum/SQLx service. The Python/FastAPI tree is a compatibility/reference implementation and is not the container entrypoint.

Requirements: Docker Engine with Compose v2, curl, and OpenSSL.

From the repository root:

~~~bash
cd universal-service-hub
bash scripts/dev-up.sh
~~~

The script generates a 256-bit root API token and PostgreSQL password in a local .env.local file with mode 0600, builds the Rust image, starts PostgreSQL 17 and the Hub, waits for the database-backed /readyz endpoint, verifies /healthz, confirms unauthenticated API requests receive 401, and confirms authenticated provider-list access. It never prints the root token.

Local endpoints:

- http://localhost:8000/healthz
- http://localhost:8000/readyz
- http://localhost:8000/v1/providers (Bearer token required)

The root API token is stored in universal-service-hub/.env.local; do not commit or share it. The local compose profile explicitly uses development mode and environment-backed secrets. It is not a production deployment configuration.

Stop the service without deleting local database data:

~~~bash
bash scripts/dev-down.sh
~~~

To intentionally remove the local database volume, run docker compose -f compose.yml down -v from this directory. This is destructive and cannot be undone.

## Direct Rust development

For developers who already have a PostgreSQL instance:

~~~bash
export USH_DATABASE_URL="postgres://ush:password@localhost:5432/ush"
export USH_API_TOKEN="$(openssl rand -hex 32)"
cargo run --release
~~~

The Rust binary applies the SQLx migrations on startup. PostgreSQL is mandatory for this runtime.

## Isolated PostgreSQL deployment

Set these variables in the deployment platform's secret/configuration manager:

- USH_ENVIRONMENT=production
- USH_SECRET_BACKEND=infisical
- USH_DATABASE_URL=postgres://... (PostgreSQL connection string; use TLS)
- USH_API_TOKEN — at least 32 random characters
- INFISICAL_CLIENT_ID
- INFISICAL_CLIENT_SECRET
- INFISICAL_PROJECT_ID
- INFISICAL_ENVIRONMENT
- Optional INFISICAL_URL and USH_INFISICAL_SECRET_PATH_PREFIX

Production startup fails closed if Infisical machine identity is incomplete. Never use the local development compose profile as a public production deployment.

## CI runtime proof

The CI pipeline builds the Rust image, starts it against PostgreSQL, waits for database readiness, and verifies health, authentication rejection and authenticated API access. It also runs the full container smoke scenario for project/token RBAC, provider selection, rate limiting, token revocation and the production secret-manager guard. A CI smoke test proves the image can start in an ephemeral runner; it does not establish a persistent public deployment.

## Production readiness gates

A deployment is blocked until all are true:

1. Customer identity, MFA/passkeys and complete tenant isolation are implemented and independently tested.
2. Durable audit events include actor, project, provider, action and correlation ID.
3. Secret references resolve through the target secret manager; no secret values are stored in the database or repository.
4. Each enabled adapter has contract tests and a sandbox integration test.
5. Fallback behavior is tested against timeouts, 4xx/5xx responses, rate limits and duplicate requests.
6. Metrics, structured logs, tracing and alerting are configured.
7. Backup, restore and disaster-recovery procedures are tested.
8. Staging smoke tests pass on the exact release commit.
9. Rollback is automated and has been exercised.
10. Budget limits and maximum retry/agent-spend limits are enforced.

## Deployment policy

No persistent public deployment is currently active. First deploy to an isolated development environment. Promote only after independent review and verified staging checks. Do not modify existing product deployments or production databases as part of Hub bootstrap. Do not provision paid resources without explicit approval.
