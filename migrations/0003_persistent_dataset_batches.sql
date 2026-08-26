CREATE TABLE IF NOT EXISTS dataset_batches (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    project_path TEXT NOT NULL,
    provider TEXT NOT NULL,
    status TEXT NOT NULL,
    max_concurrency INTEGER NOT NULL,
    workflow_version TEXT NOT NULL,
    workflow_snapshot_json TEXT NOT NULL,
    project_snapshot_json TEXT NOT NULL,
    budget_limits_json TEXT NOT NULL,
    budget_ledger_json TEXT NOT NULL,
    lease_owner TEXT,
    lease_expires_at TEXT,
    event_sequence INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dataset_batches_project_updated
ON dataset_batches(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_dataset_batches_status
ON dataset_batches(status, updated_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_dataset_batches_one_active_project
ON dataset_batches(project_id)
WHERE status IN ('pending', 'running', 'paused', 'awaiting_review');

CREATE TABLE IF NOT EXISTS batch_images (
    batch_id TEXT NOT NULL,
    image_id TEXT NOT NULL,
    image_path TEXT NOT NULL,
    position INTEGER NOT NULL,
    status TEXT NOT NULL,
    child_run_id TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    reservation_json TEXT NOT NULL,
    actual_usage_json TEXT NOT NULL,
    checkpoint_json TEXT NOT NULL,
    error TEXT,
    lease_owner TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(batch_id, image_id),
    UNIQUE(batch_id, position),
    FOREIGN KEY(batch_id) REFERENCES dataset_batches(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_batch_images_claim
ON batch_images(batch_id, status, position);

CREATE TABLE IF NOT EXISTS batch_events (
    batch_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    kind TEXT NOT NULL,
    image_id TEXT,
    detail_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    PRIMARY KEY(batch_id, sequence),
    FOREIGN KEY(batch_id) REFERENCES dataset_batches(id) ON DELETE CASCADE
);
