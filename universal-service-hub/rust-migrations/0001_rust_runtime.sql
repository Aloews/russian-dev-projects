CREATE TABLE IF NOT EXISTS projects (
    id VARCHAR(80) PRIMARY KEY,
    name VARCHAR(160) NOT NULL,
    allowed_capabilities JSONB NOT NULL DEFAULT '[]'::jsonb,
    allowed_provider_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS project_tokens (
    id VARCHAR(36) PRIMARY KEY,
    project_id VARCHAR(80) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name VARCHAR(120) NOT NULL,
    role VARCHAR(20) NOT NULL CHECK (role IN ('owner','operator','viewer')),
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ix_project_tokens_project_id ON project_tokens(project_id);

CREATE TABLE IF NOT EXISTS audit_events (
    id VARCHAR(36) PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor VARCHAR(120) NOT NULL,
    action VARCHAR(80) NOT NULL,
    resource_type VARCHAR(80) NOT NULL,
    resource_id VARCHAR(160) NOT NULL,
    details JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS ix_audit_events_occurred_at ON audit_events(occurred_at DESC);

CREATE TABLE IF NOT EXISTS providers (
    id VARCHAR(80) PRIMARY KEY,
    name VARCHAR(160) NOT NULL,
    capabilities JSONB NOT NULL DEFAULT '[]'::jsonb,
    priority INTEGER NOT NULL DEFAULT 100,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    health_url TEXT,
    adapter_type VARCHAR(32) NOT NULL DEFAULT 'none',
    model VARCHAR(160),
    base_url TEXT,
    api_key_env VARCHAR(120),
    supports_idempotency BOOLEAN NOT NULL DEFAULT FALSE,
    timeout_seconds DOUBLE PRECISION NOT NULL DEFAULT 3,
    health_status VARCHAR(20) NOT NULL DEFAULT 'unknown',
    checked_at TIMESTAMPTZ,
    latency_ms DOUBLE PRECISION,
    health_detail VARCHAR(160),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
