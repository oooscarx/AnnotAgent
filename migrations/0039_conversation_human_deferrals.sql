-- Pending review remains pending for budget admission. Deferring is not acceptance.
CREATE TABLE IF NOT EXISTS conversation_human_deferrals (
    command_id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL REFERENCES conversation_human_requests(id),
    revision INTEGER NOT NULL CHECK(revision > 0),
    deferred INTEGER NOT NULL CHECK(deferred IN (0,1)),
    UNIQUE(request_id,revision)
);
