-- Durable execution topology: role-bound workers, workspaces, worktrees, DAG edges and attempts.
-- This migration adds metadata only; it does not start workers or mutate external infrastructure.

ALTER TABLE engineering_tasks
    ADD COLUMN IF NOT EXISTS available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS lease_generation BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_checkpoint_id VARCHAR(36),
    ADD COLUMN IF NOT EXISTS blocked_reason TEXT,
    ADD COLUMN IF NOT EXISTS model_attempts INTEGER NOT NULL DEFAULT 0
        CHECK (model_attempts BETWEEN 0 AND 100),
    ADD COLUMN IF NOT EXISTS last_model_id VARCHAR(160),
    ADD COLUMN IF NOT EXISTS last_strategy VARCHAR(80),
    ADD COLUMN IF NOT EXISTS acceptance_progress NUMERIC(5,4) NOT NULL DEFAULT 0
        CHECK (acceptance_progress BETWEEN 0 AND 1);

CREATE INDEX IF NOT EXISTS ix_engineering_tasks_available_dispatch
    ON engineering_tasks(queue_name, priority ASC, available_at ASC, created_at ASC)
    WHERE status IN ('queued','queued_build','queued_test','review','recovery');

CREATE TABLE engineering_worker_tokens (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL,
    project_id VARCHAR(80) NOT NULL,
    name VARCHAR(120) NOT NULL,
    worker_id VARCHAR(120) NOT NULL,
    worker_role VARCHAR(24) NOT NULL
        CHECK (worker_role IN ('architect','implementer','builder','tester','reviewer','recovery')),
    queue_name VARCHAR(24) NOT NULL
        CHECK (
            (worker_role='architect' AND queue_name='planning') OR
            (worker_role='implementer' AND queue_name='implementation') OR
            (worker_role='builder' AND queue_name='build') OR
            (worker_role='tester' AND queue_name='test') OR
            (worker_role='reviewer' AND queue_name='review') OR
            (worker_role='recovery' AND queue_name='recovery')
        ),
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    CONSTRAINT fk_engineering_worker_tokens_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX ix_engineering_worker_tokens_active
    ON engineering_worker_tokens(tenant_id, project_id, worker_role)
    WHERE revoked_at IS NULL;
CREATE UNIQUE INDEX uq_engineering_worker_tokens_active_id
    ON engineering_worker_tokens(tenant_id, project_id, worker_id)
    WHERE revoked_at IS NULL;

CREATE TABLE engineering_workspaces (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL,
    project_id VARCHAR(80) NOT NULL,
    workspace_key VARCHAR(160) NOT NULL,
    runtime_provider VARCHAR(32) NOT NULL
        CHECK (runtime_provider IN ('railway','github_actions','local','other')),
    repository VARCHAR(200) NOT NULL,
    base_branch VARCHAR(200) NOT NULL,
    base_sha VARCHAR(40) NOT NULL CHECK (base_sha ~ '^[0-9a-fA-F]{40}$'),
    git_worktree_path TEXT,
    git_branch VARCHAR(240),
    head_sha VARCHAR(40) CHECK (head_sha IS NULL OR head_sha ~ '^[0-9a-fA-F]{40}$'),
    state VARCHAR(20) NOT NULL DEFAULT 'idle'
        CHECK (state IN ('provisioning','idle','leased','releasing','failed','destroyed')),
    lease_owner VARCHAR(120),
    lease_token_hash VARCHAR(64),
    lease_expires_at TIMESTAMPTZ,
    last_heartbeat_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, project_id, workspace_key),
    UNIQUE (tenant_id, id),
    CONSTRAINT fk_engineering_workspaces_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE,
    CHECK (
        (state = 'leased' AND lease_owner IS NOT NULL AND lease_token_hash IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR state <> 'leased'
    )
);
CREATE INDEX ix_engineering_workspaces_available
    ON engineering_workspaces(tenant_id, project_id, state, updated_at);

CREATE TABLE engineering_task_attempts (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL,
    project_id VARCHAR(80) NOT NULL,
    task_id VARCHAR(36) NOT NULL,
    attempt_no INTEGER NOT NULL CHECK (attempt_no BETWEEN 1 AND 100),
    worker_id VARCHAR(120) NOT NULL,
    worker_role VARCHAR(24) NOT NULL
        CHECK (worker_role IN ('architect','implementer','builder','tester','reviewer','recovery')),
    provider_id VARCHAR(80),
    model_id VARCHAR(160),
    strategy VARCHAR(80) NOT NULL,
    outcome VARCHAR(20) NOT NULL DEFAULT 'started'
        CHECK (outcome IN ('started','succeeded','failed','yielded','abandoned')),
    error_code VARCHAR(120),
    error_summary VARCHAR(2000),
    acceptance_progress NUMERIC(5,4) NOT NULL DEFAULT 0
        CHECK (acceptance_progress BETWEEN 0 AND 1),
    estimated_cost_minor BIGINT CHECK (estimated_cost_minor IS NULL OR estimated_cost_minor >= 0),
    actual_cost_minor BIGINT CHECK (actual_cost_minor IS NULL OR actual_cost_minor >= 0),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    UNIQUE (task_id, attempt_no),
    CONSTRAINT fk_engineering_task_attempts_tenant_task
        FOREIGN KEY (tenant_id, task_id) REFERENCES engineering_tasks(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_engineering_task_attempts_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX ix_engineering_task_attempts_task
    ON engineering_task_attempts(tenant_id, task_id, attempt_no DESC);

CREATE TABLE engineering_task_dependencies (
    tenant_id VARCHAR(80) NOT NULL,
    project_id VARCHAR(80) NOT NULL,
    parent_task_id VARCHAR(36) NOT NULL,
    child_task_id VARCHAR(36) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, parent_task_id, child_task_id),
    CHECK (parent_task_id <> child_task_id),
    CONSTRAINT fk_engineering_task_dependencies_parent
        FOREIGN KEY (tenant_id, parent_task_id) REFERENCES engineering_tasks(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_engineering_task_dependencies_child
        FOREIGN KEY (tenant_id, child_task_id) REFERENCES engineering_tasks(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_engineering_task_dependencies_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX ix_engineering_task_dependencies_child
    ON engineering_task_dependencies(tenant_id, child_task_id);

CREATE TABLE engineering_task_workspace_assignments (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL,
    project_id VARCHAR(80) NOT NULL,
    task_id VARCHAR(36) NOT NULL,
    workspace_id VARCHAR(36) NOT NULL,
    worker_role VARCHAR(24) NOT NULL
        CHECK (worker_role IN ('architect','implementer','builder','tester','reviewer','recovery')),
    writer_scope JSONB NOT NULL DEFAULT '[]'::jsonb,
    base_sha VARCHAR(40) NOT NULL CHECK (base_sha ~ '^[0-9a-fA-F]{40}$'),
    candidate_sha VARCHAR(40) CHECK (candidate_sha IS NULL OR candidate_sha ~ '^[0-9a-fA-F]{40}$'),
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    released_at TIMESTAMPTZ,
    CONSTRAINT fk_engineering_task_workspace_assignments_task
        FOREIGN KEY (tenant_id, task_id) REFERENCES engineering_tasks(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_engineering_task_workspace_assignments_workspace
        FOREIGN KEY (tenant_id, workspace_id) REFERENCES engineering_workspaces(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_engineering_task_workspace_assignments_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX uq_engineering_task_active_workspace_assignment
    ON engineering_task_workspace_assignments(tenant_id, task_id)
    WHERE released_at IS NULL;
CREATE INDEX ix_engineering_task_workspace_assignments_workspace
    ON engineering_task_workspace_assignments(tenant_id, workspace_id, assigned_at DESC);

CREATE TABLE engineering_task_path_locks (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL,
    project_id VARCHAR(80) NOT NULL,
    task_id VARCHAR(36) NOT NULL,
    workspace_id VARCHAR(36),
    path_prefix TEXT NOT NULL CHECK (length(path_prefix) BETWEEN 1 AND 500),
    lock_owner VARCHAR(120) NOT NULL,
    lease_expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_engineering_task_path_locks_task
        FOREIGN KEY (tenant_id, task_id) REFERENCES engineering_tasks(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_engineering_task_path_locks_workspace
        FOREIGN KEY (tenant_id, workspace_id) REFERENCES engineering_workspaces(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_engineering_task_path_locks_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX ix_engineering_task_path_locks_active
    ON engineering_task_path_locks(tenant_id, project_id, lease_expires_at);

ALTER TABLE engineering_task_checkpoints
    ADD COLUMN workspace_id VARCHAR(36),
    ADD COLUMN git_ref VARCHAR(240),
    ADD COLUMN worktree_path TEXT,
    ADD CONSTRAINT fk_engineering_task_checkpoints_workspace
        FOREIGN KEY (tenant_id, workspace_id) REFERENCES engineering_workspaces(tenant_id, id) ON DELETE RESTRICT;

CREATE FUNCTION ush_check_checkpoint_workspace_project() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE task_project VARCHAR(80); workspace_project VARCHAR(80);
BEGIN
    IF NEW.workspace_id IS NULL THEN RETURN NEW; END IF;
    SELECT project_id INTO task_project
      FROM engineering_tasks WHERE id=NEW.task_id AND tenant_id=NEW.tenant_id;
    SELECT project_id INTO workspace_project
      FROM engineering_workspaces WHERE id=NEW.workspace_id AND tenant_id=NEW.tenant_id;
    IF task_project IS NULL OR workspace_project IS NULL OR task_project <> workspace_project THEN
        RAISE EXCEPTION 'checkpoint workspace must belong to the task project and tenant';
    END IF;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER trg_engineering_task_checkpoints_workspace_project
    BEFORE INSERT OR UPDATE OF workspace_id ON engineering_task_checkpoints
    FOR EACH ROW EXECUTE FUNCTION ush_check_checkpoint_workspace_project();

-- Every child row derives tenant/project ownership from its canonical parent.
CREATE FUNCTION ush_assign_worker_token_tenant() RETURNS trigger LANGUAGE plpgsql AS $function$
BEGIN
    SELECT tenant_id INTO NEW.tenant_id FROM projects WHERE id=NEW.project_id;
    IF NEW.tenant_id IS NULL THEN RAISE EXCEPTION 'worker token project tenant not found'; END IF;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER trg_engineering_worker_tokens_assign_tenant
    BEFORE INSERT ON engineering_worker_tokens FOR EACH ROW
    EXECUTE FUNCTION ush_assign_worker_token_tenant();

CREATE FUNCTION ush_assign_workspace_tenant() RETURNS trigger LANGUAGE plpgsql AS $function$
BEGIN
    SELECT tenant_id INTO NEW.tenant_id FROM projects WHERE id=NEW.project_id;
    IF NEW.tenant_id IS NULL THEN RAISE EXCEPTION 'workspace project tenant not found'; END IF;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER trg_engineering_workspaces_assign_tenant
    BEFORE INSERT ON engineering_workspaces FOR EACH ROW
    EXECUTE FUNCTION ush_assign_workspace_tenant();

CREATE FUNCTION ush_assign_task_attempt_tenant() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE owning_tenant VARCHAR(80); owning_project VARCHAR(80);
BEGIN
    SELECT tenant_id, project_id INTO owning_tenant, owning_project
      FROM engineering_tasks WHERE id=NEW.task_id;
    IF owning_tenant IS NULL THEN RAISE EXCEPTION 'attempt task not found'; END IF;
    IF NEW.project_id <> owning_project THEN RAISE EXCEPTION 'attempt project mismatch'; END IF;
    NEW.tenant_id := owning_tenant;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER trg_engineering_task_attempts_assign_tenant
    BEFORE INSERT ON engineering_task_attempts FOR EACH ROW
    EXECUTE FUNCTION ush_assign_task_attempt_tenant();

CREATE FUNCTION ush_assign_task_dependency_tenant() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE parent_tenant VARCHAR(80); parent_project VARCHAR(80); child_tenant VARCHAR(80); child_project VARCHAR(80);
BEGIN
    SELECT tenant_id, project_id INTO parent_tenant, parent_project
      FROM engineering_tasks WHERE id=NEW.parent_task_id;
    SELECT tenant_id, project_id INTO child_tenant, child_project
      FROM engineering_tasks WHERE id=NEW.child_task_id;
    IF parent_tenant IS NULL OR child_tenant IS NULL THEN RAISE EXCEPTION 'dependency task not found'; END IF;
    IF parent_tenant <> child_tenant OR parent_project <> child_project OR NEW.project_id <> parent_project THEN
        RAISE EXCEPTION 'task dependencies must stay within one project and tenant';
    END IF;
    NEW.tenant_id := parent_tenant;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER trg_engineering_task_dependencies_assign_tenant
    BEFORE INSERT ON engineering_task_dependencies FOR EACH ROW
    EXECUTE FUNCTION ush_assign_task_dependency_tenant();

-- Backfill only valid same-project dependency edges from the original JSON representation.
INSERT INTO engineering_task_dependencies (tenant_id,project_id,parent_task_id,child_task_id)
SELECT task.tenant_id,task.project_id,dependency.dependency_id,task.id
FROM engineering_tasks task
CROSS JOIN LATERAL jsonb_array_elements_text(task.dependencies) AS dependency(dependency_id)
JOIN engineering_tasks parent
  ON parent.id=dependency.dependency_id
 AND parent.tenant_id=task.tenant_id
 AND parent.project_id=task.project_id
ON CONFLICT DO NOTHING;

CREATE FUNCTION ush_assign_task_workspace_assignment_tenant() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE task_tenant VARCHAR(80); task_project VARCHAR(80); workspace_tenant VARCHAR(80); workspace_project VARCHAR(80);
BEGIN
    SELECT tenant_id, project_id INTO task_tenant, task_project
      FROM engineering_tasks WHERE id=NEW.task_id;
    SELECT tenant_id, project_id INTO workspace_tenant, workspace_project
      FROM engineering_workspaces WHERE id=NEW.workspace_id;
    IF task_tenant IS NULL OR workspace_tenant IS NULL THEN RAISE EXCEPTION 'task or workspace not found'; END IF;
    IF task_tenant <> workspace_tenant OR task_project <> workspace_project OR NEW.project_id <> task_project THEN
        RAISE EXCEPTION 'task and workspace must share project and tenant';
    END IF;
    NEW.tenant_id := task_tenant;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER trg_engineering_task_workspace_assignments_assign_tenant
    BEFORE INSERT ON engineering_task_workspace_assignments FOR EACH ROW
    EXECUTE FUNCTION ush_assign_task_workspace_assignment_tenant();

CREATE FUNCTION ush_assign_task_path_lock_tenant() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE task_tenant VARCHAR(80); task_project VARCHAR(80);
BEGIN
    SELECT tenant_id, project_id INTO task_tenant, task_project
      FROM engineering_tasks WHERE id=NEW.task_id;
    IF task_tenant IS NULL THEN RAISE EXCEPTION 'path lock task not found'; END IF;
    IF NEW.project_id <> task_project THEN RAISE EXCEPTION 'path lock project mismatch'; END IF;
    IF NEW.workspace_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM engineering_workspaces
        WHERE id=NEW.workspace_id AND tenant_id=task_tenant AND project_id=task_project
    ) THEN RAISE EXCEPTION 'path lock workspace mismatch'; END IF;
    NEW.tenant_id := task_tenant;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER trg_engineering_task_path_locks_assign_tenant
    BEFORE INSERT ON engineering_task_path_locks FOR EACH ROW
    EXECUTE FUNCTION ush_assign_task_path_lock_tenant();

-- Preserve a durable event trail for attempt and workspace lifecycle transitions.
CREATE INDEX ix_engineering_task_checkpoints_workspace
    ON engineering_task_checkpoints(tenant_id, workspace_id, created_at DESC);
