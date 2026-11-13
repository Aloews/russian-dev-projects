# Project onboarding contract

Each project is onboarded independently. Existing production infrastructure remains unchanged until a separate migration has passed its own acceptance gates.

## Discovery

Collect repository URL, default branch, runtime, build/test commands, current hosting, databases, storage, queues, external APIs, secrets references, domains, regions, expected traffic, budget ceilings, RPO/RTO and deployment constraints.

## Automated checks

- Read repository instructions and project map.
- Inventory workflows, dependencies, environment variable names and deployment manifests.
- Detect provider coupling and persistence assumptions.
- Run existing tests and record baseline failures before proposing changes.
- Map each dependency to at least one viable alternative, including migration and data-export paths.
- Verify credentials using least-privilege, non-production checks.

## Onboarding result

Produce a machine-readable project manifest containing service dependencies, provider adapters, secret references, health checks, cost ceilings, deployment targets, rollback instructions and owners. Do not store secret values in the manifest.

## Activation

A project becomes managed only after its manifest validates, adapters pass contract tests, staging smoke tests pass, monitoring is live and rollback is demonstrated. Until then, Hub remains advisory and cannot change production traffic.
