-- Durable, scope-bound tool execution receipts. No arguments, prompts, or outputs are stored.
CREATE TABLE IF NOT EXISTS execution_tool_receipts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id VARCHAR(80) NOT NULL,
    job_id UUID NOT NULL,
    task_id UUID NOT NULL,
    action_id UUID NOT NULL,
    tool_id VARCHAR(96) NOT NULL CHECK (tool_id ~ '^[A-Za-z0-9._:-]+$'),
    tool_version INTEGER NOT NULL CHECK (tool_version BETWEEN 1 AND 65535),
    action_sha256 CHAR(64) NOT NULL CHECK (action_sha256 ~ '^[0-9a-f]{64}$'),
    idempotency_key VARCHAR(160) NOT NULL CHECK (idempotency_key ~ '^ush-tool-v1-[0-9a-f]{64}$'),
    status VARCHAR(16) NOT NULL CHECK (status IN ('reserved','completed','uncertain')),
    result_artifact_id UUID,
    result_sha256 CHAR(64) CHECK (result_sha256 IS NULL OR result_sha256 ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, job_id, task_id, action_id),
    UNIQUE (project_id, job_id, idempotency_key),
    FOREIGN KEY (project_id, job_id) REFERENCES execution_jobs(project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, task_id) REFERENCES project_tasks(project_id, id) ON DELETE CASCADE,
    CHECK (
        (status = 'completed' AND result_artifact_id IS NOT NULL AND result_sha256 IS NOT NULL)
        OR
        (status <> 'completed' AND result_artifact_id IS NULL AND result_sha256 IS NULL)
    )
);
CREATE INDEX IF NOT EXISTS ix_execution_tool_receipts_status
    ON execution_tool_receipts(project_id,job_id,status,updated_at);
