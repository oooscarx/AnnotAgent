CREATE TABLE IF NOT EXISTS conversation_schema_drafts (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    source_call_id TEXT NOT NULL UNIQUE REFERENCES conversation_model_calls(id),
    base_schema_revision TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS conversation_schema_revisions (
    draft_id TEXT NOT NULL REFERENCES conversation_schema_drafts(id),
    revision INTEGER NOT NULL CHECK(revision > 0),
    request_id TEXT NOT NULL UNIQUE,
    definition_json TEXT NOT NULL,
    PRIMARY KEY(draft_id, revision)
);
