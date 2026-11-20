-- Versioned, integrity-checked agent checkpoints. No prompts, transcripts, generated content or secrets.
ALTER TABLE execution_jobs
    ADD COLUMN IF NOT EXISTS agent_checkpoint JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS agent_checkpoint_version BIGINT NOT NULL DEFAULT 0
        CHECK (agent_checkpoint_version >= 0),
    ADD COLUMN IF NOT EXISTS agent_checkpoint_sha256 CHAR(64)
        CHECK (agent_checkpoint_sha256 IS NULL OR agent_checkpoint_sha256 ~ '^[0-9a-f]{64}$'),
    ADD COLUMN IF NOT EXISTS agent_checkpoint_updated_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS ix_execution_jobs_agent_checkpoint
    ON execution_jobs(project_id, id, agent_checkpoint_version)
    WHERE agent_checkpoint_version > 0;
