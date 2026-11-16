#!/usr/bin/env bash
set -euo pipefail

compose=(docker compose --env-file .env.local -f compose.yml)
"${compose[@]}" exec -T postgres psql -v ON_ERROR_STOP=1 -U ush -d ush <<'SQL'
BEGIN;
INSERT INTO tenants (id, slug, name) VALUES ('tenant-beta', 'tenant-beta', 'Tenant Beta');
INSERT INTO workspaces (tenant_id, id, slug, name)
VALUES ('tenant-beta', 'default', 'default', 'Default workspace');

INSERT INTO projects (id, tenant_id, workspace_id, name, allowed_capabilities, allowed_provider_ids)
VALUES
  ('tenant-alpha-project', 'aloews', 'default', 'Tenant Alpha project', '["ai.chat"]'::jsonb, '[]'::jsonb),
  ('tenant-beta-project', 'tenant-beta', 'default', 'Tenant Beta project', '["ai.chat"]'::jsonb, '[]'::jsonb);

INSERT INTO engineering_tasks
  (id, tenant_id, project_id, idempotency_key, title, objective, created_by)
VALUES
  ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', 'aloews', 'tenant-alpha-project', 'same-key', 'Alpha task', 'Alpha objective', 'test'),
  ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', 'aloews', 'tenant-beta-project', 'same-key', 'Beta task', 'Beta objective', 'test');

DO $test$
DECLARE actual_tenant TEXT;
BEGIN
  SELECT tenant_id INTO actual_tenant FROM engineering_tasks
    WHERE id='bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
  IF actual_tenant <> 'tenant-beta' THEN
    RAISE EXCEPTION 'task tenant was not derived from owning project';
  END IF;

  BEGIN
    UPDATE engineering_tasks SET tenant_id='aloews'
      WHERE id='bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
    RAISE EXCEPTION 'cross-tenant task ownership update unexpectedly succeeded';
  EXCEPTION WHEN foreign_key_violation THEN
    NULL;
  END;

  BEGIN
    INSERT INTO engineering_task_events
      (id, task_id, project_id, actor, event_type, payload)
    VALUES
      ('cccccccc-cccc-4ccc-8ccc-cccccccccccc',
       'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
       'tenant-alpha-project', 'test', 'comment', '{}'::jsonb);
    RAISE EXCEPTION 'cross-project task event unexpectedly succeeded';
  EXCEPTION WHEN raise_exception THEN
    IF SQLERRM = 'cross-project task event unexpectedly succeeded' THEN RAISE; END IF;
  END;
  INSERT INTO engineering_task_events
    (id, task_id, project_id, actor, event_type, payload)
  VALUES
    ('eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
     'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
     'tenant-beta-project', 'test', 'comment', '{}'::jsonb);

  INSERT INTO engineering_task_checkpoints
    (id, task_id, project_id, worker_id, stage, metadata)
  VALUES
    ('ffffffff-ffff-4fff-8fff-ffffffffffff',
     'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
     'tenant-beta-project', 'worker-beta', 'planning', '{}'::jsonb);

  INSERT INTO tenant_usage_events
    (id, tenant_id, project_id, task_id, idempotency_key, usage_type)
  VALUES
    ('11111111-1111-4111-8111-111111111111',
     'tenant-beta', 'tenant-beta-project',
     'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', 'same-usage-key', 'model_call');

  INSERT INTO audit_events
    (id, actor, action, resource_type, resource_id, details)
  VALUES
    ('22222222-2222-4222-8222-222222222222',
     'test', 'task.test', 'engineering_task',
     'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', '{}'::jsonb);

  IF (SELECT tenant_id FROM engineering_task_events
      WHERE id='eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee') <> 'tenant-beta' THEN
    RAISE EXCEPTION 'task event tenant was not derived from task';
  END IF;
  IF (SELECT tenant_id FROM engineering_task_checkpoints
      WHERE id='ffffffff-ffff-4fff-8fff-ffffffffffff') <> 'tenant-beta' THEN
    RAISE EXCEPTION 'checkpoint tenant was not derived from task';
  END IF;
  IF (SELECT tenant_id FROM audit_events
      WHERE id='22222222-2222-4222-8222-222222222222') <> 'tenant-beta' THEN
    RAISE EXCEPTION 'audit tenant was not derived from task';
  END IF;

  BEGIN
    UPDATE projects SET tenant_id='tenant-beta'
      WHERE id='tenant-alpha-project';
    RAISE EXCEPTION 'project tenant transfer unexpectedly succeeded';
  EXCEPTION WHEN raise_exception THEN
    IF SQLERRM = 'project tenant transfer unexpectedly succeeded' THEN RAISE; END IF;
  END;
END;
$test$;

INSERT INTO project_tokens (id, tenant_id, project_id, name, role, token_hash)
VALUES ('dddddddd-dddd-4ddd-8ddd-dddddddddddd', 'aloews', 'tenant-beta-project',
        'beta token', 'viewer', repeat('b', 64));

DO $test$
DECLARE actual_tenant TEXT;
BEGIN
  SELECT tenant_id INTO actual_tenant FROM project_tokens
    WHERE id='dddddddd-dddd-4ddd-8ddd-dddddddddddd';
  IF actual_tenant <> 'tenant-beta' THEN
    RAISE EXCEPTION 'project token tenant was not derived from owning project';
  END IF;
END;
$test$;
-- 0005 execution schema: worker role binding, workspace tenancy, DAG and attempt isolation.
INSERT INTO engineering_tasks
  (id, tenant_id, project_id, idempotency_key, title, objective, created_by)
VALUES
  ('33333333-3333-4333-8333-333333333333', 'aloews', 'tenant-beta-project',
   'beta-child', 'Beta child task', 'Beta child objective', 'test');

INSERT INTO engineering_workspaces
  (id, tenant_id, project_id, workspace_key, runtime_provider, repository,
   base_branch, base_sha, git_worktree_path, git_branch)
VALUES
  ('44444444-4444-4444-8444-444444444444', 'aloews', 'tenant-alpha-project',
   'alpha-workspace', 'railway', 'Aloews/russian-dev-projects', 'main',
   repeat('a', 40), '/tmp/alpha-worktree', 'ai/alpha'),
  ('55555555-5555-4555-8555-555555555555', 'aloews', 'tenant-beta-project',
   'beta-workspace', 'railway', 'Aloews/russian-dev-projects',
   'feat/ush-durable-task-dispatcher-bootstrap', repeat('b', 40),
   '/tmp/beta-worktree', 'ai/beta');

INSERT INTO engineering_worker_tokens
  (id, tenant_id, project_id, name, worker_id, worker_role, queue_name, token_hash)
VALUES
  ('66666666-6666-4666-8666-666666666666', 'aloews', 'tenant-beta-project',
   'beta tester', 'tester-beta', 'tester', 'test', repeat('c', 64));

INSERT INTO engineering_task_attempts
  (id, tenant_id, project_id, task_id, attempt_no, worker_id, worker_role,
   provider_id, model_id, strategy, outcome, acceptance_progress)
VALUES
  ('77777777-7777-4777-8777-777777777777', 'aloews', 'tenant-beta-project',
   'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', 1, 'tester-beta', 'tester',
   NULL, 'test-model', 'baseline', 'started', 0.25);

INSERT INTO engineering_task_dependencies
  (tenant_id, project_id, parent_task_id, child_task_id)
VALUES
  ('aloews', 'tenant-beta-project',
   'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
   '33333333-3333-4333-8333-333333333333');

INSERT INTO engineering_task_workspace_assignments
  (id, tenant_id, project_id, task_id, workspace_id, worker_role, writer_scope, base_sha)
VALUES
  ('88888888-8888-4888-8888-888888888888', 'aloews', 'tenant-beta-project',
   'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
   '55555555-5555-4555-8555-555555555555', 'implementer',
   '["universal-service-hub/src/"]'::jsonb, repeat('b', 40));

INSERT INTO engineering_task_path_locks
  (id, tenant_id, project_id, task_id, workspace_id, path_prefix, lock_owner, lease_expires_at)
VALUES
  ('99999999-9999-4999-8999-999999999999', 'aloews', 'tenant-beta-project',
   'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
   '55555555-5555-4555-8555-555555555555',
   'universal-service-hub/src/', 'implementer-beta', NOW() + INTERVAL '2 minutes');

UPDATE engineering_task_checkpoints
SET workspace_id='55555555-5555-4555-8555-555555555555',
    git_ref='ai/beta',
    worktree_path='/tmp/beta-worktree'
WHERE id='ffffffff-ffff-4fff-8fff-ffffffffffff';
UPDATE engineering_tasks
SET last_checkpoint_id='ffffffff-ffff-4fff-8fff-ffffffffffff'
WHERE id='bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';

DO $execution_test$
BEGIN
  IF (SELECT tenant_id FROM engineering_workspaces
      WHERE id='55555555-5555-4555-8555-555555555555') <> 'tenant-beta' THEN
    RAISE EXCEPTION 'workspace tenant was not derived from project';
  END IF;
  IF (SELECT tenant_id FROM engineering_worker_tokens
      WHERE id='66666666-6666-4666-8666-666666666666') <> 'tenant-beta' THEN
    RAISE EXCEPTION 'worker token tenant was not derived from project';
  END IF;
  IF (SELECT tenant_id FROM engineering_task_attempts
      WHERE id='77777777-7777-4777-8777-777777777777') <> 'tenant-beta' THEN
    RAISE EXCEPTION 'task attempt tenant was not derived from task';
  END IF;
  IF (SELECT tenant_id FROM engineering_task_dependencies
      WHERE parent_task_id='bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'
        AND child_task_id='33333333-3333-4333-8333-333333333333') <> 'tenant-beta' THEN
    RAISE EXCEPTION 'task dependency tenant was not derived from parent task';
  END IF;
  IF (SELECT tenant_id FROM engineering_task_workspace_assignments
      WHERE id='88888888-8888-4888-8888-888888888888') <> 'tenant-beta' THEN
    RAISE EXCEPTION 'workspace assignment tenant was not derived from task';
  END IF;
  IF (SELECT tenant_id FROM engineering_task_path_locks
      WHERE id='99999999-9999-4999-8999-999999999999') <> 'tenant-beta' THEN
    RAISE EXCEPTION 'path lock tenant was not derived from task';
  END IF;
  IF (SELECT workspace_id FROM engineering_task_checkpoints
      WHERE id='ffffffff-ffff-4fff-8fff-ffffffffffff') <> '55555555-5555-4555-8555-555555555555' THEN
    RAISE EXCEPTION 'checkpoint workspace association was not persisted';
  END IF;
  IF (SELECT lease_generation FROM engineering_tasks
      WHERE id='bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb') <> 0 THEN
    RAISE EXCEPTION 'new task lease generation must start at zero';
  END IF;

  BEGIN
    INSERT INTO engineering_worker_tokens
      (id, tenant_id, project_id, name, worker_id, worker_role, queue_name, token_hash)
    VALUES
      ('aaaaaaaa-0000-4000-8000-000000000001', 'aloews', 'tenant-beta-project',
       'invalid reviewer token', 'bad-reviewer', 'reviewer', 'implementation', repeat('d', 64));
    RAISE EXCEPTION 'worker role was allowed to claim a mismatched queue';
  EXCEPTION WHEN check_violation THEN
    NULL;
  END;

  BEGIN
    INSERT INTO engineering_task_workspace_assignments
      (id, tenant_id, project_id, task_id, workspace_id, worker_role, writer_scope, base_sha)
    VALUES
      ('aaaaaaaa-0000-4000-8000-000000000002', 'aloews', 'tenant-beta-project',
       'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
       '44444444-4444-4444-8444-444444444444', 'tester', '[]'::jsonb, repeat('b', 40));
    RAISE EXCEPTION 'cross-tenant workspace assignment unexpectedly succeeded';
  EXCEPTION WHEN raise_exception THEN
    IF SQLERRM = 'cross-tenant workspace assignment unexpectedly succeeded' THEN RAISE; END IF;
  END;

  BEGIN
    INSERT INTO engineering_task_dependencies
      (tenant_id, project_id, parent_task_id, child_task_id)
    VALUES
      ('aloews', 'tenant-beta-project',
       'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
       'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa');
    RAISE EXCEPTION 'cross-tenant task dependency unexpectedly succeeded';
  EXCEPTION WHEN raise_exception THEN
    IF SQLERRM = 'cross-tenant task dependency unexpectedly succeeded' THEN RAISE; END IF;
  END;
END;
$execution_test$;

ROLLBACK;

-- Exercise RLS as the actual non-owner, non-superuser runtime role. All
-- transactions below share one psql connection to catch pooled-session leakage.
BEGIN;
INSERT INTO tenants (id,slug,name) VALUES ('tenant-beta','tenant-beta','Tenant Beta');
INSERT INTO workspaces (tenant_id,id,slug,name) VALUES ('tenant-beta','default','default','Default workspace');
INSERT INTO projects (id,tenant_id,workspace_id,name,allowed_capabilities,allowed_provider_ids)
VALUES
 ('rls-alpha-project','aloews','default','RLS Alpha','["ai.chat"]'::jsonb,'[]'::jsonb),
 ('rls-beta-project','tenant-beta','default','RLS Beta','["ai.chat"]'::jsonb,'[]'::jsonb);
INSERT INTO tenant_memberships (id,tenant_id,identity_issuer,identity_subject,role)
VALUES
 ('aaaa0000-1111-4111-8111-aaaaaaaaaaaa','aloews','https://issuer.example','alpha-user','tenant_admin'),
 ('bbbb0000-1111-4111-8111-bbbbbbbbbbbb','tenant-beta','https://issuer.example','beta-user','developer');
INSERT INTO tenant_resource_limits (tenant_id,max_active_tasks) VALUES ('tenant-beta',7);
INSERT INTO tenant_budget_policies (tenant_id,currency,soft_limit_minor,hard_limit_minor)
VALUES ('tenant-beta','USD',10000,20000);
INSERT INTO tenant_provider_connections (id,tenant_id,provider_id,credential_reference,configuration)
SELECT 'aaaa0000-2222-4222-8222-aaaaaaaaaaaa','aloews',id,'test://alpha-provider','{}'::jsonb
FROM providers ORDER BY id LIMIT 1;
INSERT INTO tenant_provider_connections (id,tenant_id,provider_id,credential_reference,configuration)
SELECT 'bbbb0000-2222-4222-8222-bbbbbbbbbbbb','tenant-beta',id,'test://beta-provider','{}'::jsonb
FROM providers ORDER BY id LIMIT 1;
INSERT INTO engineering_worker_tokens
 (id,tenant_id,project_id,name,worker_id,worker_role,queue_name,token_hash)
VALUES
 ('aaaaaaaa-2222-4222-8222-aaaaaaaaaaaa','aloews','rls-alpha-project','Alpha implementer','rls-alpha-worker','implementer','implementation',repeat('a',64)),
 ('bbbbbbbb-2222-4222-8222-bbbbbbbbbbbb','tenant-beta','rls-beta-project','Beta tester','rls-beta-worker','tester','test',repeat('b',64));
INSERT INTO tenant_tokens (id,tenant_id,name,role,token_hash)
VALUES ('77777777-7777-4777-8777-777777777777','tenant-beta','Beta tenant token','viewer',repeat('c',64));
INSERT INTO project_tokens (id,tenant_id,project_id,name,role,token_hash)
VALUES ('88888888-8888-4888-8888-888888888888','tenant-beta','rls-beta-project','Beta project token','viewer',repeat('d',64));
INSERT INTO engineering_tasks (id,tenant_id,project_id,idempotency_key,title,objective,created_by)
VALUES
 ('aaaaaaaa-1111-4111-8111-aaaaaaaaaaaa','aloews','rls-alpha-project','rls-alpha','Alpha visible','Alpha','test'),
 ('bbbbbbbb-1111-4111-8111-bbbbbbbbbbbb','aloews','rls-beta-project','rls-beta','Beta visible','Beta','test');
INSERT INTO tenant_usage_events
  (id,tenant_id,project_id,task_id,idempotency_key,usage_type)
VALUES
  ('33333333-3333-4333-8333-333333333333','tenant-beta','rls-beta-project',
   'bbbbbbbb-1111-4111-8111-bbbbbbbbbbbb','rls-beta-usage','model_call');
INSERT INTO audit_events
  (id,actor,action,resource_type,resource_id,details)
VALUES
  ('44444444-4444-4444-8444-444444444444','rls-test','task.test','engineering_task',
   'bbbbbbbb-1111-4111-8111-bbbbbbbbbbbb','{}'::jsonb);
INSERT INTO api_rate_limits (token_hash,tenant_id,window_started_at,request_count)
VALUES
 ('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','aloews',NOW(),1),
 ('bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb','tenant-beta',NOW(),1);
COMMIT;

SET ROLE ush_runtime;
DO $role_test$
DECLARE is_superuser BOOLEAN; bypasses_rls BOOLEAN;
BEGIN
 SELECT rolsuper,rolbypassrls INTO is_superuser,bypasses_rls
 FROM pg_roles WHERE rolname=current_user;
 IF current_user <> 'ush_runtime' OR is_superuser OR bypasses_rls THEN
   RAISE EXCEPTION 'runtime role must be non-superuser and must not bypass RLS';
 END IF;
END;
$role_test$;

BEGIN;
SELECT set_config('app.is_admin','false',true),set_config('app.tenant_id','aloews',true);
DO $alpha_test$
BEGIN
 IF (SELECT COUNT(*) FROM engineering_tasks) <> 1
    OR EXISTS (SELECT 1 FROM engineering_tasks WHERE id='bbbbbbbb-1111-4111-8111-bbbbbbbbbbbb') THEN
   RAISE EXCEPTION 'tenant alpha can see another tenant task or row count is incorrect';
 END IF;
 IF (SELECT COUNT(*) FROM engineering_worker_tokens) <> 1
    OR EXISTS (SELECT 1 FROM engineering_worker_tokens WHERE worker_id='rls-beta-worker') THEN
   RAISE EXCEPTION 'tenant alpha can see another tenant worker credential';
 END IF;
 IF EXISTS (SELECT 1 FROM projects WHERE id='rls-beta-project')
    OR NOT EXISTS (SELECT 1 FROM projects WHERE id='rls-alpha-project') THEN
   RAISE EXCEPTION 'tenant alpha project RLS failed';
 END IF;
 IF EXISTS (SELECT 1 FROM tenants WHERE id='tenant-beta') THEN
   RAISE EXCEPTION 'tenant alpha can read another tenant record';
 END IF;
 IF (SELECT COUNT(*) FROM tenant_memberships) <> 1
    OR EXISTS (SELECT 1 FROM tenant_memberships WHERE identity_subject='beta-user') THEN
   RAISE EXCEPTION 'tenant alpha membership RLS failed';
 END IF;
 IF (SELECT COUNT(*) FROM tenant_resource_limits) <> 1
    OR EXISTS (SELECT 1 FROM tenant_resource_limits WHERE tenant_id='tenant-beta') THEN
   RAISE EXCEPTION 'tenant alpha resource limit RLS failed';
 END IF;
 IF (SELECT COUNT(*) FROM tenant_budget_policies) <> 1
    OR EXISTS (SELECT 1 FROM tenant_budget_policies WHERE tenant_id='tenant-beta') THEN
   RAISE EXCEPTION 'tenant alpha budget policy RLS failed';
 END IF;
 IF EXISTS (SELECT 1 FROM providers)
    AND ((SELECT COUNT(*) FROM tenant_provider_connections) <> 1
      OR EXISTS (SELECT 1 FROM tenant_provider_connections WHERE tenant_id='tenant-beta')) THEN
   RAISE EXCEPTION 'tenant alpha provider connection RLS failed';
 END IF;
 IF EXISTS (SELECT 1 FROM workspaces WHERE tenant_id='tenant-beta' AND id='default') THEN
   RAISE EXCEPTION 'tenant alpha can read tenant beta workspace';
 END IF;
 IF EXISTS (SELECT 1 FROM tenant_usage_events WHERE id='33333333-3333-4333-8333-333333333333')
    OR EXISTS (SELECT 1 FROM audit_events WHERE id='44444444-4444-4444-8444-444444444444') THEN
   RAISE EXCEPTION 'tenant alpha can read tenant beta usage or audit data';
 END IF;
 IF EXISTS (SELECT 1 FROM tenant_tokens WHERE id='77777777-7777-4777-8777-777777777777')
    OR EXISTS (SELECT 1 FROM project_tokens WHERE id='88888888-8888-4888-8888-888888888888') THEN
   RAISE EXCEPTION 'tenant alpha can read tenant beta tokens';
 END IF;
 IF EXISTS (SELECT 1 FROM api_rate_limits WHERE token_hash=repeat('b',64))
    OR NOT EXISTS (SELECT 1 FROM api_rate_limits WHERE token_hash=repeat('a',64)) THEN
   RAISE EXCEPTION 'tenant alpha API rate-limit RLS failed';
 END IF;
END;
$alpha_test$;
COMMIT;

BEGIN;
SELECT set_config('app.is_admin','false',true),set_config('app.tenant_id','tenant-beta',true);
DO $beta_test$
BEGIN
 IF (SELECT COUNT(*) FROM engineering_tasks) <> 1
    OR EXISTS (SELECT 1 FROM engineering_tasks WHERE id='aaaaaaaa-1111-4111-8111-aaaaaaaaaaaa') THEN
   RAISE EXCEPTION 'tenant beta can see another tenant task or row count is incorrect';
 END IF;
 IF (SELECT COUNT(*) FROM engineering_worker_tokens) <> 1
    OR EXISTS (SELECT 1 FROM engineering_worker_tokens WHERE worker_id='rls-alpha-worker') THEN
   RAISE EXCEPTION 'tenant beta can see another tenant worker credential';
 END IF;
 IF EXISTS (SELECT 1 FROM projects WHERE id='rls-alpha-project')
    OR NOT EXISTS (SELECT 1 FROM projects WHERE id='rls-beta-project') THEN
   RAISE EXCEPTION 'tenant beta project RLS failed';
 END IF;
 IF EXISTS (SELECT 1 FROM tenants WHERE id='aloews') THEN
   RAISE EXCEPTION 'tenant beta can read tenant alpha record';
 END IF;
 IF (SELECT COUNT(*) FROM tenant_memberships) <> 1
    OR EXISTS (SELECT 1 FROM tenant_memberships WHERE identity_subject='alpha-user') THEN
   RAISE EXCEPTION 'tenant beta membership RLS failed';
 END IF;
 IF (SELECT COUNT(*) FROM tenant_resource_limits) <> 1
    OR NOT EXISTS (SELECT 1 FROM tenant_resource_limits WHERE tenant_id='tenant-beta') THEN
   RAISE EXCEPTION 'tenant beta resource limit RLS failed';
 END IF;
 IF (SELECT COUNT(*) FROM tenant_budget_policies) <> 1
    OR NOT EXISTS (SELECT 1 FROM tenant_budget_policies WHERE tenant_id='tenant-beta') THEN
   RAISE EXCEPTION 'tenant beta budget policy RLS failed';
 END IF;
 IF EXISTS (SELECT 1 FROM providers)
    AND ((SELECT COUNT(*) FROM tenant_provider_connections) <> 1
      OR NOT EXISTS (SELECT 1 FROM tenant_provider_connections WHERE tenant_id='tenant-beta')) THEN
   RAISE EXCEPTION 'tenant beta provider connection RLS failed';
 END IF;
 IF NOT EXISTS (SELECT 1 FROM workspaces WHERE tenant_id='tenant-beta' AND id='default') THEN
   RAISE EXCEPTION 'tenant beta cannot read its own workspace';
 END IF;
 IF NOT EXISTS (SELECT 1 FROM tenant_usage_events WHERE id='33333333-3333-4333-8333-333333333333')
    OR NOT EXISTS (SELECT 1 FROM audit_events WHERE id='44444444-4444-4444-8444-444444444444') THEN
   RAISE EXCEPTION 'tenant beta cannot read its own usage or audit data';
 END IF;
 IF NOT EXISTS (SELECT 1 FROM tenant_tokens WHERE id='77777777-7777-4777-8777-777777777777')
    OR NOT EXISTS (SELECT 1 FROM project_tokens WHERE id='88888888-8888-4888-8888-888888888888') THEN
   RAISE EXCEPTION 'tenant beta cannot read its own tokens';
 END IF;
 IF EXISTS (SELECT 1 FROM api_rate_limits WHERE token_hash=repeat('a',64))
    OR NOT EXISTS (SELECT 1 FROM api_rate_limits WHERE token_hash=repeat('b',64)) THEN
   RAISE EXCEPTION 'tenant beta API rate-limit RLS failed';
 END IF;
 BEGIN
   INSERT INTO engineering_tasks (id,tenant_id,project_id,idempotency_key,title,objective,created_by)
   VALUES ('cccccccc-1111-4111-8111-cccccccccccc','tenant-beta','rls-alpha-project','forged','Forged','Cross tenant','test');
   RAISE EXCEPTION 'tenant beta inserted a task into tenant alpha project';
 EXCEPTION WHEN raise_exception THEN
   IF SQLERRM = 'tenant beta inserted a task into tenant alpha project' THEN RAISE; END IF;
 END;
END;
$beta_test$;
COMMIT;

BEGIN;
SELECT set_config('app.is_admin','false',true),set_config('app.tenant_id','',true);
DO $no_context_test$
BEGIN
 IF (SELECT COUNT(*) FROM engineering_tasks) <> 0 THEN
   RAISE EXCEPTION 'missing tenant context must fail closed';
 END IF;
 IF (SELECT COUNT(*) FROM engineering_worker_tokens) <> 0 THEN
   RAISE EXCEPTION 'missing tenant context exposed worker credentials';
 END IF;
 IF EXISTS (SELECT 1 FROM projects) OR EXISTS (SELECT 1 FROM tenants)
    OR EXISTS (SELECT 1 FROM tenant_usage_events) OR EXISTS (SELECT 1 FROM audit_events) THEN
   RAISE EXCEPTION 'missing tenant context exposed core tenant data';
 END IF;
 IF EXISTS (SELECT 1 FROM tenant_tokens) OR EXISTS (SELECT 1 FROM project_tokens) THEN
   RAISE EXCEPTION 'missing tenant context exposed token records';
 END IF;
 IF EXISTS (SELECT 1 FROM api_rate_limits) THEN
   RAISE EXCEPTION 'missing tenant context exposed API rate-limit records';
 END IF;
 IF EXISTS (SELECT 1 FROM workspaces) THEN
   RAISE EXCEPTION 'missing tenant context exposed workspace rows';
 END IF;
 IF EXISTS (SELECT 1 FROM tenant_memberships)
    OR EXISTS (SELECT 1 FROM tenant_resource_limits)
    OR EXISTS (SELECT 1 FROM tenant_budget_policies)
    OR EXISTS (SELECT 1 FROM tenant_provider_connections) THEN
   RAISE EXCEPTION 'missing tenant context exposed tenant configuration';
 END IF;
END;
$no_context_test$;
COMMIT;

RESET ROLE;
DELETE FROM tenant_tokens WHERE id='77777777-7777-4777-8777-777777777777';
DELETE FROM project_tokens WHERE id='88888888-8888-4888-8888-888888888888';
DELETE FROM api_rate_limits WHERE token_hash IN (repeat('a',64),repeat('b',64));
DELETE FROM tenant_provider_connections WHERE id IN ('aaaa0000-2222-4222-8222-aaaaaaaaaaaa','bbbb0000-2222-4222-8222-bbbbbbbbbbbb');
DELETE FROM tenant_memberships WHERE id IN ('aaaa0000-1111-4111-8111-aaaaaaaaaaaa','bbbb0000-1111-4111-8111-bbbbbbbbbbbb');
DELETE FROM tenant_resource_limits WHERE tenant_id='tenant-beta';
DELETE FROM tenant_budget_policies WHERE tenant_id='tenant-beta';
DELETE FROM audit_events WHERE id='44444444-4444-4444-8444-444444444444';
DELETE FROM tenant_usage_events WHERE id='33333333-3333-4333-8333-333333333333';
DELETE FROM engineering_worker_tokens WHERE id IN (
 'aaaaaaaa-2222-4222-8222-aaaaaaaaaaaa','bbbbbbbb-2222-4222-8222-bbbbbbbbbbbb'
);
DELETE FROM engineering_tasks WHERE id IN (
 'aaaaaaaa-1111-4111-8111-aaaaaaaaaaaa','bbbbbbbb-1111-4111-8111-bbbbbbbbbbbb'
);
DELETE FROM tenant_usage_events WHERE id='33333333-3333-4333-8333-333333333333';
DELETE FROM audit_events WHERE id='44444444-4444-4444-8444-444444444444';
DELETE FROM projects WHERE id IN ('rls-alpha-project','rls-beta-project');
DELETE FROM workspaces WHERE tenant_id='tenant-beta' AND id='default';
DELETE FROM tenants WHERE id='tenant-beta';
SQL
