ALTER TABLE conversation_journey_dispatch RENAME TO conversation_journey_dispatch_legacy;

CREATE TABLE conversation_journey_dispatch (
    consent_id TEXT PRIMARY KEY REFERENCES conversation_journey_consents(id),
    attempt_id TEXT NOT NULL,
    project_route_id TEXT,
    status TEXT NOT NULL CHECK(status IN ('queued', 'running', 'settled', 'interrupted')),
    error TEXT,
    updated_at TEXT NOT NULL
);

INSERT INTO conversation_journey_dispatch(
    consent_id, attempt_id, project_route_id, status, error, updated_at
)
SELECT consent_id, attempt_id, NULL, status, error, updated_at
FROM conversation_journey_dispatch_legacy;

DROP TABLE conversation_journey_dispatch_legacy;

CREATE INDEX IF NOT EXISTS conversation_journey_dispatch_status
    ON conversation_journey_dispatch(status, updated_at);
