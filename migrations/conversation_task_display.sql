-- Optional navigation metadata. Original messages, authorizations and ledgers stay immutable.
-- Additive and idempotent; kept outside sequential history migrations for independent integration.
CREATE TABLE IF NOT EXISTS conversation_task_display (
    task_id TEXT PRIMARY KEY REFERENCES conversation_tasks(id),
    title TEXT CHECK (title IS NULL OR length(trim(title)) BETWEEN 1 AND 160),
    archived_at TEXT,
    updated_at TEXT NOT NULL
);
