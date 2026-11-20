-- Persist media type for artifact downloads and integrity checks.
ALTER TABLE execution_artifacts
    ADD COLUMN IF NOT EXISTS content_type VARCHAR(128) NOT NULL DEFAULT 'application/octet-stream'
        CHECK (content_type <> '' AND length(content_type) <= 128);
