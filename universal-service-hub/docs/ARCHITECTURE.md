# Architecture

## Control-plane boundaries

The Hub is a control plane, not a proxy for arbitrary provider traffic. It owns provider metadata, eligibility decisions, policy evaluation and audit history. Workload data and application databases remain with the projects unless a migration is explicitly planned and validated.

### Initial runtime

- FastAPI exposes health, provider inventory, health probing and provider selection.
- `ProviderRegistry` validates unique provider identifiers and keeps the current in-memory health state.
- Provider configuration is loaded from `USH_PROVIDERS_JSON`.
- A provider is selectable only when enabled, capable of the requested operation, not explicitly excluded and not marked unhealthy.
- Higher priority wins; ties are deterministic by provider ID.

### Target architecture

```text
Projects / Agents
      |
Hub API + Policy Engine
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

### Required adapter contract

Each adapter must declare:
- stable provider ID and supported capabilities;
- authentication through secret references, never plaintext configuration;
- typed request/response models;
- timeout, retry and idempotency behavior;
- health and capability checks;
- normalized errors and usage/cost metadata;
- integration tests with a sandbox account or deterministic provider test server.

### Routing safety

The first release must not silently move stateful workloads between providers. Stateless requests may fail over only when the request is idempotent or has a stable idempotency key. Database failover requires replication, consistency checks, tested recovery objectives and an explicit migration plan.
