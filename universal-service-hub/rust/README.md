# Universal Service Hub — Rust runtime

This Rust service is the target runtime for the Hub. The existing Python control plane remains available during the parity migration; it is not silently replaced until the Rust runtime passes the full contract suite and staged rollout.

## Run

```bash
cd universal-service-hub/rust
USH_API_TOKEN="$(openssl rand -hex 32)" cargo run --release
```

## API

- `GET /healthz` — public process health.
- `GET /v1/providers` — authenticated provider catalog.
- `POST /v1/connections/{provider_id}/test` — authenticated, read-only connection check.

Set `USH_API_TOKEN` (minimum 32 characters). Provider credentials are read from environment variables at request time and are never returned or logged. The connection tests are read-only; they do not create or modify cloud resources.

## Implemented account checks

Northflank, Railway, Fly.io, Render, Netlify, DigitalOcean, Cloudflare R2, Supabase Management API, Neon and Backblaze B2. PostgreSQL, Redis and generic S3-compatible signed checks are catalogued but are not yet implemented by this increment.

A provider is only marked connected when its real API responds successfully. A catalog entry is not evidence that an account is connected. Do not configure a provider as an automatic production fallback until its operation-specific contract tests and recovery behavior are verified.

## Security

- HTTPS-only hard-coded upstream endpoints; redirects disabled.
- Hub API requires a bearer token and compares it in constant time.
- Provider secrets are read from environment variables, never accepted in request bodies.
- Upstream response bodies and credentials are not exposed to callers.
- No write operations, provisioning, billing changes, DNS changes or production mutations are performed by connection checks.
