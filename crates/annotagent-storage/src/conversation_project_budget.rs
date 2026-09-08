//! Optional explicit Project ceiling shared by conversation planners and linked Batch calls.
use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCallLimitInput {
    pub id: Uuid,
    pub expected_revision: u64,
    pub maximum_calls: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeginConversationTask, ConversationCallGrant, ConversationMessageInput};
    #[test]
    fn project_ceiling_is_shared_atomic_persistent_and_not_a_phase_authorization() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-project-ceiling.db");
        let store = std::sync::Arc::new(SqliteStore::open(&path).unwrap());
        let project = Uuid::new_v4().to_string();
        let mut tasks = Vec::new();
        for _ in 0..2 {
            let conversation = store.create_conversation(&project).unwrap();
            let message = ConversationMessageInput {
                id: Uuid::new_v4(),
                text: "TEST goal".into(),
                image: None,
                reference: None,
            };
            store
                .append_conversation_message(&project, conversation, &message)
                .unwrap();
            let task = Uuid::new_v4();
            store
                .begin_conversation_task(
                    &project,
                    conversation,
                    &BeginConversationTask {
                        id: task,
                        source_message_id: message.id,
                        schema_revision: "a".repeat(64),
                    },
                )
                .unwrap();
            tasks.push(task);
        }
        let input = ProjectCallLimitInput {
            id: Uuid::new_v4(),
            expected_revision: 0,
            maximum_calls: 1,
        };
        store
            .set_project_conversation_call_limit(&project, &input)
            .unwrap();
        assert!(
            store
                .reserve_conversation_call(
                    &project,
                    tasks[0],
                    Uuid::new_v4(),
                    &"a".repeat(64),
                    &"b".repeat(64)
                )
                .is_err()
        );
        for task in &tasks {
            store
                .authorize_conversation_calls(
                    &project,
                    &ConversationCallGrant {
                        id: Uuid::new_v4(),
                        task_id: *task,
                        scope_hash: "a".repeat(64),
                        maximum_calls: 4,
                        expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
                    },
                )
                .unwrap();
        }
        let batch = annotagent_core::BatchId::new();
        store
            .reserve_processing_operation(
                &batch.to_string(),
                "TEST-slug",
                &serde_json::json!({}),
                &serde_json::json!({"authorization":{"conversation":{"task_id":tasks[1]}}}),
            )
            .unwrap();
        store.with_connection(|db|{db.execute("INSERT INTO batch_model_call_allowances(batch_id,maximum,reserved) VALUES(?1,4,0)",[batch.to_string()])?;Ok(())}).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let planning = store.clone();
        let owner = project.clone();
        let start = barrier.clone();
        let task = tasks[0];
        let a = std::thread::spawn(move || {
            start.wait();
            planning
                .reserve_conversation_call(
                    &owner,
                    task,
                    Uuid::new_v4(),
                    &"a".repeat(64),
                    &"b".repeat(64),
                )
                .is_ok()
        });
        // Independent SQLite connection: this also exercises transaction races, not just
        // the in-process store mutex. A busy loser must not dispatch an external request.
        let processing = SqliteStore::open(&path).unwrap();
        let b = std::thread::spawn(move || {
            barrier.wait();
            processing.reserve_batch_model_call(batch).is_ok()
        });
        assert_eq!(
            usize::from(a.join().unwrap()) + usize::from(b.join().unwrap()),
            1
        );
        assert!(store.reserve_batch_model_call(batch).is_err());
        assert!(
            store
                .reserve_conversation_call(
                    &project,
                    tasks[1],
                    Uuid::new_v4(),
                    &"a".repeat(64),
                    &"b".repeat(64)
                )
                .is_err()
        );
        drop(store);
        let store = SqliteStore::open(path).unwrap();
        assert_eq!(
            store.project_conversation_call_limit(&project).unwrap(),
            ProjectCallLimit {
                revision: 1,
                maximum_calls: Some(1),
                reserved_calls: 1
            }
        );
        assert!(
            store
                .set_project_conversation_call_limit(
                    &project,
                    &ProjectCallLimitInput {
                        id: Uuid::new_v4(),
                        expected_revision: 0,
                        maximum_calls: 9
                    }
                )
                .is_err()
        );
        assert!(
            store
                .set_project_conversation_call_limit(
                    &project,
                    &ProjectCallLimitInput {
                        id: Uuid::new_v4(),
                        expected_revision: 1,
                        maximum_calls: 0
                    }
                )
                .is_err()
        );
        let expanded = ProjectCallLimitInput {
            id: Uuid::new_v4(),
            expected_revision: 1,
            maximum_calls: 2,
        };
        store
            .set_project_conversation_call_limit(&project, &expanded)
            .unwrap();
        assert_eq!(
            store
                .set_project_conversation_call_limit(&project, &input)
                .unwrap()
                .maximum_calls,
            Some(2)
        );
        store.reserve_batch_model_call(batch).unwrap();
        assert!(store.reserve_batch_model_call(batch).is_err());
        assert!(
            store
                .set_project_conversation_call_limit("foreign", &input)
                .is_err()
        );
        assert_eq!(
            store
                .project_conversation_call_limit("foreign")
                .unwrap()
                .maximum_calls,
            None
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCallLimit {
    pub revision: u64,
    pub maximum_calls: Option<u64>,
    pub reserved_calls: u64,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

fn read(db: &rusqlite::Connection, project: &str) -> Result<ProjectCallLimit, StorageError> {
    let limit:Option<(i64,i64)>=db.query_row("SELECT revision,maximum_calls FROM conversation_project_budget_revisions WHERE project_id=?1 ORDER BY revision DESC LIMIT 1",[project],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let reserved_calls:i64=db.query_row("WITH owned_tasks AS (
        SELECT t.id FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE c.project_id=?1
    ) SELECT (SELECT COUNT(*) FROM conversation_model_calls m JOIN owned_tasks t ON t.id=m.task_id)
      + (SELECT COALESCE(SUM(a.reserved),0) FROM batch_model_call_allowances a
         JOIN processing_operations p ON p.id=a.batch_id
         JOIN owned_tasks t ON t.id=json_extract(p.state_json,'$.authorization.conversation.task_id'))",
        [project],|r|r.get(0))?;
    Ok(ProjectCallLimit {
        revision: u64::try_from(limit.map_or(0, |v| v.0))
            .map_err(|_| invalid("Invalid Project budget revision"))?,
        maximum_calls: limit
            .map(|v| u64::try_from(v.1).map_err(|_| invalid("Invalid Project call limit")))
            .transpose()?,
        reserved_calls: u64::try_from(reserved_calls)
            .map_err(|_| invalid("Invalid Project reserved calls"))?,
    })
}

/// Must run inside the same transaction as the phase-specific reservation.
pub(crate) fn admit(db: &rusqlite::Connection, project: &str) -> Result<(), StorageError> {
    let budget = read(db, project)?;
    if budget
        .maximum_calls
        .is_some_and(|maximum| budget.reserved_calls >= maximum)
    {
        return Err(invalid(
            "Project conversation call limit exhausted; no request was sent. Explicitly review the cumulative Project limit before continuing.",
        ));
    }
    Ok(())
}

impl SqliteStore {
    pub fn project_conversation_call_limit(
        &self,
        project: &str,
    ) -> Result<ProjectCallLimit, StorageError> {
        self.with_connection(|db| read(db, project))
    }

    /// Setting a ceiling is not a model/data authorization. Replays cannot undo a later change.
    pub fn set_project_conversation_call_limit(
        &self,
        project: &str,
        input: &ProjectCallLimitInput,
    ) -> Result<ProjectCallLimit, StorageError> {
        let maximum = i64::try_from(input.maximum_calls)
            .map_err(|_| invalid("Project call limit is too large"))?;
        let revision = input
            .expected_revision
            .checked_add(1)
            .and_then(|v| i64::try_from(v).ok())
            .ok_or_else(|| invalid("Project budget revision overflow"))?;
        self.with_connection(|db|{
            let tx=db.unchecked_transaction()?;
            let previous:Option<(String,i64,i64)>=tx.query_row("SELECT project_id,revision,maximum_calls FROM conversation_project_budget_revisions WHERE id=?1",[input.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
            if let Some(saved)=previous {
                if saved!=(project.to_owned(),revision,maximum){return Err(invalid("Project budget request conflicts with its saved revision"));}
                return read(&tx,project);
            }
            let current=read(&tx,project)?;
            if current.revision!=input.expected_revision{return Err(invalid("Project budget changed; reload and confirm its current cumulative limit"));}
            if input.maximum_calls<current.reserved_calls{return Err(invalid("Project call limit cannot erase calls already reserved"));}
            tx.execute("INSERT INTO conversation_project_budget_revisions(id,project_id,revision,maximum_calls,created_at) VALUES(?1,?2,?3,?4,?5)",params![input.id.to_string(),project,revision,maximum,chrono::Utc::now().to_rfc3339()])?;
            let result=read(&tx,project)?;
            tx.commit()?;Ok(result)
        })
    }
}
