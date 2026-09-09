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

/// A selected Sandbox object, never permission to change project-wide labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConversationSelectionRef {
    StopRequest {
        task_id: Option<Uuid>,
    },
    SampleCandidate {
        task_id: Uuid,
        project_schema_revision: String,
        draft_id: String,
        draft_revision: u64,
        sample_test_id: String,
        candidate_id: String,
        source_artifact_id: Uuid,
    },
}

/// A user-authored journal input. It grants no model or execution authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationMessageInput {
    pub id: Uuid,
    pub text: String,
    pub image: Option<ConversationImageRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<ConversationSelectionRef>,
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

pub(crate) fn require_owner(
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
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_conversations WHERE id=?1)",
            [conversation.to_string()],
            |row| row.get(0),
        )?;
        return Err(StorageError::ConversationContract {
            code: if exists {
                "owner_mismatch"
            } else {
                "not_found"
            },
            message: "conversation does not belong to this Project".into(),
        });
    }
    Ok(())
}

pub(crate) fn append_message_in_transaction(
    db: &rusqlite::Connection,
    project: &str,
    conversation: Uuid,
    input: &ConversationMessageInput,
) -> Result<ConversationMessage, StorageError> {
    if matches!(
        input.reference,
        Some(ConversationSelectionRef::StopRequest { .. })
    ) {
        return Err(invalid(
            "stop messages require the atomic stop-command service",
        ));
    }
    if input.text.trim().is_empty() || input.text.len() > 65_536 {
        return Err(invalid(
            "message text must be nonempty and at most 65536 bytes",
        ));
    }

    require_owner(db, project, conversation)?;
    let existing: Option<(i64, String)> = db.query_row("SELECT sequence,input_json FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2", params![conversation.to_string(), input.id.to_string()], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
    if let Some((sequence, json)) = existing {
        let saved: ConversationMessageInput = serde_json::from_str(&json)?;
        if saved != *input {
            return Err(invalid(
                "message ID already has different content or references",
            ));
        }
        return Ok(ConversationMessage {
            conversation_id: conversation,
            sequence,
            input: saved,
        });
    }
    if let Some(ConversationSelectionRef::SampleCandidate {
        task_id,
        project_schema_revision,
        ..
    }) = &input.reference
    {
        if input.image.is_none() {
            return Err(invalid("Selected candidate requires an image reference"));
        }
        let task_owned: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks WHERE id=?1 AND conversation_id=?2 AND schema_revision=?3)", params![task_id.to_string(),conversation.to_string(),project_schema_revision], |row| row.get(0))?;
        if !task_owned {
            return Err(invalid(
                "Selected candidate task or Schema revision does not match this conversation",
            ));
        }
    }
    if let Some(image) = &input.image {
        let hash: Option<String> = db
            .query_row(
                "SELECT sha256 FROM images WHERE id=?1 AND project_id=?2",
                params![image.image_id, project],
                |row| row.get(0),
            )
            .optional()?;
        if hash.as_deref() != Some(image.sha256.as_str()) {
            return Err(invalid(
                "image is foreign, missing or changed since selection",
            ));
        }
    }
    let sequence: i64 = db.query_row(
        "SELECT COALESCE(MAX(sequence),0)+1 FROM conversation_messages WHERE conversation_id=?1",
        [conversation.to_string()],
        |row| row.get(0),
    )?;
    db.execute("INSERT INTO conversation_messages(conversation_id,sequence,message_id,input_json,created_at) VALUES (?1,?2,?3,?4,?5)", params![conversation.to_string(), sequence, input.id.to_string(), serde_json::to_string(input)?, chrono::Utc::now().to_rfc3339()])?;

    Ok(ConversationMessage {
        conversation_id: conversation,
        sequence,
        input: input.clone(),
    })
}

impl SqliteStore {
    /// Oldest unscoped message, without transferring a reference-only journal.
    /// This is context discovery, not task selection or execution authorization.
    pub fn conversation_first_goal(
        &self,
        project: &str,
        conversation: Uuid,
    ) -> Result<Option<ConversationMessage>, StorageError> {
        self.with_connection(|db| {
            require_owner(db, project, conversation)?;
            let row: Option<(i64, String)> = db.query_row(
                "SELECT sequence,input_json FROM conversation_messages WHERE conversation_id=?1 AND json_extract(input_json,'$.reference') IS NULL ORDER BY sequence LIMIT 1",
                [conversation.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
            row.map(|(sequence, input)| Ok(ConversationMessage {
                conversation_id: conversation,
                sequence,
                input: serde_json::from_str(&input)?,
            })).transpose()
        })
    }

    pub fn conversation_message_history(
        &self,
        project: &str,
        conversation: Uuid,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ConversationMessage>, StorageError> {
        if before.is_some_and(|value| value <= 0) {
            return Err(invalid("history cursor must be positive"));
        }
        let through = before.map_or(i64::MAX, |value| value - 1);
        self.with_connection(|db| {
            require_owner(db, project, conversation)?;
            let mut query = db.prepare("SELECT sequence,input_json FROM conversation_messages WHERE conversation_id=?1 AND sequence<=?2 ORDER BY sequence DESC LIMIT ?3")?;
            let rows = query.query_map(params![conversation.to_string(), through, limit.clamp(1, 100)], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
            let mut messages = rows.map(|row| {
                let (sequence, input) = row?;
                Ok(ConversationMessage { conversation_id: conversation, sequence, input: serde_json::from_str(&input)? })
            }).collect::<Result<Vec<_>, StorageError>>()?;
            messages.reverse();
            Ok(messages)
        })
    }

    pub fn conversation_message(
        &self,
        project: &str,
        conversation: Uuid,
        id: Uuid,
    ) -> Result<Option<ConversationMessage>, StorageError> {
        self.with_connection(|db| {
            require_owner(db, project, conversation)?;
            let row: Option<(i64,String)> = db.query_row("SELECT sequence,input_json FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2", params![conversation.to_string(),id.to_string()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
            row.map(|(sequence,input)| Ok(ConversationMessage { conversation_id: conversation, sequence, input: serde_json::from_str(&input)? })).transpose()
        })
    }
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
        self.with_connection(|db| {
            let transaction = db.unchecked_transaction()?;
            let message =
                append_message_in_transaction(&transaction, project, conversation, input)?;
            transaction.commit()?;
            Ok(message)
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
    #[test]
    fn first_goal_skips_reference_only_history_and_preserves_owner_and_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("TEST-goal.sqlite");
        let store = crate::SqliteStore::open(&path).unwrap();
        let owner = uuid::Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        assert!(
            store
                .conversation_first_goal(&owner, conversation)
                .unwrap()
                .is_none()
        );
        // Journal-only fixture: no task execution or model authority is created.
        store.with_connection(|db| {
            for sequence in 1..=250 {
                let id = uuid::Uuid::new_v4();
                let input = serde_json::json!({"id":id,"text":"stop","image":null,"reference":{"scope":"stop_request","task_id":null}});
                db.execute("INSERT INTO conversation_messages(conversation_id,sequence,message_id,input_json,created_at) VALUES (?1,?2,?3,?4,'TEST')", rusqlite::params![conversation.to_string(), sequence, id.to_string(), input.to_string()])?;
            }
            Ok(())
        }).unwrap();
        assert!(
            store
                .conversation_first_goal(&owner, conversation)
                .unwrap()
                .is_none()
        );
        let input = super::ConversationMessageInput {
            id: uuid::Uuid::new_v4(),
            text: "TEST original goal".into(),
            image: None,
            reference: None,
        };
        let oldest = store
            .append_conversation_message(&owner, conversation, &input)
            .unwrap();
        let newer = super::ConversationMessageInput {
            id: uuid::Uuid::new_v4(),
            text: "TEST newer goal".into(),
            ..input
        };
        store
            .append_conversation_message(&owner, conversation, &newer)
            .unwrap();
        assert_eq!(
            store.conversation_first_goal(&owner, conversation).unwrap(),
            Some(oldest.clone())
        );
        assert!(
            store
                .conversation_first_goal("foreign-owner", conversation)
                .is_err()
        );
        drop(store);
        let reopened = crate::SqliteStore::open(&path).unwrap();
        assert_eq!(
            reopened
                .conversation_first_goal(&owner, conversation)
                .unwrap(),
            Some(oldest)
        );
    }

    use super::*;
    #[test]
    fn reverse_history_is_bounded_stable_and_owned_after_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-history.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        for index in 1..=120 {
            store
                .append_conversation_message(
                    &project,
                    conversation,
                    &ConversationMessageInput {
                        id: Uuid::new_v4(),
                        text: format!("TEST {index}"),
                        image: None,
                        reference: None,
                    },
                )
                .unwrap();
        }
        let latest = store
            .conversation_message_history(&project, conversation, None, u32::MAX)
            .unwrap();
        assert_eq!(latest.len(), 100);
        assert_eq!(latest[0].sequence, 21);
        assert_eq!(latest[99].sequence, 120);
        let prior = store
            .conversation_message_history(&project, conversation, Some(41), 20)
            .unwrap();
        assert_eq!(prior[0].sequence, 21);
        assert_eq!(prior[19].sequence, 40);
        store
            .append_conversation_message(
                &project,
                conversation,
                &ConversationMessageInput {
                    id: Uuid::new_v4(),
                    text: "TEST new message".into(),
                    image: None,
                    reference: None,
                },
            )
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .conversation_message_history(&project, conversation, Some(41), 20)
                .unwrap(),
            prior
        );
        assert!(
            store
                .conversation_message_history("foreign", conversation, None, 20)
                .is_err()
        );
        assert!(
            store
                .conversation_message_history(&project, conversation, Some(0), 20)
                .is_err()
        );
        assert!(
            store
                .conversation_message_history(&project, conversation, Some(1), 20)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn candidate_reference_survives_restart_and_cannot_become_a_project_goal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-reference.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let image = store
            .ensure_project_image(project.parse().unwrap(), "TEST.png", "hash", "{}")
            .unwrap()
            .image_id
            .to_string();
        let goal = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST goal".into(),
            image: None,
            reference: None,
        };
        store
            .append_conversation_message(&project, conversation, &goal)
            .unwrap();
        let task = crate::BeginConversationTask {
            id: Uuid::new_v4(),
            source_message_id: goal.id,
            schema_revision: "a".repeat(64),
        };
        store
            .begin_conversation_task(&project, conversation, &task)
            .unwrap();
        // Storage tests the journal contract; Application tests validate real sample evidence.
        let input = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST only this object".into(),
            image: Some(ConversationImageRef {
                image_id: image.clone(),
                sha256: "hash".into(),
            }),
            reference: Some(ConversationSelectionRef::SampleCandidate {
                task_id: task.id,
                project_schema_revision: task.schema_revision.clone(),
                draft_id: "TEST-draft".into(),
                draft_revision: 2,
                sample_test_id: "TEST-sample".into(),
                candidate_id: "TEST-candidate".into(),
                source_artifact_id: Uuid::new_v4(),
            }),
        };
        let saved = store
            .append_conversation_message(&project, conversation, &input)
            .unwrap();
        assert!(
            store
                .begin_conversation_task(
                    &project,
                    conversation,
                    &crate::BeginConversationTask {
                        id: Uuid::new_v4(),
                        source_message_id: input.id,
                        schema_revision: task.schema_revision
                    }
                )
                .is_err()
        );
        store
            .with_connection(|db| {
                db.execute("UPDATE images SET sha256='changed' WHERE id=?1", [&image])?;
                Ok(())
            })
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .append_conversation_message(&project, conversation, &input)
                .unwrap(),
            saved
        );
        assert_eq!(
            store
                .conversation_message(&project, conversation, input.id)
                .unwrap(),
            Some(saved)
        );
        let mut conflict = input.clone();
        conflict.reference = None;
        assert!(
            store
                .append_conversation_message(&project, conversation, &conflict)
                .is_err()
        );
        let mut stale = input;
        stale.id = Uuid::new_v4();
        assert!(
            store
                .append_conversation_message(&project, conversation, &stale)
                .is_err()
        );
    }
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
            reference: None,
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
            reference: None,
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
