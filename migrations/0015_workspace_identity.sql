CREATE TABLE IF NOT EXISTS images_workspace_identity (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    relative_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    thumbnail_path TEXT,
    imported_at TEXT NOT NULL
);

INSERT INTO images_workspace_identity
    (id, project_id, relative_path, sha256, metadata_json, thumbnail_path, imported_at)
SELECT id, project_id, relative_path, sha256, metadata_json, thumbnail_path, imported_at
FROM images;

DROP TABLE images;
ALTER TABLE images_workspace_identity RENAME TO images;

CREATE INDEX idx_images_project_hash
    ON images(project_id, sha256);

CREATE INDEX idx_images_project_path
    ON images(project_id, relative_path, imported_at DESC);

CREATE INDEX IF NOT EXISTS idx_run_images_image
    ON run_images(image_id, run_id);

CREATE INDEX IF NOT EXISTS idx_runs_project_status
    ON runs(project_id, status, updated_at DESC);
