-- Durable project planning and execution metadata.
-- Prompts, generated content, credentials and raw agent transcripts must never be stored here.

CREATE TABLE IF NOT EXISTS project_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    version BIGINT NOT NULL CHECK (version > 0),
    state VARCHAR(16) NOT NULL DEFAULT 'draft'
        CHECK (state IN ('draft', 'active', 'superseded', 'archived')),
    source_revision VARCHAR(80),
    created_by VARCHAR(200) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    activated_at TIMESTAMPTZ,
    UNIQUE (project_id, id),
    UNIQUE (project_id, version)
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_project_plans_one_active
    ON project_plans(project_id) WHERE state = 'active';
CREATE INDEX IF NOT EXISTS ix_project_plans_project_created
    ON project_plans(project_id, created_at DESC);

CREATE TABLE IF NOT EXISTS project_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    plan_id UUID NOT NULL,
    task_key VARCHAR(120) NOT NULL,
    title VARCHAR(240) NOT NULL,
    objective TEXT NOT NULL DEFAULT '',
    status VARCHAR(20) NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'ready', 'in_progress', 'blocked', 'review', 'verified', 'failed', 'cancelled')),
    priority SMALLINT NOT NULL DEFAULT 100 CHECK (priority BETWEEN 0 AND 1000),
    owner_role VARCHAR(48),
    estimate_hours DOUBLE PRECISION CHECK (estimate_hours IS NULL OR (estimate_hours >= 0 AND estimate_hours <= 1000000)),
    optimistic_hours DOUBLE PRECISION CHECK (optimistic_hours IS NULL OR (optimistic_hours >= 0 AND optimistic_hours <= 1000000)),
    pessimistic_hours DOUBLE PRECISION CHECK (pessimistic_hours IS NULL OR (pessimistic_hours >= 0 AND pessimistic_hours <= 1000000)),
    due_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, id),
    UNIQUE (project_id, plan_id, task_key),
    FOREIGN KEY (project_id, plan_id) REFERENCES project_plans(project_id, id) ON DELETE CASCADE,
    CHECK (
        optimistic_hours IS NULL OR estimate_hours IS NULL OR optimistic_hours <= estimate_hours
    ),
    CHECK (
        pessimistic_hours IS NULL OR estimate_hours IS NULL OR estimate_hours <= pessimistic_hours
    )
);
CREATE INDEX IF NOT EXISTS ix_project_tasks_project_status
    ON project_tasks(project_id, status, priority DESC);
CREATE INDEX IF NOT EXISTS ix_project_tasks_plan
    ON project_tasks(project_id, plan_id);

CREATE TABLE IF NOT EXISTS task_dependencies (
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id UUID NOT NULL,
    depends_on_task_id UUID NOT NULL,
    dependency_type VARCHAR(16) NOT NULL DEFAULT 'finish_to_start'
        CHECK (dependency_type IN ('finish_to_start')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, task_id, depends_on_task_id),
    FOREIGN KEY (project_id, task_id) REFERENCES project_tasks(project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, depends_on_task_id) REFERENCES project_tasks(project_id, id) ON DELETE CASCADE,
    CHECK (task_id <> depends_on_task_id)
);
CREATE INDEX IF NOT EXISTS ix_task_dependencies_reverse
    ON task_dependencies(project_id, depends_on_task_id);

-- Append-only operational events. Keep payloads to bounded, non-sensitive metadata.
CREATE TABLE IF NOT EXISTS project_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    plan_id UUID,
    task_id UUID,
    event_type VARCHAR(80) NOT NULL,
    actor VARCHAR(200) NOT NULL,
    correlation_id UUID,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(metadata) = 'object' AND pg_column_size(metadata) <= 16384),
    FOREIGN KEY (project_id, plan_id) REFERENCES project_plans(project_id, id) ON DELETE SET NULL,
    FOREIGN KEY (project_id, task_id) REFERENCES project_tasks(project_id, id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS ix_project_events_project_time
    ON project_events(project_id, occurred_at DESC, id);
CREATE INDEX IF NOT EXISTS ix_project_events_task_time
    ON project_events(project_id, task_id, occurred_at DESC)
    WHERE task_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS ix_project_events_correlation
    ON project_events(correlation_id) WHERE correlation_id IS NOT NULL;

-- Durable queue. Workers must claim rows transactionally with FOR UPDATE SKIP LOCKED.
-- Lease token is a random fencing token; stale workers must not write after a lease changes.
CREATE TABLE IF NOT EXISTS execution_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id UUID,
    job_type VARCHAR(64) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'leased', 'succeeded', 'retry_wait', 'failed', 'dead_letter', 'cancelled')),
    priority SMALLINT NOT NULL DEFAULT 100 CHECK (priority BETWEEN 0 AND 1000),
    idempotency_key VARCHAR(160) NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(payload) = 'object' AND pg_column_size(payload) <= 32768),
    checkpoint JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(checkpoint) = 'object' AND pg_column_size(checkpoint) <= 32768),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts INTEGER NOT NULL DEFAULT 3 CHECK (max_attempts BETWEEN 1 AND 20),
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    lease_owner VARCHAR(160),
    lease_token UUID,
    lease_expires_at TIMESTAMPTZ,
    heartbeat_at TIMESTAMPTZ,
    last_error_code VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    UNIQUE (project_id, id),
    UNIQUE (project_id, idempotency_key),
    FOREIGN KEY (project_id, task_id) REFERENCES project_tasks(project_id, id) ON DELETE SET NULL,
    CHECK (attempt_count <= max_attempts),
    CHECK (
        (status = 'leased' AND lease_owner IS NOT NULL AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR
        (status <> 'leased' AND lease_owner IS NULL AND lease_token IS NULL AND lease_expires_at IS NULL)
    )
);
CREATE INDEX IF NOT EXISTS ix_execution_jobs_claim
    ON execution_jobs(status, available_at, priority DESC, created_at)
    WHERE status IN ('queued', 'retry_wait');
CREATE INDEX IF NOT EXISTS ix_execution_jobs_expired_lease
    ON execution_jobs(lease_expires_at)
    WHERE status = 'leased';
CREATE INDEX IF NOT EXISTS ix_execution_jobs_project
    ON execution_jobs(project_id, created_at DESC);

CREATE TABLE IF NOT EXISTS execution_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    job_id UUID NOT NULL,
    attempt_number INTEGER NOT NULL CHECK (attempt_number > 0),
    worker_id VARCHAR(160) NOT NULL,
    lease_token UUID NOT NULL,
    status VARCHAR(20) NOT NULL
        CHECK (status IN ('running', 'succeeded', 'retryable_failure', 'permanent_failure', 'lease_expired', 'cancelled')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    duration_ms BIGINT CHECK (duration_ms IS NULL OR duration_ms >= 0),
    error_code VARCHAR(100),
    result_metadata JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(result_metadata) = 'object' AND pg_column_size(result_metadata) <= 16384),
    UNIQUE (project_id, job_id, attempt_number),
    FOREIGN KEY (project_id, job_id) REFERENCES execution_jobs(project_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_execution_attempts_project_started
    ON execution_attempts(project_id, started_at DESC);

-- Artifact metadata only. File bytes live in a separately authorized object store.
CREATE TABLE IF NOT EXISTS execution_artifacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    job_id UUID,
    task_id UUID,
    artifact_type VARCHAR(48) NOT NULL,
    storage_backend VARCHAR(48) NOT NULL,
    storage_key VARCHAR(512) NOT NULL,
    content_sha256 CHAR(64) NOT NULL CHECK (content_sha256 ~ '^[0-9a-f]{64}$'),
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (project_id, job_id) REFERENCES execution_jobs(project_id, id) ON DELETE SET NULL,
    FOREIGN KEY (project_id, task_id) REFERENCES project_tasks(project_id, id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS ix_execution_artifacts_project_created
    ON execution_artifacts(project_id, created_at DESC);

CREATE TABLE IF NOT EXISTS execution_approvals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    action_type VARCHAR(80) NOT NULL,
    target_type VARCHAR(48) NOT NULL,
    target_id VARCHAR(160) NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected', 'expired', 'cancelled')),
    requested_by VARCHAR(200) NOT NULL,
    decided_by VARCHAR(200),
    reason_code VARCHAR(100),
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    decided_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(metadata) = 'object' AND pg_column_size(metadata) <= 8192),
    CHECK (
        (status = 'pending' AND decided_at IS NULL AND decided_by IS NULL)
        OR
        (status <> 'pending' AND decided_at IS NOT NULL AND decided_by IS NOT NULL)
    )
);
CREATE INDEX IF NOT EXISTS ix_execution_approvals_pending
    ON execution_approvals(project_id, expires_at)
    WHERE status = 'pending';
