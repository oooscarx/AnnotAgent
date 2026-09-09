CREATE TABLE IF NOT EXISTS replay_commands (
 id TEXT PRIMARY KEY NOT NULL,
 project_id TEXT NOT NULL,
 run_id TEXT NOT NULL,
 node_id TEXT NOT NULL,
 receipt_json TEXT NOT NULL
);
