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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSchemaDraft {
    pub id: Uuid,
    pub task_id: Uuid,
    pub source_call_id: Uuid,
    pub base_schema_revision: String,
    pub revision: u64,
    pub definition: ConversationSchemaDefinition,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
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
    let metadata: Option<(String,String,String)> = db.query_row("SELECT task_id,source_call_id,base_schema_revision FROM conversation_schema_drafts WHERE id=?1",[id.to_string()],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    let (task, call, base_schema_revision) =
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
        source_call_id: Uuid::parse_str(&call).map_err(|_| invalid("invalid source call ID"))?,
        base_schema_revision,
        revision: u64::try_from(saved_revision).map_err(|_| invalid("Invalid Schema revision"))?,
        definition: serde_json::from_str(&definition)?,
    })
}
impl SqliteStore {
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
