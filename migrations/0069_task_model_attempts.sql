CREATE TABLE IF NOT EXISTS task_model_attempts (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    attempt_id TEXT NOT NULL UNIQUE,
    project_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    call_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('task','probe')),
    status TEXT NOT NULL CHECK(status IN ('started','succeeded','failed','in_doubt')),
    model_profile_id TEXT NOT NULL,
    model_profile_revision INTEGER NOT NULL,
    provider_id TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    request_id TEXT,
    input_tokens INTEGER,
    cached_input_tokens INTEGER,
    output_tokens INTEGER,
    image_count INTEGER NOT NULL,
    usage_source TEXT NOT NULL,
    cost TEXT,
    currency TEXT,
    pricing_snapshot_json TEXT NOT NULL,
    effective_request_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    duration_ms INTEGER,
    failure_json TEXT,
    UNIQUE(call_id, attempt_number)
);

CREATE INDEX IF NOT EXISTS idx_task_model_attempts_task_sequence
ON task_model_attempts(task_id, sequence);

CREATE INDEX IF NOT EXISTS idx_task_model_attempts_project_task
ON task_model_attempts(project_id, conversation_id, task_id);
