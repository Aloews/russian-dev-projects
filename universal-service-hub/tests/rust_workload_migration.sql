\set ON_ERROR_STOP on
\if :{?proof_commit_sha}
\else
\echo 'Missing exact commit SHA from the CI runner.'
\quit 2
\endif
\if :{?proof_output_sha256}
\else
\echo 'Missing Rust test output digest from the CI runner.'
\quit 2
\endif
\if :{?proof_run_id}
\else
\echo 'Missing GitHub Actions run ID.'
\quit 2
\endif
BEGIN;

INSERT INTO projects (id, name, allowed_capabilities)
VALUES
  ('migration-proof-a', 'Migration proof A', '["test"]'::jsonb),
  ('migration-proof-b', 'Migration proof B', '["test"]'::jsonb)
ON CONFLICT (id) DO NOTHING;

INSERT INTO jobs (id, project_id, workload_type, idempotency_key, budget_ceiling_usd)
VALUES
  ('migration-job-a', 'migration-proof-a', 'test', 'same-key', 1.00),
  ('migration-job-b', 'migration-proof-b', 'test', 'same-key', 1.00);

DO 'BEGIN
  BEGIN
    INSERT INTO jobs (id, project_id, workload_type, idempotency_key)
    VALUES (''migration-job-duplicate'', ''migration-proof-a'', ''test'', ''same-key'');
    RAISE EXCEPTION ''idempotency constraint did not reject a duplicate key'';
  EXCEPTION WHEN unique_violation THEN
    NULL;
  END;
END';

INSERT INTO job_steps (id, job_id, project_id, ordinal, objective, capability)
VALUES
  ('migration-step-a', 'migration-job-a', 'migration-proof-a', 0, 'A', 'test'),
  ('migration-step-b', 'migration-job-b', 'migration-proof-b', 0, 'B', 'test'),
  ('migration-step-artifact', 'migration-job-a', 'migration-proof-a', 1, 'Artifact', 'test');

DO 'BEGIN
  BEGIN
    INSERT INTO job_attempts (id, job_id, step_id, project_id, attempt_number)
    VALUES (''migration-cross-attempt'', ''migration-job-a'', ''migration-step-b'', ''migration-proof-a'', 1);
    RAISE EXCEPTION ''attempt accepted a step belonging to another job/project'';
  EXCEPTION WHEN foreign_key_violation THEN
    NULL;
  END;

  BEGIN
    INSERT INTO job_artifacts (id, job_id, step_id, project_id, object_key, mime_type, byte_size, checksum_sha256)
    VALUES (''migration-cross-artifact'', ''migration-job-a'', ''migration-step-b'', ''migration-proof-a'', ''cross'', ''text/plain'', 1, repeat(''a'', 64));
    RAISE EXCEPTION ''artifact accepted a step belonging to another job/project'';
  EXCEPTION WHEN foreign_key_violation THEN
    NULL;
  END;

  BEGIN
    INSERT INTO outbox_events (id, project_id, job_id, event_type, payload)
    VALUES (''migration-cross-outbox'', ''migration-proof-a'', ''migration-job-b'', ''test'', ''{}''::jsonb);
    RAISE EXCEPTION ''outbox event accepted a job belonging to another project'';
  EXCEPTION WHEN foreign_key_violation THEN
    NULL;
  END;

  BEGIN
    INSERT INTO jobs (id, project_id, workload_type, idempotency_key, budget_ceiling_usd, budget_reserved_usd)
    VALUES (''migration-over-budget'', ''migration-proof-a'', ''test'', ''over-budget'', 0.10, 0.11);
    RAISE EXCEPTION ''job accepted a reservation above its budget ceiling'';
  EXCEPTION WHEN check_violation THEN
    NULL;
  END;

  BEGIN
    INSERT INTO job_steps (id, job_id, project_id, ordinal, objective, capability, state)
    VALUES (''migration-forged-step'', ''migration-job-a'', ''migration-proof-a'', 2, ''Forged'', ''test'', ''succeeded'');
    RAISE EXCEPTION ''step was inserted as succeeded without verification evidence'';
  EXCEPTION WHEN check_violation THEN
    NULL;
  END;

  BEGIN
    UPDATE job_steps SET state = ''succeeded'' WHERE id = ''migration-step-a'';
    RAISE EXCEPTION ''step was marked succeeded without verification evidence'';
  EXCEPTION WHEN check_violation THEN
    NULL;
  END;

  BEGIN
    INSERT INTO job_artifacts (id, job_id, step_id, project_id, object_key, mime_type, byte_size, checksum_sha256)
VALUES ('migration-artifact-a', 'migration-job-a', 'migration-step-artifact', 'migration-proof-a', 'proof/artifact', 'text/plain', 1, repeat('b', 64));

DELETE FROM job_steps WHERE id = 'migration-step-artifact';

DO 'BEGIN
  IF EXISTS (SELECT 1 FROM job_artifacts WHERE id = ''migration-artifact-a'' AND step_id IS NOT NULL) THEN
    RAISE EXCEPTION ''deleting a step did not preserve its artifact with a NULL step reference'';
  END IF;
END';

INSERT INTO job_verifications (
      id, project_id, job_id, step_id, source, verifier_id, command,
      exit_code, passed, commit_sha, output_sha256
    ) VALUES (
      ''migration-agent-report'', ''migration-proof-a'', ''migration-job-a'', ''migration-step-a'',
      ''agent_report'', ''coding-agent'', ''I completed the task'', 0, TRUE,
      repeat(''a'', 40), repeat(''b'', 64)
    );
    RAISE EXCEPTION ''agent-authored report was accepted as verification evidence'';
  EXCEPTION WHEN check_violation THEN
    NULL;
  END;

  BEGIN
    UPDATE jobs SET state = ''succeeded'' WHERE id = ''migration-job-b'';
    RAISE EXCEPTION ''job was marked succeeded while a step remained pending'';
  EXCEPTION WHEN check_violation THEN
    NULL;
  END;
END';

INSERT INTO job_verifications (
  id, project_id, job_id, step_id, source, verifier_id, command,
  exit_code, passed, commit_sha, output_sha256, evidence_ref
) VALUES (
  'migration-proof-a1', 'migration-proof-a', 'migration-job-a', 'migration-step-a',
  'ci', 'github-actions:' || :'proof_run_id', 'cargo test --all-targets', 0, TRUE,
  :'proof_commit_sha', :'proof_output_sha256',
  jsonb_build_object(
    'run_id', :'proof_run_id',
    'run_attempt', :'proof_run_attempt',
    'run_url', :'proof_run_url'
  )
);

UPDATE job_steps SET state = 'succeeded' WHERE id = 'migration-step-a';
UPDATE jobs SET state = 'succeeded' WHERE id = 'migration-job-a';

ROLLBACK;
