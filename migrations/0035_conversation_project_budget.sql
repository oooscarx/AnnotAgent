-- Explicit cumulative call ceilings; never reset by creating a conversation or task.
CREATE TABLE IF NOT EXISTS conversation_project_budget_revisions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    maximum_calls INTEGER NOT NULL CHECK(maximum_calls >= 0),
    created_at TEXT NOT NULL,
    UNIQUE(project_id, revision)
);
