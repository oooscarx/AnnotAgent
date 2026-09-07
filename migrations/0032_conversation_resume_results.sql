CREATE TABLE IF NOT EXISTS conversation_resume_results (
    request_id TEXT PRIMARY KEY REFERENCES conversation_human_requests(id),
    draft_id TEXT,
    error TEXT,
    CHECK ((draft_id IS NOT NULL AND error IS NULL) OR (draft_id IS NULL AND error IS NOT NULL))
);
