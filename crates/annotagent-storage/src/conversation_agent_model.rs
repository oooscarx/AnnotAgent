//! Next-request preference only; never an inference grant or a Workflow binding.
use crate::{SqliteStore, StorageError};
use annotagent_core::ModelProfileId;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationAgentModel {
    pub revision: u64,
    pub model_profile_id: Option<ModelProfileId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectConversationAgentModel {
    pub request_id: Uuid,
    pub expected_revision: u64,
    /// None explicitly returns to the existing project/default resolver.
    pub model_profile_id: Option<ModelProfileId>,
}

fn read(
    db: &rusqlite::Connection,
    conversation: Uuid,
) -> Result<ConversationAgentModel, StorageError> {
    let row: Option<(i64, Option<String>)> = db.query_row(
        "SELECT revision,model_profile_id FROM conversation_agent_models WHERE conversation_id=?1",
        [conversation.to_string()], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional()?;
    match row {
        None => Ok(ConversationAgentModel::default()),
        Some((revision, id)) => Ok(ConversationAgentModel {
            revision: u64::try_from(revision).map_err(|_| {
                StorageError::InvalidConversation("Invalid model preference revision".into())
            })?,
            model_profile_id: id
                .map(|value| serde_json::from_value(serde_json::json!(value)))
                .transpose()?,
        }),
    }
}

impl SqliteStore {
    pub fn conversation_agent_model_command_exists(
        &self,
        project: &str,
        conversation: Uuid,
        request: Uuid,
    ) -> Result<bool, StorageError> {
        self.with_connection(|db| {
            crate::conversations::require_owner(db, project, conversation)?;
            Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_agent_model_commands WHERE conversation_id=?1 AND request_id=?2)", params![conversation.to_string(),request.to_string()], |r| r.get(0))?)
        })
    }

    pub fn conversation_agent_model(
        &self,
        project: &str,
        conversation: Uuid,
    ) -> Result<ConversationAgentModel, StorageError> {
        self.with_connection(|db| {
            crate::conversations::require_owner(db, project, conversation)?;
            read(db, conversation)
        })
    }

    /// Application validates the Registry choice before a first write. An old exact
    /// retry returns CURRENT preference, never rolls back a more recent selection.
    pub fn select_conversation_agent_model(
        &self,
        project: &str,
        conversation: Uuid,
        input: &SelectConversationAgentModel,
    ) -> Result<ConversationAgentModel, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            crate::conversations::require_owner(&tx, project, conversation)?;
            let current = read(&tx, conversation)?;
            let saved: Option<String> = tx.query_row("SELECT input_json FROM conversation_agent_model_commands WHERE conversation_id=?1 AND request_id=?2", params![conversation.to_string(), input.request_id.to_string()], |r| r.get(0)).optional()?;
            if let Some(saved) = saved {
                if serde_json::from_str::<SelectConversationAgentModel>(&saved)? != *input {
                    return Err(StorageError::InvalidConversation("Model selection command ID conflicts with its original input".into()));
                }
                return Ok(current);
            }
            if input.request_id.is_nil() || input.expected_revision != current.revision {
                return Err(StorageError::InvalidConversation("Agent model selection changed; reload before choosing again".into()));
            }
            let revision = current.revision.checked_add(1).ok_or_else(|| StorageError::InvalidConversation("Model selection revision exhausted".into()))?;
            let db_revision = i64::try_from(revision).map_err(|_| StorageError::InvalidConversation("Model selection revision exhausted".into()))?;
            tx.execute("INSERT INTO conversation_agent_models(conversation_id,revision,model_profile_id) VALUES(?1,?2,?3) ON CONFLICT(conversation_id) DO UPDATE SET revision=excluded.revision,model_profile_id=excluded.model_profile_id", params![conversation.to_string(),db_revision,input.model_profile_id.map(|id| id.to_string())])?;
            tx.execute("INSERT INTO conversation_agent_model_commands(conversation_id,request_id,input_json) VALUES(?1,?2,?3)", params![conversation.to_string(),input.request_id.to_string(),serde_json::to_string(input)?])?;
            tx.commit()?;
            Ok(ConversationAgentModel { revision, model_profile_id: input.model_profile_id })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_model_preference_is_owned_cas_persistent_and_not_a_grant() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-model.db");
        let store = SqliteStore::open(&path).unwrap();
        let owner = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        assert_eq!(
            store
                .conversation_agent_model(&owner, conversation)
                .unwrap(),
            ConversationAgentModel::default()
        );
        let first = SelectConversationAgentModel {
            request_id: Uuid::new_v4(),
            expected_revision: 0,
            model_profile_id: Some(ModelProfileId::new()),
        };
        let chosen = store
            .select_conversation_agent_model(&owner, conversation, &first)
            .unwrap();
        assert_eq!(chosen.revision, 1);
        let second = SelectConversationAgentModel {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            model_profile_id: None,
        };
        let latest = store
            .select_conversation_agent_model(&owner, conversation, &second)
            .unwrap();
        assert_eq!(latest.revision, 2);
        assert!(
            store
                .select_conversation_agent_model(
                    &owner,
                    conversation,
                    &SelectConversationAgentModel {
                        request_id: Uuid::new_v4(),
                        ..first.clone()
                    }
                )
                .is_err()
        );
        assert!(
            store
                .select_conversation_agent_model(
                    &owner,
                    conversation,
                    &SelectConversationAgentModel {
                        model_profile_id: None,
                        ..first.clone()
                    }
                )
                .is_err()
        );
        assert!(
            store
                .conversation_agent_model("foreign", conversation)
                .is_err()
        );
        assert!(
            store
                .select_conversation_agent_model("foreign", conversation, &first)
                .is_err()
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .select_conversation_agent_model(&owner, conversation, &first)
                .unwrap(),
            latest
        );
        assert_eq!(
            store
                .conversation_agent_model(&owner, conversation)
                .unwrap(),
            latest
        );
        store
            .with_connection(|db| {
                for table in [
                    "conversation_tasks",
                    "conversation_model_calls",
                    "conversation_call_grants",
                ] {
                    let count: i64 =
                        db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
                    assert_eq!(count, 0, "{table} must remain empty");
                }
                Ok(())
            })
            .unwrap();
    }
}
