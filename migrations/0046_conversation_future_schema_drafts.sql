-- Local future-only human Schema branch; never a grant or executable continuation.
CREATE TABLE IF NOT EXISTS conversation_future_schema_drafts (
    feedback_call_id TEXT PRIMARY KEY REFERENCES conversation_feedback_scope_answers(call_id),
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    command_id TEXT NOT NULL UNIQUE,
    schema_id TEXT NOT NULL UNIQUE REFERENCES conversation_schema_drafts(id),
    input_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS future_schema_drafts_by_task
    ON conversation_future_schema_drafts(task_id, feedback_call_id);
