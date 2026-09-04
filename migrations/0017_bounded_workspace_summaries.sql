CREATE INDEX IF NOT EXISTS idx_runs_updated_id
ON runs(updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_runs_project_updated_id
ON runs(project_id, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_run_images_run_image
ON run_images(run_id, image_id);

CREATE INDEX IF NOT EXISTS idx_batch_images_child_run
ON batch_images(child_run_id, batch_id);

CREATE INDEX IF NOT EXISTS idx_annotations_run_review_created
ON annotations(run_id, review_status, created_at, id);

CREATE INDEX IF NOT EXISTS idx_annotations_review_created
ON annotations(review_status, created_at, id);

CREATE INDEX IF NOT EXISTS idx_annotation_revisions_annotation_created
ON annotation_revisions(annotation_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_task_runs_run_updated
ON task_runs(run_id, updated_at DESC, task_id DESC);

CREATE INDEX IF NOT EXISTS idx_validation_issues_run_code
ON validation_issues(run_id, code);

CREATE INDEX IF NOT EXISTS idx_usage_records_run
ON usage_records(run_id);

CREATE INDEX IF NOT EXISTS idx_model_calls_run
ON model_calls(run_id);

CREATE INDEX IF NOT EXISTS idx_review_queue_run_status_created
ON review_queue(run_id, status, created_at, annotation_id);

CREATE INDEX IF NOT EXISTS idx_workflow_sample_tests_draft_hash_completed
ON workflow_sample_tests(draft_id, draft_content_hash, completed_at DESC, id DESC);
