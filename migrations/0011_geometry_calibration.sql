CREATE TABLE IF NOT EXISTS project_geometry_policies (
    project_id TEXT NOT NULL,
    task_kind TEXT NOT NULL,
    policy_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(project_id, task_kind)
);

CREATE TABLE IF NOT EXISTS geometry_calibration_reports (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    label_id TEXT,
    model_profile_id TEXT NOT NULL,
    model_profile_revision INTEGER NOT NULL,
    node_definition_id TEXT NOT NULL,
    node_config_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    report_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_geometry_calibration_project_created
ON geometry_calibration_reports(project_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_geometry_calibration_exact_context
ON geometry_calibration_reports(
    project_id,
    task_id,
    label_id,
    model_profile_id,
    model_profile_revision,
    node_definition_id,
    node_config_hash,
    created_at DESC
);
