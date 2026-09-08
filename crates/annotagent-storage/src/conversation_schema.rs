//! Versioned annotation semantics. No formal annotation or Project YAML is stored here.
use crate::{SqliteStore, StorageError};
use annotagent_core::TaskConfig;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationSchemaDefinition {
    pub goal: String,
    pub task: TaskConfig,
    pub boundary_rules: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BeginConversationTask, ConversationCallGrant, ConversationCallStatus,
        ConversationMessageInput,
    };

    fn task(store: &SqliteStore, project: &str) -> Uuid {
        let conversation = store.create_conversation(project).unwrap();
        let message = ConversationMessageInput {
            reference: None,
            id: Uuid::new_v4(),
            text: "TEST classify objects".into(),
            image: None,
        };
        store
            .append_conversation_message(project, conversation, &message)
            .unwrap();
        let id = Uuid::new_v4();
        store
            .begin_conversation_task(
                project,
                conversation,
                &BeginConversationTask {
                    id,
                    source_message_id: message.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        id
    }

    fn definition() -> ConversationSchemaDefinition {
        ConversationSchemaDefinition {
            goal: "TEST classify objects".into(),
            task: serde_json::from_value(serde_json::json!({"id":"objects", "kind":"classification", "labels":["cup", "bottle"]})).unwrap(),
            boundary_rules: vec![],
        }
    }

    #[test]
    fn human_schema_is_owned_idempotent_versioned_and_has_no_model_calls() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-human-schema.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let task = task(&store, &project);
        let request = Uuid::new_v4();
        let input = definition();
        let draft = store
            .create_human_conversation_schema_draft(&project, task, request, &input)
            .unwrap();
        assert_eq!(draft.source_call_id, None);
        assert_eq!(draft.source_request_id, Some(request));
        assert_eq!(draft.base_schema_revision, "a".repeat(64));
        assert_eq!(
            store
                .create_human_conversation_schema_draft(&project, task, request, &input)
                .unwrap(),
            draft
        );
        assert!(
            store
                .create_human_conversation_schema_draft("foreign", task, request, &input)
                .is_err()
        );
        assert!(
            store
                .create_human_conversation_schema_draft(&project, Uuid::new_v4(), request, &input)
                .is_err()
        );
        let mut edited = input.clone();
        edited.task.labels.push("plate".into());
        assert!(
            store
                .create_human_conversation_schema_draft(&project, task, request, &edited)
                .is_err()
        );
        let revision = store
            .revise_conversation_schema_draft(&project, draft.id, Uuid::new_v4(), 1, &edited)
            .unwrap();
        assert_eq!(revision.revision, 2);
        assert_eq!(
            store
                .create_human_conversation_schema_draft(&project, task, request, &input)
                .unwrap(),
            revision
        );
        assert!(
            store
                .create_conversation_schema_draft(&project, task, request, &input)
                .is_err()
        );
        store
            .with_connection(|db| {
                for table in ["conversation_model_calls", "conversation_call_grants"] {
                    let count: i64 =
                        db.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                            row.get(0)
                        })?;
                    assert_eq!(count, 0);
                }
                Ok(())
            })
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .conversation_schema_draft(&project, draft.id, None)
                .unwrap(),
            revision
        );
        assert_eq!(
            store
                .conversation_schema_draft(&project, draft.id, Some(1))
                .unwrap(),
            draft
        );
    }

    #[test]
    fn legacy_schema_migration_preserves_model_provenance_revisions_and_foreign_keys() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST-legacy-schema.db");
        let store = SqliteStore::open(&path).unwrap();
        let project = Uuid::new_v4().to_string();
        let task = task(&store, &project);
        let grant = ConversationCallGrant {
            id: Uuid::new_v4(),
            task_id: task,
            scope_hash: "a".repeat(64),
            maximum_calls: 1,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        };
        store
            .authorize_conversation_calls(&project, &grant)
            .unwrap();
        let call = Uuid::new_v4();
        store
            .reserve_conversation_call(&project, task, call, &grant.scope_hash, &"b".repeat(64))
            .unwrap();
        store
            .finish_conversation_call(
                &project,
                task,
                call,
                ConversationCallStatus::Completed,
                serde_json::json!({"TEST":true}),
            )
            .unwrap();
        // Construct a genuine v28 metadata/revision pair in an isolated database.
        store.with_connection(|db| {
            db.execute_batch("DROP TABLE conversation_schema_revisions; DROP TABLE conversation_schema_drafts; DELETE FROM schema_migrations WHERE version=34;")?;
            db.execute_batch(include_str!("../../../migrations/0028_conversation_schema_drafts.sql"))?;
            Ok(())
        }).unwrap();
        let id = Uuid::new_v4();
        let edit_request = Uuid::new_v4();
        let input = definition();
        let mut edited = input.clone();
        edited.boundary_rules.push("TEST no bottles".into());
        store
            .with_connection(|db| {
                db.execute(
                    "INSERT INTO conversation_schema_drafts VALUES(?1,?2,?3,?4)",
                    params![
                        id.to_string(),
                        task.to_string(),
                        call.to_string(),
                        "a".repeat(64)
                    ],
                )?;
                for (revision, request, definition) in
                    [(1, call, &input), (2, edit_request, &edited)]
                {
                    db.execute(
                        "INSERT INTO conversation_schema_revisions VALUES(?1,?2,?3,?4)",
                        params![
                            id.to_string(),
                            revision,
                            request.to_string(),
                            serde_json::to_string(definition)?
                        ],
                    )?;
                }
                Ok(())
            })
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        let latest = store
            .conversation_schema_for_call(&project, task, call)
            .unwrap()
            .unwrap();
        assert_eq!(latest.id, id);
        assert_eq!(latest.source_call_id, Some(call));
        assert_eq!(latest.source_request_id, None);
        assert_eq!(latest.definition, edited);
        assert_eq!(latest.revision, 2);
        assert_eq!(
            store
                .conversation_schema_draft(&project, id, Some(1))
                .unwrap()
                .definition,
            input
        );
        assert_eq!(
            store
                .revise_conversation_schema_draft(&project, id, edit_request, 1, &edited)
                .unwrap(),
            latest
        );
        store
            .with_connection(|db| {
                let foreign_keys: bool =
                    db.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
                assert!(foreign_keys);
                assert!(
                    db.execute(
                        "DELETE FROM conversation_schema_drafts WHERE id=?1",
                        [id.to_string()]
                    )
                    .is_err()
                );
                assert!(
                    db.execute(
                        "UPDATE conversation_schema_drafts SET source_request_id=?1 WHERE id=?2",
                        params![Uuid::new_v4().to_string(), id.to_string()]
                    )
                    .is_err()
                );
                assert!(
                    db.execute(
                        "UPDATE conversation_schema_drafts SET source_call_id=NULL WHERE id=?1",
                        [id.to_string()]
                    )
                    .is_err()
                );
                Ok(())
            })
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store.conversation_schema_draft(&project, id, None).unwrap(),
            latest
        );
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSchemaDraft {
    pub id: Uuid,
    pub task_id: Uuid,
    pub source_call_id: Option<Uuid>,
    /// Present only for human-authored definitions; never an inference receipt.
    pub source_request_id: Option<Uuid>,
    pub base_schema_revision: String,
    pub revision: u64,
    pub definition: ConversationSchemaDefinition,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

pub(crate) fn migrate_sources(db: &rusqlite::Connection) -> Result<(), StorageError> {
    let tx = db.unchecked_transaction()?;
    let applied: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=34)",
        [],
        |row| row.get(0),
    )?;
    if !applied {
        tx.execute_batch(include_str!(
            "../../../migrations/0034_conversation_schema_sources.sql"
        ))?;
        let broken: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check('conversation_schema_revisions') UNION ALL SELECT 1 FROM pragma_foreign_key_check('conversation_schema_drafts'))",
            [], |row| row.get(0),
        )?;
        if broken {
            return Err(invalid(
                "Schema source migration failed foreign-key validation",
            ));
        }
        tx.execute("INSERT INTO schema_migrations(version,name,applied_at) VALUES(34,'conversation_schema_sources',?1)", [chrono::Utc::now().to_rfc3339()])?;
    }
    tx.commit()?;
    Ok(())
}
fn owned(db: &rusqlite::Connection, project: &str, task: Uuid) -> Result<(), StorageError> {
    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.project_id=?2)", params![task.to_string(),project], |row| row.get(0))?;
    if !exists {
        return Err(invalid("Schema task does not belong to this Project"));
    }
    Ok(())
}
fn read(
    db: &rusqlite::Connection,
    project: &str,
    id: Uuid,
    revision: Option<u64>,
) -> Result<ConversationSchemaDraft, StorageError> {
    let metadata = db.query_row("SELECT task_id,source_call_id,source_request_id,base_schema_revision FROM conversation_schema_drafts WHERE id=?1",[id.to_string()],|row| Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,Option<String>>(2)?,row.get::<_,String>(3)?))).optional()?;
    let (task, call, request, base_schema_revision) =
        metadata.ok_or_else(|| invalid("Schema Draft not found"))?;
    let task_id = Uuid::parse_str(&task).map_err(|_| invalid("invalid task ID"))?;
    owned(db, project, task_id)?;
    let revision = revision
        .map(i64::try_from)
        .transpose()
        .map_err(|_| invalid("Schema revision overflow"))?;
    let (saved_revision,definition): (i64,String) = db.query_row("SELECT revision,definition_json FROM conversation_schema_revisions WHERE draft_id=?1 AND (?2 IS NULL OR revision=?2) ORDER BY revision DESC LIMIT 1",params![id.to_string(),revision],|row| Ok((row.get(0)?,row.get(1)?)))?;
    Ok(ConversationSchemaDraft {
        id,
        task_id,
        source_call_id: call
            .map(|value| Uuid::parse_str(&value))
            .transpose()
            .map_err(|_| invalid("invalid source call ID"))?,
        source_request_id: request
            .map(|value| Uuid::parse_str(&value))
            .transpose()
            .map_err(|_| invalid("invalid source request ID"))?,
        base_schema_revision,
        revision: u64::try_from(saved_revision).map_err(|_| invalid("Invalid Schema revision"))?,
        definition: serde_json::from_str(&definition)?,
    })
}
impl SqliteStore {
    pub fn human_conversation_schema_drafts(
        &self,
        project: &str,
        task: Uuid,
    ) -> Result<Vec<ConversationSchemaDraft>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, task)?;
            let mut statement = db.prepare("SELECT id FROM conversation_schema_drafts WHERE task_id=?1 AND source_request_id IS NOT NULL ORDER BY rowid DESC")?;
            let ids = statement.query_map([task.to_string()], |row| row.get::<_, String>(0))?;
            ids.map(|id| read(db, project, Uuid::parse_str(&id?).map_err(|_| invalid("invalid Schema ID"))?, None)).collect()
        })
    }

    /// Application validates the human definition with the same Core rules as model proposals.
    /// This operation does not create a grant, model call, Workflow or formal annotation.
    pub fn create_human_conversation_schema_draft(
        &self,
        project: &str,
        task: Uuid,
        request: Uuid,
        definition: &ConversationSchemaDefinition,
    ) -> Result<ConversationSchemaDraft, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            owned(&tx, project, task)?;
            let existing: Option<String> = tx.query_row("SELECT id FROM conversation_schema_drafts WHERE source_request_id=?1", [request.to_string()], |row| row.get(0)).optional()?;
            if let Some(existing) = existing {
                let id = Uuid::parse_str(&existing).map_err(|_| invalid("invalid Schema ID"))?;
                let initial = read(&tx, project, id, Some(1))?;
                if initial.task_id != task || &initial.definition != definition {
                    return Err(invalid("Schema creation conflicts with saved human input"));
                }
                return read(&tx, project, id, None);
            }
            let base: String = tx.query_row("SELECT schema_revision FROM conversation_tasks WHERE id=?1", [task.to_string()], |row| row.get(0))?;
            let id = Uuid::new_v4();
            tx.execute("INSERT INTO conversation_schema_drafts(id,task_id,source_request_id,base_schema_revision) VALUES(?1,?2,?3,?4)", params![id.to_string(),task.to_string(),request.to_string(),base])?;
            tx.execute("INSERT INTO conversation_schema_revisions(draft_id,revision,request_id,definition_json) VALUES(?1,1,?2,?3)", params![id.to_string(),request.to_string(),serde_json::to_string(definition)?])?;
            tx.commit()?;
            read(db, project, id, None)
        })
    }
    pub fn conversation_schema_for_call(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationSchemaDraft>, StorageError> {
        self.with_connection(|db| {
            owned(db, project, task)?;
            let id: Option<String> = db.query_row("SELECT id FROM conversation_schema_drafts WHERE task_id=?1 AND source_call_id=?2", params![task.to_string(),call.to_string()], |row| row.get(0)).optional()?;
            id.map(|id| read(db, project, Uuid::parse_str(&id).map_err(|_| invalid("invalid Schema ID"))?, None)).transpose()
        })
    }
    /// Application validates the completed proposal and Core `TaskConfig` before calling.
    pub fn create_conversation_schema_draft(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
        definition: &ConversationSchemaDefinition,
    ) -> Result<ConversationSchemaDraft, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?; owned(&tx,project,task)?;
            let existing: Option<String> = tx.query_row("SELECT id FROM conversation_schema_drafts WHERE source_call_id=?1",[call.to_string()],|row| row.get(0)).optional()?;
            if let Some(existing) = existing {
                let id = Uuid::parse_str(&existing).map_err(|_| invalid("invalid schema ID"))?;
                let initial = read(&tx,project,id,Some(1))?;
                if initial.task_id != task || &initial.definition != definition { return Err(invalid("Schema creation conflicts with saved model proposal")); }
                return read(&tx,project,id,None);
            }
            let call_owned: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_model_calls WHERE id=?1 AND task_id=?2 AND status='completed')",params![call.to_string(),task.to_string()],|row| row.get(0))?;
            if !call_owned { return Err(invalid("Schema Draft requires this task's completed model call")); }
            let base: String = tx.query_row("SELECT schema_revision FROM conversation_tasks WHERE id=?1",[task.to_string()],|row| row.get(0))?;
            let id = Uuid::new_v4();
            tx.execute("INSERT INTO conversation_schema_drafts(id,task_id,source_call_id,base_schema_revision) VALUES(?1,?2,?3,?4)",params![id.to_string(),task.to_string(),call.to_string(),base])?;
            tx.execute("INSERT INTO conversation_schema_revisions(draft_id,revision,request_id,definition_json) VALUES(?1,1,?2,?3)",params![id.to_string(),call.to_string(),serde_json::to_string(definition)?])?;
            tx.commit()?; read(db,project,id,None)
        })
    }
    pub fn conversation_schema_draft(
        &self,
        project: &str,
        id: Uuid,
        revision: Option<u64>,
    ) -> Result<ConversationSchemaDraft, StorageError> {
        self.with_connection(|db| read(db, project, id, revision))
    }
    pub fn revise_conversation_schema_draft(
        &self,
        project: &str,
        id: Uuid,
        request: Uuid,
        expected_revision: u64,
        definition: &ConversationSchemaDefinition,
    ) -> Result<ConversationSchemaDraft, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let current = read(&tx,project,id,None)?;
            let existing: Option<(String,i64,String)> = tx.query_row("SELECT draft_id,revision,definition_json FROM conversation_schema_revisions WHERE request_id=?1",[request.to_string()],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
            if let Some((draft,revision,body)) = existing {
                let revision = u64::try_from(revision).map_err(|_| invalid("Invalid Schema revision"))?;
                if draft != id.to_string() || expected_revision.checked_add(1) != Some(revision) || serde_json::from_str::<ConversationSchemaDefinition>(&body)? != *definition { return Err(invalid("Schema edit idempotency key conflicts")); }
                return read(&tx,project,id,Some(revision));
            }
            if current.revision != expected_revision { return Err(invalid("Schema Draft changed; retain edits and reload before saving")); }
            if current.definition.task.id != definition.task.id { return Err(invalid("Schema task identity cannot change on edit")); }
            let next = expected_revision.checked_add(1).ok_or_else(|| invalid("Schema revision overflow"))?;
            let stored_next = i64::try_from(next).map_err(|_| invalid("Schema revision overflow"))?;
            tx.execute("INSERT INTO conversation_schema_revisions(draft_id,revision,request_id,definition_json) VALUES(?1,?2,?3,?4)",params![id.to_string(),stored_next,request.to_string(),serde_json::to_string(definition)?])?;
            tx.commit()?; read(db,project,id,Some(next))
        })
    }
}
