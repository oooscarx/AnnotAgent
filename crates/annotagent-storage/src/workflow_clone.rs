use crate::{SqliteStore, StorageError, persisted_workflow_draft};
use annotagent_core::{PublishedWorkflowVersion, WorkflowDraft, WorkflowDraftStatus};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCloneCommand {
    pub command_id: Uuid,
    pub project_id: String,
    pub workflow_id: String,
    pub version: u32,
    pub source_snapshot_hash: String,
}
fn error(code: &str, message: &str) -> StorageError {
    StorageError::Management {
        code: code.into(),
        message: message.into(),
    }
}
impl SqliteStore {
    /// Atomically creates one copy and its immutable command receipt. Replay
    /// returns the original creation receipt without writing the editable copy.
    pub fn clone_workflow_version_command(
        &self,
        command: &WorkflowCloneCommand,
    ) -> Result<WorkflowDraft, StorageError> {
        self.with_connection(|db| {
            let tx = rusqlite::Transaction::new_unchecked(db, TransactionBehavior::Immediate)?;
            let receipt: Option<(String,String)> = tx.query_row("SELECT request_json,result_json FROM workflow_clone_commands WHERE command_id=?1", [command.command_id.to_string()], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((request,result)) = receipt {
                if serde_json::from_str::<WorkflowCloneCommand>(&request)? != *command {
                    return Err(error("workflow_clone_command_conflict", "Clone command was already used with a different scope"));
                }
                return Ok(serde_json::from_str(&result)?);
            }
            let source: Option<String> = tx.query_row("SELECT version_json FROM workflow_versions WHERE workflow_id=?1 AND version=?2 AND project_id=?3", params![command.workflow_id,command.version,command.project_id], |r|r.get(0)).optional()?;
            let source: PublishedWorkflowVersion = serde_json::from_str(&source.ok_or_else(||error("foreign_project_object", "Published Workflow was not found in this Project"))?)?;
            if command.source_snapshot_hash.is_empty() || source.content_hash != command.source_snapshot_hash {
                return Err(error("workflow_clone_source_conflict", "Frozen snapshot differs from the confirmed hash"));
            }
            let now = chrono::Utc::now();
            let mut draft = source.draft;
            draft.id = Uuid::new_v4().to_string();
            draft.name = format!("{} (from v{})", draft.name, command.version);
            draft.status = WorkflowDraftStatus::Editing;
            draft.revision = 1;
            draft.content_hash.clear();
            draft.created_at = now;
            draft.updated_at = now;
            let draft = persisted_workflow_draft(&draft, None)?;
            let result = serde_json::to_string(&draft)?;
            tx.execute("INSERT INTO workflow_drafts(id,project_id,status,draft_json,created_at,updated_at,revision,content_hash) VALUES(?1,?2,'editing',?3,?4,?4,?5,?6)",params![draft.id,draft.project_id,result,now.to_rfc3339(),i64::try_from(draft.revision).unwrap_or(i64::MAX),draft.content_hash])?;
            tx.execute("INSERT INTO workflow_pipelines(workflow_id,project_id,display_name,lifecycle_revision,created_at,updated_at) VALUES(?1,?2,?3,1,?4,?4)",params![draft.id,draft.project_id,draft.name,now.to_rfc3339()])?;
            tx.execute("INSERT INTO workflow_clone_commands(command_id,request_json,result_json) VALUES(?1,?2,?3)",params![command.command_id.to_string(),serde_json::to_string(command)?,result])?;
            tx.commit()?;
            Ok(draft)
        })
    }
}
