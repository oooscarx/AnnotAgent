CREATE TABLE IF NOT EXISTS sample_operations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    draft_id TEXT NOT NULL,
    authorization_fingerprint TEXT NOT NULL,
    request_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('queued','running','cancelling','cancelled','interrupted','failed','succeeded')),
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS one_active_sample_per_project
ON sample_operations(project_id) WHERE status IN ('queued','running','cancelling');
