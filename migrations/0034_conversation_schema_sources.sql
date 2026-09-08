-- Rebuild both tables in one transaction with foreign keys enabled. Preserve all
-- IDs, revisions and idempotency keys; manual input never needs a model-call row.
CREATE TABLE conversation_schema_drafts_v34 (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    source_call_id TEXT UNIQUE REFERENCES conversation_model_calls(id),
    source_request_id TEXT UNIQUE,
    base_schema_revision TEXT NOT NULL,
    CHECK ((source_call_id IS NOT NULL AND source_request_id IS NULL)
        OR (source_call_id IS NULL AND source_request_id IS NOT NULL))
);
INSERT INTO conversation_schema_drafts_v34(id, task_id, source_call_id, base_schema_revision)
SELECT id, task_id, source_call_id, base_schema_revision FROM conversation_schema_drafts;
CREATE TABLE conversation_schema_revisions_v34 (
    draft_id TEXT NOT NULL REFERENCES conversation_schema_drafts_v34(id),
    revision INTEGER NOT NULL CHECK(revision > 0),
    request_id TEXT NOT NULL UNIQUE,
    definition_json TEXT NOT NULL,
    PRIMARY KEY(draft_id, revision)
);
INSERT INTO conversation_schema_revisions_v34 SELECT * FROM conversation_schema_revisions;
DROP TABLE conversation_schema_revisions;
DROP TABLE conversation_schema_drafts;
ALTER TABLE conversation_schema_drafts_v34 RENAME TO conversation_schema_drafts;
ALTER TABLE conversation_schema_revisions_v34 RENAME TO conversation_schema_revisions;
