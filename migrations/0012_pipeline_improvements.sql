CREATE TABLE IF NOT EXISTS pipeline_improvement_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    baseline_workflow_id TEXT NOT NULL,
    baseline_workflow_version INTEGER NOT NULL,
    status TEXT NOT NULL,
    session_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pipeline_improvements_project_updated
ON pipeline_improvement_sessions(project_id, updated_at DESC);
