//! Persistent three-slot intent with owner checks, CAS and exact command replay.
use crate::{SqliteStore, StorageError};
use annotagent_core::dataset_delivery::TaskDeliveryIntent;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDeliveryRevision {
    pub revision: u32,
    pub content_sha256: String,
    pub intent: TaskDeliveryIntent,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

fn owner(
    db: &rusqlite::Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
) -> Result<(), StorageError> {
    let owned: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.id=?2 AND c.project_id=?3)",
        params![task.to_string(), conversation.to_string(), project], |row| row.get(0),
    )?;
    if !owned {
        return Err(invalid(
            "delivery Task does not belong to this Project and conversation",
        ));
    }
    Ok(())
}

impl SqliteStore {
    /// Read-only restore; unresolved slots are restored from structured saved data.
    pub fn task_delivery_intent(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Option<TaskDeliveryRevision>, StorageError> {
        self.with_connection(|db| {
            owner(db, project, conversation, task)?;
            let row: Option<(u32,String,String)> = db.query_row(
                "SELECT revision,content_sha256,intent_json FROM task_delivery_intents WHERE task_id=?1 ORDER BY revision DESC LIMIT 1",
                [task.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).optional()?;
            row.map(|(revision,content_sha256,json)| Ok(TaskDeliveryRevision { revision, content_sha256, intent: serde_json::from_str(&json)? })).transpose()
        })
    }

    /// Application resolves images/Schema and permissions before using this intent.
    /// This write alone never modifies a Schema, grants a call or starts packaging.
    pub fn save_task_delivery_intent(
        &self,
        command: Uuid,
        expected_revision: u32,
        intent: &TaskDeliveryIntent,
    ) -> Result<TaskDeliveryRevision, StorageError> {
        intent.validate().map_err(invalid)?;
        if command.is_nil() {
            return Err(invalid("delivery command ID is required"));
        }
        let json = serde_json::to_string(intent)?;
        let digest = format!("{:x}", Sha256::digest(json.as_bytes()));
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owner(&tx, &intent.project_id, intent.conversation_id, intent.task_id)?;
            let existing: Option<(u32,u32,String)> = tx.query_row(
                "SELECT revision,expected_revision,intent_json FROM task_delivery_intents WHERE task_id=?1 AND command_id=?2",
                params![intent.task_id.to_string(), command.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
            ).optional()?;
            if let Some((revision, expected, saved)) = existing {
                if expected != expected_revision || saved != json {
                    return Err(invalid("delivery retry cannot change its revision or content"));
                }
                return Ok(TaskDeliveryRevision { revision, content_sha256: digest, intent: intent.clone() });
            }
            let current: u32 = tx.query_row("SELECT COALESCE(MAX(revision),0) FROM task_delivery_intents WHERE task_id=?1", [intent.task_id.to_string()], |r| r.get(0))?;
            if current != expected_revision { return Err(invalid("delivery intent changed; reload before saving")); }
            let revision = current.checked_add(1).ok_or_else(|| invalid("delivery revision overflow"))?;
            tx.execute("INSERT INTO task_delivery_intents(task_id,revision,command_id,expected_revision,content_sha256,intent_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![intent.task_id.to_string(),revision,command.to_string(),expected_revision,digest,json,chrono::Utc::now().to_rfc3339()])?;
            tx.commit()?;
            Ok(TaskDeliveryRevision { revision, content_sha256: digest, intent: intent.clone() })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeginConversationTask, ConversationMessageInput};
    use annotagent_core::dataset_delivery::{
        DeliveryReviewPolicy, DeliverySlot, DeliverySplitPolicy,
    };

    #[test]
    fn delivery_intent_restores_slots_and_enforces_owner_cas_and_exact_retry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-delivery.db");
        let store = SqliteStore::open(&path).unwrap();
        let project_identity = Uuid::new_v4().to_string();
        let project = project_identity.as_str();
        let conversation = store.create_conversation(project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST delivery".into(),
            reference: None,
            image: None,
        };
        store
            .append_conversation_message(project, conversation, &message)
            .unwrap();
        let task = Uuid::new_v4();
        store
            .begin_conversation_task(
                project,
                conversation,
                &BeginConversationTask {
                    id: task,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        let mut intent = TaskDeliveryIntent {
            version: 1,
            project_id: project.into(),
            conversation_id: conversation,
            task_id: task,
            dataset_scope: None,
            label_spec: None,
            training_target: None,
            split_policy: DeliverySplitPolicy::default(),
            review_policy: DeliveryReviewPolicy::HumanWholeImage,
        };
        let command = Uuid::new_v4();
        let saved = store
            .save_task_delivery_intent(command, 0, &intent)
            .unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(saved.content_sha256.len(), 64);
        assert_eq!(
            saved,
            store
                .save_task_delivery_intent(command, 0, &intent)
                .unwrap()
        );
        intent.split_policy.seed = 42;
        assert!(
            store
                .save_task_delivery_intent(command, 0, &intent)
                .is_err()
        );
        assert!(
            store
                .save_task_delivery_intent(Uuid::new_v4(), 0, &intent)
                .is_err()
        );
        assert_eq!(
            store
                .save_task_delivery_intent(Uuid::new_v4(), 1, &intent)
                .unwrap()
                .revision,
            2
        );
        // Retrying an old successful command returns its immutable receipt,
        // without rolling the current intent back from revision 2 to revision 1.
        assert_eq!(
            store
                .save_task_delivery_intent(command, 0, &saved.intent)
                .unwrap(),
            saved
        );
        assert_eq!(
            store
                .task_delivery_intent(project, conversation, task)
                .unwrap()
                .unwrap()
                .revision,
            2
        );
        assert!(
            store
                .task_delivery_intent("OTHER", conversation, task)
                .is_err()
        );
        intent.project_id = "OTHER".into();
        assert!(
            store
                .save_task_delivery_intent(Uuid::new_v4(), 2, &intent)
                .is_err()
        );
        drop(store);
        let restored = SqliteStore::open(&path)
            .unwrap()
            .task_delivery_intent(project, conversation, task)
            .unwrap()
            .unwrap();
        assert_eq!(restored.revision, 2);
        assert_eq!(restored.intent.split_policy.seed, 42);
        assert_eq!(
            restored.intent.missing_slots(),
            vec![
                DeliverySlot::DatasetScope,
                DeliverySlot::LabelSpec,
                DeliverySlot::TrainingTarget
            ]
        );
    }
}
