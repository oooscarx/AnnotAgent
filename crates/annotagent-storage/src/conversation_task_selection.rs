use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_is_owned_versioned_and_old_retry_cannot_reset_current_task() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-selection.db");
        let store = SqliteStore::open(&path).unwrap();
        let owner = Uuid::new_v4().to_string();
        let project = owner.as_str();
        let conversation = store.create_conversation(project).unwrap();
        let mut tasks = Vec::new();
        for text in ["TEST cups", "TEST scene"] {
            let input = crate::ConversationMessageInput {
                id: Uuid::new_v4(),
                text: text.into(),
                image: None,
                reference: None,
            };
            store
                .append_conversation_message(project, conversation, &input)
                .unwrap();
            let task = crate::BeginConversationTask {
                id: Uuid::new_v4(),
                source_message_id: input.id,
                schema_revision: "a".repeat(64),
            };
            store
                .begin_conversation_task(project, conversation, &task)
                .unwrap();
            tasks.push(task.id);
        }
        assert_eq!(
            store
                .conversation_task_selection(project, conversation)
                .unwrap(),
            ConversationTaskSelection {
                revision: 0,
                task_id: None
            }
        );
        let first = SelectConversationTask {
            request_id: Uuid::new_v4(),
            expected_revision: 0,
            task_id: tasks[0],
        };
        let second = SelectConversationTask {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            task_id: tasks[1],
        };
        store
            .select_conversation_task(project, conversation, &first)
            .unwrap();
        let latest = store
            .select_conversation_task(project, conversation, &second)
            .unwrap();
        assert_eq!(latest.revision, 2);
        assert!(
            store
                .select_conversation_task(
                    project,
                    conversation,
                    &SelectConversationTask {
                        request_id: Uuid::new_v4(),
                        ..first.clone()
                    }
                )
                .is_err()
        );
        assert!(
            store
                .select_conversation_task(
                    project,
                    conversation,
                    &SelectConversationTask {
                        task_id: tasks[1],
                        ..first.clone()
                    }
                )
                .is_err()
        );
        assert!(
            store
                .select_conversation_task("foreign", conversation, &second)
                .is_err()
        );
        let other = store.create_conversation(project).unwrap();
        // The Project has one canonical conversation, so use another Project to test task ownership.
        assert_eq!(other, conversation);
        let foreign_owner = Uuid::new_v4().to_string();
        let foreign = store.create_conversation(&foreign_owner).unwrap();
        assert!(
            store
                .select_conversation_task(&foreign_owner, foreign, &first)
                .is_err()
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .select_conversation_task(project, conversation, &first)
                .unwrap(),
            latest
        );
        assert_eq!(
            store
                .conversation_task_selection(project, conversation)
                .unwrap(),
            latest
        );
        for task in tasks {
            assert_eq!(
                store
                    .conversation_task_budget(project, task)
                    .unwrap()
                    .total_reserved_calls,
                0
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectConversationTask {
    pub request_id: Uuid,
    pub expected_revision: u64,
    pub task_id: Uuid,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationTaskSelection {
    pub revision: u64,
    pub task_id: Option<Uuid>,
}
fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}
fn read(
    db: &rusqlite::Connection,
    project: &str,
    conversation: Uuid,
) -> Result<ConversationTaskSelection, StorageError> {
    let owned: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM project_conversations WHERE id=?1 AND project_id=?2)",
        params![conversation.to_string(), project],
        |r| r.get(0),
    )?;
    if !owned {
        return Err(invalid("Conversation selection belongs to another Project"));
    }
    let row:Option<(i64,String)>=db.query_row("SELECT revision,task_id FROM conversation_task_selections WHERE conversation_id=?1 ORDER BY revision DESC LIMIT 1",[conversation.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    row.map_or(
        Ok(ConversationTaskSelection {
            revision: 0,
            task_id: None,
        }),
        |(revision, task)| {
            Ok(ConversationTaskSelection {
                revision: u64::try_from(revision)
                    .map_err(|_| invalid("Invalid task selection revision"))?,
                task_id: Some(
                    Uuid::parse_str(&task).map_err(|_| invalid("Invalid saved task identity"))?,
                ),
            })
        },
    )
}
impl SqliteStore {
    pub fn conversation_task_selection(
        &self,
        project: &str,
        conversation: Uuid,
    ) -> Result<ConversationTaskSelection, StorageError> {
        self.with_connection(|db| read(db, project, conversation))
    }
    pub fn select_conversation_task(
        &self,
        project: &str,
        conversation: Uuid,
        input: &SelectConversationTask,
    ) -> Result<ConversationTaskSelection, StorageError> {
        self.with_connection(|db|{
            let tx=db.unchecked_transaction()?;
            let current=read(&tx,project,conversation)?;
            let owned:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks WHERE id=?1 AND conversation_id=?2)",params![input.task_id.to_string(),conversation.to_string()],|r|r.get(0))?;
            if !owned{return Err(invalid("Selected task belongs to another conversation"));}
            let saved:Option<(String,i64,String)>=tx.query_row("SELECT conversation_id,revision,task_id FROM conversation_task_selections WHERE request_id=?1",[input.request_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
            if let Some((owner,revision,task))=saved {
                if owner!=conversation.to_string()||u64::try_from(revision).ok().and_then(|value|value.checked_sub(1))!=Some(input.expected_revision)||task!=input.task_id.to_string(){return Err(invalid("Selection retry changed its original request"));}
                // A delayed retry never overwrites a newer selection, nor returns an obsolete pointer.
                return Ok(current);
            }
            if current.revision!=input.expected_revision{return Err(invalid("Task selection changed in another view; reload before selecting again"));}
            let revision=current.revision.checked_add(1).filter(|value|i64::try_from(*value).is_ok()).ok_or_else(||invalid("Task selection revision overflow"))?;
            tx.execute("INSERT INTO conversation_task_selections(request_id,conversation_id,revision,task_id) VALUES(?1,?2,?3,?4)",params![input.request_id.to_string(),conversation.to_string(),i64::try_from(revision).map_err(|_|invalid("Task selection revision overflow"))?,input.task_id.to_string()])?;
            tx.commit()?;
            Ok(ConversationTaskSelection{revision,task_id:Some(input.task_id)})
        })
    }
}
