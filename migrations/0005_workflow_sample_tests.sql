CREATE TABLE IF NOT EXISTS workflow_sample_tests (
    draft_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    report_json TEXT NOT NULL,
    completed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workflow_sample_tests_project_completed
ON workflow_sample_tests(project_id, completed_at DESC);

