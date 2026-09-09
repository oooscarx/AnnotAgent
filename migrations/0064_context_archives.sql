-- Imported evidence is deliberately separate from every live execution/authority table.
CREATE TABLE IF NOT EXISTS context_archives (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    project_owner_id TEXT NOT NULL,
    command_id TEXT NOT NULL UNIQUE,
    preview_hash TEXT NOT NULL,
    archive_json TEXT NOT NULL,
    receipt_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS context_archives_owner_sequence
ON context_archives(project_owner_id, sequence);
