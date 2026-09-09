-- Immutable whole-image human receipts, separate from object acceptance.
CREATE TABLE IF NOT EXISTS delivery_image_reviews (
    task_id TEXT NOT NULL,
    intent_revision INTEGER NOT NULL,
    image_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    command_id TEXT NOT NULL,
    input_json TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(task_id, intent_revision, image_id, revision),
    UNIQUE(task_id, command_id),
    FOREIGN KEY(task_id, intent_revision) REFERENCES task_delivery_intents(task_id, revision)
);
