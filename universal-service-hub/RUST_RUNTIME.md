# Universal Service Hub — Rust runtime

The production control-plane runtime is Rust (Axum + Tokio + SQLx + PostgreSQL). Python files are retained temporarily as a compatibility/reference implementation during API parity migration; the production Docker image starts only the Rust binary.

## Runtime

- Rust stable, edition 2024
- Axum HTTP API, Tokio async runtime
- SQLx PostgreSQL with embedded SQL migrations
- Reqwest with rustls for outbound provider calls
- Structured tracing; bearer-token project isolation

## Required environment

- `USH_DATABASE_URL`: PostgreSQL URL
- `USH_API_TOKEN`: root administrator token (minimum 32 characters)
- `PORT`: optional, defaults to 8000
- `RUST_LOG`: optional tracing filter

## Implemented Rust routes

- `GET /healthz`, `GET /readyz`
- `GET/POST /v1/projects` (root administrator)
- `POST /v1/projects/{project_id}/tokens` (root administrator; raw token returned once)
- `DELETE /v1/projects/{project_id}/tokens/{token_id}` (root administrator)
- `GET /v1/providers` (root administrator)
- `POST /v1/providers/select` (root administrator or scoped project owner/operator)
- `GET /v1/audit` (root administrator)

Project tokens are random, stored as SHA-256 hashes, revocable, and restricted by role and project capability/provider allowlists.

## Migration status

Rust migrations create the shared PostgreSQL tables with idempotent `CREATE TABLE IF NOT EXISTS` statements. Production cutover remains blocked until endpoint parity, migration compatibility against an existing database, security tests, CI, and a staged deployment/rollback are verified.
