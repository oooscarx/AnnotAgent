-- Local packaging permission only. Export history remains conversation_exports.
CREATE TABLE IF NOT EXISTS delivery_package_consents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    input_json TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('armed','consumed','cancelled')),
    created_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS delivery_package_one_armed
ON delivery_package_consents(task_id) WHERE state='armed';
