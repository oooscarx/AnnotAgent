-- A scope answer records human intent only. It is not a model grant, an annotation
-- revision, a Schema answer, or an executable continuation.
CREATE TABLE IF NOT EXISTS conversation_feedback_scope_answers (
    call_id TEXT PRIMARY KEY REFERENCES conversation_feedback_authorizations(call_id),
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    command_id TEXT NOT NULL UNIQUE,
    input_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS feedback_scope_answers_by_task
    ON conversation_feedback_scope_answers(task_id, call_id);
