CREATE TABLE IF NOT EXISTS conversation_task_lifecycle (
    task_id TEXT PRIMARY KEY REFERENCES conversation_tasks(id),
    state TEXT NOT NULL CHECK(state IN ('active', 'archived', 'trashed')),
    revision INTEGER NOT NULL CHECK(revision >= 1),
    archived_at TEXT,
    trashed_at TEXT,
    deletion_operation_id TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS conversation_task_lifecycle_state
    ON conversation_task_lifecycle(state, updated_at, task_id);

CREATE TABLE IF NOT EXISTS conversation_task_lifecycle_commands (
    command_id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    input_json TEXT NOT NULL,
    result_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS conversation_task_lifecycle_commands_task
    ON conversation_task_lifecycle_commands(task_id, created_at, command_id);
