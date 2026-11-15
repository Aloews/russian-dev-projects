CREATE TABLE IF NOT EXISTS api_rate_limits (
    token_hash VARCHAR(64) PRIMARY KEY,
    window_started_at TIMESTAMPTZ NOT NULL,
    request_count INTEGER NOT NULL CHECK (request_count > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS ix_api_rate_limits_updated_at
    ON api_rate_limits(updated_at);
