CREATE TABLE IF NOT EXISTS conversation_tasks (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    source_message_id TEXT NOT NULL,
    schema_revision TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(conversation_id, source_message_id),
    FOREIGN KEY(conversation_id, source_message_id)
        REFERENCES conversation_messages(conversation_id, message_id)
);
