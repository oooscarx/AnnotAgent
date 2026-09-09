CREATE TABLE IF NOT EXISTS history_scopes (
 singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
 id TEXT NOT NULL UNIQUE,
 scope_json TEXT NOT NULL,
 request_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS history_scope_exclusions (
 scope_id TEXT NOT NULL REFERENCES history_scopes(id),
 project_id TEXT NOT NULL,
 kind TEXT NOT NULL,
 object_id TEXT NOT NULL,
 version INTEGER NOT NULL DEFAULT 0,
 PRIMARY KEY(scope_id,project_id,kind,object_id,version)
);
