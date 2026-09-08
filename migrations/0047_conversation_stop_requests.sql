-- A frozen user command, not an executor or an additional spending authority.
CREATE TABLE IF NOT EXISTS conversation_stop_requests (
    message_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    project_id TEXT NOT NULL,
    record_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(conversation_id,message_id)
        REFERENCES conversation_messages(conversation_id,message_id)
);
CREATE INDEX IF NOT EXISTS conversation_stop_requests_conversation
    ON conversation_stop_requests(conversation_id);
