# Adaptive Delivery & Recovery Engine

## Scope of the first implementation slice

The first slice provides a deterministic, tenant-scoped schedule-recovery preview. It accepts a bounded task DAG, calculates the remaining critical path, measures projected slip against the plan's remaining-hour baseline, and returns a recovery tier with actions.

It is an advisory calculation only. It does not silently change deadlines, reprioritize persisted work, create agents, move tasks, dispatch providers, spend money or modify project data. Those actions require a durable task system, explicit policies and separate acceptance tests.

## API

`POST /v1/projects/{project_id}/delivery/recovery-plan`

Authentication:
- Requires a project owner or operator token scoped to the path's project, or the Hub administrator token.
- Viewer tokens are denied.
- Cross-project tokens are denied even if the token is otherwise valid.
- The project must exist.

Example request:

```json
{
  "planned_remaining_hours": 40,
  "tasks": [
    {"id": "architecture", "remaining_hours": 4, "completed": false, "depends_on": []},
    {"id": "api", "remaining_hours": 12, "completed": false, "depends_on": ["architecture"]},
    {"id": "ui", "remaining_hours": 10, "completed": false, "depends_on": ["architecture"]},
    {"id": "integration", "remaining_hours": 8, "completed": false, "depends_on": ["api", "ui"]},
    {"id": "release-checks", "remaining_hours": 5, "completed": false, "depends_on": ["integration"]}
  ]
}
```

The endpoint returns:
- `status`: `complete`, `on_track`, `optimize`, `parallelize`, `rescope` or `replan`;
- planned and forecast remaining hours, signed variance, positive delay hours and delay percentage;
- critical-path task IDs;
- currently ready tasks (all dependencies complete) and blocked tasks;
- completed/total task counts and recommended actions.

## Calculation and thresholds

The forecast is the longest remaining dependency path, not the sum of all task effort. Completed tasks contribute zero remaining effort. Independent tasks can therefore run in parallel in the forecast; the engine does not assume unlimited agent capacity or guarantee that the forecast will be achieved.

`delay_percent = max(forecast_remaining_hours - planned_remaining_hours, 0) / planned_remaining_hours * 100`

| Delay vs. planned remaining hours | Tier | Suggested response |
|---:|---|---|
| 0–5% | `on_track` | Preserve the plan; monitor estimates and blockers |
| >5–10% | `optimize` | Rebalance work on the critical path; inspect review/CI queues |
| >10–20% | `parallelize` | Isolate independent workspaces and assign distinct roles |
| >20–30% | `rescope` | Defer non-essential scope; retain security and release gates |
| >30% | `replan` | Rebuild the plan from evidence and perform independent diagnosis |

The thresholds are initial policy defaults, not universal productivity benchmarks. They must be calibrated against measured delivery data before any automated schedule mutation is enabled.

## Input limits and validation

- Maximum 250 tasks per request.
- Task IDs are 1–80 ASCII characters from letters, digits, dot, underscore and hyphen.
- Planned remaining hours must be finite and greater than zero.
- Remaining effort must be finite and non-negative.
- Completed tasks cannot retain remaining effort.
- Dependencies must refer to known, distinct task IDs; self-dependencies and cycles are rejected.
- Maximum 250 dependencies per task.
- No prompt, task description, secret, source diff or generated content is accepted or written to the audit log.
- Audit records contain only project ID, task count, status, delay metrics and critical-path length.

## Verification

The Rust unit tests cover:
- longest dependency path vs. total effort;
- every recovery threshold;
- dependency release after completion;
- all-completed plans;
- cycle, unknown dependency and duplicate-ID rejection;
- invalid hours and inconsistent completion state;
- owner/operator/admin access and viewer/cross-tenant denial.

The CI workflow must pass on the exact candidate SHA. A green unit/CI result validates the calculation and route code, not durable schedule monitoring or actual agent reassignment.

## Next increments

1. Persist project plans, task DAG versions, estimates and immutable progress events in PostgreSQL.
2. Calculate forecasts from persisted, timestamped task history rather than caller-supplied snapshots.
3. Add schedule snapshots, forecast history, drift trends and three-point estimates with confidence ranges.
4. Add an authenticated dashboard and alert policy with per-project notification preferences.
5. Add a policy-gated recovery executor: only create isolated assignments for ready independent tasks; never bypass review, security, spend or production gates.
6. Run simulations with worker loss, stale estimates, blocked dependencies, CI queue delays, provider outages and project-level concurrency limits.
7. Calibrate thresholds using actual cycle-time and accepted-delivery data.

## Release boundary

No production deployment, paid resource creation, provider call, secret access or production-data mutation is part of this increment.
