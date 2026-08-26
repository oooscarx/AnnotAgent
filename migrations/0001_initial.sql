PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    config_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS project_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    relative_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    thumbnail_path TEXT,
    imported_at TEXT NOT NULL,
    UNIQUE(project_id, sha256)
);
CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    task_json TEXT NOT NULL,
    UNIQUE(project_id, task_id)
);
CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    project_name TEXT NOT NULL,
    skill_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    status TEXT NOT NULL,
    project_schema_json TEXT NOT NULL,
    terminal_reason TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS active_project_runs (
    project_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    idempotency_key TEXT,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS run_start_requests (
    project_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    run_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(project_id, idempotency_key)
);
CREATE TABLE IF NOT EXISTS run_images (
    run_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    status TEXT NOT NULL,
    PRIMARY KEY(run_id, image_id)
);
CREATE TABLE IF NOT EXISTS task_runs (
    run_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    status TEXT NOT NULL,
    reason TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(run_id, image_id, task_id)
);
CREATE TABLE IF NOT EXISTS run_steps (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    image_id TEXT,
    task_id TEXT,
    step_index INTEGER NOT NULL,
    summary TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS run_events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    run_id TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    event_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_run_events_run_sequence ON run_events(run_id, sequence);
CREATE TABLE IF NOT EXISTS model_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    endpoint_summary TEXT NOT NULL,
    request_id TEXT,
    success INTEGER NOT NULL,
    retry_count INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS model_messages (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    image_id TEXT,
    task_id TEXT,
    message_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_model_messages_run_sequence
ON model_messages(run_id, sequence);
CREATE TABLE IF NOT EXISTS tool_calls (
    call_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    name TEXT NOT NULL,
    arguments_json TEXT NOT NULL,
    result_json TEXT,
    error TEXT,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS vision_artifacts (
    artifact_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    task_id TEXT,
    source_node TEXT NOT NULL,
    validation_state TEXT NOT NULL,
    artifact_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_vision_artifacts_run
ON vision_artifacts(run_id, created_at);
CREATE TABLE IF NOT EXISTS annotations (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    label TEXT,
    review_status TEXT NOT NULL,
    annotation_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS annotation_revisions (
    revision_id TEXT PRIMARY KEY,
    annotation_id TEXT NOT NULL,
    parent_revision_id TEXT,
    revision_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS validation_issues (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    code TEXT NOT NULL,
    severity TEXT NOT NULL,
    issue_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS usage_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    usage_json TEXT NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER,
    cost TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS correction_records (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    skill_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    predicted_label TEXT,
    corrected_label TEXT,
    reason_code TEXT NOT NULL,
    record_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_corrections_lookup
ON correction_records(project_id, skill_id, task_id, predicted_label, reason_code, created_at);
CREATE TABLE IF NOT EXISTS review_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    annotation_id TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    reasons_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    resolved_at TEXT
);
CREATE TABLE IF NOT EXISTS settings_metadata (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (lower(key) NOT LIKE '%api_key%' AND lower(key) NOT LIKE '%secret%')
);
CREATE TABLE IF NOT EXISTS workflow_drafts (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    status TEXT NOT NULL,
    draft_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_workflow_drafts_project_updated
ON workflow_drafts(project_id, updated_at DESC);
CREATE TABLE IF NOT EXISTS workflow_versions (
    workflow_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    source_draft_id TEXT NOT NULL,
    version_json TEXT NOT NULL,
    published_at TEXT NOT NULL,
    PRIMARY KEY(workflow_id, version)
);
