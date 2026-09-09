//! Atomic message-to-task admission. No model, budget grant or execution is created.
use crate::{
    ConversationMessage, ConversationMessageInput, ConversationSelectionRef, SqliteStore,
    StorageError,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationSendInput {
    pub message: ConversationMessageInput,
    pub task_id: Option<Uuid>,
    pub schema_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationSendDisposition {
    NewTask,
    TaskMessage,
    CandidateFeedback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationSendReceipt {
    pub message: ConversationMessage,
    pub task_id: Uuid,
    pub disposition: ConversationSendDisposition,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

impl SqliteStore {
    pub fn conversation_send_receipt(
        &self,
        project: &str,
        conversation: Uuid,
        message: Uuid,
    ) -> Result<Option<(ConversationSendInput, ConversationSendReceipt)>, StorageError> {
        self.with_connection(|db| {
            crate::conversations::require_owner(db, project, conversation)?;
            let row: Option<(String, String)> = db.query_row("SELECT input_json,receipt_json FROM conversation_send_receipts WHERE conversation_id=?1 AND message_id=?2", params![conversation.to_string(),message.to_string()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
            row.map(|(input, receipt)| Ok((serde_json::from_str(&input)?,serde_json::from_str(&receipt)?))).transpose()
        })
    }

    /// Application validates current Project schema and terminal artifact provenance before
    /// first admission, under its schema lock. Transaction resolves identity, never UI state.
    pub fn send_conversation_message(
        &self,
        project: &str,
        conversation: Uuid,
        input: &ConversationSendInput,
    ) -> Result<ConversationSendReceipt, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            crate::conversations::require_owner(&tx, project, conversation)?;
            let saved: Option<(String,String)> = tx.query_row("SELECT input_json,receipt_json FROM conversation_send_receipts WHERE conversation_id=?1 AND message_id=?2",params![conversation.to_string(),input.message.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((original,receipt))=saved {
                if serde_json::from_str::<ConversationSendInput>(&original)? != *input { return Err(invalid("Send ID conflicts with its frozen task, message or schema")); }
                return Ok(serde_json::from_str(&receipt)?);
            }
            if input.schema_revision.len()!=64 || !input.schema_revision.bytes().all(|b|b.is_ascii_hexdigit()) { return Err(invalid("Send requires a schema revision digest")); }
            // A legacy journal ID cannot silently acquire a new dispatch meaning.
            let existing:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2)",params![conversation.to_string(),input.message.id.to_string()],|r|r.get(0))?;
            if existing { return Err(invalid("Message already belongs to the legacy journal; create a new send command")); }
            let (task_id,disposition)=match &input.message.reference {
                Some(ConversationSelectionRef::StopRequest{..})=>return Err(invalid("Use the explicit stop endpoint")),
                Some(ConversationSelectionRef::SampleCandidate{task_id,project_schema_revision,..})=>{
                    if input.task_id!=Some(*task_id) || input.schema_revision!=*project_schema_revision { return Err(invalid("Candidate reference conflicts with send task or schema")); }
                    (*task_id,ConversationSendDisposition::CandidateFeedback)
                },
                None=>match input.task_id {
                    Some(task)=>(task,ConversationSendDisposition::TaskMessage),
                    None=>(Uuid::new_v4(),ConversationSendDisposition::NewTask),
                },
            };
            if disposition!=ConversationSendDisposition::NewTask {
                let owned:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks WHERE id=?1 AND conversation_id=?2 AND schema_revision=?3)",params![task_id.to_string(),conversation.to_string(),input.schema_revision],|r|r.get(0))?;
                if !owned { return Err(invalid("Send task is foreign, missing or has a different frozen schema")); }
            }
            let message=crate::conversations::append_message_in_transaction(&tx,project,conversation,&input.message)?;
            if disposition==ConversationSendDisposition::NewTask {
                tx.execute("INSERT INTO conversation_tasks(id,conversation_id,source_message_id,schema_revision,created_at) VALUES(?1,?2,?3,?4,?5)",params![task_id.to_string(),conversation.to_string(),message.input.id.to_string(),input.schema_revision,chrono::Utc::now().to_rfc3339()])?;
            }
            let receipt=ConversationSendReceipt{message,task_id,disposition};
            tx.execute("INSERT INTO conversation_send_receipts(conversation_id,message_id,input_json,receipt_json) VALUES(?1,?2,?3,?4)",params![conversation.to_string(),input.message.id.to_string(),serde_json::to_string(input)?,serde_json::to_string(&receipt)?])?;
            tx.commit()?;
            Ok(receipt)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> ConversationSendInput {
        ConversationSendInput {
            message: ConversationMessageInput {
                id: Uuid::new_v4(),
                text: "TEST find cups".into(),
                image: None,
                reference: None,
            },
            task_id: None,
            schema_revision: "a".repeat(64),
        }
    }

    #[test]
    fn send_restores_exact_task_and_rejects_retargeting_without_a_second_message() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-send.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let owner = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        let command = input();
        let first = store
            .send_conversation_message(&owner, conversation, &command)
            .unwrap();
        assert_eq!(first.disposition, ConversationSendDisposition::NewTask);
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .send_conversation_message(&owner, conversation, &command)
                .unwrap(),
            first
        );
        let mut conflict = command.clone();
        conflict.task_id = Some(first.task_id);
        assert!(
            store
                .send_conversation_message(&owner, conversation, &conflict)
                .is_err()
        );
        let mut followup = input();
        followup.task_id = Some(first.task_id);
        let reply = store
            .send_conversation_message(&owner, conversation, &followup)
            .unwrap();
        assert_eq!(reply.disposition, ConversationSendDisposition::TaskMessage);
        assert_eq!(reply.task_id, first.task_id);
        assert_eq!(
            store
                .conversation_tasks(&owner, conversation)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .conversation_messages(&owner, conversation, 0, 100)
                .unwrap()
                .len(),
            2
        );
        assert!(
            store
                .conversation_send_receipt("foreign", conversation, command.message.id)
                .is_err()
        );
        let mut foreign = input();
        foreign.task_id = Some(Uuid::new_v4());
        assert!(
            store
                .send_conversation_message(&owner, conversation, &foreign)
                .is_err()
        );
        assert_eq!(
            store
                .conversation_messages(&owner, conversation, 0, 100)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn failed_receipt_insert_rolls_back_message_and_task_together() {
        let temp = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(temp.path().join("TEST-rollback.sqlite")).unwrap();
        let owner = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        store.with_connection(|db| { db.execute_batch("CREATE TRIGGER fail_send BEFORE INSERT ON conversation_send_receipts BEGIN SELECT RAISE(ABORT,'TEST receipt failure'); END;")?; Ok(()) }).unwrap();
        let command = input();
        assert!(
            store
                .send_conversation_message(&owner, conversation, &command)
                .is_err()
        );
        assert!(
            store
                .conversation_messages(&owner, conversation, 0, 100)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .conversation_tasks(&owner, conversation)
                .unwrap()
                .is_empty()
        );
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_send")?;
                Ok(())
            })
            .unwrap();
        let receipt = store
            .send_conversation_message(&owner, conversation, &command)
            .unwrap();
        assert_eq!(receipt.message.sequence, 1);
    }
}
