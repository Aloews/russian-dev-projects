# Architecture

## Control-plane boundaries

The Hub is a control plane, not a proxy for arbitrary provider traffic. It owns provider metadata, eligibility decisions, policy evaluation and audit history. Workload data and application databases remain with the projects unless a migration is explicitly planned and validated.

## Current runtime

- Rust Axum/Tokio exposes the production HTTP API; the Docker image starts the Rust binary.
- SQLx persists provider, project, token and audit state in PostgreSQL; SQLx migrations run during startup.
- FastAPI and SQLAlchemy files remain as a temporary compatibility/reference implementation, not the production container entrypoint.
- Root bearer authentication and project-scoped hashed tokens enforce owner/operator/viewer roles and project capability/provider allowlists.
- Provider selection is based on enabled state, capability, health and priority; ties are deterministic by provider ID.

## Infrastructure account verification

The Rust runtime supports read-only identity/connection checks for Northflank, Railway, Cloudflare, Netlify, Render, DigitalOcean, Fly.io, Supabase Management API and Neon. Cloudflare R2 bucket inventory and Backblaze B2 account authorization are also checked through their management/native APIs. These use fixed official HTTPS API endpoints rather than caller-supplied URLs, resolve credentials through Infisical, apply bounded request timeouts, reject redirects, normalize the response and audit only provider ID/type, latency and non-secret counts. This is a connection-verification layer, not yet a resource provisioning/deployment adapter or general S3 object client.

## Target architecture

```text
Projects / Agents
      |
Hub API + AuthN/AuthZ + Policy Engine
      |
Provider Registry ---- Secret References ---- Audit Log
      |
Capability Router
  |       |       |
Cloud   AI API   Data / Storage / Queue
Adapters (per-vendor implementations)
      |
Health + Cost + Latency + Quota Signals
      |
Fallback Policy / Circuit Breakers
```

## Required adapter contract

Each adapter must declare:
- stable provider ID and supported capabilities;
- authentication through secret references, never plaintext configuration;
- typed request/response models;
- timeout, retry and idempotency behavior;
- health and capability checks;
- normalized errors and usage/cost metadata;
- integration tests with a sandbox account or deterministic provider test server.

## Routing safety

The first release must not silently move stateful workloads between providers. Stateless requests may fail over only when the request is idempotent or has a stable idempotency key. Database failover requires replication, consistency checks, tested recovery objectives and an explicit migration plan.
