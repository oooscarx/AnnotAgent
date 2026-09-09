-- One explicitly linked answer delivery. Claiming the existing journey dispatch
-- consumes this intent atomically; uncertain in-flight calls are never replayed.
CREATE TABLE IF NOT EXISTS conversation_answer_delivery (
    request_id TEXT PRIMARY KEY REFERENCES conversation_human_requests(id),
    consent_id TEXT NOT NULL UNIQUE REFERENCES conversation_journey_consents(id),
    feedback_revision_id TEXT NOT NULL REFERENCES sample_feedback_revisions(revision_id),
    status TEXT NOT NULL CHECK(status IN ('pending','dispatched','failed')),
    error TEXT,
    created_at TEXT NOT NULL
);
