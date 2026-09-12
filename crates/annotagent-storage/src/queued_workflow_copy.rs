//! Atomic, non-destructive working copies for explicit queued-plan edits.
use crate::{SqliteStore, StorageError};
use annotagent_core::WorkflowDraft;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuedWorkflowSource {
    pub message_id: Uuid,
    pub draft_id: String,
    pub revision: u64,
    pub content_hash: String,
    pub schema_id: Uuid,
    pub schema_revision: u64,
    pub evidence_hash: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuedWorkflowCopy {
    pub conversation_id: Uuid,
    pub task_id: Uuid,
    pub copy_id: Uuid,
    pub source: QueuedWorkflowSource,
}
fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

fn available(
    db: &rusqlite::Connection,
    project: &str,
    id: &str,
) -> Result<WorkflowDraft, StorageError> {
    let row=db.query_row("SELECT draft_json,revision,content_hash FROM workflow_drafts d WHERE id=?1 AND project_id=?2 AND deleted_at IS NULL AND archived_at IS NULL AND status NOT IN ('published','archived') AND NOT EXISTS(SELECT 1 FROM workflow_pipelines p WHERE p.workflow_id=d.id AND (p.project_id<>?2 OR p.deleted_at IS NOT NULL OR p.archived_at IS NOT NULL))",params![id,project],crate::workflow_draft_row).optional()?.ok_or_else(||invalid("Workflow source or copy is unavailable"))?;
    crate::workflow_draft_from_columns(row)
}
impl SqliteStore {
    /// Caller has validated explicit Draft-edit authority; this does not grant or
    /// execute a model call. Receipt and copy are written in one transaction.
    pub fn copy_queued_workflow(
        &self,
        owner: &str,
        project: &str,
        input: &QueuedWorkflowCopy,
    ) -> Result<WorkflowDraft, StorageError> {
        if input.copy_id.is_nil() || input.copy_id.to_string() == input.source.draft_id {
            return Err(invalid("A working copy requires a distinct identity"));
        }
        self.with_connection(|db|{
            let tx=db.unchecked_transaction()?;
            crate::conversation_message_queue::require_task(&tx,owner,input.conversation_id,input.task_id)?;
            let queued=crate::conversation_message_queue::read(&tx,input.conversation_id,input.task_id,input.source.message_id)?;
            let saved:Option<String>=tx.query_row("SELECT request_json FROM conversation_queued_workflow_copies WHERE copy_id=?1",[input.copy_id.to_string()],|row|row.get(0)).optional()?;
            if let Some(saved)=saved {
                if serde_json::from_str::<QueuedWorkflowCopy>(&saved)?!=*input{return Err(invalid("Copy retry changed its admitted source"));}
                return available(&tx,project,&input.copy_id.to_string());
            }
            if queued.cancelled_at.is_some(){return Err(invalid("Cancelled queued instruction cannot create a new copy"));}
            let mut draft=available(&tx,project,&input.source.draft_id)?;
            if draft.revision!=input.source.revision||draft.content_hash!=input.source.content_hash{return Err(invalid("Workflow changed before copy admission"));}
            let binding=draft.annotation_schema.as_ref().ok_or_else(||invalid("Source has no stable Schema owner"))?;
            if binding.schema_draft_id!=input.source.schema_id.to_string()||binding.revision!=input.source.schema_revision{return Err(invalid("Copy Schema identity differs from selected source"));}
            let same_task:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_schema_drafts WHERE id=?1 AND task_id=?2)",params![input.source.schema_id.to_string(),input.task_id.to_string()],|row|row.get(0))?;
            if !same_task{return Err(invalid("Source Schema belongs to another task"));}
            let evidence:Option<(String,String,String)>=tx.query_row("SELECT project_id,sample_test_id,feedback_json FROM sample_plan_revisions WHERE draft_id=?1",[&input.source.draft_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
            let digest=evidence.as_ref().map(|(p,test,feedback)|->Result<String,StorageError>{
                if p!=project{return Err(invalid("Sample evidence belongs to another Project"));}
                let value=serde_json::json!({"project_id":p,"sample_test_id":test,"feedback":serde_json::from_str::<serde_json::Value>(feedback)?});
                Ok(annotagent_image_tools::sha256(&serde_json::to_vec(&value)?))
            }).transpose()?;
            if digest!=input.source.evidence_hash{return Err(invalid("Sample evidence changed before copy admission"));}
            draft.id=input.copy_id.to_string();draft.name=format!("{} · working copy",draft.name);draft.status=annotagent_core::WorkflowDraftStatus::Editing;
            draft.created_at=chrono::Utc::now();draft.updated_at=draft.created_at;
            let draft=crate::persisted_workflow_draft(&draft,None)?;
            // INSERT, never UPSERT: a colliding id must not overwrite any user Draft.
            tx.execute("INSERT INTO workflow_drafts(id,project_id,status,draft_json,created_at,updated_at,revision,content_hash) VALUES(?1,?2,'editing',?3,?4,?4,?5,?6)",params![draft.id,project,serde_json::to_string(&draft)?,draft.created_at.to_rfc3339(),i64::try_from(draft.revision).map_err(|_|invalid("Copy revision overflow"))?,draft.content_hash])?;
            if let Some((p,test,feedback))=evidence {
                tx.execute("INSERT INTO sample_plan_revisions(draft_id,project_id,sample_test_id,feedback_json,created_at) VALUES(?1,?2,?3,?4,?5)",params![draft.id,p,test,feedback,draft.created_at.to_rfc3339()])?;
            }
            tx.execute("INSERT INTO conversation_queued_workflow_copies(copy_id,conversation_id,task_id,message_id,request_json) VALUES(?1,?2,?3,?4,?5)",params![draft.id,input.conversation_id.to_string(),input.task_id.to_string(),input.source.message_id.to_string(),serde_json::to_string(input)?])?;
            tx.commit()?;Ok(draft)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copy_receipt_failure_rolls_back_and_collision_never_overwrites() {
        let temp = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(temp.path().join("TEST-copy.sqlite")).unwrap();
        let project_id = Uuid::new_v4().to_string();
        let project = project_id.as_str();
        let conversation = store.create_conversation(project).unwrap();
        let mut send = crate::ConversationSendInput {
            message: crate::ConversationMessageInput {
                id: Uuid::new_v4(),
                text: "TEST cups".into(),
                image: None,
                reference: None,
            },
            task_images: vec![],
            task_id: None,
            schema_revision: "a".repeat(64),
            agent_model: None,
            mode: Some(crate::ConversationSendMode::Plan),
        };
        let root = store
            .send_conversation_message(project, conversation, &send)
            .unwrap();
        let task = root.task_id;
        let definition = crate::ConversationSchemaDefinition {
            goal: "TEST cups".into(),
            task: serde_json::from_value(
                serde_json::json!({"id":"TEST-objects","kind":"bounding_box","labels":["cup"]}),
            )
            .unwrap(),
            boundary_rules: vec![],
        };
        let schema = store
            .create_human_conversation_schema_draft(project, task, Uuid::new_v4(), &definition)
            .unwrap();
        send.task_id = Some(task);
        send.message.id = Uuid::new_v4();
        store
            .send_conversation_message(project, conversation, &send)
            .unwrap();
        let draft:WorkflowDraft=serde_json::from_value(serde_json::json!({"id":"TEST-original","project_id":project,"name":"TEST plan","status":"editing","nodes":[],"annotation_schema":{"schema_draft_id":schema.id,"revision":1,"goal":definition.goal,"task":definition.task,"boundary_rules":[]},"created_at":chrono::Utc::now(),"updated_at":chrono::Utc::now()})).unwrap();
        store.save_workflow_draft(&draft).unwrap();
        let draft = store.get_workflow_draft(&draft.id).unwrap();
        // Synthetic saved reference metadata, not a claim of live sample execution.
        store.with_connection(|db|{db.execute("INSERT INTO sample_plan_revisions(draft_id,project_id,sample_test_id,feedback_json,created_at) VALUES(?1,?2,'TEST-saved-sample','[]',?3)",params![draft.id,project,chrono::Utc::now().to_rfc3339()])?;Ok(())}).unwrap();
        let evidence = store.sample_plan_evidence(&draft.id).unwrap().unwrap();
        let input = QueuedWorkflowCopy {
            conversation_id: conversation,
            task_id: task,
            copy_id: Uuid::new_v4(),
            source: QueuedWorkflowSource {
                message_id: send.message.id,
                draft_id: draft.id.clone(),
                revision: draft.revision,
                content_hash: draft.content_hash.clone(),
                schema_id: schema.id,
                schema_revision: 1,
                evidence_hash: Some(annotagent_image_tools::sha256(
                    &serde_json::to_vec(&evidence).unwrap(),
                )),
            },
        };
        store.with_connection(|db|{db.execute_batch("CREATE TRIGGER fail_copy_receipt BEFORE INSERT ON conversation_queued_workflow_copies BEGIN SELECT RAISE(ABORT,'TEST receipt failure'); END;")?;Ok(())}).unwrap();
        assert!(
            store
                .copy_queued_workflow(project, project, &input)
                .is_err()
        );
        assert!(
            store
                .get_workflow_draft_optional(&input.copy_id.to_string())
                .unwrap()
                .is_none()
        );
        assert_eq!(store.get_workflow_draft(&draft.id).unwrap(), draft);
        assert!(
            store
                .sample_plan_evidence(&input.copy_id.to_string())
                .unwrap()
                .is_none()
        );
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_copy_receipt")?;
                Ok(())
            })
            .unwrap();
        let mut colliding = draft.clone();
        colliding.id = input.copy_id.to_string();
        colliding.name = "TEST unrelated saved draft".into();
        store.save_workflow_draft(&colliding).unwrap();
        let collision = store.get_workflow_draft(&colliding.id).unwrap();
        assert!(
            store
                .copy_queued_workflow(project, project, &input)
                .is_err()
        );
        assert_eq!(store.get_workflow_draft(&colliding.id).unwrap(), collision);
        let clean = QueuedWorkflowCopy {
            copy_id: Uuid::new_v4(),
            ..input
        };
        assert!(store.copy_queued_workflow(project, project, &clean).is_ok());
        assert_eq!(
            store
                .sample_plan_evidence(&clean.copy_id.to_string())
                .unwrap(),
            Some(evidence)
        );
        store
            .with_connection(|db| {
                db.execute(
                    "DELETE FROM workflow_drafts WHERE id=?1",
                    [clean.copy_id.to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(
            store
                .copy_queued_workflow(project, project, &clean)
                .is_err(),
            "purged copy must not be resurrected by retry"
        );
        assert!(
            store
                .get_workflow_draft_optional(&clean.copy_id.to_string())
                .unwrap()
                .is_none()
        );
    }
}
