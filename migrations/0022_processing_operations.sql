CREATE TABLE IF NOT EXISTS sample_scope_seals (
    sample_test_id TEXT PRIMARY KEY,
    scope_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS processing_operations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    request_json TEXT NOT NULL,
    state_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS batch_model_call_allowances (
    batch_id TEXT PRIMARY KEY,
    maximum INTEGER NOT NULL CHECK(maximum > 0),
    reserved INTEGER NOT NULL DEFAULT 0 CHECK(reserved >= 0)
);
