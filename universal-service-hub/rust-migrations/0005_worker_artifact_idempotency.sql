-- Make worker artifact uploads idempotent within a project/job lease.
ALTER TABLE execution_artifacts
    ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(160)
        CHECK (idempotency_key IS NULL OR (
            idempotency_key <> '' AND
            idempotency_key ~ '^[A-Za-z0-9._:-]{1,160}$'
        ));

CREATE UNIQUE INDEX IF NOT EXISTS uq_execution_artifacts_job_idempotency
    ON execution_artifacts(project_id, job_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL AND job_id IS NOT NULL;
