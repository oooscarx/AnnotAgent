-- Intent only: an explicitly saved clarification answer may wake its existing
-- authorized worker. Startup recovery never dispatches this table by itself.
CREATE TABLE IF NOT EXISTS conversation_journey_answer_continuations (
    consent_id TEXT PRIMARY KEY REFERENCES conversation_journey_consents(id),
    schema_draft_id TEXT NOT NULL REFERENCES conversation_schema_drafts(id)
);
