ALTER TABLE runs ADD COLUMN lifecycle_revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE runs ADD COLUMN archived_at TEXT;
ALTER TABLE runs ADD COLUMN deleted_at TEXT;
ALTER TABLE runs ADD COLUMN deletion_operation_id TEXT;

ALTER TABLE dataset_batches ADD COLUMN lifecycle_revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE dataset_batches ADD COLUMN archived_at TEXT;
ALTER TABLE dataset_batches ADD COLUMN deleted_at TEXT;
ALTER TABLE dataset_batches ADD COLUMN deletion_operation_id TEXT;

ALTER TABLE workflow_drafts ADD COLUMN archived_at TEXT;
ALTER TABLE workflow_drafts ADD COLUMN deleted_at TEXT;
ALTER TABLE workflow_drafts ADD COLUMN deletion_operation_id TEXT;

ALTER TABLE workflow_versions ADD COLUMN display_name TEXT;
ALTER TABLE workflow_versions ADD COLUMN lifecycle_revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE workflow_versions ADD COLUMN archived_at TEXT;
ALTER TABLE workflow_versions ADD COLUMN deleted_at TEXT;
ALTER TABLE workflow_versions ADD COLUMN deletion_operation_id TEXT;

CREATE TABLE workflow_pipelines (
    workflow_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    lifecycle_revision INTEGER NOT NULL DEFAULT 1,
    archived_at TEXT,
    deleted_at TEXT,
    deletion_operation_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO workflow_pipelines (
    workflow_id, project_id, display_name, lifecycle_revision, created_at, updated_at
)
SELECT
    id,
    project_id,
    COALESCE(json_extract(draft_json, '$.name'), id),
    1,
    created_at,
    updated_at
FROM workflow_drafts;

INSERT OR IGNORE INTO workflow_pipelines (
    workflow_id, project_id, display_name, lifecycle_revision, created_at, updated_at
)
SELECT
    workflow_id,
    project_id,
    COALESCE(json_extract(version_json, '$.draft.name'), workflow_id),
    1,
    published_at,
    published_at
FROM workflow_versions;

CREATE TABLE project_workflow_defaults (
    project_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO project_workflow_defaults (project_id, workflow_id, version, updated_at)
SELECT current.project_id, current.workflow_id, current.version, current.published_at
FROM workflow_versions current
WHERE NOT EXISTS (
    SELECT 1
    FROM workflow_versions later
    WHERE later.project_id = current.project_id
      AND (
          later.published_at > current.published_at
          OR (later.published_at = current.published_at AND later.workflow_id > current.workflow_id)
          OR (
              later.published_at = current.published_at
              AND later.workflow_id = current.workflow_id
              AND later.version > current.version
          )
      )
);

CREATE TABLE management_operations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    action TEXT NOT NULL,
    status TEXT NOT NULL,
    request_json TEXT NOT NULL,
    preview_json TEXT NOT NULL,
    result_json TEXT,
    confirmation_token TEXT NOT NULL,
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    UNIQUE(project_id, idempotency_key)
);

CREATE TABLE management_operation_items (
    operation_id TEXT NOT NULL,
    object_kind TEXT NOT NULL,
    object_id TEXT NOT NULL,
    object_version INTEGER NOT NULL DEFAULT 0,
    previous_state_json TEXT NOT NULL,
    PRIMARY KEY(operation_id, object_kind, object_id, object_version),
    FOREIGN KEY(operation_id) REFERENCES management_operations(id)
);

CREATE TABLE management_entity_leases (
    project_id TEXT NOT NULL,
    object_kind TEXT NOT NULL,
    object_id TEXT NOT NULL,
    object_version INTEGER NOT NULL DEFAULT 0,
    lease_kind TEXT NOT NULL,
    owner TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(project_id, object_kind, object_id, object_version, lease_kind, owner)
);

CREATE TABLE run_provenance_tombstones (
    run_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    workflow_id TEXT,
    workflow_version INTEGER,
    workflow_content_hash TEXT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    summary_json TEXT NOT NULL,
    purged_at TEXT NOT NULL
);

CREATE TABLE historical_usage_ledger (
    run_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    cost TEXT NOT NULL,
    currency TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);

CREATE INDEX idx_runs_visible_project_updated
ON runs(project_id, deleted_at, updated_at DESC, id DESC);

CREATE INDEX idx_batches_visible_project_updated
ON dataset_batches(project_id, deleted_at, updated_at DESC, id DESC);

CREATE INDEX idx_workflow_drafts_lifecycle
ON workflow_drafts(project_id, deleted_at, archived_at, updated_at DESC);

CREATE INDEX idx_workflow_versions_lifecycle
ON workflow_versions(project_id, deleted_at, archived_at, published_at DESC);

CREATE INDEX idx_workflow_pipelines_lifecycle
ON workflow_pipelines(project_id, deleted_at, archived_at, updated_at DESC);

CREATE INDEX idx_management_operations_project_updated
ON management_operations(project_id, updated_at DESC);

CREATE INDEX idx_management_leases_expiry
ON management_entity_leases(expires_at);
