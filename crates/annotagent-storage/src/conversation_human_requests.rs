//! Durable human correction checkpoint, reusing Sandbox feedback and its validation.
use crate::{SampleFeedbackRevision, SqliteStore, StorageError};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationHumanRequestInput {
    pub id: Uuid,
    pub task_id: Uuid,
    pub conversation_id: Uuid,
    pub sample_test_id: String,
    pub image_id: String,
    pub content_hash: String,
    pub outcome_id: String,
    pub expected_feedback_sequence: u64,
    pub reason_code: String,
    pub question: String,
    /// Stable operation/checkpoint identity, not executable code or an arbitrary URL.
    pub resume_checkpoint_ref: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeginConversationTask, ConversationMessageInput, SampleOperation};

    fn setup(
        store: &SqliteStore,
    ) -> (
        String,
        ConversationHumanRequestInput,
        SampleFeedbackRevision,
    ) {
        let answer = crate::sample_feedback::tests::fixture(store);
        let owner = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&owner).unwrap();
        let message = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST correct this example".into(),
            image: None,
        };
        store
            .append_conversation_message(&owner, conversation, &message)
            .unwrap();
        let task = BeginConversationTask {
            id: Uuid::new_v4(),
            source_message_id: message.id,
            schema_revision: "a".repeat(64),
        };
        store
            .begin_conversation_task(&owner, conversation, &task)
            .unwrap();
        store.reserve_sample_operation(&SampleOperation { id:answer.sample_test_id.clone(),project_id:"project-1".into(),draft_id:"draft-1".into(),authorization_fingerprint:"TEST".into(),request:serde_json::json!({"conversation":{"conversation_id":conversation,"task_id":task.id}}),status:"queued".into(),error:None,created_at:chrono::Utc::now().to_rfc3339(),updated_at:chrono::Utc::now().to_rfc3339() }).unwrap();
        let input = ConversationHumanRequestInput {
            id: Uuid::new_v4(),
            task_id: task.id,
            conversation_id: conversation,
            sample_test_id: answer.sample_test_id.clone(),
            image_id: answer.image_id.clone(),
            content_hash: "content-hash".into(),
            outcome_id: answer.outcome_id.clone().unwrap(),
            expected_feedback_sequence: 0,
            reason_code: "poor_boundary".into(),
            question: "TEST: correct this boundary".into(),
            resume_checkpoint_ref: Uuid::new_v4(),
        };
        (owner, input, answer)
    }

    #[test]
    fn human_answer_and_resume_event_survive_restart_and_duplicate_delivery() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("TEST.db");
        let store = SqliteStore::open(&path).unwrap();
        let (owner, input, answer) = setup(&store);
        let original = store
            .get_workflow_sample_test_by_id(&input.sample_test_id)
            .unwrap();
        let pending = store
            .create_conversation_human_request(&owner, &input)
            .unwrap();
        assert_eq!(pending.status, ConversationHumanRequestStatus::Pending);
        assert_eq!(
            store
                .create_conversation_human_request(&owner, &input)
                .unwrap(),
            pending
        );
        assert!(
            store
                .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
                .unwrap()
                .is_empty()
        );
        let answered = store
            .answer_conversation_human_request(&owner, input.id, &answer)
            .unwrap();
        assert_eq!(answered.status, ConversationHumanRequestStatus::Answered);
        let event = store
            .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
            .unwrap()
            .remove(0);
        assert!(
            store
                .complete_conversation_plan_resume(&owner, &event, "unrelated-draft")
                .is_err()
        );
        assert_eq!(
            store.conversation_human_request(&owner, input.id).unwrap(),
            answered
        );
        assert_eq!(
            store
                .answer_conversation_human_request(&owner, input.id, &answer)
                .unwrap(),
            answered
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .conversation_human_requests(&owner, input.conversation_id, input.task_id)
                .unwrap(),
            vec![answered]
        );
        let events = store
            .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            store
                .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
                .unwrap(),
            events
        );
        let mut forged = events[0].clone();
        forged.checkpoint_ref = Uuid::new_v4();
        assert!(
            store
                .acknowledge_conversation_resume(&owner, &forged)
                .is_err()
        );
        store
            .acknowledge_conversation_resume(&owner, &events[0])
            .unwrap();
        store
            .acknowledge_conversation_resume(&owner, &events[0])
            .unwrap();
        assert!(
            store
                .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .answer_conversation_human_request(&owner, input.id, &answer)
                .unwrap()
                .status,
            ConversationHumanRequestStatus::Applied
        );
        assert_eq!(
            store
                .sample_feedback(&input.sample_test_id, &input.image_id)
                .unwrap(),
            vec![answer]
        );
        assert_eq!(
            store
                .get_workflow_sample_test_by_id(&input.sample_test_id)
                .unwrap(),
            original
        );
    }

    #[test]
    fn human_answer_rejects_foreign_stale_and_invalid_subjects_without_partial_writes() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, input, answer) = setup(&store);
        assert!(
            store
                .create_conversation_human_request("foreign", &input)
                .is_err()
        );
        let mut bad = input.clone();
        bad.content_hash = "changed".into();
        assert!(
            store
                .create_conversation_human_request(&owner, &bad)
                .is_err()
        );
        store
            .create_conversation_human_request(&owner, &input)
            .unwrap();
        assert!(
            store
                .answer_conversation_human_request("foreign", input.id, &answer)
                .is_err()
        );
        let mut invalid = answer.clone();
        assert!(
            serde_json::from_value::<annotagent_core::VisionArtifactValue>(
                serde_json::json!({"kind":"bounding_box","rect":[0.5,0.5,0.0,0.1]})
            )
            .is_err()
        );
        invalid.corrected_value = Some(
            serde_json::from_value(serde_json::json!({"kind":"classification","labels":["other"]}))
                .unwrap(),
        );
        assert!(
            store
                .answer_conversation_human_request(&owner, input.id, &invalid)
                .is_err()
        );
        assert!(
            store
                .sample_feedback(&input.sample_test_id, &input.image_id)
                .unwrap()
                .is_empty()
        );
        // Another tab edits the sample; this frozen request may not overwrite it.
        let mut other = answer.clone();
        other.revision_id = "other-tab".into();
        store.save_sample_feedback(&other).unwrap();
        assert!(
            store
                .answer_conversation_human_request(&owner, input.id, &answer)
                .is_err()
        );
        assert_eq!(
            store
                .conversation_human_request(&owner, input.id)
                .unwrap()
                .status,
            ConversationHumanRequestStatus::Pending
        );
        assert!(
            store
                .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .sample_feedback(&input.sample_test_id, &input.image_id)
                .unwrap(),
            vec![other]
        );
    }

    #[test]
    fn feedback_transaction_rolls_back_when_resume_outbox_cannot_be_written() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, input, answer) = setup(&store);
        store
            .create_conversation_human_request(&owner, &input)
            .unwrap();
        store.with_connection(|db| { db.execute_batch("CREATE TRIGGER fail_test_outbox BEFORE INSERT ON conversation_resume_outbox BEGIN SELECT RAISE(ABORT,'TEST outbox unavailable'); END;")?; Ok(()) }).unwrap();
        assert!(
            store
                .answer_conversation_human_request(&owner, input.id, &answer)
                .is_err()
        );
        assert!(
            store
                .sample_feedback(&input.sample_test_id, &input.image_id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .conversation_human_request(&owner, input.id)
                .unwrap()
                .status,
            ConversationHumanRequestStatus::Pending
        );
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_test_outbox;")?;
                Ok(())
            })
            .unwrap();
        store
            .answer_conversation_human_request(&owner, input.id, &answer)
            .unwrap();
        assert_eq!(
            store
                .sample_feedback(&input.sample_test_id, &input.image_id)
                .unwrap(),
            vec![answer]
        );
    }

    #[test]
    fn waiting_blocks_new_spending_and_closed_requests_reject_late_answers() {
        for stale in [false, true] {
            let store = SqliteStore::open_in_memory().unwrap();
            let (owner, input, answer) = setup(&store);
            let grant = crate::ConversationCallGrant {
                id: Uuid::new_v4(),
                task_id: input.task_id,
                scope_hash: "a".repeat(64),
                maximum_calls: 2,
                expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
            };
            store.authorize_conversation_calls(&owner, &grant).unwrap();
            store
                .create_conversation_human_request(&owner, &input)
                .unwrap();
            assert!(
                store
                    .reserve_conversation_call(
                        &owner,
                        input.task_id,
                        Uuid::new_v4(),
                        &grant.scope_hash,
                        &"b".repeat(64)
                    )
                    .is_err()
            );
            assert!(
                store
                    .conversation_call_history(&owner, input.task_id)
                    .unwrap()
                    .is_empty()
            );
            let closed = store
                .close_conversation_human_request(&owner, input.id, stale)
                .unwrap();
            assert_eq!(
                store
                    .close_conversation_human_request(&owner, input.id, stale)
                    .unwrap(),
                closed
            );
            assert!(
                store
                    .answer_conversation_human_request(&owner, input.id, &answer)
                    .is_err()
            );
            // Rejection occurred after the feedback INSERT, but the transaction rolls it back.
            assert!(
                store
                    .sample_feedback(&input.sample_test_id, &input.image_id)
                    .unwrap()
                    .is_empty()
            );
            assert!(
                store
                    .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
                    .unwrap()
                    .is_empty()
            );
            assert!(matches!(
                store
                    .reserve_conversation_call(
                        &owner,
                        input.task_id,
                        Uuid::new_v4(),
                        &grant.scope_hash,
                        &"b".repeat(64)
                    )
                    .unwrap(),
                crate::ConversationCallAdmission::Admitted
            ));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationHumanRequestStatus {
    Pending,
    Answered,
    Applied,
    Cancelled,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationHumanRequest {
    pub input: ConversationHumanRequestInput,
    pub status: ConversationHumanRequestStatus,
    pub answer: Option<SampleFeedbackRevision>,
    pub resume_draft_id: Option<String>,
    pub resume_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationResumeEvent {
    pub request_id: Uuid,
    pub task_id: Uuid,
    pub checkpoint_ref: Uuid,
    pub feedback_revision_id: String,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}
fn owned(
    db: &Connection,
    project: &str,
    task: Uuid,
    conversation: Uuid,
) -> Result<(), StorageError> {
    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks t JOIN project_conversations c ON c.id=t.conversation_id WHERE t.id=?1 AND c.project_id=?2 AND c.id=?3)",params![task.to_string(),project,conversation.to_string()],|row|row.get(0))?;
    if !exists {
        return Err(invalid(
            "Human request task does not belong to this Project and conversation",
        ));
    }
    Ok(())
}
fn read(
    db: &Connection,
    project: &str,
    id: Uuid,
) -> Result<ConversationHumanRequest, StorageError> {
    let (input, status, answer): (String, String, Option<String>) = db.query_row(
        "SELECT request_json,status,answer_json FROM conversation_human_requests WHERE id=?1",
        [id.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let input: ConversationHumanRequestInput = serde_json::from_str(&input)?;
    owned(db, project, input.task_id, input.conversation_id)?;
    let result: Option<(Option<String>, Option<String>)> = db
        .query_row(
            "SELECT draft_id,error FROM conversation_resume_results WHERE request_id=?1",
            [id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (resume_draft_id, resume_error) = result.unwrap_or_default();
    Ok(ConversationHumanRequest {
        resume_draft_id,
        resume_error,
        input,
        status: serde_json::from_value(serde_json::Value::String(status))?,
        answer: answer
            .map(|value| serde_json::from_str(&value))
            .transpose()?,
    })
}

impl SqliteStore {
    /// A superseded or cancelled request cannot accept a late answer. No event is sent.
    pub fn close_conversation_human_request(
        &self,
        project: &str,
        id: Uuid,
        stale: bool,
    ) -> Result<ConversationHumanRequest, StorageError> {
        self.with_connection(|db| {
            let tx = db.unchecked_transaction()?;
            let current = read(&tx, project, id)?;
            let target = if stale {
                ConversationHumanRequestStatus::Stale
            } else {
                ConversationHumanRequestStatus::Cancelled
            };
            if current.status == target {
                return Ok(current);
            }
            if current.status != ConversationHumanRequestStatus::Pending {
                return Err(invalid("Only a pending human request may be closed"));
            }
            tx.execute(
                "UPDATE conversation_human_requests SET status=?2 WHERE id=?1",
                params![id.to_string(), if stale { "stale" } else { "cancelled" }],
            )?;
            tx.commit()?;
            read(db, project, id)
        })
    }
    /// Application checks the current image hash and terminal projection before admission.
    /// Storage also binds the sample to an authorized operation of this exact task.
    pub fn create_conversation_human_request(
        &self,
        project: &str,
        input: &ConversationHumanRequestInput,
    ) -> Result<ConversationHumanRequest, StorageError> {
        if input.question.trim().is_empty()
            || input.question.len() > 4000
            || input.reason_code.trim().is_empty()
            || input.reason_code.len() > 128
        {
            return Err(invalid(
                "Human request requires a bounded question and reason",
            ));
        }
        let sample = self
            .get_workflow_sample_test_by_id(&input.sample_test_id)?
            .ok_or_else(|| invalid("Sample Test not found"))?;
        let index = sample
            .inputs
            .iter()
            .position(|image| {
                image.image_id == input.image_id && image.content_hash == input.content_hash
            })
            .ok_or_else(|| invalid("Human request image does not match the saved sample"))?;
        if !sample.report.samples.get(index).is_some_and(|result| {
            result
                .outcomes
                .iter()
                .any(|outcome| outcome.id == input.outcome_id)
        }) {
            return Err(invalid(
                "Human request outcome does not belong to the sample image",
            ));
        }
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            owned(&tx,project,input.task_id,input.conversation_id)?;
            let exists: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_human_requests WHERE id=?1)",[input.id.to_string()],|row|row.get(0))?;
            if exists { let saved=read(&tx,project,input.id)?; if saved.input!=*input { return Err(invalid("Human request idempotency conflict")); } return Ok(saved); }
            let associated: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sample_operations WHERE id=?1 AND project_id=?2 AND json_extract(request_json,'$.conversation.task_id')=?3 AND json_extract(request_json,'$.conversation.conversation_id')=?4)",params![input.sample_test_id,sample.project_id,input.task_id.to_string(),input.conversation_id.to_string()],|row|row.get(0))?;
            if !associated { return Err(invalid("Sample Test is not linked to this conversation task")); }
            let sequence: i64=tx.query_row("SELECT COALESCE(MAX(sequence),0) FROM sample_feedback_revisions WHERE sample_test_id=?1 AND image_id=?2",params![input.sample_test_id,input.image_id],|row|row.get(0))?;
            if u64::try_from(sequence).ok()!=Some(input.expected_feedback_sequence) { return Err(invalid("Sample feedback changed before the request was saved")); }
            tx.execute("INSERT INTO conversation_human_requests VALUES(?1,?2,?3,'pending',NULL,?4)",params![input.id.to_string(),input.task_id.to_string(),serde_json::to_string(input)?,chrono::Utc::now().to_rfc3339()])?;
            tx.commit()?; read(db,project,input.id)
        })
    }

    pub fn conversation_human_request(
        &self,
        project: &str,
        id: Uuid,
    ) -> Result<ConversationHumanRequest, StorageError> {
        self.with_connection(|db| read(db, project, id))
    }

    pub fn conversation_human_requests(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<ConversationHumanRequest>, StorageError> {
        self.with_connection(|db| {
            owned(db,project,task,conversation)?;
            let mut statement=db.prepare("SELECT id FROM conversation_human_requests WHERE task_id=?1 ORDER BY created_at,id")?;
            let rows=statement.query_map([task.to_string()],|row|row.get::<_,String>(0))?;
            rows.map(|id|read(db,project,Uuid::parse_str(&id?).map_err(|_|invalid("Invalid saved request ID"))?)).collect()
        })
    }

    /// Save actual feedback, answer and durable resume event together, or save none.
    /// This does not dispatch a model, publish or accept a formal annotation.
    pub fn answer_conversation_human_request(
        &self,
        project: &str,
        id: Uuid,
        answer: &SampleFeedbackRevision,
    ) -> Result<ConversationHumanRequest, StorageError> {
        let request = self.conversation_human_request(project, id)?;
        let input = &request.input;
        if answer.sample_test_id != input.sample_test_id
            || answer.image_id != input.image_id
            || answer.outcome_id.as_deref() != Some(&input.outcome_id)
            || answer.addition_id.is_some()
            || input.expected_feedback_sequence.checked_add(1) != Some(answer.sequence)
        {
            return Err(invalid(
                "Human answer does not match the requested subject and revision",
            ));
        }
        self.save_sample_feedback_with(answer, |tx| {
            let current=read(tx,project,id)?;
            if let Some(saved)=current.answer { if saved==*answer { return Ok(()); } return Err(invalid("Human answer conflicts with an already saved answer")); }
            if current.status!=ConversationHumanRequestStatus::Pending { return Err(invalid("Human request is no longer pending")); }
            tx.execute("UPDATE conversation_human_requests SET status='answered',answer_json=?2 WHERE id=?1",params![id.to_string(),serde_json::to_string(answer)?])?;
            tx.execute("INSERT INTO conversation_resume_outbox(request_id,task_id,checkpoint_ref,feedback_revision_id) VALUES(?1,?2,?3,?4)",params![id.to_string(),input.task_id.to_string(),input.resume_checkpoint_ref.to_string(),answer.revision_id])?;
            Ok(())
        })?;
        self.conversation_human_request(project, id)
    }

    /// Pending delivery survives restart. Consumers resume with the same operation key;
    /// delivery is at-least-once and must never be treated as fresh spending authority.
    pub fn pending_conversation_resumes(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
    ) -> Result<Vec<ConversationResumeEvent>, StorageError> {
        self.with_connection(|db| {
            owned(db,project,task,conversation)?;
            let mut statement=db.prepare("SELECT request_id,checkpoint_ref,feedback_revision_id FROM conversation_resume_outbox WHERE task_id=?1 AND applied_at IS NULL ORDER BY request_id")?;
            let rows=statement.query_map([task.to_string()],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?)))?;
            rows.map(|row| { let (id,checkpoint,feedback_revision_id)=row?; Ok(ConversationResumeEvent { request_id:Uuid::parse_str(&id).map_err(|_|invalid("Invalid saved request ID"))?,task_id:task,checkpoint_ref:Uuid::parse_str(&checkpoint).map_err(|_|invalid("Invalid saved checkpoint"))?,feedback_revision_id }) }).collect()
        })
    }

    /// Only the coordinator calls this after durably accepting the same checkpoint.
    pub fn acknowledge_conversation_resume(
        &self,
        project: &str,
        event: &ConversationResumeEvent,
    ) -> Result<(), StorageError> {
        self.acknowledge_conversation_resume_with(project, event, |_| Ok(()))
    }

    pub fn complete_conversation_plan_resume(
        &self,
        project: &str,
        event: &ConversationResumeEvent,
        draft_id: &str,
    ) -> Result<(), StorageError> {
        self.acknowledge_conversation_resume_with(project,event, |tx| {
            let request=read(tx,project,event.request_id)?;
            let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sample_plan_revisions WHERE draft_id=?1 AND sample_test_id=?2 AND json_array_length(feedback_json)=1 AND json_extract(feedback_json,'$[0].revision_id')=?3)",params![draft_id,request.input.sample_test_id,event.feedback_revision_id],|row|row.get(0))?;
            if draft_id!=event.checkpoint_ref.to_string() || !valid { return Err(invalid("Resume Draft does not carry this checkpoint's exact feedback")); }
            tx.execute("INSERT INTO conversation_resume_results(request_id,draft_id,error) VALUES(?1,?2,NULL) ON CONFLICT(request_id) DO UPDATE SET draft_id=excluded.draft_id,error=NULL",params![event.request_id.to_string(),draft_id])?;
            Ok(())
        })
    }

    pub fn record_conversation_resume_failure(
        &self,
        project: &str,
        id: Uuid,
        error: &str,
    ) -> Result<(), StorageError> {
        self.with_connection(|db|{
            let tx=db.unchecked_transaction()?;let request=read(&tx,project,id)?;
            // A competing delivery may already have committed success. Its immutable
            // result wins over this late failure and the caller can restore it.
            if request.status==ConversationHumanRequestStatus::Applied {return Ok(());}
            if request.status!=ConversationHumanRequestStatus::Answered {return Err(invalid("Only an answered request can record a continuation failure"));}
            let bounded:String=error.chars().take(1600).collect();
            tx.execute("INSERT INTO conversation_resume_results(request_id,draft_id,error) VALUES(?1,NULL,?2) ON CONFLICT(request_id) DO UPDATE SET error=excluded.error WHERE draft_id IS NULL",params![id.to_string(),bounded])?;
            tx.commit()?;Ok(())
        })
    }

    /// Startup retries only delivery interrupted before any terminal attempt result.
    /// Failed preconditions require explicit retry, not a background retry loop.
    pub fn undelivered_conversation_corrections(
        &self,
    ) -> Result<Vec<(String, String, ConversationHumanRequestInput)>, StorageError> {
        self.with_connection(|db|{
            let mut statement=db.prepare("SELECT s.project_id,c.project_id,h.request_json FROM conversation_human_requests h JOIN conversation_tasks t ON t.id=h.task_id JOIN project_conversations c ON c.id=t.conversation_id JOIN conversation_resume_outbox o ON o.request_id=h.id JOIN sample_operations s ON s.id=json_extract(h.request_json,'$.sample_test_id') LEFT JOIN conversation_resume_results r ON r.request_id=h.id WHERE h.status='answered' AND o.applied_at IS NULL AND r.request_id IS NULL ORDER BY h.created_at,h.id")?;
            let rows=statement.query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?)))?;
            rows.map(|row|{let (project,owner,input)=row?;Ok((project,owner,serde_json::from_str(&input)?))}).collect()
        })
    }

    fn acknowledge_conversation_resume_with(
        &self,
        project: &str,
        event: &ConversationResumeEvent,
        after_write: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?; let request=read(&tx,project,event.request_id)?;
            if request.input.task_id!=event.task_id || request.input.resume_checkpoint_ref!=event.checkpoint_ref || request.answer.as_ref().map(|answer|&answer.revision_id)!=Some(&event.feedback_revision_id) { return Err(invalid("Resume event does not match the saved answer")); }
            if !matches!(request.status,ConversationHumanRequestStatus::Answered|ConversationHumanRequestStatus::Applied) { return Err(invalid("Human answer is not ready to apply")); }
            tx.execute("UPDATE conversation_resume_outbox SET applied_at=COALESCE(applied_at,?2) WHERE request_id=?1",params![event.request_id.to_string(),chrono::Utc::now().to_rfc3339()])?;
            tx.execute("UPDATE conversation_human_requests SET status='applied' WHERE id=?1",[event.request_id.to_string()])?;
            after_write(&tx)?;
            tx.commit()?; Ok(())
        })
    }
}
