-- Extend fail-closed tenant RLS to core tenant-owned product data.
-- Token lookup tables are intentionally excluded because authentication must
-- resolve a token before a request-scoped tenant context exists. Their APIs
-- enforce tenant/project ownership explicitly.
DO $migration$
DECLARE table_name TEXT;
BEGIN
    FOREACH table_name IN ARRAY ARRAY[
        'workspaces',
        'tenant_memberships',
        'tenant_provider_connections',
        'tenant_resource_limits',
        'tenant_budget_policies',
        'tenant_usage_events',
        'projects',
        'audit_events'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', table_name);
        EXECUTE format('DROP POLICY IF EXISTS tenant_core_isolation ON %I', table_name);
        EXECUTE format(
            'CREATE POLICY tenant_core_isolation ON %I USING (COALESCE(current_setting(''app.is_admin'', true), ''false'') = ''true'' OR tenant_id = NULLIF(current_setting(''app.tenant_id'', true), '''')) WITH CHECK (COALESCE(current_setting(''app.is_admin'', true), ''false'') = ''true'' OR tenant_id = NULLIF(current_setting(''app.tenant_id'', true), ''''))',
            table_name
        );
    END LOOP;
END;
$migration$;

ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenants FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_core_isolation ON tenants;
CREATE POLICY tenant_core_isolation ON tenants
    USING (
        COALESCE(current_setting('app.is_admin', true), 'false') = 'true'
        OR id = NULLIF(current_setting('app.tenant_id', true), '')
    )
    WITH CHECK (
        COALESCE(current_setting('app.is_admin', true), 'false') = 'true'
        OR id = NULLIF(current_setting('app.tenant_id', true), '')
    );

COMMENT ON POLICY tenant_core_isolation ON projects IS
    'Project rows are visible only within the authenticated transaction tenant or explicit platform maintenance context.';
COMMENT ON POLICY tenant_core_isolation ON tenants IS
    'Tenant rows are visible only to that tenant or explicit platform maintenance context.';
