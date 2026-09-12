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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome_id: Option<String>,
    /// A new human reference has its own identity, never a fabricated model outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addition_id: Option<String>,
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

    #[test]
    fn answer_delivery_is_atomic_immutable_and_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-answer-outbox.db");
        let store = SqliteStore::open(&path).unwrap();
        let (owner, input, answer) = setup(&store);
        store
            .create_conversation_human_request(&owner, &input)
            .unwrap();
        assert!(
            store
                .answer_conversation_human_request_in_journey(
                    &owner,
                    input.id,
                    &answer,
                    Some(Uuid::new_v4())
                )
                .is_err()
        );
        assert!(
            store
                .sample_feedback(&input.sample_test_id, &input.image_id)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .conversation_human_request(&owner, input.id)
                .unwrap()
                .answer
                .is_none()
        );
        let (_, _, mut consent, _) = crate::conversation_journey::tests::setup(&store);
        consent.task_id = input.task_id;
        consent.repair_after_answer = Some(input.clone());
        // Isolate answer/outbox atomicity; real envelope preview and ownership
        // are covered by the journey and HTTP integration tests.
        store.with_connection(|db| {
            db.execute("INSERT INTO conversation_journey_consents(id,task_id,input_json,created_at,builder_operation_id,sample_operation_id) VALUES(?1,?2,?3,?4,?5,?6)",params![consent.id.to_string(),input.task_id.to_string(),serde_json::to_string(&consent)?,chrono::Utc::now().to_rfc3339(),consent.builder_operation_id.to_string(),consent.sample_operation_id.to_string()])?;
            Ok(())
        }).unwrap();
        store
            .answer_conversation_human_request_in_journey(
                &owner,
                input.id,
                &answer,
                Some(consent.id),
            )
            .unwrap();
        assert!(
            store
                .pending_conversation_answer_deliveries()
                .unwrap()
                .is_empty()
        );
        let saved = store
            .conversation_answer_delivery(&owner, input.conversation_id, input.task_id, consent.id)
            .unwrap()
            .unwrap();
        assert_eq!(saved["status"], "pending");
        assert_eq!(
            store.conversation_answer_consent(&owner, input.id).unwrap(),
            Some(consent.id)
        );
        assert!(
            store
                .conversation_answer_consent("TEST-foreign", input.id)
                .is_err()
        );
        assert_eq!(saved["feedback_revision_id"], answer.revision_id);
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        store
            .answer_conversation_human_request_in_journey(
                &owner,
                input.id,
                &answer,
                Some(consent.id),
            )
            .unwrap();
        assert_eq!(
            store
                .sample_feedback(&input.sample_test_id, &input.image_id)
                .unwrap(),
            vec![answer.clone()]
        );
        assert_eq!(
            store
                .conversation_answer_delivery(
                    &owner,
                    input.conversation_id,
                    input.task_id,
                    consent.id
                )
                .unwrap(),
            Some(saved)
        );
        let event = store
            .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
            .unwrap()
            .remove(0);
        store
            .acknowledge_conversation_resume(&owner, &event)
            .unwrap();
        assert_eq!(
            store.pending_conversation_answer_deliveries().unwrap(),
            vec![(
                "project-1".into(),
                input.conversation_id,
                input.task_id,
                consent.id
            )]
        );
        store
            .fail_conversation_answer_delivery(consent.id, "TEST changed recipient")
            .unwrap();
        assert!(
            store
                .pending_conversation_answer_deliveries()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .conversation_answer_delivery(
                    &owner,
                    input.conversation_id,
                    input.task_id,
                    consent.id
                )
                .unwrap()
                .unwrap()["error"],
            "TEST changed recipient"
        );
        store
            .answer_conversation_human_request_in_journey(
                &owner,
                input.id,
                &answer,
                Some(consent.id),
            )
            .unwrap();
        assert!(
            store
                .pending_conversation_answer_deliveries()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn new_reference_answer_is_distinct_atomic_and_idempotent() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("TEST-reference.db");
        let store = SqliteStore::open(&path).unwrap();
        let (owner, mut input, mut answer) = setup(&store);
        let prediction = store
            .get_workflow_sample_test_by_id(&input.sample_test_id)
            .unwrap();
        input.outcome_id = None;
        input.addition_id = Some(Uuid::new_v4().to_string());
        input.reason_code = "identify_target".into();
        assert!(
            store
                .create_conversation_human_request("foreign", &input)
                .is_err()
        );
        let mut ambiguous = input.clone();
        ambiguous.outcome_id = answer.outcome_id.clone();
        assert!(
            store
                .create_conversation_human_request(&owner, &ambiguous)
                .is_err()
        );
        store
            .create_conversation_human_request(&owner, &input)
            .unwrap();
        let mut duplicate = input.clone();
        duplicate.id = Uuid::new_v4();
        assert!(
            store
                .create_conversation_human_request(&owner, &duplicate)
                .is_err()
        );
        assert!(
            store
                .answer_conversation_human_request(&owner, input.id, &answer)
                .is_err()
        );
        answer.outcome_id = None;
        answer.addition_id = input.addition_id.clone();
        answer.corrected_label = Some("ball".into());
        answer.reason = crate::SampleFeedbackReason::MissingTarget;
        let saved = store
            .answer_conversation_human_request(&owner, input.id, &answer)
            .unwrap();
        assert_eq!(saved.status, ConversationHumanRequestStatus::Answered);
        assert_eq!(
            store
                .answer_conversation_human_request(&owner, input.id, &answer)
                .unwrap(),
            saved
        );
        assert_eq!(
            store
                .create_conversation_human_request(&owner, &input)
                .unwrap(),
            saved
        );
        assert_eq!(
            store
                .sample_feedback(&input.sample_test_id, &input.image_id)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .get_workflow_sample_test_by_id(&input.sample_test_id)
                .unwrap(),
            prediction
        );
        let mut conflicting = answer.clone();
        conflicting.addition_id = Some(Uuid::new_v4().to_string());
        assert!(
            store
                .answer_conversation_human_request(&owner, input.id, &conflicting)
                .is_err()
        );
        drop(store);
        let restored = SqliteStore::open(&path).unwrap();
        assert_eq!(
            restored
                .conversation_human_request(&owner, input.id)
                .unwrap(),
            saved
        );
        assert_eq!(
            restored
                .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
                .unwrap()
                .len(),
            1
        );
    }

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
            reference: None,
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
            outcome_id: answer.outcome_id.clone(),
            addition_id: None,
            expected_feedback_sequence: 0,
            reason_code: "poor_boundary".into(),
            question: "TEST: correct this boundary".into(),
            resume_checkpoint_ref: Uuid::new_v4(),
        };
        (owner, input, answer)
    }

    fn feedback_source(
        store: &SqliteStore,
    ) -> (
        String,
        ConversationHumanRequestInput,
        crate::ConversationCallReceipt,
    ) {
        let (owner, mut input, _) = setup(store);
        let call = Uuid::new_v4();
        input.id = Uuid::new_v5(&call, b"feedback-human-request-v1");
        input.resume_checkpoint_ref = Uuid::new_v5(&input.id, b"prepared-repair-draft");
        let grant = crate::ConversationCallGrant {
            id: Uuid::new_v4(),
            task_id: input.task_id,
            scope_hash: "a".repeat(64),
            maximum_calls: 1,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        };
        store.authorize_conversation_calls(&owner, &grant).unwrap();
        store
            .reserve_conversation_call(
                &owner,
                input.task_id,
                call,
                &grant.scope_hash,
                &"b".repeat(64),
            )
            .unwrap();
        let artifact = Uuid::new_v4();
        let message = crate::ConversationMessage {
            conversation_id: input.conversation_id,
            sequence: 2,
            input: crate::ConversationMessageInput {
                id: Uuid::new_v4(),
                text: "TEST this box is too big".into(),
                image: Some(crate::ConversationImageRef {
                    image_id: input.image_id.clone(),
                    sha256: input.content_hash.clone(),
                }),
                reference: Some(crate::ConversationSelectionRef::SampleCandidate {
                    task_id: input.task_id,
                    project_schema_revision: "a".repeat(64),
                    draft_id: "draft-1".into(),
                    draft_revision: 1,
                    sample_test_id: input.sample_test_id.clone(),
                    candidate_id: input.outcome_id.clone().unwrap(),
                    source_artifact_id: artifact,
                }),
            },
        };
        let response = annotagent_core::ModelResponse {
            content: None,
            tool_calls: vec![annotagent_core::ModelToolCall {
                id: "TEST proposal".into(),
                name: "propose_candidate_feedback".into(),
                arguments: serde_json::json!({"decision":"request_correction","reason":input.reason_code,"question":input.question,"rationale":"TEST saved text only"}),
            }],
            usage: annotagent_core::TokenUsage::known(10, 5, annotagent_core::UsageSource::Mock),
            request_id: Some("TEST offline response".into()),
            provider_metadata: std::collections::BTreeMap::new(),
        };
        let evidence = serde_json::json!({
            "phase":"feedback_text", "cancelled":false, "response":response,
            "context":{"contract":"conversation-feedback-v1", "subject":{
                "message":message,"expected_feedback_sequence":input.expected_feedback_sequence,"pixels_supplied":false,
                "candidate":{"source_artifact_id":artifact,"outcome":{"id":input.outcome_id,"value":{"kind":"bounding_box"}}}
            }}
        });
        let receipt = store
            .finish_conversation_call(
                &owner,
                input.task_id,
                call,
                crate::ConversationCallStatus::Completed,
                evidence,
            )
            .unwrap();
        (owner, input, receipt)
    }

    #[test]
    fn feedback_request_guard_rejects_cancel_after_receipt_read_without_writes() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, input, source) = feedback_source(&store);
        store
            .request_conversation_call_cancel(&owner, input.task_id, source.id)
            .unwrap();
        assert!(
            store
                .create_conversation_feedback_human_request(&owner, &input, &source)
                .is_err()
        );
        assert!(
            store
                .conversation_human_requests(&owner, input.conversation_id, input.task_id)
                .unwrap()
                .is_empty()
        );
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
        assert_eq!(
            store
                .conversation_call_history(&owner, input.task_id)
                .unwrap(),
            vec![source]
        );
        assert_eq!(
            store
                .conversation_call_budget(&owner, input.task_id)
                .unwrap()
                .unwrap()
                .used_calls,
            1
        );
    }

    #[test]
    fn feedback_request_guard_binds_exact_source_subject_and_proposal() {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, input, source) = feedback_source(&store);
        assert!(
            store
                .create_conversation_feedback_human_request("foreign", &input, &source)
                .is_err()
        );
        for index in 0..6 {
            let mut bad = input.clone();
            match index {
                0 => bad.question.push_str(" changed"),
                1 => bad.reason_code = "wrong_target".into(),
                2 => bad.expected_feedback_sequence += 1,
                3 => bad.id = Uuid::new_v4(),
                4 => bad.resume_checkpoint_ref = Uuid::new_v4(),
                _ => bad.task_id = Uuid::new_v4(),
            }
            assert!(
                store
                    .create_conversation_feedback_human_request(&owner, &bad, &source)
                    .is_err()
            );
        }
        let mut bad = source.clone();
        bad.evidence.as_mut().unwrap()["context"]["subject"]["expected_feedback_sequence"] =
            serde_json::json!(1);
        assert!(
            store
                .create_conversation_feedback_human_request(&owner, &input, &bad)
                .is_err()
        );
        // Even a matching saved receipt cannot authorize a different phase, unknown
        // outcome, cancellation flag, clarification decision or changed candidate.
        for index in 0..5 {
            let mut invalid_source = source.clone();
            let evidence = invalid_source.evidence.as_mut().unwrap();
            match index {
                0 => evidence["phase"] = serde_json::json!("builder_text"),
                1 => invalid_source.status = crate::ConversationCallStatus::InDoubt,
                2 => evidence["cancelled"] = serde_json::json!(true),
                3 => {
                    evidence["response"]["tool_calls"][0]["arguments"]["decision"] =
                        serde_json::json!("clarify_scope");
                }
                _ => {
                    evidence["context"]["subject"]["candidate"]["source_artifact_id"] =
                        serde_json::json!(Uuid::new_v4());
                }
            }
            store.with_connection(|db| {
                db.execute("UPDATE conversation_model_calls SET evidence_json=?2,status=?3 WHERE id=?1", params![source.id.to_string(), serde_json::to_string(&invalid_source.evidence)?, if invalid_source.status == crate::ConversationCallStatus::Completed { "completed" } else { "in_doubt" }])?;
                Ok(())
            }).unwrap();
            assert!(
                store
                    .create_conversation_feedback_human_request(&owner, &input, &invalid_source)
                    .is_err()
            );
        }
        assert!(
            store
                .conversation_human_requests(&owner, input.conversation_id, input.task_id)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn feedback_request_insert_failure_rolls_back_and_later_cancel_does_not_retract_saved_request()
    {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, input, source) = feedback_source(&store);
        store.with_connection(|db| {
            db.execute_batch("CREATE TRIGGER fail_feedback_request BEFORE INSERT ON conversation_human_requests BEGIN SELECT RAISE(ABORT,'TEST request unavailable'); END;")?;
            Ok(())
        }).unwrap();
        assert!(
            store
                .create_conversation_feedback_human_request(&owner, &input, &source)
                .is_err()
        );
        assert!(
            store
                .conversation_human_requests(&owner, input.conversation_id, input.task_id)
                .unwrap()
                .is_empty()
        );
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
        assert_eq!(
            store
                .conversation_call_history(&owner, input.task_id)
                .unwrap(),
            vec![source.clone()]
        );
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_feedback_request;")?;
                Ok(())
            })
            .unwrap();
        let saved = store
            .create_conversation_feedback_human_request(&owner, &input, &source)
            .unwrap();
        store
            .request_conversation_call_cancel(&owner, input.task_id, source.id)
            .unwrap();
        assert_eq!(saved.status, ConversationHumanRequestStatus::Pending);
        assert_eq!(
            store
                .create_conversation_feedback_human_request(&owner, &input, &source)
                .unwrap(),
            saved
        );
        let mut conflicting = input.clone();
        conflicting.question.push_str(" changed");
        assert!(
            store
                .create_conversation_feedback_human_request(&owner, &conflicting, &source)
                .is_err()
        );
        assert_eq!(
            store.conversation_human_request(&owner, input.id).unwrap(),
            saved
        );
    }

    #[test]
    fn concurrent_feedback_preparation_and_cancellation_have_one_ordering() {
        for _ in 0..8 {
            let store = std::sync::Arc::new(SqliteStore::open_in_memory().unwrap());
            let (owner, input, source) = feedback_source(&store);
            let barrier = std::sync::Barrier::new(2);
            let inserted = std::thread::scope(|scope| {
                let prepare = scope.spawn(|| {
                    barrier.wait();
                    store.create_conversation_feedback_human_request(&owner, &input, &source)
                });
                barrier.wait();
                store
                    .request_conversation_call_cancel(&owner, input.task_id, source.id)
                    .unwrap();
                prepare.join().unwrap()
            });
            let requests = store
                .conversation_human_requests(&owner, input.conversation_id, input.task_id)
                .unwrap();
            match inserted {
                Ok(saved) => {
                    assert_eq!(saved.status, ConversationHumanRequestStatus::Pending);
                    assert_eq!(requests, vec![saved.clone()]);
                    assert_eq!(
                        store
                            .create_conversation_feedback_human_request(&owner, &input, &source)
                            .unwrap(),
                        saved
                    );
                }
                Err(_) => assert!(requests.is_empty()),
            }
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
        }
    }

    #[test]
    fn deferral_survives_restart_without_answering_or_replaying_old_resume() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-deferral.db");
        let store = SqliteStore::open(&path).unwrap();
        let (owner, input, answer) = setup(&store);
        store
            .create_conversation_human_request(&owner, &input)
            .unwrap();
        let defer = ConversationHumanDeferral {
            command_id: Uuid::new_v4(),
            expected_revision: 0,
            deferred: true,
        };
        assert!(
            store
                .set_conversation_human_deferral("foreign", input.id, &defer)
                .is_err()
        );
        let saved = store
            .set_conversation_human_deferral(&owner, input.id, &defer)
            .unwrap();
        assert!(saved.deferred);
        assert_eq!(saved.status, ConversationHumanRequestStatus::Pending);
        assert!(
            store
                .answer_conversation_human_request(&owner, input.id, &answer)
                .is_err()
        );
        assert!(
            store
                .pending_conversation_resumes(&owner, input.conversation_id, input.task_id)
                .unwrap()
                .is_empty()
        );
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(
            store
                .set_conversation_human_deferral(&owner, input.id, &defer)
                .unwrap(),
            saved
        );
        let resume = ConversationHumanDeferral {
            command_id: Uuid::new_v4(),
            expected_revision: 1,
            deferred: false,
        };
        let resumed = store
            .set_conversation_human_deferral(&owner, input.id, &resume)
            .unwrap();
        assert!(!resumed.deferred);
        assert_eq!(resumed.deferral_revision, 2);
        assert_eq!(
            store
                .set_conversation_human_deferral(&owner, input.id, &defer)
                .unwrap(),
            resumed
        );
        assert!(
            store
                .set_conversation_human_deferral(
                    &owner,
                    input.id,
                    &ConversationHumanDeferral {
                        command_id: Uuid::new_v4(),
                        ..defer
                    }
                )
                .is_err()
        );
        store
            .answer_conversation_human_request(&owner, input.id, &answer)
            .unwrap();
        assert!(
            store
                .set_conversation_human_deferral(
                    &owner,
                    input.id,
                    &ConversationHumanDeferral {
                        command_id: Uuid::new_v4(),
                        expected_revision: 2,
                        deferred: true
                    }
                )
                .is_err()
        );
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
    fn queued_human_gate_rolls_back_new_grants_and_recovers_frozen_authorization() {
        use crate::{
            ConversationCallAdmission, ConversationCallGrant, ConversationSendInput,
            QueuedPlanningAuthorization,
        };
        for previously_authorized in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("TEST-queued-human.db");
            let store = SqliteStore::open(&path).unwrap();
            let (owner, human, answer) = setup(&store);
            store
                .finish_sample_operation(&human.sample_test_id, None)
                .unwrap();
            let command = ConversationSendInput {
                message: ConversationMessageInput {
                    id: Uuid::new_v4(),
                    text: "TEST queued supplement".into(),
                    image: None,
                    reference: None,
                },
                task_images: vec![],
                task_id: Some(human.task_id),
                schema_revision: "a".repeat(64),
                agent_model: None,
                mode: Some(crate::ConversationSendMode::Plan),
            };
            store
                .send_conversation_message(&owner, human.conversation_id, &command)
                .unwrap();
            let authorization = QueuedPlanningAuthorization {
                conversation_id: human.conversation_id,
                message_id: command.message.id,
                previous_grant_id: None,
                model_id: annotagent_core::ModelProfileId::new(),
                request_hash: "c".repeat(64),
                grant: ConversationCallGrant {
                    id: Uuid::new_v4(),
                    task_id: human.task_id,
                    scope_hash: "b".repeat(64),
                    maximum_calls: 1,
                    expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
                },
            };
            if previously_authorized {
                store
                    .authorize_queued_planning(&owner, &authorization)
                    .unwrap();
            }
            store
                .create_conversation_human_request(&owner, &human)
                .unwrap();
            let gate = store
                .check_queued_call_admission(
                    &owner,
                    human.conversation_id,
                    human.task_id,
                    authorization.grant.id,
                )
                .unwrap_err();
            assert!(matches!(
                gate,
                StorageError::ConversationContract {
                    code: "human_input_pending",
                    ..
                }
            ));
            if previously_authorized {
                // A persisted authorization is immutable, not silently revoked.
                store
                    .authorize_queued_planning(&owner, &authorization)
                    .unwrap();
                assert!(
                    store
                        .reserve_conversation_call(
                            &owner,
                            human.task_id,
                            authorization.grant.id,
                            &authorization.grant.scope_hash,
                            &authorization.request_hash
                        )
                        .is_err()
                );
                assert_eq!(
                    store
                        .conversation_task_budget(&owner, human.task_id)
                        .unwrap()
                        .planning_reserved_calls,
                    0
                );
            } else {
                let error = store
                    .authorize_queued_planning(&owner, &authorization)
                    .unwrap_err();
                assert!(
                    matches!(
                        error,
                        StorageError::ConversationContract {
                            code: "human_input_pending",
                            ..
                        }
                    ),
                    "{error:?}"
                );
                assert_eq!(
                    store
                        .conversation_task_budget(&owner, human.task_id)
                        .unwrap()
                        .planning_authorized_calls,
                    0
                );
            }
            assert_eq!(
                store
                    .queued_planning_authorization(
                        &owner,
                        human.conversation_id,
                        human.task_id,
                        authorization.grant.id
                    )
                    .unwrap(),
                previously_authorized.then_some(authorization.clone())
            );
            assert!(
                store
                    .conversation_call(&owner, human.task_id, authorization.grant.id)
                    .unwrap()
                    .is_none()
            );
            store
                .answer_conversation_human_request(&owner, human.id, &answer)
                .unwrap();
            drop(store);
            let store = SqliteStore::open(&path).unwrap();
            store
                .check_queued_call_admission(
                    &owner,
                    human.conversation_id,
                    human.task_id,
                    authorization.grant.id,
                )
                .unwrap();
            store
                .authorize_queued_planning(&owner, &authorization)
                .unwrap();
            assert_eq!(
                store
                    .reserve_conversation_call(
                        &owner,
                        human.task_id,
                        authorization.grant.id,
                        &authorization.grant.scope_hash,
                        &authorization.request_hash
                    )
                    .unwrap(),
                ConversationCallAdmission::Admitted
            );
            assert!(matches!(
                store
                    .reserve_conversation_call(
                        &owner,
                        human.task_id,
                        authorization.grant.id,
                        &authorization.grant.scope_hash,
                        &authorization.request_hash
                    )
                    .unwrap(),
                ConversationCallAdmission::Existing(_)
            ));
            assert_eq!(
                store
                    .conversation_task_budget(&owner, human.task_id)
                    .unwrap()
                    .planning_reserved_calls,
                1
            );
        }
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
    #[serde(default)]
    pub deferred: bool,
    #[serde(default)]
    pub deferral_revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationHumanDeferral {
    pub command_id: Uuid,
    pub expected_revision: i64,
    pub deferred: bool,
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
pub(crate) fn read(
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
    let (deferral_revision,deferred)=db.query_row("SELECT revision,deferred FROM conversation_human_deferrals WHERE request_id=?1 ORDER BY revision DESC LIMIT 1",[id.to_string()],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,bool>(1)?))).optional()?.unwrap_or((0,false));
    Ok(ConversationHumanRequest {
        deferred: deferred && status == "pending",
        deferral_revision,
        resume_draft_id,
        resume_error,
        input,
        status: serde_json::from_value(serde_json::Value::String(status))?,
        answer: answer
            .map(|value| serde_json::from_str(&value))
            .transpose()?,
    })
}

/// Bind a fresh request to the exact, still-actionable interpretation read by the
/// Application. This runs in the request INSERT transaction, so cancellation and
/// preparation have one ordering; cancellation never retracts an existing request.
pub(crate) fn validate_feedback_source<'a>(
    db: &Connection,
    expected_task: Uuid,
    source: &'a crate::ConversationCallReceipt,
) -> Result<&'a serde_json::Value, StorageError> {
    let current: Option<(String, String, String, Option<String>)> = db
        .query_row(
            "SELECT task_id,request_hash,status,evidence_json FROM conversation_model_calls WHERE id=?1",
            [source.id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let (task, request_hash, status, evidence) =
        current.ok_or_else(|| invalid("Feedback source receipt is missing"))?;
    let evidence: Option<serde_json::Value> = evidence
        .map(|value| serde_json::from_str(&value))
        .transpose()?;
    if source.id.is_nil()
        || source.task_id != expected_task
        || task != expected_task.to_string()
        || source.status != crate::ConversationCallStatus::Completed
        || status != "completed"
        || request_hash != source.request_hash
        || evidence != source.evidence
    {
        return Err(invalid(
            "Feedback source receipt changed or belongs to another task",
        ));
    }
    let evidence = source
        .evidence
        .as_ref()
        .ok_or_else(|| invalid("Feedback source evidence is missing"))?;
    let cancelled: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversation_call_cancellations WHERE call_id=?1)",
        [source.id.to_string()],
        |row| row.get(0),
    )?;
    if evidence["phase"] != "feedback_text"
        || evidence["context"]["contract"] != "conversation-feedback-v1"
        || evidence["cancelled"] == true
        || cancelled
    {
        return Err(invalid(
            "Feedback source is not an active completed interpretation",
        ));
    }
    Ok(evidence)
}

pub(crate) fn validate_feedback_request_subject(
    input: &ConversationHumanRequestInput,
    source: &crate::ConversationCallReceipt,
    evidence: &serde_json::Value,
) -> Result<(), StorageError> {
    let subject = &evidence["context"]["subject"];
    let message: crate::ConversationMessage = serde_json::from_value(subject["message"].clone())?;
    let Some(crate::ConversationSelectionRef::SampleCandidate {
        task_id,
        sample_test_id,
        candidate_id,
        source_artifact_id,
        ..
    }) = &message.input.reference
    else {
        return Err(invalid(
            "Feedback source lacks a frozen candidate reference",
        ));
    };
    let image = message
        .input
        .image
        .as_ref()
        .ok_or_else(|| invalid("Feedback source lacks its image reference"))?;
    if message.conversation_id != input.conversation_id
        || *task_id != input.task_id
        || *sample_test_id != input.sample_test_id
        || input.outcome_id.as_ref() != Some(candidate_id)
        || input.addition_id.is_some()
        || image.image_id != input.image_id
        || image.sha256 != input.content_hash
        || subject["expected_feedback_sequence"].as_u64() != Some(input.expected_feedback_sequence)
        || subject["candidate"]["outcome"]["id"].as_str() != input.outcome_id.as_deref()
        || subject["candidate"]["source_artifact_id"] != source_artifact_id.to_string()
        || subject["pixels_supplied"] != false
        || input.id != Uuid::new_v5(&source.id, b"feedback-human-request-v1")
        || input.resume_checkpoint_ref != Uuid::new_v5(&input.id, b"prepared-repair-draft")
    {
        return Err(invalid(
            "Human request does not match its frozen feedback subject",
        ));
    }
    Ok(())
}

fn validate_feedback_request_source(
    db: &Connection,
    input: &ConversationHumanRequestInput,
    source: &crate::ConversationCallReceipt,
) -> Result<(), StorageError> {
    let evidence = validate_feedback_source(db, input.task_id, source)?;
    validate_feedback_request_subject(input, source, evidence)?;
    let subject = &evidence["context"]["subject"];
    // Application owns the proposal parser. Storage only binds the request fields
    // to that saved single-tool proposal, without interpreting new text or executing it.
    let response: annotagent_core::ModelResponse =
        serde_json::from_value(evidence["response"].clone())?;
    let proposal = response
        .tool_calls
        .first()
        .ok_or_else(|| invalid("Feedback source has no saved proposal"))?;
    let arguments = &proposal.arguments;
    let kind = subject["candidate"]["outcome"]["value"]["kind"].as_str();
    if response.tool_calls.len() != 1
        || proposal.name != "propose_candidate_feedback"
        || response
            .content
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
        || arguments["decision"] != "request_correction"
        || arguments["reason"] != input.reason_code
        || arguments["question"] != input.question
        || !matches!(
            input.reason_code.as_str(),
            "poor_boundary" | "wrong_label" | "wrong_target"
        )
        || !matches!(kind, Some("bounding_box" | "classification"))
        || (input.reason_code == "poor_boundary" && kind != Some("bounding_box"))
    {
        return Err(invalid(
            "Human request does not match its saved correction proposal",
        ));
    }
    Ok(())
}

impl SqliteStore {
    /// Human scheduling preference only: no answer, outbox delivery or new spending authority.
    pub fn set_conversation_human_deferral(
        &self,
        project: &str,
        id: Uuid,
        input: &ConversationHumanDeferral,
    ) -> Result<ConversationHumanRequest, StorageError> {
        self.with_connection(|db|{
            let tx=db.unchecked_transaction()?;
            let current=read(&tx,project,id)?;
            let replay:Option<(String,i64,bool)>=tx.query_row("SELECT request_id,revision,deferred FROM conversation_human_deferrals WHERE command_id=?1",[input.command_id.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
            if let Some((request,revision,deferred))=replay{
                if request!=id.to_string()||revision.checked_sub(1)!=Some(input.expected_revision)||deferred!=input.deferred{return Err(invalid("Deferral command conflicts with its saved payload"));}
                return Ok(current);
            }
            if current.status!=ConversationHumanRequestStatus::Pending{return Err(invalid("Only an unanswered request can be deferred or reopened"));}
            if current.deferral_revision!=input.expected_revision{return Err(invalid("Request scheduling changed; reload before deferring or reopening"));}
            let revision=input.expected_revision.checked_add(1).filter(|value|*value>0).ok_or_else(||invalid("Invalid deferral revision"))?;
            tx.execute("INSERT INTO conversation_human_deferrals(command_id,request_id,revision,deferred) VALUES(?1,?2,?3,?4)",params![input.command_id.to_string(),id.to_string(),revision,input.deferred])?;
            let saved=read(&tx,project,id)?;tx.commit()?;Ok(saved)
        })
    }
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
        self.create_conversation_human_request_with_policy(project, input, false, None, None)
    }
    /// Atomically keep at most one pending task request per sample image.
    pub fn create_exclusive_conversation_human_request(
        &self,
        project: &str,
        input: &ConversationHumanRequestInput,
    ) -> Result<ConversationHumanRequest, StorageError> {
        self.create_conversation_human_request_with_policy(project, input, true, None, None)
    }
    /// Only fresh requests require an uncancelled feedback source in the same
    /// transaction. Exact saved-request retries restore history without a new write.
    pub fn create_conversation_feedback_human_request(
        &self,
        project: &str,
        input: &ConversationHumanRequestInput,
        source: &crate::ConversationCallReceipt,
    ) -> Result<ConversationHumanRequest, StorageError> {
        self.create_conversation_human_request_with_policy(project, input, true, Some(source), None)
    }
    /// A saved current-candidate answer supplies human intent; the original model
    /// receipt remains a clarification and is never rewritten as a correction.
    pub fn create_conversation_scoped_feedback_human_request(
        &self,
        project: &str,
        input: &ConversationHumanRequestInput,
        source: &crate::ConversationCallReceipt,
        answer: &crate::ConversationFeedbackScopeAnswer,
    ) -> Result<ConversationHumanRequest, StorageError> {
        self.create_conversation_human_request_with_policy(
            project,
            input,
            true,
            Some(source),
            Some(answer),
        )
    }
    fn create_conversation_human_request_with_policy(
        &self,
        project: &str,
        input: &ConversationHumanRequestInput,
        exclusive: bool,
        feedback_source: Option<&crate::ConversationCallReceipt>,
        scope_answer: Option<&crate::ConversationFeedbackScopeAnswer>,
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
        let valid_subject = match (&input.outcome_id, &input.addition_id) {
            (Some(id), None) => sample
                .report
                .samples
                .get(index)
                .is_some_and(|result| result.outcomes.iter().any(|outcome| &outcome.id == id)),
            (None, Some(id)) => {
                Uuid::parse_str(id).is_ok()
                    && input.reason_code == "identify_target"
                    && sample.report.sandbox
                    && sample.report.samples.get(index).is_some()
            }
            _ => false,
        };
        if !valid_subject {
            return Err(invalid(
                "Human request requires an owned outcome or a distinct new reference target",
            ));
        }
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            owned(&tx,project,input.task_id,input.conversation_id)?;
            let exists: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_human_requests WHERE id=?1)",[input.id.to_string()],|row|row.get(0))?;
            if exists { let saved=read(&tx,project,input.id)?; if saved.input!=*input { return Err(invalid("Human request idempotency conflict")); } return Ok(saved); }
            if let Some(addition) = &input.addition_id {
                let used:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sample_feedback_revisions WHERE sample_test_id=?1 AND image_id=?2 AND json_extract(feedback_json,'$.addition_id')=?3) OR EXISTS(SELECT 1 FROM conversation_human_requests WHERE json_extract(request_json,'$.sample_test_id')=?1 AND json_extract(request_json,'$.image_id')=?2 AND json_extract(request_json,'$.addition_id')=?3)",params![input.sample_test_id,input.image_id,addition],|row|row.get(0))?;
                if used {return Err(invalid("Reference target identity already belongs to saved feedback"));}
            }
            if let Some(source) = feedback_source {
                if let Some(answer) = scope_answer {
                    crate::conversation_feedback_scope::validate_scoped_request(&tx, project, input, source, answer)?;
                } else { validate_feedback_request_source(&tx, input, source)?; }
            }
            crate::conversation_image_class::require_no_pending_on_image(&tx,input.task_id,&input.sample_test_id,&input.image_id)?;
            if exclusive {
                let waiting:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_human_requests WHERE task_id=?1 AND status='pending' AND json_extract(request_json,'$.sample_test_id')=?2 AND json_extract(request_json,'$.image_id')=?3)",params![input.task_id.to_string(),input.sample_test_id,input.image_id],|row|row.get(0))?;
                if waiting {return Err(invalid("An existing request on this sample image must be resolved first"));}
            }
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
        self.answer_conversation_human_request_in_journey(project, id, answer, None)
    }

    pub fn answer_conversation_human_request_in_journey(
        &self,
        project: &str,
        id: Uuid,
        answer: &SampleFeedbackRevision,
        consent_id: Option<Uuid>,
    ) -> Result<ConversationHumanRequest, StorageError> {
        let request = self.conversation_human_request(project, id)?;
        let input = &request.input;
        if answer.sample_test_id != input.sample_test_id
            || answer.image_id != input.image_id
            || answer.outcome_id != input.outcome_id
            || answer.addition_id != input.addition_id
            || input.expected_feedback_sequence.checked_add(1) != Some(answer.sequence)
        {
            return Err(invalid(
                "Human answer does not match the requested subject and revision",
            ));
        }
        self.save_sample_feedback_with(answer, |tx| {
            let current=read(tx,project,id)?;
            if let Some(consent_id) = consent_id {
                let consent = crate::conversation_journey::read(tx,input.task_id,consent_id)?
                    .ok_or_else(|| invalid("Answer continuation consent not found"))?;
                if consent.consent.repair_after_answer.as_ref()!=Some(input) {
                    return Err(invalid("Answer continuation does not match the saved request"));
                }
                let existing:Option<(String,String)>=tx.query_row("SELECT consent_id,feedback_revision_id FROM conversation_answer_delivery WHERE request_id=?1",[id.to_string()],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
                if let Some(existing)=existing {
                    if existing!=(consent_id.to_string(),answer.revision_id.clone()) {return Err(invalid("Answer continuation retry changed its original intent"));}
                } else {
                    if current.answer.is_some() { return Err(invalid("An already saved answer cannot acquire a new automatic continuation")); }
                    let error = (consent.revoked || consent.consent.expires_at<=chrono::Utc::now())
                        .then_some("Correction saved; continuation permission is revoked or expired");
                    tx.execute("INSERT INTO conversation_answer_delivery(request_id,consent_id,feedback_revision_id,status,error,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id.to_string(),consent_id.to_string(),answer.revision_id,if error.is_some(){"failed"}else{"pending"},error,chrono::Utc::now().to_rfc3339()])?;
                }
            }
            if let Some(saved)=current.answer { if saved==*answer { return Ok(()); } return Err(invalid("Human answer conflicts with an already saved answer")); }
            if current.status!=ConversationHumanRequestStatus::Pending || current.deferred { return Err(invalid("Human request is no longer pending or is deferred; reopen it before answering")); }
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
