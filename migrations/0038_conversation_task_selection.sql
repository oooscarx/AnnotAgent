-- Navigation preference only: never an inference grant or task state transition.
CREATE TABLE IF NOT EXISTS conversation_task_selections (
    request_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    revision INTEGER NOT NULL CHECK(revision > 0),
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    UNIQUE(conversation_id, revision)
);
