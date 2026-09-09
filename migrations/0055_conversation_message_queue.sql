CREATE TABLE IF NOT EXISTS conversation_message_queue (
    conversation_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    sequence INTEGER NOT NULL,
    cancelled_at TEXT,
    PRIMARY KEY(conversation_id, message_id),
    FOREIGN KEY(conversation_id, message_id)
        REFERENCES conversation_send_receipts(conversation_id, message_id)
);
CREATE INDEX IF NOT EXISTS conversation_message_queue_order
    ON conversation_message_queue(task_id, sequence);
