CREATE TABLE IF NOT EXISTS conversation_exports (
 id TEXT PRIMARY KEY,
 project_id TEXT NOT NULL,
 conversation_id TEXT NOT NULL,
 task_id TEXT NOT NULL,
 format TEXT NOT NULL,
 result_json TEXT,
 error TEXT,
 created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS conversation_exports_owner ON conversation_exports(project_id,conversation_id,task_id,created_at);
