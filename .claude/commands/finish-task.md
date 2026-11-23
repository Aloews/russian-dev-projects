# Finish task

- Compare the final diff with the requested scope; remove unrelated changes.
- Verify every acceptance criterion, including failure and recovery paths.
- Confirm required checks are green on the exact final HEAD; inspect skipped steps.
- Verify real integrations and persistence where the task claims them.
- Update the applicable project map, architecture and operational docs.
- Record commit SHA, PR, run IDs, commands, exit codes, runtime evidence and residual risks.
- Check that secrets and sensitive data did not enter diffs, logs or artifacts.
- Do not merge, deploy, mutate production or incur paid costs without explicit authorization.
- Missing mandatory evidence means PARTIAL or BLOCKED, never DONE.
