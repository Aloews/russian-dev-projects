CREATE TABLE IF NOT EXISTS jobs (
    id VARCHAR(36) PRIMARY KEY,
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_job_id VARCHAR(36),
    workload_type VARCHAR(80) NOT NULL,
    state VARCHAR(32) NOT NULL DEFAULT 'queued' CHECK (state IN ('queued','leased','running','checkpointed','succeeded','paused','cancel_requested','cancelled','retry_wait','submission_unknown','failed_permanent','dead_letter')),
    priority INTEGER NOT NULL DEFAULT 0,
    input_ref JSONB NOT NULL DEFAULT '{}'::jsonb,
    idempotency_key VARCHAR(160) NOT NULL,
    budget_ceiling_usd NUMERIC(12,4),
    budget_reserved_usd NUMERIC(12,4) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), started_at TIMESTAMPTZ, finished_at TIMESTAMPTZ,
    UNIQUE (project_id, id),
    UNIQUE (project_id, idempotency_key),
    FOREIGN KEY (project_id, parent_job_id) REFERENCES jobs(project_id, id) ON DELETE RESTRICT,
    CHECK (budget_ceiling_usd IS NULL OR budget_ceiling_usd >= 0),
    CHECK (budget_reserved_usd >= 0 AND (budget_ceiling_usd IS NULL OR budget_reserved_usd <= budget_ceiling_usd))
);
CREATE INDEX IF NOT EXISTS ix_jobs_project_state ON jobs(project_id, state, priority DESC, created_at);

CREATE TABLE IF NOT EXISTS job_steps (
    id VARCHAR(36) PRIMARY KEY, job_id VARCHAR(36) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE, project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0), objective VARCHAR(200) NOT NULL, capability VARCHAR(120) NOT NULL,
    dependencies JSONB NOT NULL DEFAULT '[]'::jsonb, state VARCHAR(32) NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','ready','leased','running','checkpointed','succeeded','retry_wait','submission_unknown','failed_permanent','cancelled')),
    input_ref JSONB NOT NULL DEFAULT '{}'::jsonb, output_ref JSONB, acceptance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (job_id, ordinal), UNIQUE (project_id, id), UNIQUE (project_id, job_id, id),
    FOREIGN KEY (project_id, job_id) REFERENCES jobs(project_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_job_steps_ready ON job_steps(project_id, state, created_at);

CREATE TABLE IF NOT EXISTS job_attempts (
    id VARCHAR(36) PRIMARY KEY, job_id VARCHAR(36) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE, step_id VARCHAR(36) NOT NULL REFERENCES job_steps(id) ON DELETE CASCADE, project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL CHECK (attempt_number > 0), worker_id VARCHAR(160), provider_id VARCHAR(80), model VARCHAR(160), fencing_token BIGINT,
    state VARCHAR(32) NOT NULL DEFAULT 'started' CHECK (state IN ('started','succeeded','failed','submission_unknown')), failure_class VARCHAR(32) CHECK (failure_class IS NULL OR failure_class IN ('transient','permanent','billing_ambiguous')), failure_code VARCHAR(120), provider_request_id VARCHAR(200), billable BOOLEAN NOT NULL DEFAULT FALSE,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), finished_at TIMESTAMPTZ, UNIQUE (step_id, attempt_number), UNIQUE (project_id, id), FOREIGN KEY (project_id, job_id, step_id) REFERENCES job_steps(project_id, job_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS job_checkpoints (
    id VARCHAR(36) PRIMARY KEY, job_id VARCHAR(36) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE, step_id VARCHAR(36) NOT NULL REFERENCES job_steps(id) ON DELETE CASCADE, project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    version INTEGER NOT NULL CHECK (version > 0), output_ref JSONB NOT NULL, checksum_sha256 VARCHAR(64) NOT NULL, schema_version INTEGER NOT NULL CHECK (schema_version > 0), created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (step_id, version), UNIQUE (project_id, id), FOREIGN KEY (project_id, job_id, step_id) REFERENCES job_steps(project_id, job_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS job_verifications (
    id VARCHAR(36) PRIMARY KEY,
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    job_id VARCHAR(36) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    step_id VARCHAR(36) NOT NULL,
    source VARCHAR(24) NOT NULL CHECK (source IN ('ci','runtime_probe','artifact')),
    verifier_id VARCHAR(160) NOT NULL,
    command TEXT NOT NULL,
    exit_code INTEGER NOT NULL,
    passed BOOLEAN NOT NULL,
    commit_sha VARCHAR(40) NOT NULL CHECK (commit_sha ~ '^[0-9a-f]{40}
    id VARCHAR(36) PRIMARY KEY, job_id VARCHAR(36) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE, step_id VARCHAR(36), project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    object_key VARCHAR(500) NOT NULL, mime_type VARCHAR(160) NOT NULL, byte_size BIGINT NOT NULL CHECK (byte_size >= 0), checksum_sha256 VARCHAR(64) NOT NULL, access_policy VARCHAR(32) NOT NULL DEFAULT 'project' CHECK (access_policy IN ('project','private')), created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (project_id, object_key), UNIQUE (project_id, id), FOREIGN KEY (project_id, job_id, step_id) REFERENCES job_steps(project_id, job_id, id) ON DELETE SET NULL (step_id)
);

CREATE TABLE IF NOT EXISTS job_leases (
    job_id VARCHAR(36) PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE, project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE, owner_id VARCHAR(160) NOT NULL, fencing_token BIGINT NOT NULL CHECK (fencing_token > 0), expires_at TIMESTAMPTZ NOT NULL, heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (project_id, job_id), FOREIGN KEY (project_id, job_id) REFERENCES jobs(project_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS outbox_events (
    id VARCHAR(36) PRIMARY KEY, project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE, job_id VARCHAR(36), event_type VARCHAR(80) NOT NULL, payload JSONB NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), published_at TIMESTAMPTZ, UNIQUE (project_id, id), FOREIGN KEY (project_id, job_id) REFERENCES jobs(project_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_outbox_unpublished ON outbox_events(created_at) WHERE published_at IS NULL;
),
    output_sha256 VARCHAR(64) NOT NULL CHECK (output_sha256 ~ '^[0-9a-f]{64}
    id VARCHAR(36) PRIMARY KEY, job_id VARCHAR(36) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE, step_id VARCHAR(36), project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    object_key VARCHAR(500) NOT NULL, mime_type VARCHAR(160) NOT NULL, byte_size BIGINT NOT NULL CHECK (byte_size >= 0), checksum_sha256 VARCHAR(64) NOT NULL, access_policy VARCHAR(32) NOT NULL DEFAULT 'project' CHECK (access_policy IN ('project','private')), created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (project_id, object_key), UNIQUE (project_id, id), FOREIGN KEY (project_id, job_id, step_id) REFERENCES job_steps(project_id, job_id, id) ON DELETE SET NULL (step_id)
);

CREATE TABLE IF NOT EXISTS job_leases (
    job_id VARCHAR(36) PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE, project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE, owner_id VARCHAR(160) NOT NULL, fencing_token BIGINT NOT NULL CHECK (fencing_token > 0), expires_at TIMESTAMPTZ NOT NULL, heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (project_id, job_id), FOREIGN KEY (project_id, job_id) REFERENCES jobs(project_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS outbox_events (
    id VARCHAR(36) PRIMARY KEY, project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE, job_id VARCHAR(36), event_type VARCHAR(80) NOT NULL, payload JSONB NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), published_at TIMESTAMPTZ, UNIQUE (project_id, id), FOREIGN KEY (project_id, job_id) REFERENCES jobs(project_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_outbox_unpublished ON outbox_events(created_at) WHERE published_at IS NULL;
),
    evidence_ref JSONB NOT NULL DEFAULT '{}'::jsonb,
    verified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (passed = (exit_code = 0)),
    FOREIGN KEY (project_id, job_id, step_id) REFERENCES job_steps(project_id, job_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_job_verifications_step_passed
    ON job_verifications(project_id, step_id, verified_at DESC) WHERE passed = TRUE;

CREATE OR REPLACE FUNCTION ush_require_step_verification() RETURNS trigger AS $
BEGIN
    IF NEW.state = 'succeeded' AND OLD.state IS DISTINCT FROM 'succeeded'
       AND NOT EXISTS (
           SELECT 1 FROM job_verifications v
           WHERE v.project_id = NEW.project_id
             AND v.job_id = NEW.job_id
             AND v.step_id = NEW.id
             AND v.passed = TRUE
       ) THEN
        RAISE EXCEPTION 'step cannot succeed without independently recorded passing verification'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_job_step_requires_verification ON job_steps;
CREATE TRIGGER trg_job_step_requires_verification
    BEFORE UPDATE OF state ON job_steps
    FOR EACH ROW EXECUTE FUNCTION ush_require_step_verification();

CREATE OR REPLACE FUNCTION ush_require_job_steps_succeeded() RETURNS trigger AS $
BEGIN
    IF NEW.state = 'succeeded' AND OLD.state IS DISTINCT FROM 'succeeded'
       AND (
           NOT EXISTS (SELECT 1 FROM job_steps s WHERE s.project_id = NEW.project_id AND s.job_id = NEW.id)
           OR EXISTS (
               SELECT 1 FROM job_steps s
               WHERE s.project_id = NEW.project_id
                 AND s.job_id = NEW.id
                 AND s.state <> 'succeeded'
           )
       ) THEN
        RAISE EXCEPTION 'job cannot succeed until every step has passed verification'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_job_requires_verified_steps ON jobs;
CREATE TRIGGER trg_job_requires_verified_steps
    BEFORE UPDATE OF state ON jobs
    FOR EACH ROW EXECUTE FUNCTION ush_require_job_steps_succeeded();

CREATE TABLE IF NOT EXISTS job_artifacts (
    id VARCHAR(36) PRIMARY KEY, job_id VARCHAR(36) NOT NULL REFERENCES jobs(id) ON DELETE CASCADE, step_id VARCHAR(36), project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    object_key VARCHAR(500) NOT NULL, mime_type VARCHAR(160) NOT NULL, byte_size BIGINT NOT NULL CHECK (byte_size >= 0), checksum_sha256 VARCHAR(64) NOT NULL, access_policy VARCHAR(32) NOT NULL DEFAULT 'project' CHECK (access_policy IN ('project','private')), created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (project_id, object_key), UNIQUE (project_id, id), FOREIGN KEY (project_id, job_id, step_id) REFERENCES job_steps(project_id, job_id, id) ON DELETE SET NULL (step_id)
);

CREATE TABLE IF NOT EXISTS job_leases (
    job_id VARCHAR(36) PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE, project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE, owner_id VARCHAR(160) NOT NULL, fencing_token BIGINT NOT NULL CHECK (fencing_token > 0), expires_at TIMESTAMPTZ NOT NULL, heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (project_id, job_id), FOREIGN KEY (project_id, job_id) REFERENCES jobs(project_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS outbox_events (
    id VARCHAR(36) PRIMARY KEY, project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE, job_id VARCHAR(36), event_type VARCHAR(80) NOT NULL, payload JSONB NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), published_at TIMESTAMPTZ, UNIQUE (project_id, id), FOREIGN KEY (project_id, job_id) REFERENCES jobs(project_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_outbox_unpublished ON outbox_events(created_at) WHERE published_at IS NULL;
