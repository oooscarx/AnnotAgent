-- Intent revisions attach to the existing Task; they grant no inference permission.
CREATE TABLE IF NOT EXISTS task_delivery_intents (
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    revision INTEGER NOT NULL CHECK(revision > 0),
    command_id TEXT NOT NULL,
    expected_revision INTEGER NOT NULL CHECK(expected_revision >= 0),
    content_sha256 TEXT NOT NULL,
    intent_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(task_id, revision),
    UNIQUE(task_id, command_id)
);
