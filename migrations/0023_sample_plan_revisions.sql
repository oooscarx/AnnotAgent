-- Authoring lineage only: these copies still execute through the existing Draft/Run APIs.
CREATE TABLE IF NOT EXISTS sample_plan_revisions (
    draft_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    sample_test_id TEXT NOT NULL,
    feedback_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
