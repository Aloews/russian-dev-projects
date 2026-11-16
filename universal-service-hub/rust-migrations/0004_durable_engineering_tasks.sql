CREATE TABLE IF NOT EXISTS engineering_tasks (
    tenant_id VARCHAR(80) NOT NULL,
    id VARCHAR(36) PRIMARY KEY,
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    idempotency_key VARCHAR(160) NOT NULL,
    title VARCHAR(200) NOT NULL,
    objective TEXT NOT NULL,
    acceptance_criteria JSONB NOT NULL DEFAULT '[]'::jsonb,
    dependencies JSONB NOT NULL DEFAULT '[]'::jsonb,
    status VARCHAR(32) NOT NULL DEFAULT 'received'
        CHECK (status IN ('received','classified','planned','queued','in_progress','candidate_ready','queued_build','building','queued_test','testing','review','verified','blocked','stalled','recovery','cancelled','failed')),
    queue_name VARCHAR(24) NOT NULL DEFAULT 'implementation'
        CHECK (queue_name IN ('planning','implementation','build','test','review','recovery')),
    priority INTEGER NOT NULL DEFAULT 100 CHECK (priority BETWEEN 0 AND 1000),
    version BIGINT NOT NULL DEFAULT 1,
    worker_id VARCHAR(120),
    lease_token_hash VARCHAR(64),
    lease_expires_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3 CHECK (max_attempts BETWEEN 1 AND 20),
    last_error_code VARCHAR(120),
    created_by VARCHAR(200) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, idempotency_key),
    UNIQUE(tenant_id, id),
    CONSTRAINT fk_engineering_tasks_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);
-- Derive tenant ownership from the owning project; never trust request-supplied tenant IDs.
CREATE FUNCTION ush_assign_engineering_task_tenant() RETURNS trigger LANGUAGE plpgsql AS $function$
BEGIN
    SELECT tenant_id INTO NEW.tenant_id FROM projects WHERE id=NEW.project_id;
    IF NEW.tenant_id IS NULL THEN RAISE EXCEPTION 'engineering task project tenant not found'; END IF;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER trg_engineering_tasks_assign_tenant BEFORE INSERT ON engineering_tasks
FOR EACH ROW EXECUTE FUNCTION ush_assign_engineering_task_tenant();

CREATE INDEX IF NOT EXISTS ix_engineering_tasks_dispatch
    ON engineering_tasks(queue_name, priority ASC, created_at ASC)
    WHERE status = 'queued';
CREATE INDEX IF NOT EXISTS ix_engineering_tasks_project_updated
    ON engineering_tasks(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS ix_engineering_tasks_lease
    ON engineering_tasks(lease_expires_at)
    WHERE lease_expires_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS engineering_task_events (
    tenant_id VARCHAR(80) NOT NULL,
    id VARCHAR(36) PRIMARY KEY,
    task_id VARCHAR(36) NOT NULL REFERENCES engineering_tasks(id) ON DELETE CASCADE,
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    actor VARCHAR(200) NOT NULL,
    event_type VARCHAR(40) NOT NULL,
    idempotency_key VARCHAR(160),
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(task_id, idempotency_key),
    CONSTRAINT fk_engineering_task_events_tenant_task FOREIGN KEY (tenant_id, task_id) REFERENCES engineering_tasks(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_engineering_task_events_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_engineering_task_events_task
    ON engineering_task_events(task_id, created_at ASC);

CREATE TABLE IF NOT EXISTS engineering_task_checkpoints (
    tenant_id VARCHAR(80) NOT NULL,
    id VARCHAR(36) PRIMARY KEY,
    task_id VARCHAR(36) NOT NULL REFERENCES engineering_tasks(id) ON DELETE CASCADE,
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    worker_id VARCHAR(120) NOT NULL,
    stage VARCHAR(32) NOT NULL,
    base_sha VARCHAR(64),
    candidate_sha VARCHAR(64),
    artifact_digest VARCHAR(128),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_engineering_task_checkpoints_tenant_task FOREIGN KEY (tenant_id, task_id) REFERENCES engineering_tasks(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_engineering_task_checkpoints_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_engineering_task_checkpoints_task
    ON engineering_task_checkpoints(task_id, created_at DESC);

-- Child records inherit ownership from the task and cannot claim another project.
CREATE FUNCTION ush_assign_engineering_child_tenant() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE
    owning_tenant VARCHAR(80);
    owning_project VARCHAR(80);
BEGIN
    SELECT tenant_id, project_id INTO owning_tenant, owning_project
      FROM engineering_tasks WHERE id=NEW.task_id;
    IF owning_tenant IS NULL THEN RAISE EXCEPTION 'engineering task tenant not found'; END IF;
    IF NEW.project_id <> owning_project THEN RAISE EXCEPTION 'engineering task project mismatch'; END IF;
    NEW.tenant_id := owning_tenant;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER trg_engineering_task_events_assign_tenant BEFORE INSERT ON engineering_task_events
FOR EACH ROW EXECUTE FUNCTION ush_assign_engineering_child_tenant();
CREATE TRIGGER trg_engineering_task_checkpoints_assign_tenant BEFORE INSERT ON engineering_task_checkpoints
FOR EACH ROW EXECUTE FUNCTION ush_assign_engineering_child_tenant();
