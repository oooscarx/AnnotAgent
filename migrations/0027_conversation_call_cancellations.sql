CREATE TABLE IF NOT EXISTS conversation_call_cancellations (
    call_id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    requested_at TEXT NOT NULL
);
