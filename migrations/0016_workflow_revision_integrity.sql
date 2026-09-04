ALTER TABLE workflow_drafts ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE workflow_drafts ADD COLUMN content_hash TEXT NOT NULL DEFAULT '';

ALTER TABLE workflow_sample_tests RENAME TO workflow_sample_tests_legacy;

CREATE TABLE workflow_sample_tests (
    id TEXT PRIMARY KEY,
    draft_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    draft_revision INTEGER NOT NULL,
    request_revision INTEGER NOT NULL,
    draft_content_hash TEXT NOT NULL,
    image_set_hash TEXT NOT NULL,
    model_snapshot_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    input_json TEXT NOT NULL,
    model_bindings_json TEXT NOT NULL,
    report_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT NOT NULL
);

INSERT INTO workflow_sample_tests (
    id,
    draft_id,
    project_id,
    draft_revision,
    request_revision,
    draft_content_hash,
    image_set_hash,
    model_snapshot_hash,
    status,
    input_json,
    model_bindings_json,
    report_json,
    started_at,
    completed_at
)
SELECT
    'legacy-' || draft_id,
    draft_id,
    project_id,
    1,
    1,
    '',
    '',
    '',
    'legacy_unverified',
    '[]',
    '{}',
    report_json,
    completed_at,
    completed_at
FROM workflow_sample_tests_legacy;

DROP TABLE workflow_sample_tests_legacy;

CREATE INDEX idx_workflow_sample_tests_draft_completed
ON workflow_sample_tests(draft_id, completed_at DESC, id DESC);

CREATE INDEX idx_workflow_sample_tests_draft_revision_hash
ON workflow_sample_tests(draft_id, draft_revision, draft_content_hash);

CREATE INDEX idx_workflow_sample_tests_exact_evidence
ON workflow_sample_tests(
    draft_id,
    request_revision,
    draft_content_hash,
    completed_at DESC,
    id DESC
);

CREATE INDEX idx_workflow_sample_tests_project_completed_v2
ON workflow_sample_tests(project_id, completed_at DESC, id DESC);
