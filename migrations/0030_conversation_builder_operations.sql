CREATE TABLE IF NOT EXISTS conversation_builder_operations (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    request_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('reserved','completed','interrupted')),
    evidence_json TEXT
);
