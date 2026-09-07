//! Thin task identity and frozen goal linkage, not a parallel workflow executor.
use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginConversationTask {
    pub id: Uuid,
    pub source_message_id: Uuid,
    pub schema_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationTask {
    pub conversation_id: Uuid,
    pub input: BeginConversationTask,
    pub created_at: String,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

impl SqliteStore {
    /// Caller resolves Project identity and checks the current schema under its write lock.
    /// Repeated admissions for the same message return the original task, not a new budget.
    pub fn begin_conversation_task(
        &self,
        project: &str,
        conversation: Uuid,
        input: &BeginConversationTask,
    ) -> Result<ConversationTask, StorageError> {
        if input.schema_revision.len() != 64
            || !input
                .schema_revision
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid("schema revision must be a SHA-256 digest"));
        }
        self.with_connection(|db| {
            let transaction = db.unchecked_transaction()?;
            let owned: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM project_conversations WHERE id=?1 AND project_id=?2)",
                params![conversation.to_string(), project], |row| row.get(0),
            )?;
            if !owned { return Err(invalid("conversation does not belong to this Project")); }
            let existing: Option<(String,String,String,String,String)> = transaction.query_row(
                "SELECT id,conversation_id,source_message_id,schema_revision,created_at FROM conversation_tasks WHERE id=?1 OR (conversation_id=?2 AND source_message_id=?3) ORDER BY CASE WHEN id=?1 THEN 0 ELSE 1 END LIMIT 1",
                params![input.id.to_string(), conversation.to_string(), input.source_message_id.to_string()],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
            ).optional()?;
            if let Some((id, owner, message, revision, created_at)) = existing {
                if owner != conversation.to_string() || message != input.source_message_id.to_string() || revision != input.schema_revision {
                    return Err(invalid("task request conflicts with its frozen goal or schema revision"));
                }
                return Ok(ConversationTask { conversation_id: conversation, input: BeginConversationTask { id: Uuid::parse_str(&id).map_err(|_| invalid("invalid saved task ID"))?, ..input.clone() }, created_at });
            }
            let source_exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2)",
                params![conversation.to_string(),input.source_message_id.to_string()], |row| row.get(0),
            )?;
            if !source_exists { return Err(invalid("task requires a saved message in this conversation")); }
            let created_at = chrono::Utc::now().to_rfc3339();
            transaction.execute("INSERT INTO conversation_tasks(id,conversation_id,source_message_id,schema_revision,created_at) VALUES(?1,?2,?3,?4,?5)", params![input.id.to_string(),conversation.to_string(),input.source_message_id.to_string(),input.schema_revision,created_at])?;
            transaction.commit()?;
            Ok(ConversationTask { conversation_id: conversation, input: input.clone(), created_at })
        })
    }

    pub fn conversation_tasks(
        &self,
        project: &str,
        conversation: Uuid,
    ) -> Result<Vec<ConversationTask>, StorageError> {
        // Reuse the journal owner boundary even when there are no tasks.
        self.conversation_messages(project, conversation, 0, 1)?;
        self.with_connection(|db| {
            let mut statement = db.prepare("SELECT id,source_message_id,schema_revision,created_at FROM conversation_tasks WHERE conversation_id=?1 ORDER BY created_at,id")?;
            let rows = statement.query_map([conversation.to_string()], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?)))?;
            rows.map(|row| {
                let (id,source,schema_revision,created_at) = row?;
                Ok(ConversationTask { conversation_id: conversation, input: BeginConversationTask { id: Uuid::parse_str(&id).map_err(|_| invalid("invalid saved task ID"))?, source_message_id: Uuid::parse_str(&source).map_err(|_| invalid("invalid saved message ID"))?, schema_revision }, created_at })
            }).collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversationMessageInput;

    #[test]
    fn task_admission_is_owned_idempotent_and_restores_its_frozen_goal() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("history.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let other_project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let other = store.create_conversation(&other_project).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "Find cups, not bottles".into(),
            image: None,
        };
        let input = BeginConversationTask {
            id: Uuid::new_v4(),
            source_message_id: message.id,
            schema_revision: "a".repeat(64),
        };
        assert!(
            store
                .begin_conversation_task(&project, conversation, &input)
                .is_err()
        );
        assert!(
            store
                .conversation_tasks(&project, conversation)
                .unwrap()
                .is_empty()
        );
        store
            .append_conversation_message(&project, conversation, &message)
            .unwrap();
        let saved = store
            .begin_conversation_task(&project, conversation, &input)
            .unwrap();
        assert_eq!(
            store
                .begin_conversation_task(&project, conversation, &input)
                .unwrap(),
            saved
        );
        let new_key = BeginConversationTask {
            id: Uuid::new_v4(),
            ..input.clone()
        };
        assert_eq!(
            store
                .begin_conversation_task(&project, conversation, &new_key)
                .unwrap(),
            saved
        );
        assert!(
            store
                .begin_conversation_task(&other_project, conversation, &input)
                .is_err()
        );
        assert!(
            store
                .conversation_tasks(&other_project, conversation)
                .is_err()
        );
        let changed = BeginConversationTask {
            schema_revision: "b".repeat(64),
            ..input.clone()
        };
        assert!(
            store
                .begin_conversation_task(&project, conversation, &changed)
                .is_err()
        );
        store
            .append_conversation_message(&other_project, other, &message)
            .unwrap();
        assert!(
            store
                .begin_conversation_task(&other_project, other, &input)
                .is_err()
        );
        let different_task = store
            .begin_conversation_task(&other_project, other, &new_key)
            .unwrap();
        // An ID collision must not be hidden by a valid same-message lookup.
        assert!(
            store
                .begin_conversation_task(
                    &project,
                    conversation,
                    &BeginConversationTask {
                        id: different_task.input.id,
                        ..input
                    }
                )
                .is_err()
        );
        drop(store);
        let restored = SqliteStore::open(&path).unwrap();
        assert_eq!(
            restored.conversation_tasks(&project, conversation).unwrap(),
            vec![saved]
        );
    }

    #[test]
    fn task_start_cannot_smuggle_execution_permissions_or_annotations() {
        let mut input = serde_json::json!({ "id": Uuid::new_v4(), "source_message_id": Uuid::new_v4(), "schema_revision": "a".repeat(64) });
        for key in [
            "authorized",
            "max_cost",
            "annotations",
            "status",
            "provider_url",
        ] {
            input[key] = serde_json::json!(true);
            assert!(serde_json::from_value::<BeginConversationTask>(input.clone()).is_err());
            input.as_object_mut().unwrap().remove(key);
        }
    }
}
