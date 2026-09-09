CREATE TABLE IF NOT EXISTS conversation_queued_workflow_copies (
    -- Keep the idempotency receipt after explicit Draft purge. Recovery fails
    -- closed when the copy is gone; it must never resurrect it or block purge.
    copy_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    message_id TEXT NOT NULL,
    request_json TEXT NOT NULL,
    FOREIGN KEY(conversation_id,message_id) REFERENCES conversation_message_queue(conversation_id,message_id)
);
