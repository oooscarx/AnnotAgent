-- Request answers reference existing Sandbox feedback; never copy formal annotations.
CREATE TABLE IF NOT EXISTS conversation_human_requests (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    request_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','answered','applied','cancelled','stale')),
    answer_json TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS human_requests_by_task ON conversation_human_requests(task_id,created_at,id);
CREATE TABLE IF NOT EXISTS conversation_resume_outbox (
    request_id TEXT PRIMARY KEY REFERENCES conversation_human_requests(id),
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    checkpoint_ref TEXT NOT NULL,
    feedback_revision_id TEXT NOT NULL REFERENCES sample_feedback_revisions(revision_id),
    applied_at TEXT
);
