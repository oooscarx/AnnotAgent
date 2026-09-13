//! Recoverable lifecycle for user-visible conversation tasks.
//!
//! A Project has one canonical conversation container. Archiving or deleting a
//! "conversation" therefore changes only its task's navigation lifecycle; it
//! never deletes messages, receipts, images, Runs, annotations or exports.

use crate::{ConversationTask, SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) fn migrate(db: &rusqlite::Connection) -> Result<(), StorageError> {
    db.execute_batch(include_str!(
        "../../../migrations/0073_conversation_task_lifecycle.sql"
    ))?;
    db.execute(
        "INSERT OR IGNORE INTO schema_migrations(version,name,applied_at) VALUES(73,'conversation_task_lifecycle',?1)",
        [chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationTaskLifecycleState {
    Active,
    Archived,
    Trashed,
}

impl ConversationTaskLifecycleState {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Trashed => "trashed",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationTaskLifecycleFilter {
    #[default]
    Active,
    Archived,
    Trashed,
    All,
}

impl ConversationTaskLifecycleFilter {
    pub(crate) const fn sql_name(self) -> Option<&'static str> {
        match self {
            Self::Active => Some("active"),
            Self::Archived => Some("archived"),
            Self::Trashed => Some("trashed"),
            Self::All => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationTaskLifecycle {
    pub state: ConversationTaskLifecycleState,
    pub revision: u64,
    pub archived_at: Option<String>,
    pub trashed_at: Option<String>,
    pub deletion_operation_id: Option<Uuid>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationTaskLifecycleAction {
    Archive,
    MoveToTrash,
    Restore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationTaskLifecycleCommand {
    pub command_id: Uuid,
    pub expected_revision: u64,
    pub action: ConversationTaskLifecycleAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationTaskLifecycleReceipt {
    pub command_id: Uuid,
    pub action: ConversationTaskLifecycleAction,
    pub replayed: bool,
    pub task: ConversationTask,
    pub lifecycle: ConversationTaskLifecycle,
    pub warnings: Vec<String>,
}

fn contract(code: &'static str, message: impl Into<String>) -> StorageError {
    StorageError::ConversationContract {
        code,
        message: message.into(),
    }
}

fn state(value: &str) -> Result<ConversationTaskLifecycleState, StorageError> {
    match value {
        "active" => Ok(ConversationTaskLifecycleState::Active),
        "archived" => Ok(ConversationTaskLifecycleState::Archived),
        "trashed" => Ok(ConversationTaskLifecycleState::Trashed),
        _ => Err(contract(
            "invalid_task_lifecycle",
            "invalid saved conversation task lifecycle",
        )),
    }
}

pub(crate) fn read_in(
    db: &rusqlite::Connection,
    task: Uuid,
    created_at: &str,
) -> Result<ConversationTaskLifecycle, StorageError> {
    let row: Option<(
        String,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
    )> = db
        .query_row(
            "SELECT state,revision,archived_at,trashed_at,deletion_operation_id,updated_at
             FROM conversation_task_lifecycle WHERE task_id=?1",
            [task.to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?;
    match row {
        Some((saved_state, revision, archived_at, trashed_at, operation, updated_at)) => {
            Ok(ConversationTaskLifecycle {
                state: state(&saved_state)?,
                revision: u64::try_from(revision).map_err(|_| {
                    contract(
                        "invalid_task_lifecycle",
                        "invalid saved task lifecycle revision",
                    )
                })?,
                archived_at,
                trashed_at,
                deletion_operation_id: operation
                    .map(|value| {
                        Uuid::parse_str(&value).map_err(|_| {
                            contract(
                                "invalid_task_lifecycle",
                                "invalid saved deletion operation identity",
                            )
                        })
                    })
                    .transpose()?,
                updated_at,
            })
        }
        None => Ok(ConversationTaskLifecycle {
            state: ConversationTaskLifecycleState::Active,
            revision: 0,
            archived_at: None,
            trashed_at: None,
            deletion_operation_id: None,
            updated_at: created_at.to_owned(),
        }),
    }
}

pub(crate) fn require_active_in(db: &rusqlite::Connection, task: Uuid) -> Result<(), StorageError> {
    let active: bool = db.query_row(
        "SELECT COALESCE((SELECT state='active' FROM conversation_task_lifecycle WHERE task_id=?1),1)",
        [task.to_string()],
        |row| row.get(0),
    )?;
    if !active {
        return Err(contract(
            "task_not_active",
            "task is archived or in trash; restore it before making changes",
        ));
    }
    Ok(())
}

fn task_in(
    db: &rusqlite::Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
) -> Result<ConversationTask, StorageError> {
    crate::conversations::require_owner(db, project, conversation)?;
    let row: Option<(String, String, String)> = db
        .query_row(
            "SELECT source_message_id,schema_revision,created_at FROM conversation_tasks
             WHERE id=?1 AND conversation_id=?2",
            params![task.to_string(), conversation.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (source_message_id, schema_revision, created_at) = row.ok_or_else(|| {
        contract(
            "task_not_found",
            "conversation task does not belong to this Project and conversation",
        )
    })?;
    Ok(ConversationTask {
        conversation_id: conversation,
        input: crate::BeginConversationTask {
            id: task,
            source_message_id: Uuid::parse_str(&source_message_id).map_err(|_| {
                contract(
                    "invalid_task_lifecycle",
                    "invalid saved task source message",
                )
            })?,
            schema_revision,
        },
        lifecycle: read_in(db, task, &created_at)?,
        created_at,
    })
}

fn active_reasons(db: &rusqlite::Connection, task: Uuid) -> Result<Vec<String>, StorageError> {
    let id = task.to_string();
    let checks = [
        (
            "model_call",
            "SELECT EXISTS(SELECT 1 FROM conversation_model_calls WHERE task_id=?1 AND status='reserved')",
        ),
        (
            "model_attempt",
            "SELECT EXISTS(SELECT 1 FROM task_model_attempts WHERE task_id=?1 AND status='started')",
        ),
        (
            "pipeline_builder",
            "SELECT EXISTS(SELECT 1 FROM conversation_builder_operations WHERE task_id=?1 AND status='reserved')",
        ),
        (
            "journey",
            "SELECT EXISTS(SELECT 1 FROM conversation_journey_dispatch d JOIN conversation_journey_consents c ON c.id=d.consent_id WHERE c.task_id=?1 AND d.status IN ('queued','running'))",
        ),
        (
            "queued_message",
            "SELECT EXISTS(SELECT 1 FROM conversation_message_queue q LEFT JOIN conversation_queued_planning p USING(conversation_id,message_id) WHERE q.task_id=?1 AND q.cancelled_at IS NULL AND p.call_id IS NULL)",
        ),
        (
            "sample_test",
            "SELECT EXISTS(SELECT 1 FROM sample_operations WHERE json_extract(request_json,'$.conversation.task_id')=?1 AND status IN ('queued','running','cancelling'))",
        ),
        (
            "dataset_processing",
            "SELECT EXISTS(SELECT 1 FROM processing_operations p JOIN dataset_batches b ON b.id=p.id WHERE json_extract(p.state_json,'$.authorization.conversation.task_id')=?1 AND b.status IN ('pending','running','paused'))",
        ),
        (
            "delivery_package",
            "SELECT EXISTS(SELECT 1 FROM conversation_exports e JOIN delivery_export_snapshots p ON p.export_id=e.id WHERE e.task_id=?1 AND p.phase IN ('preparing','exporting','validating'))",
        ),
        (
            "delivery_package_consent",
            "SELECT EXISTS(SELECT 1 FROM delivery_package_consents WHERE task_id=?1 AND state='armed')",
        ),
        (
            "resume_delivery",
            "SELECT EXISTS(SELECT 1 FROM conversation_resume_outbox WHERE task_id=?1 AND applied_at IS NULL UNION ALL SELECT 1 FROM conversation_answer_delivery a JOIN conversation_human_requests h ON h.id=a.request_id WHERE h.task_id=?1 AND a.status IN ('pending','dispatched'))",
        ),
        (
            "delivery_package_authorization",
            "SELECT EXISTS(SELECT 1 FROM delivery_package_consents WHERE task_id=?1 AND state='armed')",
        ),
        (
            "queued_instruction",
            "SELECT EXISTS(SELECT 1 FROM conversation_message_queue q LEFT JOIN conversation_queued_planning p ON p.conversation_id=q.conversation_id AND p.message_id=q.message_id LEFT JOIN conversation_model_calls c ON c.id=p.call_id WHERE q.task_id=?1 AND q.cancelled_at IS NULL AND (p.call_id IS NULL OR c.status='reserved'))",
        ),
        (
            "resume_checkpoint",
            "SELECT EXISTS(SELECT 1 FROM conversation_resume_outbox WHERE task_id=?1 AND applied_at IS NULL)",
        ),
    ];
    checks
        .into_iter()
        .filter_map(
            |(name, sql)| match db.query_row(sql, [&id], |row| row.get::<_, bool>(0)) {
                Ok(true) => Some(Ok(name.to_owned())),
                Ok(false) => None,
                Err(error) => Some(Err(StorageError::from(error))),
            },
        )
        .collect()
}

fn warnings(db: &rusqlite::Connection, task: Uuid) -> Result<Vec<String>, StorageError> {
    let unknown: bool = db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM conversation_model_calls WHERE task_id=?1 AND status='in_doubt'
            UNION ALL
            SELECT 1 FROM task_model_attempts WHERE task_id=?1 AND status='in_doubt'
        )",
        [task.to_string()],
        |row| row.get(0),
    )?;
    Ok(if unknown {
        vec!["outcome_unknown: remote completion and final cost remain unknown; this lifecycle change does not retry or cancel the request".into()]
    } else {
        vec![]
    })
}

impl SqliteStore {
    pub fn conversation_task_lifecycle(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<ConversationTaskLifecycle, StorageError> {
        self.with_connection(|db| {
            let task = task_in(db, project, conversation, task)?;
            read_in(db, task.input.id, &task.created_at)
        })
    }

    pub fn change_conversation_task_lifecycle(
        &self,
        project: &str,
        conversation: Uuid,
        task_id: Uuid,
        input: &ConversationTaskLifecycleCommand,
    ) -> Result<ConversationTaskLifecycleReceipt, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let task = task_in(&tx, project, conversation, task_id)?;
            let saved: Option<(String, String)> = tx
                .query_row(
                    "SELECT input_json,result_json FROM conversation_task_lifecycle_commands WHERE command_id=?1",
                    [input.command_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((saved_input, result)) = saved {
                if serde_json::from_str::<ConversationTaskLifecycleCommand>(&saved_input)? != *input {
                    return Err(contract(
                        "task_lifecycle_command_conflict",
                        "lifecycle command ID was already used with a different request",
                    ));
                }
                let mut receipt: ConversationTaskLifecycleReceipt = serde_json::from_str(&result)?;
                if receipt.task.input.id != task_id {
                    return Err(contract(
                        "task_lifecycle_command_conflict",
                        "lifecycle command belongs to another task",
                    ));
                }
                receipt.replayed = true;
                return Ok(receipt);
            }
            let current = read_in(&tx, task_id, &task.created_at)?;
            if current.revision != input.expected_revision {
                return Err(contract(
                    "task_lifecycle_revision_conflict",
                    format!(
                        "task lifecycle changed; expected revision {}, current revision {}",
                        input.expected_revision, current.revision
                    ),
                ));
            }
            let next_state = match (current.state, input.action) {
                (ConversationTaskLifecycleState::Active, ConversationTaskLifecycleAction::Archive) => {
                    ConversationTaskLifecycleState::Archived
                }
                (
                    ConversationTaskLifecycleState::Active
                    | ConversationTaskLifecycleState::Archived,
                    ConversationTaskLifecycleAction::MoveToTrash,
                ) => ConversationTaskLifecycleState::Trashed,
                (
                    ConversationTaskLifecycleState::Archived
                    | ConversationTaskLifecycleState::Trashed,
                    ConversationTaskLifecycleAction::Restore,
                ) => ConversationTaskLifecycleState::Active,
                _ => {
                    return Err(contract(
                        "invalid_task_lifecycle_transition",
                        format!(
                            "cannot apply {:?} while task is {}",
                            input.action,
                            current.state.name()
                        ),
                    ));
                }
            };
            if next_state != ConversationTaskLifecycleState::Active {
                let reasons = active_reasons(&tx, task_id)?;
                if !reasons.is_empty() {
                    return Err(contract(
                        "task_lifecycle_active_work",
                        format!(
                            "task has active work ({}); stop it and wait for settlement before changing lifecycle",
                            reasons.join(", ")
                        ),
                    ));
                }
            }
            let now = chrono::Utc::now().to_rfc3339();
            let revision = current
                .revision
                .checked_add(1)
                .ok_or_else(|| contract("invalid_task_lifecycle", "task lifecycle revision overflow"))?;
            let lifecycle = ConversationTaskLifecycle {
                state: next_state,
                revision,
                archived_at: if next_state == ConversationTaskLifecycleState::Archived {
                    Some(now.clone())
                } else {
                    current.archived_at
                },
                trashed_at: if next_state == ConversationTaskLifecycleState::Trashed {
                    Some(now.clone())
                } else {
                    current.trashed_at
                },
                deletion_operation_id: if input.action
                    == ConversationTaskLifecycleAction::MoveToTrash
                {
                    Some(input.command_id)
                } else {
                    current.deletion_operation_id
                },
                updated_at: now.clone(),
            };
            tx.execute(
                "INSERT INTO conversation_task_lifecycle(task_id,state,revision,archived_at,trashed_at,deletion_operation_id,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)
                 ON CONFLICT(task_id) DO UPDATE SET state=excluded.state,revision=excluded.revision,
                    archived_at=excluded.archived_at,trashed_at=excluded.trashed_at,
                    deletion_operation_id=excluded.deletion_operation_id,updated_at=excluded.updated_at",
                params![
                    task_id.to_string(),
                    lifecycle.state.name(),
                    i64::try_from(lifecycle.revision).map_err(|_| contract(
                        "invalid_task_lifecycle",
                        "task lifecycle revision overflow"
                    ))?,
                    lifecycle.archived_at,
                    lifecycle.trashed_at,
                    lifecycle.deletion_operation_id.map(|value| value.to_string()),
                    lifecycle.updated_at,
                ],
            )?;
            let receipt = ConversationTaskLifecycleReceipt {
                command_id: input.command_id,
                action: input.action,
                replayed: false,
                task,
                lifecycle,
                warnings: warnings(&tx, task_id)?,
            };
            tx.execute(
                "INSERT INTO conversation_task_lifecycle_commands(command_id,task_id,input_json,result_json,created_at)
                 VALUES(?1,?2,?3,?4,?5)",
                params![
                    input.command_id.to_string(),
                    task_id.to_string(),
                    serde_json::to_string(input)?,
                    serde_json::to_string(&receipt)?,
                    now,
                ],
            )?;
            tx.commit()?;
            Ok(receipt)
        })
    }

    pub fn conversation_task_lifecycle_receipt(
        &self,
        project: &str,
        conversation: Uuid,
        command_id: Uuid,
    ) -> Result<Option<ConversationTaskLifecycleReceipt>, StorageError> {
        self.with_connection(|db| {
            crate::conversations::require_owner(db, project, conversation)?;
            let row: Option<(String, String)> = db
                .query_row(
                    "SELECT c.task_id,c.result_json FROM conversation_task_lifecycle_commands c
                     JOIN conversation_tasks t ON t.id=c.task_id
                     WHERE c.command_id=?1 AND t.conversation_id=?2",
                    params![command_id.to_string(), conversation.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            row.map(|(_, value)| serde_json::from_str(&value).map_err(Into::into))
                .transpose()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(store: &SqliteStore, project: &str, conversation: Uuid) -> ConversationTask {
        let message = crate::ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST find the ball".into(),
            image: None,
            reference: None,
        };
        store
            .append_conversation_message(project, conversation, &message)
            .unwrap();
        store
            .begin_conversation_task(
                project,
                conversation,
                &crate::BeginConversationTask {
                    id: Uuid::new_v4(),
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap()
    }

    #[test]
    fn lifecycle_is_owned_cas_idempotent_recoverable_and_non_destructive() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("TEST-task-lifecycle.sqlite");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let foreign_project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let foreign = store.create_conversation(&foreign_project).unwrap();
        let saved = task(&store, &project, conversation);
        let foreign_task = task(&store, &foreign_project, foreign);
        assert_eq!(
            saved.lifecycle.state,
            ConversationTaskLifecycleState::Active
        );
        assert_eq!(saved.lifecycle.revision, 0);

        let archive = ConversationTaskLifecycleCommand {
            command_id: Uuid::new_v4(),
            expected_revision: 0,
            action: ConversationTaskLifecycleAction::Archive,
        };
        let archived = store
            .change_conversation_task_lifecycle(&project, conversation, saved.input.id, &archive)
            .unwrap();
        assert_eq!(
            archived.lifecycle.state,
            ConversationTaskLifecycleState::Archived
        );
        assert_eq!(archived.lifecycle.revision, 1);
        assert!(archived.lifecycle.archived_at.is_some());
        assert!(!archived.replayed);
        let replay = store
            .change_conversation_task_lifecycle(&project, conversation, saved.input.id, &archive)
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.lifecycle, archived.lifecycle);
        assert!(
            store
                .change_conversation_task_lifecycle(
                    &project,
                    conversation,
                    saved.input.id,
                    &ConversationTaskLifecycleCommand {
                        command_id: archive.command_id,
                        expected_revision: 1,
                        action: ConversationTaskLifecycleAction::MoveToTrash,
                    },
                )
                .is_err()
        );
        assert!(
            store
                .change_conversation_task_lifecycle(
                    &foreign_project,
                    foreign,
                    saved.input.id,
                    &ConversationTaskLifecycleCommand {
                        command_id: Uuid::new_v4(),
                        expected_revision: 1,
                        action: ConversationTaskLifecycleAction::MoveToTrash,
                    },
                )
                .is_err()
        );

        let trash = ConversationTaskLifecycleCommand {
            command_id: Uuid::new_v4(),
            expected_revision: 1,
            action: ConversationTaskLifecycleAction::MoveToTrash,
        };
        let trashed = store
            .change_conversation_task_lifecycle(&project, conversation, saved.input.id, &trash)
            .unwrap();
        assert_eq!(
            trashed.lifecycle.state,
            ConversationTaskLifecycleState::Trashed
        );
        assert_eq!(
            trashed.lifecycle.deletion_operation_id,
            Some(trash.command_id)
        );
        assert!(trashed.lifecycle.trashed_at.is_some());
        assert!(
            store
                .conversation_tasks_filtered(
                    &project,
                    conversation,
                    ConversationTaskLifecycleFilter::Active,
                )
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .conversation_tasks_filtered(
                    &project,
                    conversation,
                    ConversationTaskLifecycleFilter::Trashed,
                )
                .unwrap()
                .len(),
            1
        );
        // The task and its source journal were never deleted.
        assert_eq!(
            store
                .conversation_tasks(&project, conversation)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .conversation_messages(&project, conversation, 0, 10)
                .unwrap()
                .len(),
            1
        );

        let restore = ConversationTaskLifecycleCommand {
            command_id: Uuid::new_v4(),
            expected_revision: 2,
            action: ConversationTaskLifecycleAction::Restore,
        };
        let restored = store
            .change_conversation_task_lifecycle(&project, conversation, saved.input.id, &restore)
            .unwrap();
        assert_eq!(
            restored.lifecycle.state,
            ConversationTaskLifecycleState::Active
        );
        assert_eq!(restored.lifecycle.revision, 3);
        assert_eq!(
            store
                .conversation_task_lifecycle_receipt(&project, conversation, trash.command_id)
                .unwrap()
                .unwrap()
                .lifecycle,
            trashed.lifecycle
        );
        drop(store);
        let reopened = SqliteStore::open(&path).unwrap();
        assert_eq!(
            reopened
                .conversation_task_lifecycle(&project, conversation, saved.input.id)
                .unwrap(),
            restored.lifecycle
        );
        assert!(
            reopened
                .conversation_task_lifecycle(&project, conversation, foreign_task.input.id,)
                .is_err()
        );
    }

    #[test]
    fn active_work_blocks_lifecycle_but_unknown_outcome_only_warns() {
        let store = SqliteStore::open_in_memory().unwrap();
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let task = task(&store, &project, conversation);
        let builder = Uuid::new_v4();
        store
            .with_connection(|db| {
                db.execute(
                    "INSERT INTO conversation_builder_operations(id,task_id,request_hash,status) VALUES(?1,?2,?3,'reserved')",
                    params![builder.to_string(), task.input.id.to_string(), "a".repeat(64)],
                )?;
                Ok(())
            })
            .unwrap();
        let archive = ConversationTaskLifecycleCommand {
            command_id: Uuid::new_v4(),
            expected_revision: 0,
            action: ConversationTaskLifecycleAction::Archive,
        };
        let error = store
            .change_conversation_task_lifecycle(&project, conversation, task.input.id, &archive)
            .unwrap_err();
        assert!(matches!(
            error,
            StorageError::ConversationContract {
                code: "task_lifecycle_active_work",
                ..
            }
        ));
        let package_consent = Uuid::new_v4();
        store
            .with_connection(|db| {
                db.execute(
                    "UPDATE conversation_builder_operations SET status='completed' WHERE id=?1",
                    [builder.to_string()],
                )?;
                db.execute(
                    "INSERT INTO delivery_package_consents(id,project_id,conversation_id,task_id,input_json,state,created_at) VALUES(?1,?2,?3,?4,'{}','armed','2026-01-01T00:00:00Z')",
                    params![package_consent.to_string(),project,conversation.to_string(),task.input.id.to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        let error = store
            .change_conversation_task_lifecycle(&project, conversation, task.input.id, &archive)
            .unwrap_err();
        assert!(matches!(
            error,
            StorageError::ConversationContract {
                code: "task_lifecycle_active_work",
                ..
            }
        ));
        store
            .with_connection(|db| {
                db.execute(
                    "UPDATE delivery_package_consents SET state='cancelled' WHERE id=?1",
                    [package_consent.to_string()],
                )?;
                let grant = Uuid::new_v4();
                db.execute(
                    "INSERT INTO conversation_call_grants(task_id,id,scope_hash,maximum_calls,expires_at) VALUES(?1,?2,?3,1,'2099-01-01T00:00:00Z')",
                    params![task.input.id.to_string(),grant.to_string(),"b".repeat(64)],
                )?;
                db.execute(
                    "INSERT INTO conversation_model_calls(id,task_id,request_hash,status,created_at) VALUES(?1,?2,?3,'in_doubt','2026-01-01T00:00:00Z')",
                    params![Uuid::new_v4().to_string(),task.input.id.to_string(),"c".repeat(64)],
                )?;
                Ok(())
            })
            .unwrap();
        let archived = store
            .change_conversation_task_lifecycle(&project, conversation, task.input.id, &archive)
            .unwrap();
        assert_eq!(archived.warnings.len(), 1);
        assert!(archived.warnings[0].contains("outcome_unknown"));
    }
}
