-- Original user consent, saved atomically with its grant; not an inference receipt.
CREATE TABLE IF NOT EXISTS conversation_schema_authorizations (
    call_id TEXT PRIMARY KEY REFERENCES conversation_authorization_revisions(id),
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    input_json TEXT NOT NULL
);
