CREATE TABLE IF NOT EXISTS conversation_queued_planning (
    call_id TEXT PRIMARY KEY REFERENCES conversation_authorization_revisions(id),
    conversation_id TEXT NOT NULL,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    message_id TEXT NOT NULL,
    record_json TEXT NOT NULL,
    UNIQUE(conversation_id,message_id),
    FOREIGN KEY(conversation_id,message_id)
        REFERENCES conversation_message_queue(conversation_id,message_id)
);
