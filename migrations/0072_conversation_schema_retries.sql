CREATE TABLE IF NOT EXISTS conversation_schema_retries (
    call_id TEXT PRIMARY KEY REFERENCES conversation_authorization_revisions(id),
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    retry_of TEXT NOT NULL REFERENCES conversation_model_calls(id),
    input_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(task_id, retry_of)
);

CREATE INDEX IF NOT EXISTS conversation_schema_retries_source
    ON conversation_schema_retries(task_id, retry_of);
