//! Ordered follow-up admission, not another executor. A waiting entry grants no
//! model call and does not mean an interpreter has applied its instruction.
use crate::{ConversationSendInput, ConversationSendReceipt, SqliteStore, StorageError};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationQueuedMessageStatus {
    WaitingForDispatch,
    Authorized,
    Running,
    Completed,
    Failed,
    InDoubt,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConversationQueuedMessage {
    pub input: ConversationSendInput,
    pub receipt: ConversationSendReceipt,
    pub status: ConversationQueuedMessageStatus,
    pub cancelled_at: Option<String>,
    pub planning_call_id: Option<Uuid>,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

pub(crate) fn require_task(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
) -> Result<(), StorageError> {
    crate::conversations::require_owner(db, project, conversation)?;
    let owned: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_tasks WHERE id=?1 AND conversation_id=?2)",
        params![task.to_string(), conversation.to_string()],
        |row| row.get(0),
    )?;
    if !owned {
        return Err(invalid(
            "Queued message task is missing or belongs to another conversation",
        ));
    }
    Ok(())
}

pub(crate) fn read(
    db: &Connection,
    conversation: Uuid,
    task: Uuid,
    message: Uuid,
) -> Result<ConversationQueuedMessage, StorageError> {
    let row: Option<(String, String, Option<String>)> = db.query_row(
        "SELECT s.input_json,s.receipt_json,q.cancelled_at FROM conversation_message_queue q JOIN conversation_send_receipts s USING(conversation_id,message_id) WHERE q.conversation_id=?1 AND q.task_id=?2 AND q.message_id=?3",
        params![conversation.to_string(),task.to_string(),message.to_string()],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).optional()?;
    let (input, receipt, cancelled_at) =
        row.ok_or_else(|| invalid("Queued message was not found in this task"))?;
    let planning: Option<(String, Option<String>)> = db.query_row(
        "SELECT p.call_id,c.status FROM conversation_queued_planning p LEFT JOIN conversation_model_calls c ON c.id=p.call_id WHERE p.conversation_id=?1 AND p.task_id=?2 AND p.message_id=?3",
        params![conversation.to_string(), task.to_string(), message.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()?;
    let planning_call_id = planning
        .as_ref()
        .map(|(id, _)| {
            Uuid::parse_str(id).map_err(|_| invalid("Invalid queued planning call identity"))
        })
        .transpose()?;
    Ok(ConversationQueuedMessage {
        input: serde_json::from_str(&input)?,
        receipt: serde_json::from_str(&receipt)?,
        status: if cancelled_at.is_some() {
            ConversationQueuedMessageStatus::Cancelled
        } else {
            match planning.as_ref().map(|(_, status)| status.as_deref()) {
                None => ConversationQueuedMessageStatus::WaitingForDispatch,
                Some(None) => ConversationQueuedMessageStatus::Authorized,
                Some(Some("reserved")) => ConversationQueuedMessageStatus::Running,
                Some(Some("completed")) => ConversationQueuedMessageStatus::Completed,
                Some(Some("failed")) => ConversationQueuedMessageStatus::Failed,
                Some(Some("in_doubt")) => ConversationQueuedMessageStatus::InDoubt,
                Some(Some(_)) => return Err(invalid("Unknown queued planning call status")),
            }
        },
        cancelled_at,
        planning_call_id,
    })
}

impl SqliteStore {
    /// Read-only, bounded page; sequence comes from the existing message journal.
    pub fn conversation_message_queue(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        after: i64,
    ) -> Result<Vec<ConversationQueuedMessage>, StorageError> {
        if after < 0 {
            return Err(invalid("Queue sequence must not be negative"));
        }
        self.with_connection(|db| {
            require_task(db,project,conversation,task)?;
            let mut query=db.prepare("SELECT message_id FROM conversation_message_queue WHERE conversation_id=?1 AND task_id=?2 AND sequence>?3 ORDER BY sequence LIMIT 100")?;
            let ids=query.query_map(params![conversation.to_string(),task.to_string(),after],|row| row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
            ids.into_iter().map(|id|read(db,conversation,task,Uuid::parse_str(&id).map_err(|_|invalid("Invalid queued message identity"))?)).collect()
        })
    }

    /// Idempotent terminal cancellation of this queue entry only. The message,
    /// current inference, independent Batch, artifacts and budget remain intact.
    pub fn cancel_queued_conversation_message(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        message: Uuid,
    ) -> Result<ConversationQueuedMessage, StorageError> {
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            require_task(&tx,project,conversation,task)?;
            read(&tx,conversation,task,message)?;
            crate::conversation_queued_planning::require_queue_cancel_safe(&tx,task,message)?;
            tx.execute("UPDATE conversation_message_queue SET cancelled_at=?4 WHERE conversation_id=?1 AND task_id=?2 AND message_id=?3 AND cancelled_at IS NULL", params![conversation.to_string(),task.to_string(),message.to_string(),chrono::Utc::now().to_rfc3339()])?;
            let result=read(&tx,conversation,task,message)?;
            tx.commit()?;
            Ok(result)
        })
    }
}
