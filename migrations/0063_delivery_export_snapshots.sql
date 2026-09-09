-- Extends existing conversation Export jobs; does not create another export history.
CREATE TABLE IF NOT EXISTS delivery_export_snapshots (
    export_id TEXT PRIMARY KEY REFERENCES conversation_exports(id),
    input_json TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    snapshot_sha256 TEXT NOT NULL,
    phase TEXT NOT NULL CHECK(phase IN ('preparing','exporting','validating','ready','failed','cancelled')),
    updated_at TEXT NOT NULL
);
