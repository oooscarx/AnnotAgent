//! Idempotent orchestration receipt; does not create another workflow executor.
use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationBuilderOperation {
    pub id: Uuid,
    pub task_id: Uuid,
    pub request_hash: String,
    pub status: String,
    pub evidence: Option<serde_json::Value>,
}
fn owned(db: &rusqlite::Connection, project: &str, task: Uuid) -> Result<(), StorageError> {
    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.project_id=?2)",params![task.to_string(),project],|row|row.get(0))?;
    if !exists {
        return Err(StorageError::InvalidConversation(
            "Builder task belongs to another Project".into(),
        ));
    }
    Ok(())
}
fn read(
    db: &rusqlite::Connection,
    id: Uuid,
) -> Result<Option<ConversationBuilderOperation>, StorageError> {
    let row: Option<(String,String,String,Option<String>)> = db.query_row("SELECT task_id,request_hash,status,evidence_json FROM conversation_builder_operations WHERE id=?1",[id.to_string()],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    row.map(|(task, request_hash, status, evidence)| {
        Ok(ConversationBuilderOperation {
            id,
            task_id: Uuid::parse_str(&task)
                .map_err(|_| StorageError::InvalidConversation("Invalid task ID".into()))?,
            request_hash,
            status,
            evidence: evidence
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
        })
    })
    .transpose()
}
impl SqliteStore {
    /// Only `None` admits new execution. A saved receipt must never be dispatched again.
    pub fn reserve_conversation_builder(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        hash: &str,
    ) -> Result<Option<ConversationBuilderOperation>, StorageError> {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(StorageError::InvalidConversation(
                "Invalid Builder request hash".into(),
            ));
        }
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?; owned(&tx,project,task)?;
            if let Some(saved) = read(&tx,id)? {
                if saved.task_id != task || saved.request_hash != hash { return Err(StorageError::InvalidConversation("Builder request key conflicts".into())); }
                return Ok(Some(saved));
            }
            tx.execute("INSERT INTO conversation_builder_operations(id,task_id,request_hash,status) VALUES(?1,?2,?3,'reserved')",params![id.to_string(),task.to_string(),hash])?;
            tx.commit()?; Ok(None)
        })
    }
    pub fn conversation_builder_operation(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
    ) -> Result<Option<ConversationBuilderOperation>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, task)?;
            let saved = read(db, id)?;
            if saved.as_ref().is_some_and(|saved| saved.task_id != task) {
                return Err(StorageError::InvalidConversation(
                    "Builder belongs to another task".into(),
                ));
            }
            Ok(saved)
        })
    }
    pub fn settle_conversation_builder(
        &self,
        project: &str,
        task: Uuid,
        id: Uuid,
        completed: bool,
        evidence: &serde_json::Value,
    ) -> Result<(), StorageError> {
        self.with_connection(|db| { owned(db,project,task)?;
            db.execute("UPDATE conversation_builder_operations SET status=?3,evidence_json=?4 WHERE id=?1 AND task_id=?2 AND status='reserved'",params![id.to_string(),task.to_string(),if completed {"completed"} else {"interrupted"},serde_json::to_string(evidence)?])?; Ok(())
        })
    }
    pub fn recover_conversation_builders(&self) -> Result<(), StorageError> {
        self.with_connection(|db| { db.execute("UPDATE conversation_builder_operations SET status='interrupted',evidence_json=?1 WHERE status='reserved'",[serde_json::json!({"error":"Server restarted; saved Draft and model-call receipts remain. No automatic re-execution."}).to_string()])?; Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_and_restart_keep_one_operation_without_reexecuting_or_rewriting_result() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-builder.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let message = crate::ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST target".into(),
            image: None,
        };
        store
            .append_conversation_message(&project, conversation, &message)
            .unwrap();
        let task = Uuid::new_v4();
        store
            .begin_conversation_task(
                &project,
                conversation,
                &crate::BeginConversationTask {
                    id: task,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        let id = Uuid::new_v4();
        let hash = "b".repeat(64);
        assert!(
            store
                .reserve_conversation_builder(&project, task, id, &hash)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store
                .reserve_conversation_builder(&project, task, id, &hash)
                .unwrap()
                .unwrap()
                .status,
            "reserved"
        );
        assert!(
            store
                .reserve_conversation_builder(&project, task, id, &"c".repeat(64))
                .is_err()
        );
        assert!(
            store
                .conversation_builder_operation("foreign", task, id)
                .is_err()
        );
        drop(store);
        let store = SqliteStore::open(path).unwrap();
        store.recover_conversation_builders().unwrap();
        let interrupted = store
            .reserve_conversation_builder(&project, task, id, &hash)
            .unwrap()
            .unwrap();
        assert_eq!(interrupted.status, "interrupted");
        store
            .settle_conversation_builder(
                &project,
                task,
                id,
                true,
                &serde_json::json!({"fake":"late result"}),
            )
            .unwrap();
        assert_eq!(
            store
                .conversation_builder_operation(&project, task, id)
                .unwrap()
                .unwrap(),
            interrupted
        );
    }
}
