CREATE TABLE IF NOT EXISTS conversation_call_grants (
    task_id TEXT PRIMARY KEY REFERENCES conversation_tasks(id),
    id TEXT NOT NULL UNIQUE,
    scope_hash TEXT NOT NULL,
    maximum_calls INTEGER NOT NULL CHECK(maximum_calls BETWEEN 1 AND 128),
    expires_at TEXT NOT NULL,
    revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1))
);
CREATE TABLE IF NOT EXISTS conversation_model_calls (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES conversation_call_grants(task_id),
    request_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('reserved','completed','failed','in_doubt')),
    evidence_json TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS conversation_model_calls_task ON conversation_model_calls(task_id);
