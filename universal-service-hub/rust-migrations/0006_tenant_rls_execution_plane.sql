-- Row-level security for the engineering execution plane.
-- Application requests must set app.tenant_id inside a transaction. Platform
-- maintenance uses app.is_admin=true inside a transaction. Missing context fails closed.

DO $migration$
DECLARE table_name TEXT;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'engineering_tasks',
        'engineering_worker_tokens',
        'engineering_task_events',
        'engineering_task_checkpoints',
        'engineering_task_dependencies',
        'engineering_task_attempts',
        'engineering_workspaces',
        'engineering_task_workspace_assignments',
        'engineering_task_path_locks'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', table_name);
        EXECUTE format('DROP POLICY IF EXISTS tenant_execution_isolation ON %I', table_name);
        EXECUTE format(
            'CREATE POLICY tenant_execution_isolation ON %I USING (COALESCE(current_setting(''app.is_admin'', true), ''false'') = ''true'' OR tenant_id = NULLIF(current_setting(''app.tenant_id'', true), '''')) WITH CHECK (COALESCE(current_setting(''app.is_admin'', true), ''false'') = ''true'' OR tenant_id = NULLIF(current_setting(''app.tenant_id'', true), ''''))',
            table_name
        );
    END LOOP;
END;
$migration$;

COMMENT ON POLICY tenant_execution_isolation ON engineering_tasks IS
    'Requires transaction-local app.tenant_id or explicit platform maintenance context; missing context sees no tenant rows.';
