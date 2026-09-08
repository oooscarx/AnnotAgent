CREATE TABLE IF NOT EXISTS conversation_image_class_reviews (
    id TEXT PRIMARY KEY,
    feedback_call_id TEXT NOT NULL UNIQUE REFERENCES conversation_feedback_scope_answers(call_id),
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    sample_test_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','answered','applied','cancelled')),
    answer_command_id TEXT UNIQUE,
    record_json TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS one_pending_image_class_review
ON conversation_image_class_reviews(task_id,sample_test_id,image_id) WHERE status='pending';
