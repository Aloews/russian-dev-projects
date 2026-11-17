---
name: verified-autonomous-continuation
description: Automatically verify CI and continue safe, authorized engineering work without asking the user to confirm routine green checks. Use for PR monitoring, build/test completion, issue resolution, and multi-step repository delivery.
---

# Verified Autonomous Continuation

## Mission

Remove routine status-checking and confirmation work from the user. The agent owns the verification loop and continues the next safe engineering step when objective evidence satisfies the gate. A green check is permission to continue the already-authorized engineering task; it is not permission to merge, deploy, spend money, change credentials, or mutate production.

## Operating rules

1. Inspect the current repository, project map, active PRs, branch dependencies, and exact head SHA before acting.
2. Treat GitHub check state as authoritative only for the exact current PR head. A successful run for an older SHA does not verify a newer commit.
3. Check all required workflows and jobs. A skipped, cancelled, neutral, missing, stale, or still-running required check is not a pass.
4. Inspect failed jobs and relevant logs. Fix the root cause, add a regression test where practical, and rerun checks against the new exact head.
5. If all required checks pass on the exact head, do not ask the user whether to proceed. Continue with the next independent, pre-authorized milestone in the project map.
6. Keep work bounded to one coherent vertical slice. Define acceptance criteria before editing; inspect the complete diff after editing; run relevant local and CI checks.
7. Use one writer per overlapping file set. Use independent read-only review and QA where available. Do not treat an implementer's self-review as independent review.
8. Keep PRs open and Draft unless the user has separately authorized a merge. Passing CI never grants merge or deployment permission.
9. Continue independent work when blocked. Record the precise blocker and move to other safe tasks instead of repeatedly asking the user for routine decisions.
10. Do not send routine progress messages. Update the project map, PR description and verification evidence. Notify the user only at a meaningful milestone, on a genuinely unavoidable blocker, or when an explicit approval boundary is reached.

## Automatic continuation gate

Proceed without user confirmation only when all are true:
- The repository and target branch are the intended ones.
- The PR is open and the exact head SHA has been identified.
- Every required CI check for that exact SHA has conclusion success.
- No unresolved merge conflict, failing required check, security alert, or blocking review request is present.
- The next action is within the existing task scope and safety boundaries below.
- Acceptance criteria and a rollback/recovery path are understood.

If any condition is false, do not label the candidate verified. Diagnose, repair, or continue with unrelated safe work.

## Authorized without additional confirmation

- Read repository files, issues, PRs, CI runs, logs, test results and project documentation.
- Create branches and Draft PRs for bounded implementation work in the authorized repository.
- Edit source, tests and documentation within the repository's existing contribution policy.
- Run non-billable local checks and repository CI already configured by the owner.
- Repair failing tests, lint, formatting, migration smoke tests and build failures.
- Recompute project plans and update project maps using evidence.
- Continue the next planned implementation milestone after exact-head verification.
- Request read-only review and record its findings.

## Always stop for explicit approval

- Paid provider calls, cloud resource creation, subscriptions, billing changes, or actions with uncertain spend.
- Secret, credential, token, permission, identity-provider, or account-security changes.
- Production deployment, production database migration, production-data mutation, DNS changes, or changes to live customer traffic.
- Merging to the protected/default branch, releasing/publishing, or deleting repositories, branches, environments, data, or backups.
- Expanding repository/path permissions, disabling security controls, bypassing CI/review gates, or changing budget ceilings.
- Any irreversible or externally visible action not already explicitly authorized.

## Evidence standard

For each milestone record repository, PR, branch and exact commit SHA; required workflow/run IDs and conclusions; job-level outcomes and relevant test/migration evidence; changed paths and diff review result; acceptance criteria mapped to checks; known limitations and unverified runtime/provider/production behaviour.

Use these labels precisely:
- VERIFIED: all relevant acceptance criteria have direct evidence on the exact candidate SHA.
- IN PROGRESS: work or required checks are still running.
- BLOCKED: a concrete external dependency or approval prevents progress.
- NOT VERIFIED: evidence is absent, stale, incomplete or from a different SHA.

Never infer successful deployment, real provider connectivity, production readiness, or autonomous worker execution from a passing repository CI run.

## Recovery and anti-loop rules

- Never repeatedly poll a running job in a tight loop. Use event notifications where available or a supported scheduled check.
- On a transient CI failure, inspect the failure before retrying. Rerun only when the failure is plausibly infrastructure-related or the root cause has been corrected.
- Do not retry provider calls or paid operations without idempotency and a known spend bound.
- Preserve candidate diffs and checkpoints before recovery operations.
- If the current branch head changes during verification, discard the old pass result and verify the new exact SHA.
- Do not mark work complete solely because a PR exists or a workflow was queued.

## User communication

The user should not need to manually check routine status or approve continuation after successful checks. Report concise milestone outcomes with links and exact evidence. Ask only for decisions that cross the approval boundaries above. Never claim that this skill creates continuous background execution by itself; it must be invoked by an active agent or a configured automation/event workflow.