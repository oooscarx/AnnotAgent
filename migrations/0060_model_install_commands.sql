CREATE TABLE IF NOT EXISTS model_install_commands (
 command_id TEXT PRIMARY KEY,
 operation_id TEXT NOT NULL UNIQUE,
 scope_json TEXT NOT NULL,
 operation_json TEXT NOT NULL
);
