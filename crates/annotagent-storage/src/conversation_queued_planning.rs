//! Exact one-call planning admission from the follow-up inbox. Reuses the task's
//! cumulative authorization ledger; does not create a task or execute a Provider.
use crate::{ConversationCallGrant, ConversationQueuedMessage, SqliteStore, StorageError};
use annotagent_core::ModelProfileId;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuedPlanningAuthorization {
    pub conversation_id: Uuid,
    pub message_id: Uuid,
    pub previous_grant_id: Option<Uuid>,
    pub grant: ConversationCallGrant,
    pub model_id: ModelProfileId,
    /// Server-computed exact model request digest, not merely the consent hash.
    pub request_hash: String,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}
fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|v| v.is_ascii_hexdigit())
}

pub(crate) fn read_call(
    db: &Connection,
    task: Uuid,
    call: Uuid,
) -> Result<Option<QueuedPlanningAuthorization>, StorageError> {
    let row: Option<(String, String)> = db
        .query_row(
            "SELECT task_id,record_json FROM conversation_queued_planning WHERE call_id=?1",
            [call.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    row.map(|(owner, json)| {
        if owner != task.to_string() {
            return Err(invalid("Queued planning call belongs to another task"));
        }
        Ok(serde_json::from_str(&json)?)
    })
    .transpose()
}

fn validate_source(
    db: &Connection,
    project: &str,
    input: &QueuedPlanningAuthorization,
) -> Result<ConversationQueuedMessage, StorageError> {
    crate::conversation_message_queue::require_task(
        db,
        project,
        input.conversation_id,
        input.grant.task_id,
    )?;
    let queued = crate::conversation_message_queue::read(
        db,
        input.conversation_id,
        input.grant.task_id,
        input.message_id,
    )?;
    if queued.cancelled_at.is_some() {
        return Err(invalid(
            "Queued message was cancelled; no planning call is permitted",
        ));
    }
    if queued.receipt.resolved_agent_model_id.or_else(|| queued.receipt.agent_model.as_ref().and_then(|model| model.model_profile_id))
        .is_some_and(|model| model != input.model_id)
    {
        return Err(invalid("Queued planning must use the model frozen at Send"));
    }
    let earlier:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_message_queue q LEFT JOIN conversation_queued_planning p USING(conversation_id,message_id) LEFT JOIN conversation_model_calls c ON c.id=p.call_id WHERE q.conversation_id=?1 AND q.task_id=?2 AND q.sequence<?3 AND q.cancelled_at IS NULL AND (c.status IS NULL OR c.status NOT IN ('completed','failed')))",params![input.conversation_id.to_string(),input.grant.task_id.to_string(),queued.receipt.message.sequence],|row|row.get(0))?;
    if earlier {
        return Err(invalid(
            "An earlier queued instruction must settle or be cancelled first",
        ));
    }
    let active:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_model_calls WHERE task_id=?1 AND status='reserved' UNION ALL SELECT 1 FROM conversation_builder_operations WHERE task_id=?1 AND status='reserved' UNION ALL SELECT 1 FROM conversation_journey_dispatch d JOIN conversation_journey_consents c ON c.id=d.consent_id WHERE c.task_id=?1 AND d.status='running' UNION ALL SELECT 1 FROM sample_operations WHERE json_extract(request_json,'$.conversation.task_id')=?1 AND status IN ('queued','running','cancelling'))",[input.grant.task_id.to_string()],|row|row.get(0))?;
    if active {
        return Err(invalid(
            "Wait for this task's active work to settle before dispatching a supplement",
        ));
    }
    Ok(queued)
}

/// Called inside the existing model-call reservation transaction. A queue grant
/// permits exactly its one bound call even if cumulative remaining budget is larger.
pub(crate) fn require_call(
    db: &Connection,
    project: &str,
    task: Uuid,
    call: Uuid,
    request_hash: &str,
) -> Result<(), StorageError> {
    let current: Option<String> = db
        .query_row(
            "SELECT id FROM conversation_call_grants WHERE task_id=?1",
            [task.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(current) = current {
        let bound: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_queued_planning WHERE call_id=?1)",
            [&current],
            |row| row.get(0),
        )?;
        if bound && current != call.to_string() {
            return Err(invalid(
                "Queued planning authorization permits only its exact bound call",
            ));
        }
    }
    if let Some(record) = read_call(db, task, call)? {
        if record.request_hash != request_hash {
            return Err(invalid("Queued planning request changed after approval"));
        }
        validate_source(db, project, &record)?;
    }
    Ok(())
}

pub(crate) fn require_queue_cancel_safe(
    db: &Connection,
    task: Uuid,
    message: Uuid,
) -> Result<(), StorageError> {
    let status:Option<String>=db.query_row("SELECT c.status FROM conversation_queued_planning p JOIN conversation_model_calls c ON c.id=p.call_id WHERE p.task_id=?1 AND p.message_id=?2",params![task.to_string(),message.to_string()],|row|row.get(0)).optional()?;
    if status.as_deref() == Some("reserved") {
        return Err(invalid(
            "This queued instruction is already running; use the explicit task stop control",
        ));
    }
    Ok(())
}

impl SqliteStore {
    /// Application must verify explicit model/data/cost consent before calling.
    /// Advancing the existing ledger and binding this message are atomic.
    pub fn authorize_queued_planning(
        &self,
        project: &str,
        input: &QueuedPlanningAuthorization,
    ) -> Result<(), StorageError> {
        if input.message_id.is_nil() || input.grant.id.is_nil() || !digest(&input.request_hash) {
            return Err(invalid("Queued planning requires exact bounded identities"));
        }
        let write = |tx: &rusqlite::Transaction<'_>, created: bool| -> Result<(), StorageError> {
            if let Some(saved) = read_call(tx, input.grant.task_id, input.grant.id)? {
                if saved != *input {
                    return Err(invalid(
                        "Queued planning retry changed its frozen authorization",
                    ));
                }
                return Ok(());
            }
            if !created {
                return Err(invalid(
                    "Existing authorization cannot acquire a new queued meaning",
                ));
            }
            validate_source(tx, project, input)?;
            tx.execute("INSERT INTO conversation_queued_planning(call_id,conversation_id,task_id,message_id,record_json) VALUES(?1,?2,?3,?4,?5)",params![input.grant.id.to_string(),input.conversation_id.to_string(),input.grant.task_id.to_string(),input.message_id.to_string(),serde_json::to_string(input)?])?;
            Ok(())
        };
        if let Some(previous) = input.previous_grant_id {
            self.advance_conversation_authorization_with(project, previous, &input.grant, write)
        } else {
            self.authorize_initial_request_with(project, &input.grant, None, write)
        }
    }

    pub fn queued_planning_authorization(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<QueuedPlanningAuthorization>, StorageError> {
        self.with_connection(|db| {
            crate::conversation_message_queue::require_task(db, project, conversation, task)?;
            let record = read_call(db, task, call)?;
            if record
                .as_ref()
                .is_some_and(|record| record.conversation_id != conversation)
            {
                return Err(invalid("Queued planning belongs to another conversation"));
            }
            Ok(record)
        })
    }
}
