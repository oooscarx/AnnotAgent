CREATE TABLE IF NOT EXISTS model_profiles (
    id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    remote_model_id TEXT NOT NULL,
    status TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    locked INTEGER NOT NULL,
    profile_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(id, revision),
    CHECK(revision > 0)
);

CREATE INDEX IF NOT EXISTS idx_model_profiles_provider
ON model_profiles(provider_id, id, revision DESC);

CREATE TABLE IF NOT EXISTS project_model_bindings (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    match_kind TEXT NOT NULL,
    match_value TEXT NOT NULL,
    capability TEXT NOT NULL,
    role TEXT NOT NULL,
    model_profile_id TEXT NOT NULL,
    locked INTEGER NOT NULL,
    binding_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(project_id, match_kind, match_value)
);

CREATE INDEX IF NOT EXISTS idx_project_model_bindings_model
ON project_model_bindings(model_profile_id, project_id);

CREATE TABLE IF NOT EXISTS global_model_defaults (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    defaults_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
