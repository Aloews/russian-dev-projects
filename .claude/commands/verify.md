# Verify current task

1. Read all applicable AGENTS.md, CLAUDE.md, project maps and standards.
2. Record branch, exact HEAD, dirty state, open PRs and changed paths.
3. Map acceptance criteria to tests and identify real dependencies.
4. Run targeted tests, then relevant format, lint, typecheck, integration, migration and build checks.
5. Start the actual runtime and exercise critical user flows with intended dependencies.
6. Inspect raw logs, artifacts, database read-after-write and browser behavior.
7. Test mock substitution, skipped tests, stale SHA, hidden exceptions, partial writes, authorization bypass and recovery.
8. Fix root causes and rerun affected gates on the exact final HEAD.
9. Report PASS only with evidence; otherwise PARTIAL/BLOCKED and exact reason.
Never deploy, merge, mutate production or incur paid provider costs without explicit authorization.
