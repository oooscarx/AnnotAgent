CREATE TABLE IF NOT EXISTS conversation_journey_dispatch (
    consent_id TEXT PRIMARY KEY REFERENCES conversation_journey_consents(id),
    attempt_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('running', 'settled', 'interrupted')),
    error TEXT,
    updated_at TEXT NOT NULL
);
