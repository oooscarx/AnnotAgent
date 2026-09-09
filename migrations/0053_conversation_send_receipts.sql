CREATE TABLE IF NOT EXISTS conversation_send_receipts (
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    message_id TEXT NOT NULL,
    input_json TEXT NOT NULL,
    receipt_json TEXT NOT NULL,
    PRIMARY KEY (conversation_id, message_id)
);
