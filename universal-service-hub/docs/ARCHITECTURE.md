# Architecture

## Control-plane boundaries

The Hub is a control plane, not a proxy for arbitrary provider traffic. It owns provider metadata, eligibility decisions, policy evaluation and audit history. Workload data and application databases remain with the projects unless a migration is explicitly planned and validated.

## Current runtime

- Rust Axum/Tokio exposes the production HTTP API; the Docker image starts the Rust binary.
- SQLx persists provider, project, token and audit state in PostgreSQL; SQLx migrations run during startup.
- FastAPI and SQLAlchemy remain temporary compatibility/reference implementations, not the production container entrypoint.
- Root bearer authentication and project-scoped hashed tokens enforce owner/operator/viewer roles and project capability/provider allowlists.
- Provider selection is based on enabled state, capability, health and priority; ties are deterministic by provider ID.

## Infrastructure account verification

The Rust runtime includes read-only account/API checks for Northflank, Railway, Cloudflare, Netlify, Render, DigitalOcean, Fly.io, Supabase Management API, Neon, Supabase Storage, Cloudflare R2 and Backblaze B2. It resolves credentials through Infisical, uses fixed HTTPS endpoints, bounded timeouts, no redirects, normalized responses and redacted audit events. Supabase Storage bucket inventory, R2 bucket inventory, B2 account authorization and S3-compatible SigV4 bucket verification are supported for AWS S3, Cloudflare R2, Supabase Storage S3 protocol and Backblaze B2. Object upload/download lifecycle operations are not yet implemented. A self-managed PostgreSQL connection probe uses a secret URL, forces TLS, rejects private destinations unless an exact hostname is explicitly allowlisted and runs a read-only SELECT 1. Redis connectivity requires rediss://, rejects private destinations unless an exact hostname is explicitly allowlisted and sends a read-only PING.

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
