-- One bounded text proposal per saved future-rule choice. Responses use the call ledger.
CREATE TABLE IF NOT EXISTS conversation_future_schema_proposal_authorizations (
    call_id TEXT PRIMARY KEY REFERENCES conversation_authorization_revisions(id),
    feedback_call_id TEXT NOT NULL UNIQUE REFERENCES conversation_feedback_scope_answers(call_id),
    task_id TEXT NOT NULL REFERENCES conversation_tasks(id),
    conversation_id TEXT NOT NULL REFERENCES project_conversations(id),
    record_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS future_schema_proposals_by_task
    ON conversation_future_schema_proposal_authorizations(task_id,call_id);
