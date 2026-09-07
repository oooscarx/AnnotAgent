//! Project conversation journal. Messages reference images; they are not annotations.

use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationImageRef {
    pub image_id: String,
    pub sha256: String,
}

/// A user-authored journal input. It grants no model or execution authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationMessageInput {
    pub id: Uuid,
    pub text: String,
    pub image: Option<ConversationImageRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub conversation_id: Uuid,
    pub sequence: i64,
    pub input: ConversationMessageInput,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

fn require_owner(
    db: &rusqlite::Connection,
    project: &str,
    conversation: Uuid,
) -> Result<(), StorageError> {
    let owned: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM project_conversations WHERE id=?1 AND project_id=?2)",
        params![conversation.to_string(), project],
        |row| row.get(0),
    )?;
    if !owned {
        return Err(invalid("conversation does not belong to this Project"));
    }
    Ok(())
}

impl SqliteStore {
    /// Read-only discovery; opening a workspace must not create a conversation.
    pub fn project_conversation(&self, project: &str) -> Result<Option<Uuid>, StorageError> {
        self.with_connection(|db| {
            let id: Option<String> = db
                .query_row(
                    "SELECT id FROM project_conversations WHERE project_id=?1",
                    [project],
                    |row| row.get(0),
                )
                .optional()?;
            id.map(|value| {
                Uuid::parse_str(&value)
                    .map_err(|_| invalid("invalid persisted conversation identity"))
            })
            .transpose()
        })
    }

    /// Explicit creation command, never a GET side effect. One main conversation
    /// per Project in this release; repeated creation returns the same identity.
    /// Application must resolve the existing Project and pass its stable UUID,
    /// not the URL slug or a client-asserted owner. Storage has no Project catalog.
    pub fn create_conversation(&self, project: &str) -> Result<Uuid, StorageError> {
        Uuid::parse_str(project).map_err(|_| invalid("Project identity must be a stable UUID"))?;
        self.with_connection(|db| {
            let transaction = db.unchecked_transaction()?;
            transaction.execute("INSERT INTO project_conversations(id,project_id,created_at) VALUES (?1,?2,?3) ON CONFLICT(project_id) DO NOTHING", params![Uuid::new_v4().to_string(), project, chrono::Utc::now().to_rfc3339()])?;
            let id: String = transaction.query_row("SELECT id FROM project_conversations WHERE project_id=?1", [project], |row| row.get(0))?;
            let id = Uuid::parse_str(&id).map_err(|_| invalid("invalid persisted conversation identity"))?;
            transaction.commit()?;
            Ok(id)
        })
    }

    pub fn append_conversation_message(
        &self,
        project: &str,
        conversation: Uuid,
        input: &ConversationMessageInput,
    ) -> Result<ConversationMessage, StorageError> {
        if input.text.trim().is_empty() || input.text.len() > 65_536 {
            return Err(invalid(
                "message text must be nonempty and at most 65536 bytes",
            ));
        }
        self.with_connection(|db| {
            let transaction = db.unchecked_transaction()?;
            require_owner(&transaction, project, conversation)?;
            let existing: Option<(i64, String)> = transaction.query_row("SELECT sequence,input_json FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2", params![conversation.to_string(), input.id.to_string()], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
            if let Some((sequence, json)) = existing {
                let saved: ConversationMessageInput = serde_json::from_str(&json)?;
                if saved != *input { return Err(invalid("message ID already has different content or references")); }
                return Ok(ConversationMessage { conversation_id: conversation, sequence, input: saved });
            }
            if let Some(image) = &input.image {
                let hash: Option<String> = transaction.query_row("SELECT sha256 FROM images WHERE id=?1 AND project_id=?2", params![image.image_id, project], |row| row.get(0)).optional()?;
                if hash.as_deref() != Some(image.sha256.as_str()) { return Err(invalid("image is foreign, missing or changed since selection")); }
            }
            let sequence: i64 = transaction.query_row("SELECT COALESCE(MAX(sequence),0)+1 FROM conversation_messages WHERE conversation_id=?1", [conversation.to_string()], |row| row.get(0))?;
            transaction.execute("INSERT INTO conversation_messages(conversation_id,sequence,message_id,input_json,created_at) VALUES (?1,?2,?3,?4,?5)", params![conversation.to_string(), sequence, input.id.to_string(), serde_json::to_string(input)?, chrono::Utc::now().to_rfc3339()])?;
            transaction.commit()?;
            Ok(ConversationMessage { conversation_id: conversation, sequence, input: input.clone() })
        })
    }

    pub fn conversation_messages(
        &self,
        project: &str,
        conversation: Uuid,
        after: i64,
        limit: u32,
    ) -> Result<Vec<ConversationMessage>, StorageError> {
        if after < 0 {
            return Err(invalid("message cursor cannot be negative"));
        }
        self.with_connection(|db| {
            require_owner(db, project, conversation)?;
            let mut query = db.prepare("SELECT sequence,input_json FROM conversation_messages WHERE conversation_id=?1 AND sequence>?2 ORDER BY sequence LIMIT ?3")?;
            let rows = query.query_map(params![conversation.to_string(), after, limit.clamp(1, 100)], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
            rows.map(|row| { let (sequence, json) = row?; Ok(ConversationMessage { conversation_id: conversation, sequence, input: serde_json::from_str(&json)? }) }).collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteStore;

    #[test]
    fn message_cannot_smuggle_an_authorization_or_annotation() {
        let base = serde_json::json!({"id": Uuid::new_v4(), "text": "yes", "image": null});
        for field in [
            "authorized",
            "human_verified",
            "annotation",
            "run_id",
            "role",
        ] {
            let mut value = base.clone();
            value[field] = serde_json::json!(true);
            assert!(serde_json::from_value::<ConversationMessageInput>(value).is_err());
        }
        assert!(serde_json::from_value::<ConversationMessageInput>(base).is_ok());
    }

    #[test]
    fn frozen_message_is_idempotent_and_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let project = uuid::Uuid::new_v4().to_string();
        let store = SqliteStore::open(&path).unwrap();
        let image = store
            .ensure_project_image(project.parse().unwrap(), "test.png", "hash-a", "{}")
            .unwrap()
            .image_id
            .to_string();
        let conversation = store.create_conversation(&project).unwrap();
        assert_eq!(store.create_conversation(&project).unwrap(), conversation);
        let input = ConversationMessageInput {
            id: uuid::Uuid::new_v4(),
            text: "Annotate this image".into(),
            image: Some(ConversationImageRef {
                image_id: image.clone(),
                sha256: "hash-a".into(),
            }),
        };
        let first = store
            .append_conversation_message(&project, conversation, &input)
            .unwrap();
        assert_eq!(first.sequence, 1);
        let mut conflict = input.clone();
        conflict.text = "Different command".into();
        assert!(
            store
                .append_conversation_message(&project, conversation, &conflict)
                .is_err()
        );
        store
            .with_connection(|db| {
                db.execute("UPDATE images SET sha256='hash-b' WHERE id=?1", [&image])?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            store
                .append_conversation_message(&project, conversation, &input)
                .unwrap(),
            first
        );
        let mut stale = input.clone();
        stale.id = uuid::Uuid::new_v4();
        assert!(
            store
                .append_conversation_message(&project, conversation, &stale)
                .is_err()
        );
        assert!(
            store
                .append_conversation_message("foreign-project", conversation, &input)
                .is_err()
        );
        drop(store);
        let restored = SqliteStore::open(&path).unwrap();
        assert_eq!(
            restored
                .conversation_messages(&project, conversation, 0, 10)
                .unwrap(),
            vec![first]
        );
        assert!(
            restored
                .conversation_messages("foreign-project", conversation, 0, 10)
                .is_err()
        );
        let goal_only = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "Goal before upload".into(),
            image: None,
        };
        let second = restored
            .append_conversation_message(&project, conversation, &goal_only)
            .unwrap();
        assert_eq!(second.sequence, 2);
        assert_eq!(
            restored
                .conversation_messages(&project, conversation, 1, 1)
                .unwrap(),
            vec![second]
        );
        assert!(
            restored
                .conversation_messages(&project, conversation, -1, 10)
                .is_err()
        );
    }
}
