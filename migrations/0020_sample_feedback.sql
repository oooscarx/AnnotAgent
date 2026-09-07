CREATE TABLE IF NOT EXISTS sample_feedback_revisions (
    revision_id TEXT PRIMARY KEY,
    sample_test_id TEXT NOT NULL REFERENCES workflow_sample_tests(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    feedback_json TEXT NOT NULL,
    UNIQUE(sample_test_id, image_id, sequence)
);
