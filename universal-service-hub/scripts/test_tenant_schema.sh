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
ROLLBACK;
SQL
