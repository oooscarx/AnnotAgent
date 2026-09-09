CREATE TABLE IF NOT EXISTS workflow_clone_commands (
    command_id TEXT PRIMARY KEY NOT NULL,
    request_json TEXT NOT NULL,
    result_json TEXT NOT NULL
);
