CREATE TABLE IF NOT EXISTS geometry_quality_reports (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    candidate_artifact_id TEXT NOT NULL,
    reference_artifact_id TEXT,
    source TEXT NOT NULL,
    report_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_geometry_quality_project_created
ON geometry_quality_reports(project_id, created_at DESC);

CREATE TABLE IF NOT EXISTS geometry_correction_evidence (
    quality_report_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    annotation_id TEXT NOT NULL,
    source_node_id TEXT NOT NULL,
    source_model_profile_id TEXT,
    source_model_revision INTEGER,
    reason TEXT NOT NULL,
    evidence_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(quality_report_id) REFERENCES geometry_quality_reports(id)
);

CREATE INDEX IF NOT EXISTS idx_geometry_corrections_project_created
ON geometry_correction_evidence(project_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_geometry_corrections_run_created
ON geometry_correction_evidence(run_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_geometry_corrections_model_revision
ON geometry_correction_evidence(
    project_id,
    source_model_profile_id,
    source_model_revision,
    created_at DESC
);
