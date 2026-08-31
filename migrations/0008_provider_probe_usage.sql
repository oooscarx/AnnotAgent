CREATE TABLE IF NOT EXISTS provider_probe_usage (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    model_profile_id TEXT NOT NULL,
    model_profile_revision INTEGER NOT NULL,
    request_id TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER,
    cost TEXT NOT NULL,
    currency TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    succeeded INTEGER NOT NULL,
    safe_message TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_provider_probe_usage_model
ON provider_probe_usage(model_profile_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_provider_probe_usage_provider
ON provider_probe_usage(provider_id, created_at DESC);
