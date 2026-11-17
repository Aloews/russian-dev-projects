# Workspace and Git Worktree Control Plane

## Status and boundary

This document defines the next implementation contract. It is an architecture decision and acceptance plan, not evidence that workspace provisioning or Git commands already exist. The implementation must follow the durable worker queue and pass its exact-head checks first. No repository is cloned and no worktree is created by this document.

## Product model

A **project** is the tenant-owned product boundary. A **repository** is a version-controlled source belonging to a project. A **workspace** is a durable, user-visible development environment bound to one project and one repository revision. A **worktree** is an isolated Git checkout allocated to one workspace/task/worker execution. A workspace may contain several worktrees over time; a worktree has at most one active writer lease.

The Hub database is the control-plane source of truth. Git and the worker host are execution substrates whose state must be reconciled against durable metadata after restart.

## Data model to implement

### `project_repositories`

- `id UUID`, `project_id`, canonical provider/repository identity, owner/name, default branch, visibility, installation reference, encrypted/secret-manager credential reference (never token value), fetch policy, enabled state and timestamps.
- Unique provider/repository identity within a project; composite project foreign keys.
- Repository access is granted through a scoped GitHub App installation or another explicit provider connection. A repository URL supplied by a client is not an unrestricted network destination.

### `workspaces`

- `id UUID`, `project_id`, `repository_id`, human-readable name, lifecycle state (`provisioning`, `ready`, `busy`, `suspended`, `failed`, `deleting`, `deleted`), base ref and immutable base SHA, owner/creator, retention policy, last activity and timestamps.
- Composite foreign keys bind repository and workspace to the same project.
- Workspace creation records the exact repository and base SHA. Moving a workspace to a new base is an explicit operation that creates a new worktree or reviewed integration checkpoint; it never silently rewrites a running worker's base.

### `workspace_worktrees`

- `id UUID`, `project_id`, `workspace_id`, optional `task_id` and `execution_job_id`, generated branch name, immutable base SHA, current HEAD SHA, host/runner identity, generated absolute path, lifecycle state (`creating`, `ready`, `dirty`, `committing`, `checkpointed`, `integrating`, `quarantined`, `removing`, `removed`), last observed Git status and timestamps.
- Composite tenant foreign keys for workspace, task and job.
- Unique active branch/worktree identity within the repository; no client-controlled filesystem path.
- Worktree metadata is never accepted as proof of its on-disk state; the worker must inspect Git directly.

### `workspace_locks`

- One active writer lease per worktree, with worker ID, execution job ID, random fencing token, lease expiry, heartbeat and acquisition time.
- Lock acquisition is transactional. Expired locks can be reclaimed only after the old worker is fenced out.
- A lease token must be checked before write, commit, reset, merge, push or cleanup operations. Read-only inspection may use a separate bounded shared-reader policy.

### `workspace_events`

- Append-only metadata events for create, checkout, lock, checkpoint, commit, review, integration, quarantine and deletion.
- Store identifiers, SHAs, state transitions, durations and sanitized error codes only. Never store file content, diffs, credentials, prompts or raw agent transcripts.

## Runtime layout and Git operations

- Configure one owner-controlled root directory per worker host (for example `USH_WORKSPACE_ROOT`). Never derive a path from a project name, branch, repository URL or user input.
- Use a bare mirror cache per repository identity, then `git worktree add` for isolated task branches. Mirror and worktree paths are generated from UUIDs beneath the configured root; canonicalize and verify containment before every filesystem operation.
- Create a unique branch per task/execution attempt. A retry may resume the same worktree only when its checkpoint, branch, base SHA, current HEAD and lock ownership match; otherwise quarantine and create a new isolated worktree from the recorded checkpoint.
- Use a credential helper/askpass bridge that supplies short-lived GitHub App installation tokens only for the required Git operation. Do not persist tokens in remotes, config, process arguments, logs or workspace files. Remotes remain canonical HTTPS URLs without userinfo.
- Restrict outbound network access to approved Git providers. Validate repository identity against the connected installation and allowed project repository record before fetch/clone.
- Do not run arbitrary repository hooks or untrusted build scripts with Hub credentials. Git hooks are disabled for managed operations; build/test execution occurs in a separate sandbox with no provider credentials.
- Each worktree has exactly one writer at a time. Independent agents may inspect immutable commits or separate worktrees, but must not concurrently mutate the same checkout or branch.
- Checkpoint before executor replacement: preserve base SHA, branch, HEAD SHA, staged/unstaged status, untracked-file inventory, task contract, acceptance criteria, completed checks and sanitized failure evidence. Stop the previous writer before a replacement acquires the lock.
- Integrate through a separate integration worktree. Rebase/merge is never performed inside a worker's active task worktree. Conflict resolution produces a candidate commit and requires the full affected test suite plus independent review.
- Push and PR publication are distinct approval/policy gates. Workspace creation and local branch commits do not imply permission to push, merge or deploy.

## Crash recovery and reconciliation

On worker startup and periodically thereafter:

1. Load non-terminal workspace/worktree rows for the worker host.
2. Enumerate actual Git worktrees from each bare mirror; compare path, branch and HEAD against durable metadata.
3. Verify filesystem containment, repository identity and lock lease/fencing token.
4. Reattach only when the persisted worker/job checkpoint and actual Git state agree.
5. Reclaim expired writer locks only after fencing the previous worker; never allow a resumed stale process to continue writes.
6. If a path is missing, a branch is unexpectedly changed, metadata disagrees, or a dirty worktree cannot be safely attributed, mark it `quarantined` and preserve it for inspection. Do not silently reset or delete it.
7. Resume a job from its last durable checkpoint, not from a worker's in-memory state. The worker reports the exact current HEAD SHA before continuing.

A host restart must not lose the repository mirror, worktree files, branch, commits or checkpoint. This requires a persistent worker volume or an explicit rematerialization procedure from remote Git plus durable checkpoints/artifacts. Ephemeral runners cannot promise local filesystem survival; they must reconstruct from Git and stored artifacts and must not claim the previous checkout survived.

## API surface (planned; do not expose until implemented and tested)

- Register/list/disable a project repository.
- Create/list/get/suspend/resume/delete a workspace.
- Create a task worktree from an exact base SHA.
- Acquire/heartbeat/release a writer lease.
- Inspect status/diff/branch/HEAD and checkpoint metadata.
- Commit a validated candidate with a deterministic idempotency key.
- Create an isolated integration worktree and produce a conflict report.
- Quarantine/restore/remove worktrees under explicit retention rules.

All operations derive project scope from the authenticated principal. Client-supplied project IDs are checked against the token scope. Every mutating Git operation is recorded in the append-only event stream.

## Acceptance matrix

### Isolation and authorization
- Cross-project repository/workspace/worktree/task/job references fail at SQL constraints and API authorization.
- Repository URLs cannot target localhost, private/link-local/metadata networks or unapproved Git hosts.
- No Git credential appears in process arguments, remote config, files, logs, artifacts or audit records.
- Path traversal, symlink escape, malicious branch names and repository-name collisions are rejected.

### Concurrency and fencing
- Two concurrent writers cannot acquire the same worktree.
- Expired lease recovery fences the old writer; old token cannot write, commit, push or delete.
- Parallel tasks receive distinct worktrees and branches; shared files are never edited by multiple writers.
- Branch uniqueness and idempotent workspace creation hold under concurrent requests.

### Recovery
- Kill the worker during clone, checkout, file edit, commit and checkpoint; restart and reconcile without losing acknowledged state.
- Resume only when the actual Git state matches persisted checkpoint metadata.
- Missing or inconsistent worktrees are quarantined, never silently reset/deleted.
- Dirty worktrees survive worker process restart on persistent hosts; ephemeral runners reconstruct only from remote commits and durable artifacts.

### Git correctness
- Exact base SHA is recorded and verified.
- Status, diff, untracked files and HEAD SHA are inspected before checkpoint/commit.
- Integration conflicts are isolated from task worktrees and produce a reviewable candidate.
- CI and independent review inspect the exact candidate SHA before publication.

### Lifecycle and operations
- Cleanup refuses active leases, dirty/uncheckpointed changes, pending reviews or unresolved artifacts.
- Retention policy and storage quotas are enforced per project.
- Metrics cover active workspaces, worktrees, lock contention, recovery, quarantine, disk use and cleanup failures.
- Backup/restore and worker-volume loss are rehearsed before production release.

## Implementation sequence

1. Finish and verify the durable queue API and recovery semantics.
2. Add repository/workspace/worktree/lock schema with composite tenant constraints and indexes.
3. Implement the filesystem/Git adapter behind an allowlisted repository provider interface.
4. Implement lease-fenced worktree lifecycle and crash reconciliation.
5. Add worker daemon orchestration so jobs acquire/release worktree leases and persist HEAD/checkpoint/artifact references.
6. Add isolated integration worktrees, conflict resolution and candidate review gates.
7. Add metrics, retention, quotas, recovery drills and end-to-end tests.

No step authorizes production repository access, push, merge, deployment or paid infrastructure without the separate approval required by the Hub safety policy.
