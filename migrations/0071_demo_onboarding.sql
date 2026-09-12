CREATE TABLE IF NOT EXISTS demo_start_commands (
    command_id TEXT PRIMARY KEY,
    scope_json TEXT NOT NULL,
    receipt_json TEXT NOT NULL,
    project_owner_id TEXT NOT NULL UNIQUE,
    conversation_id TEXT NOT NULL UNIQUE REFERENCES project_conversations(id),
    task_id TEXT NOT NULL UNIQUE REFERENCES conversation_tasks(id),
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS demo_preset_candidate_imports (
    command_id TEXT PRIMARY KEY REFERENCES demo_start_commands(command_id),
    project_owner_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL,
    source_asset_id TEXT NOT NULL,
    source_asset_sha256 TEXT NOT NULL,
    review_status TEXT NOT NULL CHECK(review_status = 'needs_review'),
    candidates_json TEXT NOT NULL,
    imported_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS demo_preset_annotations (
    annotation_id TEXT PRIMARY KEY,
    command_id TEXT NOT NULL REFERENCES demo_start_commands(command_id),
    project_owner_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    source_artifact_id TEXT NOT NULL,
    annotation_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(command_id, source_artifact_id)
);

CREATE INDEX IF NOT EXISTS idx_demo_preset_annotations_task_image
ON demo_preset_annotations(project_owner_id, conversation_id, task_id, image_id, annotation_id);
