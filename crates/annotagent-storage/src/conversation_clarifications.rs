use crate::{SqliteStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaClarificationRef {
    pub call_id: Uuid,
    pub expected_schema_revision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaClarification {
    pub id: Uuid,
    pub task_id: Uuid,
    pub conversation_id: Uuid,
    pub source_message_id: Uuid,
    pub kind: String,
    pub question: String,
    pub reason_code: String,
    pub expected_schema_revision: String,
    pub schema_draft_id: Option<Uuid>,
    pub status: String,
}

pub(crate) fn read(
    db: &rusqlite::Connection,
    project: &str,
    task: Uuid,
    call: Uuid,
) -> Result<SchemaClarification, StorageError> {
    let (conversation,message,revision,evidence,status):(String,String,String,String,String)=db.query_row("SELECT t.conversation_id,t.source_message_id,t.schema_revision,m.evidence_json,m.status FROM conversation_model_calls m JOIN conversation_tasks t ON t.id=m.task_id JOIN project_conversations c ON c.id=t.conversation_id WHERE m.id=?1 AND t.id=?2 AND c.project_id=?3",params![call.to_string(),task.to_string(),project],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
    let evidence: serde_json::Value = serde_json::from_str(&evidence)?;
    let decision = &evidence["decision"]["Ok"];
    if status != "completed" || decision["decision"] != "clarify" {
        return Err(StorageError::InvalidConversation(
            "This call is not a completed Schema clarification".into(),
        ));
    }
    let question = decision["question"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            StorageError::InvalidConversation("Saved clarification has no question".into())
        })?;
    let draft:Option<String>=db.query_row("SELECT schema_draft_id FROM conversation_schema_clarification_answers WHERE call_id=?1",[call.to_string()],|r|r.get(0)).optional()?;
    let id = |value: String| {
        Uuid::parse_str(&value).map_err(|_| {
            StorageError::InvalidConversation("Invalid saved clarification identity".into())
        })
    };
    Ok(SchemaClarification {
        id: call,
        task_id: task,
        conversation_id: id(conversation)?,
        source_message_id: id(message)?,
        kind: "clarify_task".into(),
        question: question.into(),
        reason_code: "annotation_semantics_ambiguous".into(),
        expected_schema_revision: revision,
        status: if draft.is_some() {
            "applied"
        } else {
            "pending"
        }
        .into(),
        schema_draft_id: draft.map(id).transpose()?,
    })
}

impl SqliteStore {
    pub fn conversation_schema_clarification(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<SchemaClarification, StorageError> {
        self.with_connection(|db| read(db, project, task, call))
    }
}
