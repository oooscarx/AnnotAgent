-- Only new explicitly opted-in Sample Operations enqueue assistance. No history backfill.
CREATE TABLE IF NOT EXISTS conversation_sample_assistance (
    sample_id TEXT PRIMARY KEY REFERENCES sample_operations(id),
    status TEXT NOT NULL DEFAULT 'waiting' CHECK(status IN ('waiting','completed','failed')),
    error TEXT
);
