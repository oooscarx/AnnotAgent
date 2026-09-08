-- A clarification is the immutable completed Schema call; its answer links to real Schema data.
CREATE TABLE IF NOT EXISTS conversation_schema_clarification_answers (
    call_id TEXT PRIMARY KEY REFERENCES conversation_model_calls(id),
    request_id TEXT NOT NULL UNIQUE,
    schema_draft_id TEXT NOT NULL UNIQUE REFERENCES conversation_schema_drafts(id)
);
