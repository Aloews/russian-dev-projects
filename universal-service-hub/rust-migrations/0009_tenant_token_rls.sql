-- Token tables are tenant-owned. Authentication performs a short-lived
-- platform-scoped lookup before it constructs the request principal; all
-- management endpoints use the authenticated tenant/project transaction.
DO $migration$
DECLARE table_name TEXT;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'tenant_tokens',
        'project_tokens'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', table_name);
        EXECUTE format('DROP POLICY IF EXISTS tenant_token_isolation ON %I', table_name);
        EXECUTE format(
            'CREATE POLICY tenant_token_isolation ON %I USING (COALESCE(current_setting(''app.is_admin'', true), ''false'') = ''true'' OR tenant_id = NULLIF(current_setting(''app.tenant_id'', true), '''')) WITH CHECK (COALESCE(current_setting(''app.is_admin'', true), ''false'') = ''true'' OR tenant_id = NULLIF(current_setting(''app.tenant_id'', true), ''''))',
            table_name
        );
    END LOOP;
END;
$migration$;
