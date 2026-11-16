-- Multi-tenant identity and ownership foundation.
-- Aloews is the first tenant; existing projects are backfilled into its default workspace.
CREATE TABLE tenants (
    id VARCHAR(80) PRIMARY KEY,
    slug VARCHAR(80) NOT NULL UNIQUE,
    name VARCHAR(160) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended','deleting','deleted')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE workspaces (
    id VARCHAR(80) NOT NULL,
    tenant_id VARCHAR(80) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    slug VARCHAR(80) NOT NULL,
    name VARCHAR(160) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended','archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, id),
    UNIQUE (tenant_id, slug)
);

INSERT INTO tenants (id, slug, name) VALUES ('aloews', 'aloews', 'Aloews Development Center')
ON CONFLICT (id) DO NOTHING;
INSERT INTO workspaces (tenant_id, id, slug, name) VALUES ('aloews', 'default', 'default', 'Default workspace')
ON CONFLICT (tenant_id, id) DO NOTHING;

ALTER TABLE projects ADD COLUMN tenant_id VARCHAR(80);
ALTER TABLE projects ADD COLUMN workspace_id VARCHAR(80);
UPDATE projects SET tenant_id='aloews', workspace_id='default' WHERE tenant_id IS NULL;
ALTER TABLE projects ALTER COLUMN tenant_id SET NOT NULL;
ALTER TABLE projects ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE projects ADD CONSTRAINT uq_projects_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE projects ADD CONSTRAINT fk_projects_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id);
ALTER TABLE projects ADD CONSTRAINT fk_projects_workspace FOREIGN KEY (tenant_id, workspace_id) REFERENCES workspaces(tenant_id, id);

CREATE FUNCTION ush_assign_project_tenant() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.tenant_id IS NULL THEN NEW.tenant_id := 'aloews'; END IF;
    IF NEW.workspace_id IS NULL THEN NEW.workspace_id := 'default'; END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER trg_projects_assign_tenant BEFORE INSERT ON projects
FOR EACH ROW EXECUTE FUNCTION ush_assign_project_tenant();

ALTER TABLE project_tokens ADD COLUMN tenant_id VARCHAR(80);
UPDATE project_tokens pt SET tenant_id=p.tenant_id FROM projects p WHERE p.id=pt.project_id;
ALTER TABLE project_tokens ALTER COLUMN tenant_id SET NOT NULL;
ALTER TABLE project_tokens ADD CONSTRAINT fk_project_tokens_tenant_project
    FOREIGN KEY (tenant_id, project_id) REFERENCES projects(tenant_id, id) ON DELETE CASCADE;

CREATE FUNCTION ush_assign_project_token_tenant() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    SELECT tenant_id INTO NEW.tenant_id FROM projects WHERE id=NEW.project_id;
    IF NEW.tenant_id IS NULL THEN RAISE EXCEPTION 'project tenant not found'; END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER trg_project_tokens_assign_tenant BEFORE INSERT ON project_tokens
FOR EACH ROW EXECUTE FUNCTION ush_assign_project_token_tenant();

CREATE TABLE tenant_memberships (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    identity_issuer VARCHAR(500) NOT NULL,
    identity_subject VARCHAR(500) NOT NULL,
    role VARCHAR(32) NOT NULL CHECK (role IN ('tenant_owner','tenant_admin','workspace_admin','project_maintainer','developer','reviewer','operator','auditor','viewer')),
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active','suspended','revoked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, identity_issuer, identity_subject)
);
CREATE INDEX ix_tenant_memberships_subject ON tenant_memberships(identity_issuer, identity_subject);

CREATE TABLE tenant_tokens (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name VARCHAR(120) NOT NULL,
    role VARCHAR(32) NOT NULL CHECK (role IN ('tenant_owner','tenant_admin','workspace_admin','project_maintainer','developer','reviewer','operator','auditor','viewer','service_agent')),
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX ix_tenant_tokens_tenant ON tenant_tokens(tenant_id);

ALTER TABLE audit_events ADD COLUMN tenant_id VARCHAR(80) REFERENCES tenants(id);
CREATE INDEX ix_audit_events_tenant_time ON audit_events(tenant_id, occurred_at DESC);

ALTER TABLE api_rate_limits ADD COLUMN tenant_id VARCHAR(80) REFERENCES tenants(id);
CREATE INDEX ix_api_rate_limits_tenant_updated ON api_rate_limits(tenant_id, updated_at);

CREATE TABLE tenant_provider_connections (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    provider_id VARCHAR(80) NOT NULL REFERENCES providers(id) ON DELETE RESTRICT,
    credential_reference VARCHAR(500) NOT NULL,
    configuration JSONB NOT NULL DEFAULT '{}'::jsonb,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, provider_id),
    UNIQUE (tenant_id, id)
);

CREATE TABLE tenant_resource_limits (
    tenant_id VARCHAR(80) PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    max_active_tasks INTEGER NOT NULL DEFAULT 5 CHECK (max_active_tasks BETWEEN 1 AND 10000),
    max_concurrent_workers INTEGER NOT NULL DEFAULT 2 CHECK (max_concurrent_workers BETWEEN 1 AND 1000),
    max_cpu_millis INTEGER NOT NULL DEFAULT 4000 CHECK (max_cpu_millis > 0),
    max_memory_bytes BIGINT NOT NULL DEFAULT 8589934592 CHECK (max_memory_bytes > 0),
    max_storage_bytes BIGINT NOT NULL DEFAULT 10737418240 CHECK (max_storage_bytes > 0),
    max_requests_per_minute INTEGER NOT NULL DEFAULT 120 CHECK (max_requests_per_minute BETWEEN 1 AND 1000000),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
INSERT INTO tenant_resource_limits (tenant_id) VALUES ('aloews') ON CONFLICT DO NOTHING;

CREATE TABLE tenant_budget_policies (
    tenant_id VARCHAR(80) PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    currency CHAR(3) NOT NULL DEFAULT 'USD',
    soft_limit_minor BIGINT CHECK (soft_limit_minor >= 0),
    hard_limit_minor BIGINT CHECK (hard_limit_minor >= 0),
    period VARCHAR(16) NOT NULL DEFAULT 'monthly' CHECK (period IN ('daily','weekly','monthly')),
    require_approval_above_limit BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (soft_limit_minor IS NULL OR hard_limit_minor IS NULL OR soft_limit_minor <= hard_limit_minor)
);
INSERT INTO tenant_budget_policies (tenant_id) VALUES ('aloews') ON CONFLICT DO NOTHING;

CREATE TABLE tenant_usage_events (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(80) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    project_id VARCHAR(80),
    task_id VARCHAR(36),
    idempotency_key VARCHAR(160) NOT NULL,
    usage_type VARCHAR(48) NOT NULL,
    provider_id VARCHAR(80),
    model_id VARCHAR(160),
    quantity NUMERIC(24,8),
    amount_minor BIGINT CHECK (amount_minor >= 0),
    currency CHAR(3),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    UNIQUE (tenant_id, idempotency_key),
    CHECK (amount_minor IS NULL OR currency IS NOT NULL)
);
CREATE INDEX ix_tenant_usage_period ON tenant_usage_events(tenant_id, occurred_at DESC);
