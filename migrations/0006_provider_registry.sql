CREATE TABLE IF NOT EXISTS provider_profiles (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    preset_id TEXT,
    adapter TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    credential_source TEXT,
    profile_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_provider_profiles_updated
ON provider_profiles(updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_provider_profiles_preset
ON provider_profiles(preset_id, updated_at DESC);
