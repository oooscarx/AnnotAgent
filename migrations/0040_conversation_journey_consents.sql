-- Bounded consent evidence, not a second executor or a replacement call ledger.
CREATE TABLE IF NOT EXISTS conversation_journey_consents (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    builder_operation_id TEXT NOT NULL UNIQUE,
    sample_operation_id TEXT NOT NULL UNIQUE,
    input_json TEXT NOT NULL,
    revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1)),
    sample_scope_json TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS conversation_journey_consents_task
    ON conversation_journey_consents(task_id);
