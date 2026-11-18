-- Immutable receipts from a completed, successful source CI run and a
-- separate trusted workflow_run attestation.
CREATE TABLE engineering_task_verification_evidence (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL,
    project_id VARCHAR(80) NOT NULL,
    task_id VARCHAR(36) NOT NULL,
    candidate_sha VARCHAR(40) NOT NULL CHECK (candidate_sha ~ '^[0-9a-f]{40}$'),
    repository VARCHAR(200) NOT NULL,
    workflow_ref VARCHAR(500) NOT NULL,
    job_workflow_ref VARCHAR(500) NOT NULL,
    job_workflow_sha VARCHAR(40) NOT NULL CHECK (job_workflow_sha ~ '^[0-9a-fA-F]{40}$'),
    workflow_path VARCHAR(300) NOT NULL,
    git_ref VARCHAR(240) NOT NULL,
    event_name VARCHAR(40) NOT NULL CHECK (event_name IN ('push','workflow_dispatch')),
    run_id BIGINT NOT NULL CHECK (run_id > 0),
    run_attempt INTEGER NOT NULL CHECK (run_attempt BETWEEN 1 AND 100),
    run_started_at TIMESTAMPTZ NOT NULL,
    run_completed_at TIMESTAMPTZ NOT NULL,
    artifact_digest VARCHAR(71) CHECK (artifact_digest IS NULL OR artifact_digest ~ '^sha256:[0-9a-f]{64}
    attestation_sha VARCHAR(40) NOT NULL CHECK (attestation_sha ~ '^[0-9a-f]{40}$'),
    attestation_run_id BIGINT NOT NULL CHECK (attestation_run_id > 0),
    attestation_run_attempt INTEGER NOT NULL CHECK (attestation_run_attempt BETWEEN 1 AND 100),
    conclusion VARCHAR(20) NOT NULL CHECK (conclusion IN ('success','failure','cancelled','timed_out','action_required','neutral','skipped','stale','startup_failure')),
    attestation_hash VARCHAR(64) NOT NULL CHECK (attestation_hash ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (run_completed_at >= run_started_at),
    UNIQUE (repository, run_id, run_attempt, task_id),
    UNIQUE (repository, attestation_run_id, attestation_run_attempt, task_id),
    CONSTRAINT fk_verification_evidence_tenant_task
        FOREIGN KEY (tenant_id, task_id) REFERENCES engineering_tasks(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_verification_evidence_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX ix_verification_evidence_task_candidate
    ON engineering_task_verification_evidence(tenant_id, project_id, task_id, candidate_sha, created_at DESC);

CREATE FUNCTION ush_validate_verification_evidence_parent() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE
    owning_tenant VARCHAR(80);
    owning_project VARCHAR(80);
    owning_sha VARCHAR(64);
    owning_repository TEXT;
BEGIN
    SELECT tenant_id, project_id, candidate_sha, metadata->>'source_repository'
      INTO owning_tenant, owning_project, owning_sha, owning_repository
      FROM engineering_tasks WHERE id=NEW.task_id;
    IF owning_tenant IS NULL OR owning_project IS NULL THEN
        RAISE EXCEPTION 'verification evidence task not found';
    END IF;
    IF owning_sha IS NULL OR owning_repository IS NULL
       OR NEW.project_id <> owning_project OR NEW.candidate_sha <> lower(owning_sha)
       OR NEW.repository <> owning_repository THEN
        RAISE EXCEPTION 'verification evidence does not match task candidate and repository';
    END IF;
    NEW.tenant_id := owning_tenant;
    RETURN NEW;
END;
$function$;

CREATE TRIGGER trg_verification_evidence_parent
    BEFORE INSERT ON engineering_task_verification_evidence
    FOR EACH ROW EXECUTE FUNCTION ush_validate_verification_evidence_parent();

CREATE FUNCTION ush_prevent_verification_evidence_update() RETURNS trigger LANGUAGE plpgsql AS $function$
BEGIN
    RAISE EXCEPTION 'verification evidence is immutable';
END;
$function$;

CREATE TRIGGER trg_verification_evidence_immutable
    BEFORE UPDATE ON engineering_task_verification_evidence
    FOR EACH ROW EXECUTE FUNCTION ush_prevent_verification_evidence_update();

ALTER TABLE engineering_task_verification_evidence ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_task_verification_evidence FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_execution_isolation ON engineering_task_verification_evidence
    USING (
        COALESCE(current_setting('app.is_admin', true), 'false') = 'true'
        OR tenant_id = NULLIF(current_setting('app.tenant_id', true), '')
    )
    WITH CHECK (
        COALESCE(current_setting('app.is_admin', true), 'false') = 'true'
        OR tenant_id = NULLIF(current_setting('app.tenant_id', true), '')
    );

GRANT SELECT, INSERT ON engineering_task_verification_evidence TO ush_runtime;
REVOKE UPDATE, DELETE ON engineering_task_verification_evidence FROM ush_runtime;
),
    attestation_sha VARCHAR(40) NOT NULL CHECK (attestation_sha ~ '^[0-9a-f]{40}$'),
    attestation_run_id BIGINT NOT NULL CHECK (attestation_run_id > 0),
    attestation_run_attempt INTEGER NOT NULL CHECK (attestation_run_attempt BETWEEN 1 AND 100),
    conclusion VARCHAR(20) NOT NULL CHECK (conclusion = 'success'),
    attestation_hash VARCHAR(64) NOT NULL CHECK (attestation_hash ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (run_completed_at >= run_started_at),
    UNIQUE (repository, run_id, run_attempt, task_id),
    UNIQUE (repository, attestation_run_id, attestation_run_attempt, task_id),
    CONSTRAINT fk_verification_evidence_tenant_task
        FOREIGN KEY (tenant_id, task_id) REFERENCES engineering_tasks(tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT fk_verification_evidence_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX ix_verification_evidence_task_candidate
    ON engineering_task_verification_evidence(tenant_id, project_id, task_id, candidate_sha, created_at DESC);

CREATE FUNCTION ush_validate_verification_evidence_parent() RETURNS trigger LANGUAGE plpgsql AS $function$
DECLARE
    owning_tenant VARCHAR(80);
    owning_project VARCHAR(80);
    owning_sha VARCHAR(64);
    owning_repository TEXT;
BEGIN
    SELECT tenant_id, project_id, candidate_sha, metadata->>'source_repository'
      INTO owning_tenant, owning_project, owning_sha, owning_repository
      FROM engineering_tasks WHERE id=NEW.task_id;
    IF owning_tenant IS NULL OR owning_project IS NULL THEN
        RAISE EXCEPTION 'verification evidence task not found';
    END IF;
    IF owning_sha IS NULL OR owning_repository IS NULL
       OR NEW.project_id <> owning_project OR NEW.candidate_sha <> lower(owning_sha)
       OR NEW.repository <> owning_repository THEN
        RAISE EXCEPTION 'verification evidence does not match task candidate and repository';
    END IF;
    NEW.tenant_id := owning_tenant;
    RETURN NEW;
END;
$function$;

CREATE TRIGGER trg_verification_evidence_parent
    BEFORE INSERT ON engineering_task_verification_evidence
    FOR EACH ROW EXECUTE FUNCTION ush_validate_verification_evidence_parent();

CREATE FUNCTION ush_prevent_verification_evidence_update() RETURNS trigger LANGUAGE plpgsql AS $function$
BEGIN
    RAISE EXCEPTION 'verification evidence is immutable';
END;
$function$;

CREATE TRIGGER trg_verification_evidence_immutable
    BEFORE UPDATE ON engineering_task_verification_evidence
    FOR EACH ROW EXECUTE FUNCTION ush_prevent_verification_evidence_update();

ALTER TABLE engineering_task_verification_evidence ENABLE ROW LEVEL SECURITY;
ALTER TABLE engineering_task_verification_evidence FORCE ROW LEVEL SECURITY;

CREATE POLICY tenant_execution_isolation ON engineering_task_verification_evidence
    USING (
        COALESCE(current_setting('app.is_admin', true), 'false') = 'true'
        OR tenant_id = NULLIF(current_setting('app.tenant_id', true), '')
    )
    WITH CHECK (
        COALESCE(current_setting('app.is_admin', true), 'false') = 'true'
        OR tenant_id = NULLIF(current_setting('app.tenant_id', true), '')
    );

GRANT SELECT, INSERT ON engineering_task_verification_evidence TO ush_runtime;
REVOKE UPDATE, DELETE ON engineering_task_verification_evidence FROM ush_runtime;
