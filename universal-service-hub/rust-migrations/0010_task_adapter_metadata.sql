ALTER TABLE engineering_tasks
    ADD COLUMN IF NOT EXISTS metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS candidate_sha VARCHAR(64);

ALTER TABLE engineering_tasks
    DROP CONSTRAINT IF EXISTS ck_engineering_tasks_candidate_sha;

ALTER TABLE engineering_tasks
    ADD CONSTRAINT ck_engineering_tasks_candidate_sha
    CHECK (candidate_sha IS NULL OR candidate_sha ~ '^[0-9a-fA-F]{40}$');

CREATE INDEX IF NOT EXISTS ix_engineering_tasks_candidate
    ON engineering_tasks(tenant_id, project_id, candidate_sha)
    WHERE candidate_sha IS NOT NULL;
