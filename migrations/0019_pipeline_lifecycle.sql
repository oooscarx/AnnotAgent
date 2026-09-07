ALTER TABLE workflow_drafts ADD COLUMN lifecycle_revision INTEGER NOT NULL DEFAULT 1;

CREATE INDEX idx_workflow_drafts_management_revision
ON workflow_drafts(project_id, lifecycle_revision, deleted_at, archived_at);
