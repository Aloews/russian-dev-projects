---
name: release-gate
description: Mandatory acceptance gates for merge, deployment and product completion claims.
---
# Release gate

1. Map every requirement to an observable acceptance criterion and test.
2. Inspect the complete diff, generated files, dependencies and secrets.
3. Run relevant format, lint, typecheck, unit, integration, migration, security and build checks.
4. Start the actual runtime and exercise critical workflows with intended dependencies.
5. Verify authorization, persistence, read-after-write, idempotency, observability and rollback.
6. Run independent adversarial tests against failure, retry and recovery paths.
7. Verify real UI controls, accessibility, responsive layouts and failure states where applicable.
8. Require all mandatory checks green on the exact final HEAD; stale runs do not count.
9. Document rollback/recovery for risky changes.
10. Attach commit SHA, run IDs, commands, exit codes, logs, artifacts and residual risks.

Missing or failed critical evidence means PARTIAL or BLOCKED, never DONE. A passing CI does not by itself authorize merge or production deployment.
