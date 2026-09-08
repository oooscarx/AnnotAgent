CREATE TABLE IF NOT EXISTS conversation_export_events (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
 project_id TEXT NOT NULL,
 conversation_id TEXT NOT NULL,
 task_id TEXT NOT NULL,
 export_id TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('requested','completed','failed')),
 UNIQUE(export_id,kind)
);
CREATE INDEX IF NOT EXISTS conversation_export_event_owner ON conversation_export_events(project_id,conversation_id,task_id,sequence);
