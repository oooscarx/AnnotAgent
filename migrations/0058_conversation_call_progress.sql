-- Additive metadata: old settled calls retain unknown end time/stage.
CREATE TABLE IF NOT EXISTS conversation_call_progress (
    call_id TEXT PRIMARY KEY REFERENCES conversation_model_calls(id),
    stage TEXT NOT NULL,
    completed_at TEXT,
    failure_json TEXT
);
