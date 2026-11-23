---
name: reality-first
description: Evidence protocol for planning, implementation, debugging, testing and delivery.
---
# Reality-first protocol

Inspect the actual repository, runtime and target environment before acting. Establish branch, exact HEAD, dirty state, project map, architecture, configuration, service availability and baseline.

Classify material statements:
- VERIFIED: directly observed in current repository, runtime, logs or provider response.
- CORROBORATED: supported by independent evidence.
- ESTIMATE: calculated from explicit inputs.
- ASSUMPTION: plausible but unverified.
- UNKNOWN: evidence unavailable.
- DISPUTED: evidence conflicts.

For every test record command, environment, exit code, exact HEAD and mock usage. For provider calls record protocol, model/API ID, sanitized request/response evidence and cost boundary. For persistence, read the written record back. For deployment, verify deployed commit and safe smoke test. Never fabricate progress, agent activity, test output, provider connectivity or deployment.
