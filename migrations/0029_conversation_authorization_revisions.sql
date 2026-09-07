CREATE TABLE IF NOT EXISTS conversation_authorization_revisions (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    previous_id TEXT UNIQUE REFERENCES conversation_authorization_revisions(id),
    scope_hash TEXT NOT NULL,
    maximum_calls INTEGER NOT NULL CHECK(maximum_calls BETWEEN 1 AND 128),
    expires_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS conversation_call_authorizations (
    call_id TEXT PRIMARY KEY REFERENCES conversation_model_calls(id),
    grant_id TEXT NOT NULL REFERENCES conversation_authorization_revisions(id)
);
-- Existing immutable single-phase grants retain the exact authorization they used.
INSERT OR IGNORE INTO conversation_authorization_revisions(id,task_id,scope_hash,maximum_calls,expires_at)
SELECT id,task_id,scope_hash,maximum_calls,expires_at FROM conversation_call_grants;
INSERT OR IGNORE INTO conversation_call_authorizations(call_id,grant_id)
SELECT c.id,g.id FROM conversation_model_calls c JOIN conversation_call_grants g ON g.task_id=c.task_id;
