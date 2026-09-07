CREATE TABLE IF NOT EXISTS project_conversations (
    id TEXT PRIMARY KEY,
    -- Current Project identity is resolved from project.yaml by Application;
    -- the legacy `projects` table is not populated by current GUI projects.
    project_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS conversation_messages (
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    message_id TEXT NOT NULL,
    input_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (conversation_id, sequence),
    UNIQUE (conversation_id, message_id)
);
