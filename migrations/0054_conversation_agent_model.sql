CREATE TABLE IF NOT EXISTS conversation_agent_models (
    conversation_id TEXT PRIMARY KEY REFERENCES project_conversations(id),
    revision INTEGER NOT NULL CHECK(revision >= 1),
    model_profile_id TEXT
);
CREATE TABLE IF NOT EXISTS conversation_agent_model_commands (
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    request_id TEXT NOT NULL,
    input_json TEXT NOT NULL,
    PRIMARY KEY(conversation_id, request_id)
);
