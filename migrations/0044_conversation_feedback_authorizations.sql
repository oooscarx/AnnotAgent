-- Original consent and frozen context survive the pre-dispatch gap. The existing
-- conversation call ledger remains the sole execution and spending authority.
CREATE TABLE IF NOT EXISTS conversation_feedback_authorizations (
    call_id TEXT PRIMARY KEY REFERENCES conversation_authorization_revisions(id),
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    message_id TEXT NOT NULL,
    record_json TEXT NOT NULL,
    UNIQUE(conversation_id, message_id),
    FOREIGN KEY(conversation_id, message_id)
        REFERENCES conversation_messages(conversation_id, message_id)
);
CREATE INDEX IF NOT EXISTS feedback_authorizations_by_task
    ON conversation_feedback_authorizations(task_id, message_id);
