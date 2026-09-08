CREATE TABLE IF NOT EXISTS conversation_journey_schema_resolution (
    consent_id TEXT PRIMARY KEY REFERENCES conversation_journey_consents(id),
    resolved_json TEXT NOT NULL
);
