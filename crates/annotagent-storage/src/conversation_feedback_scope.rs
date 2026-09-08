//! Durable human scope choices; not feedback, Schema changes or execution authority.

use crate::{
    ConversationCallReceipt, ConversationFeedbackAuthorizationRecord,
    ConversationHumanRequestInput, ConversationMessage, ConversationMessageInput,
    ConversationSelectionRef, SqliteStore, StorageError,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationFeedbackCorrectionReason {
    PoorBoundary,
    WrongLabel,
    WrongTarget,
}

impl ConversationFeedbackCorrectionReason {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::PoorBoundary => "poor_boundary",
            Self::WrongLabel => "wrong_label",
            Self::WrongTarget => "wrong_target",
        }
    }
    #[must_use]
    pub fn question(self) -> &'static str {
        match self {
            Self::PoorBoundary => "Correct the boundary of this saved candidate in the canvas.",
            Self::WrongLabel => {
                "Review and correct the label of this saved candidate in the canvas."
            }
            Self::WrongTarget => {
                "Review whether this saved candidate is a false positive in the canvas."
            }
        }
    }
    fn validate(self, context: &Value) -> Result<(), StorageError> {
        let kind = context["candidate"]["outcome"]["value"]["kind"].as_str();
        if !matches!(kind, Some("bounding_box" | "classification"))
            || (self == Self::PoorBoundary && kind != Some("bounding_box"))
        {
            return Err(invalid(
                "This correction reason is unsupported for the frozen candidate type",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum ConversationFeedbackScopeChoice {
    CurrentCandidate {
        reason: ConversationFeedbackCorrectionReason,
    },
    CurrentImageClass,
    ProjectFutureRule,
}

impl<'de> Deserialize<'de> for ConversationFeedbackScopeChoice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Serde's internally tagged unit variants ignore extra fields even with
        // deny_unknown_fields. Empty struct variants enforce the required boundary.
        #[derive(Deserialize)]
        #[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
        enum StrictChoice {
            CurrentCandidate {
                reason: ConversationFeedbackCorrectionReason,
            },
            CurrentImageClass {},
            ProjectFutureRule {},
        }
        Ok(match StrictChoice::deserialize(deserializer)? {
            StrictChoice::CurrentCandidate { reason } => Self::CurrentCandidate { reason },
            StrictChoice::CurrentImageClass {} => Self::CurrentImageClass,
            StrictChoice::ProjectFutureRule {} => Self::ProjectFutureRule,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFeedbackScopeAnswerInput {
    pub command_id: Uuid,
    pub expected_context_digest: String,
    pub choice: ConversationFeedbackScopeChoice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationFeedbackScopeAnswer {
    pub call_id: Uuid,
    pub task_id: Uuid,
    pub conversation_id: Uuid,
    pub input: ConversationFeedbackScopeAnswerInput,
    pub created_at: String,
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidConversation(message.into())
}

/// Canonical for this API: hash the persisted JSON Value, not an Application struct.
pub fn conversation_feedback_context_digest(context: &Value) -> Result<String, StorageError> {
    Ok(annotagent_image_tools::sha256(&serde_json::to_vec(
        context,
    )?))
}

pub(crate) fn read(
    db: &Connection,
    task: Uuid,
    call: Uuid,
) -> Result<Option<ConversationFeedbackScopeAnswer>, StorageError> {
    let row: Option<(String, String, String, String)> = db.query_row(
        "SELECT task_id,conversation_id,input_json,created_at FROM conversation_feedback_scope_answers WHERE call_id=?1",
        [call.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
    ).optional()?;
    row.map(|(owner, conversation, input, created_at)| {
        if owner != task.to_string() {
            return Err(invalid("Scope answer belongs to another task"));
        }
        Ok(ConversationFeedbackScopeAnswer {
            call_id: call,
            task_id: task,
            conversation_id: Uuid::parse_str(&conversation)
                .map_err(|_| invalid("Invalid saved scope conversation"))?,
            input: serde_json::from_str(&input)?,
            created_at,
        })
    })
    .transpose()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClarificationProposal {
    decision: String,
    question: String,
    rationale: String,
}

fn bounded(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.contains('\0')
}

/// Uses the original completed response, never a synthesized `RequestCorrection`.
pub(crate) fn validate_clarification_source(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
    call: Uuid,
    source: &ConversationCallReceipt,
) -> Result<ConversationFeedbackAuthorizationRecord, StorageError> {
    if crate::conversation_feedback::owned(db, project, task)? != conversation || source.id != call
    {
        return Err(invalid(
            "Scope answer source does not match its conversation task and call",
        ));
    }
    let evidence = crate::conversation_human_requests::validate_feedback_source(db, task, source)?;
    let record = crate::conversation_feedback::read(db, task, call)?
        .ok_or_else(|| invalid("Scope answer requires the saved feedback authorization"))?;
    if record.consent.call_id != call
        || record.grant.task_id != task
        || evidence["context"]["subject"] != record.context
        || evidence["context"]["scope_hash"] != record.consent.scope_hash
        || evidence["context"]["remote_model"] != record.summary["remote_model"]
        || !record.context.is_object()
        || !record.context["candidate"]["outcome"].is_object()
        || record.context["pixels_supplied"] != false
        || serde_json::to_vec(&record.context)?.len() > 32_768
    {
        return Err(invalid(
            "Scope answer source does not match its frozen authorization context",
        ));
    }
    let response: annotagent_core::ModelResponse =
        serde_json::from_value(evidence["response"].clone())?;
    if response.tool_calls.len() != 1
        || response.tool_calls[0].name != "propose_candidate_feedback"
        || response
            .content
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(invalid(
            "Scope answer requires exactly one saved clarification proposal",
        ));
    }
    let proposal: ClarificationProposal =
        serde_json::from_value(response.tool_calls[0].arguments.clone())?;
    if proposal.decision != "clarify_scope"
        || !bounded(&proposal.question, 2_000)
        || !bounded(&proposal.rationale, 4_000)
    {
        return Err(invalid(
            "Scope answer requires a valid completed clarification",
        ));
    }
    Ok(record)
}

/// The Application additionally hashes live file bytes and validates the terminal
/// sample projection. Here all DB-backed checks share the answer/request INSERT.
pub(crate) fn validate_live_context(
    db: &Connection,
    project: &str,
    conversation: Uuid,
    task: Uuid,
    record: &ConversationFeedbackAuthorizationRecord,
    expected_digest: &str,
) -> Result<(), StorageError> {
    if conversation_feedback_context_digest(&record.context)? != expected_digest {
        return Err(invalid(
            "Feedback context changed; reload before choosing its scope",
        ));
    }
    let message: ConversationMessage = serde_json::from_value(record.context["message"].clone())?;
    let saved: Option<(i64, String)> = db.query_row(
        "SELECT sequence,input_json FROM conversation_messages WHERE conversation_id=?1 AND message_id=?2",
        params![conversation.to_string(), record.consent.message_id.to_string()], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional()?;
    let (sequence, input) = saved.ok_or_else(|| invalid("Scope answer message is missing"))?;
    let input: ConversationMessageInput = serde_json::from_str(&input)?;
    if message
        != (ConversationMessage {
            conversation_id: conversation,
            sequence,
            input: input.clone(),
        })
    {
        return Err(invalid(
            "Scope answer does not match the exact saved message",
        ));
    }
    let Some(ConversationSelectionRef::SampleCandidate {
        task_id,
        project_schema_revision,
        sample_test_id,
        candidate_id,
        source_artifact_id,
        ..
    }) = &input.reference
    else {
        return Err(invalid(
            "Scope answer requires the original saved candidate reference",
        ));
    };
    let revision: String = db.query_row(
        "SELECT schema_revision FROM conversation_tasks WHERE id=?1",
        [task.to_string()],
        |row| row.get(0),
    )?;
    if *task_id != task
        || *project_schema_revision != revision
        || record.context["candidate"]["outcome"]["id"] != *candidate_id
        || record.context["candidate"]["source_artifact_id"] != source_artifact_id.to_string()
    {
        return Err(invalid(
            "Scope answer candidate or Schema revision does not match the saved reference",
        ));
    }
    let image = input
        .image
        .as_ref()
        .ok_or_else(|| invalid("Scope answer has no frozen image"))?;
    let hash: Option<String> = db
        .query_row(
            "SELECT sha256 FROM images WHERE id=?1 AND project_id=?2",
            params![image.image_id, project],
            |row| row.get(0),
        )
        .optional()?;
    if hash.as_deref() != Some(image.sha256.as_str()) {
        return Err(invalid("Scope answer image was removed or changed"));
    }
    let sequence:i64=db.query_row("SELECT COALESCE(MAX(sequence),0) FROM sample_feedback_revisions WHERE sample_test_id=?1 AND image_id=?2",params![sample_test_id,image.image_id],|row|row.get(0))?;
    if record.context["expected_feedback_sequence"].as_u64() != u64::try_from(sequence).ok() {
        return Err(invalid(
            "Feedback changed before the scope choice was saved",
        ));
    }
    Ok(())
}

pub(crate) fn validate_scoped_request(
    db: &Connection,
    project: &str,
    input: &ConversationHumanRequestInput,
    source: &ConversationCallReceipt,
    answer: &ConversationFeedbackScopeAnswer,
) -> Result<(), StorageError> {
    let saved = read(db, input.task_id, source.id)?
        .ok_or_else(|| invalid("Save a scope answer before requesting a correction"))?;
    if saved != *answer || saved.conversation_id != input.conversation_id {
        return Err(invalid(
            "Scoped request does not match its saved human answer",
        ));
    }
    let ConversationFeedbackScopeChoice::CurrentCandidate { reason } = saved.input.choice else {
        return Err(invalid(
            "Broader scope choices do not authorize a correction or any expanded action",
        ));
    };
    let record = validate_clarification_source(
        db,
        project,
        input.conversation_id,
        input.task_id,
        source.id,
        source,
    )?;
    validate_live_context(
        db,
        project,
        input.conversation_id,
        input.task_id,
        &record,
        &saved.input.expected_context_digest,
    )?;
    reason.validate(&record.context)?;
    crate::conversation_human_requests::validate_feedback_request_subject(
        input,
        source,
        source
            .evidence
            .as_ref()
            .ok_or_else(|| invalid("Feedback source evidence missing"))?,
    )?;
    if input.reason_code != reason.code() || input.question != reason.question() {
        return Err(invalid(
            "Scoped correction requires the saved human reason and fixed canvas question",
        ));
    }
    Ok(())
}

impl SqliteStore {
    pub fn conversation_feedback_scope_answer(
        &self,
        project: &str,
        task: Uuid,
        call: Uuid,
    ) -> Result<Option<ConversationFeedbackScopeAnswer>, StorageError> {
        self.with_connection(|db| {
            crate::conversation_feedback::owned(db, project, task)?;
            read(db, task, call)
        })
    }
    /// Human intent only. Neither expired grants nor pending human work prevent a
    /// local answer; no model call, feedback revision, Schema or outbox is written.
    pub fn answer_conversation_feedback_scope(
        &self,
        project: &str,
        conversation: Uuid,
        task: Uuid,
        call: Uuid,
        input: &ConversationFeedbackScopeAnswerInput,
        source: &ConversationCallReceipt,
    ) -> Result<ConversationFeedbackScopeAnswer, StorageError> {
        self.with_connection(|db| {
            let tx=db.unchecked_transaction()?;
            if crate::conversation_feedback::owned(&tx,project,task)? != conversation {return Err(invalid("Scope answer task belongs to another conversation"));}
            if let Some(saved)=read(&tx,task,call)? {
                if saved.input != *input || saved.conversation_id != conversation {return Err(invalid("Scope answer retry conflicts with its saved choice"));}
                return Ok(saved);
            }
            if input.command_id.is_nil() || call.is_nil() || input.expected_context_digest.len()!=64 || !input.expected_context_digest.bytes().all(|value|value.is_ascii_hexdigit()) {
                return Err(invalid("Scope answer requires a stable command and exact context digest"));
            }
            let collision:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_feedback_scope_answers WHERE command_id=?1)",[input.command_id.to_string()],|row|row.get(0))?;
            if collision {return Err(invalid("Scope answer command belongs to another saved answer"));}
            let record=validate_clarification_source(&tx,project,conversation,task,call,source)?;
            validate_live_context(&tx,project,conversation,task,&record,&input.expected_context_digest)?;
            if let ConversationFeedbackScopeChoice::CurrentCandidate {reason}=input.choice {reason.validate(&record.context)?;}
            let saved=ConversationFeedbackScopeAnswer {call_id:call,task_id:task,conversation_id:conversation,input:input.clone(),created_at:chrono::Utc::now().to_rfc3339()};
            tx.execute("INSERT INTO conversation_feedback_scope_answers(call_id,task_id,conversation_id,command_id,input_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![call.to_string(),task.to_string(),conversation.to_string(),input.command_id.to_string(),serde_json::to_string(input)?,saved.created_at])?;
            tx.commit()?;Ok(saved)
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::*;
    use chrono::Utc;
    use rusqlite::params;
    use serde_json::json;
    use uuid::Uuid;

    pub(crate) struct Fixture {
        pub(crate) project: String,
        pub(crate) conversation: Uuid,
        pub(crate) task: Uuid,
        pub(crate) record: ConversationFeedbackAuthorizationRecord,
        pub(crate) source: ConversationCallReceipt,
        pub(crate) answer: ConversationFeedbackScopeAnswerInput,
    }

    pub(crate) fn fixture(store: &SqliteStore, kind: &str) -> Fixture {
        let project = Uuid::new_v4().to_string();
        let conversation = store.create_conversation(&project).unwrap();
        let goal = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST goal".into(),
            image: None,
            reference: None,
        };
        store
            .append_conversation_message(&project, conversation, &goal)
            .unwrap();
        let task = Uuid::new_v4();
        store
            .begin_conversation_task(
                &project,
                conversation,
                &BeginConversationTask {
                    id: task,
                    source_message_id: goal.id,
                    schema_revision: "a".repeat(64),
                },
            )
            .unwrap();
        let image = store
            .ensure_project_image(
                project.parse().unwrap(),
                "TEST-scope.png",
                "TEST-pixels",
                "{}",
            )
            .unwrap()
            .image_id;
        let original = crate::sample_feedback::tests::fixture(store);
        let mut sample = store
            .get_workflow_sample_test_by_id(&original.sample_test_id)
            .unwrap()
            .unwrap();
        sample.id = "TEST-scoped-sample".into();
        sample.project_id = project.clone();
        sample.inputs[0].image_id = image.to_string();
        sample.inputs[0].content_hash = "TEST-pixels".into();
        store.save_workflow_sample_test(&sample).unwrap();
        store
            .reserve_sample_operation(&SampleOperation {
                id: sample.id.clone(),
                project_id: project.clone(),
                draft_id: sample.draft_id.clone(),
                authorization_fingerprint: "TEST".into(),
                request: json!({"conversation":{"conversation_id":conversation,"task_id":task}}),
                status: "completed".into(),
                error: None,
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
            })
            .unwrap();
        let artifact = Uuid::new_v4();
        let input = ConversationMessageInput {
            id: Uuid::new_v4(),
            text: "TEST remove this".into(),
            image: Some(ConversationImageRef {
                image_id: image.to_string(),
                sha256: "TEST-pixels".into(),
            }),
            reference: Some(ConversationSelectionRef::SampleCandidate {
                task_id: task,
                project_schema_revision: "a".repeat(64),
                draft_id: sample.draft_id.clone(),
                draft_revision: sample.draft_revision,
                sample_test_id: sample.id.clone(),
                candidate_id: original.outcome_id.clone().unwrap(),
                source_artifact_id: artifact,
            }),
        };
        let message = store
            .append_conversation_message(&project, conversation, &input)
            .unwrap();
        let call = Uuid::new_v4();
        let expiry = Utc::now() + chrono::Duration::minutes(5);
        let record = ConversationFeedbackAuthorizationRecord {
            consent: ConversationFeedbackAuthorization {
                call_id: call,
                message_id: input.id,
                model_id: annotagent_core::ModelProfileId::new(),
                previous_grant_id: None,
                scope_hash: "b".repeat(64),
                expires_at: expiry,
                allow_unknown_cost: true,
            },
            grant: ConversationCallGrant {
                id: call,
                task_id: task,
                scope_hash: "b".repeat(64),
                maximum_calls: 1,
                expires_at: expiry,
            },
            context: json!({"message":message,"candidate":{"source_artifact_id":artifact,"outcome":{"id":original.outcome_id,"value":{"kind":kind}}},"expected_feedback_sequence":0,"sample_content_hash":sample.draft_content_hash,"pixels_supplied":false}),
            summary: json!({"remote_model":"TEST model","destination":"TEST only"}),
        };
        store
            .authorize_conversation_feedback(&project, conversation, task, &record)
            .unwrap();
        store
            .reserve_conversation_call(
                &project,
                task,
                call,
                &record.consent.scope_hash,
                &"c".repeat(64),
            )
            .unwrap();
        let response = annotagent_core::ModelResponse {
            content: None,
            tool_calls: vec![annotagent_core::ModelToolCall {
                id: "TEST tool".into(),
                name: "propose_candidate_feedback".into(),
                arguments: json!({"decision":"clarify_scope","question":"Which scope do you mean?","rationale":"The saved text is ambiguous; no pixels were inspected."}),
            }],
            usage: annotagent_core::TokenUsage::known(10, 5, annotagent_core::UsageSource::Mock),
            request_id: Some("TEST response".into()),
            provider_metadata: std::collections::BTreeMap::new(),
        };
        let source = store.finish_conversation_call(&project, task, call, ConversationCallStatus::Completed, json!({"phase":"feedback_text","cancelled":false,"response":response,"context":{"contract":"conversation-feedback-v1","subject":record.context,"remote_model":"TEST model","scope_hash":record.consent.scope_hash}})).unwrap();
        let answer = ConversationFeedbackScopeAnswerInput {
            command_id: Uuid::new_v4(),
            expected_context_digest: annotagent_image_tools::sha256(
                &serde_json::to_vec(&record.context).unwrap(),
            ),
            choice: ConversationFeedbackScopeChoice::CurrentCandidate {
                reason: ConversationFeedbackCorrectionReason::WrongTarget,
            },
        };
        Fixture {
            project,
            conversation,
            task,
            record,
            source,
            answer,
        }
    }

    impl Fixture {
        fn save(
            &self,
            store: &SqliteStore,
        ) -> Result<ConversationFeedbackScopeAnswer, StorageError> {
            store.answer_conversation_feedback_scope(
                &self.project,
                self.conversation,
                self.task,
                self.source.id,
                &self.answer,
                &self.source,
            )
        }
        fn human(&self, answer: &ConversationFeedbackScopeAnswer) -> ConversationHumanRequestInput {
            let message: ConversationMessage =
                serde_json::from_value(self.record.context["message"].clone()).unwrap();
            let Some(ConversationSelectionRef::SampleCandidate {
                sample_test_id,
                candidate_id,
                ..
            }) = message.input.reference
            else {
                panic!("TEST candidate")
            };
            let image = message.input.image.unwrap();
            let reason = match answer.input.choice {
                ConversationFeedbackScopeChoice::CurrentCandidate { reason } => reason,
                _ => ConversationFeedbackCorrectionReason::WrongTarget,
            };
            let id = Uuid::new_v5(&self.source.id, b"feedback-human-request-v1");
            ConversationHumanRequestInput {
                id,
                task_id: self.task,
                conversation_id: self.conversation,
                sample_test_id,
                image_id: image.image_id,
                content_hash: image.sha256,
                outcome_id: Some(candidate_id),
                addition_id: None,
                expected_feedback_sequence: 0,
                reason_code: reason.code().into(),
                question: reason.question().into(),
                resume_checkpoint_ref: Uuid::new_v5(&id, b"prepared-repair-draft"),
            }
        }
        fn assert_local_only(&self, store: &SqliteStore) {
            assert_eq!(
                store
                    .conversation_call_history(&self.project, self.task)
                    .unwrap(),
                vec![self.source.clone()]
            );
            assert_eq!(
                store
                    .conversation_call_budget(&self.project, self.task)
                    .unwrap()
                    .unwrap()
                    .used_calls,
                1
            );
            assert!(
                store
                    .pending_conversation_resumes(&self.project, self.conversation, self.task)
                    .unwrap()
                    .is_empty()
            );
            let message: ConversationMessage =
                serde_json::from_value(self.record.context["message"].clone()).unwrap();
            assert!(
                store
                    .sample_feedback("TEST-scoped-sample", &message.input.image.unwrap().image_id)
                    .unwrap()
                    .is_empty()
            );
            store
                .with_connection(|db| {
                    for table in [
                        "conversation_schema_drafts",
                        "conversation_schema_revisions",
                        "conversation_resume_results",
                    ] {
                        let count: i64 =
                            db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                                row.get(0)
                            })?;
                        assert_eq!(count, 0);
                    }
                    Ok(())
                })
                .unwrap();
        }
    }

    #[test]
    fn scope_answer_contract_rejects_injected_fields_and_broad_corrections() {
        let valid = json!({"command_id":Uuid::new_v4(),"expected_context_digest":"a".repeat(64),"choice":{"scope":"current_candidate","reason":"wrong_target"}});
        assert!(
            serde_json::from_value::<ConversationFeedbackScopeAnswerInput>(valid.clone()).is_ok()
        );
        for choice in [
            json!({"scope":"current_image_class","reason":"wrong_target"}),
            json!({"scope":"project_future_rule","candidate_id":"foreign"}),
            json!({"scope":"current_candidate","reason":"delete_all"}),
            json!({"scope":"current_candidate","reason":"wrong_label","rect":[0,0,1,1]}),
            json!({"scope":"all_projects"}),
        ] {
            let mut bad = valid.clone();
            bad["choice"] = choice;
            assert!(serde_json::from_value::<ConversationFeedbackScopeAnswerInput>(bad).is_err());
        }
        let mut bad = valid;
        bad["model_id"] = json!(Uuid::new_v4());
        assert!(serde_json::from_value::<ConversationFeedbackScopeAnswerInput>(bad).is_err());
    }

    #[test]
    fn scope_answer_restores_after_restart_cancellation_changed_pixels_and_expired_grant() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TEST-scope.db");
        let store = SqliteStore::open(&path).unwrap();
        let f = fixture(&store, "bounding_box");
        store.with_connection(|db| {db.execute("UPDATE conversation_call_grants SET expires_at='2000-01-01T00:00:00Z' WHERE task_id=?1",[f.task.to_string()])?;Ok(())}).unwrap();
        let saved = f.save(&store).unwrap();
        assert_eq!(f.save(&store).unwrap(), saved);
        f.assert_local_only(&store);
        store
            .with_connection(|db| {
                db.execute(
                    "UPDATE images SET sha256='TEST changed' WHERE project_id=?1",
                    [&f.project],
                )?;
                Ok(())
            })
            .unwrap();
        store
            .request_conversation_call_cancel(&f.project, f.task, f.source.id)
            .unwrap();
        drop(store);
        let store = SqliteStore::open(&path).unwrap();
        assert_eq!(f.save(&store).unwrap(), saved);
        assert_eq!(
            store
                .conversation_feedback_scope_answer(&f.project, f.task, f.source.id)
                .unwrap(),
            Some(saved)
        );
        let mut changed = f.answer.clone();
        changed.command_id = Uuid::new_v4();
        assert!(
            store
                .answer_conversation_feedback_scope(
                    &f.project,
                    f.conversation,
                    f.task,
                    f.source.id,
                    &changed,
                    &f.source
                )
                .is_err()
        );
        changed = f.answer.clone();
        changed.choice = ConversationFeedbackScopeChoice::ProjectFutureRule;
        assert!(
            store
                .answer_conversation_feedback_scope(
                    &f.project,
                    f.conversation,
                    f.task,
                    f.source.id,
                    &changed,
                    &f.source
                )
                .is_err()
        );
        f.assert_local_only(&store);
    }

    #[test]
    fn scope_answer_rejects_foreign_stale_cancelled_and_invalid_receipt_without_partial_writes() {
        for variant in 0..13 {
            let store = SqliteStore::open_in_memory().unwrap();
            let mut f = fixture(&store, "bounding_box");
            match variant {
                0 => f.answer.expected_context_digest = "d".repeat(64),
                1 => f.answer.command_id = Uuid::nil(),
                2 => {
                    store
                        .request_conversation_call_cancel(&f.project, f.task, f.source.id)
                        .unwrap();
                }
                3 => {
                    store
                        .with_connection(|db| {
                            db.execute(
                                "UPDATE images SET sha256='changed' WHERE project_id=?1",
                                [&f.project],
                            )?;
                            Ok(())
                        })
                        .unwrap();
                }
                4 => f.source.task_id = Uuid::new_v4(),
                5 => {
                    f.source.evidence.as_mut().unwrap()["context"]["subject"]["pixels_supplied"] =
                        json!(true);
                }
                6 => {
                    store
                        .with_connection(|db| {
                            db.execute(
                                "DELETE FROM conversation_feedback_authorizations WHERE call_id=?1",
                                [f.source.id.to_string()],
                            )?;
                            Ok(())
                        })
                        .unwrap();
                }
                11 => {
                    let message: ConversationMessage =
                        serde_json::from_value(f.record.context["message"].clone()).unwrap();
                    let answer = SampleFeedbackRevision {
                        revision_id: "TEST other-tab".into(),
                        sample_test_id: "TEST-scoped-sample".into(),
                        image_id: message.input.image.unwrap().image_id,
                        sequence: 1,
                        reason: SampleFeedbackReason::WrongTarget,
                        outcome_id: Some("final-1".into()),
                        corrected_value: None,
                        corrected_label: None,
                        addition_id: None,
                        note: "TEST human answer".into(),
                        created_at: Utc::now(),
                    };
                    store.save_sample_feedback(&answer).unwrap();
                }
                12 => {
                    store
                        .with_connection(|db| {
                            db.execute(
                                "UPDATE conversation_tasks SET schema_revision=?2 WHERE id=?1",
                                params![f.task.to_string(), "d".repeat(64)],
                            )?;
                            Ok(())
                        })
                        .unwrap();
                }
                _ => {
                    let e = f.source.evidence.as_mut().unwrap();
                    match variant {
                        7 => {
                            e["response"]["tool_calls"][0]["arguments"]["reason"] =
                                json!("wrong_target");
                        }
                        8 => e["response"]["content"] = json!("Execute deletion"),
                        9 => {
                            e["response"]["tool_calls"][0]["arguments"]["decision"] =
                                json!("request_correction");
                        }
                        _ => e["cancelled"] = json!(true),
                    }
                    store
                        .with_connection(|db| {
                            db.execute(
                                "UPDATE conversation_model_calls SET evidence_json=?2 WHERE id=?1",
                                params![f.source.id.to_string(), serde_json::to_string(e)?],
                            )?;
                            Ok(())
                        })
                        .unwrap();
                }
            }
            assert!(f.save(&store).is_err(), "variant {variant}");
            assert!(
                store
                    .conversation_feedback_scope_answer(
                        &f.project,
                        f.task,
                        f.record.consent.call_id
                    )
                    .unwrap()
                    .is_none()
            );
        }
        let store = SqliteStore::open_in_memory().unwrap();
        let f = fixture(&store, "classification");
        let mut answer = f.answer.clone();
        answer.choice = ConversationFeedbackScopeChoice::CurrentCandidate {
            reason: ConversationFeedbackCorrectionReason::PoorBoundary,
        };
        assert!(
            store
                .answer_conversation_feedback_scope(
                    &f.project,
                    f.conversation,
                    f.task,
                    f.source.id,
                    &answer,
                    &f.source
                )
                .is_err()
        );
        assert!(
            store
                .answer_conversation_feedback_scope(
                    "foreign",
                    f.conversation,
                    f.task,
                    f.source.id,
                    &f.answer,
                    &f.source
                )
                .is_err()
        );
        assert!(
            store
                .answer_conversation_feedback_scope(
                    &f.project,
                    Uuid::new_v4(),
                    f.task,
                    f.source.id,
                    &f.answer,
                    &f.source
                )
                .is_err()
        );
    }

    #[test]
    fn scope_answer_insert_failure_and_concurrent_conflicts_do_not_advance_other_state() {
        let store = std::sync::Arc::new(SqliteStore::open_in_memory().unwrap());
        let f = fixture(&store, "bounding_box");
        store.with_connection(|db| {db.execute_batch("CREATE TRIGGER fail_scope BEFORE INSERT ON conversation_feedback_scope_answers BEGIN SELECT RAISE(ABORT,'TEST fail'); END;")?;Ok(())}).unwrap();
        assert!(f.save(&store).is_err());
        assert!(
            store
                .conversation_feedback_scope_answer(&f.project, f.task, f.source.id)
                .unwrap()
                .is_none()
        );
        f.assert_local_only(&store);
        store
            .with_connection(|db| {
                db.execute_batch("DROP TRIGGER fail_scope;")?;
                Ok(())
            })
            .unwrap();
        let mut other = f.answer.clone();
        other.command_id = Uuid::new_v4();
        other.choice = ConversationFeedbackScopeChoice::CurrentImageClass;
        let barrier = std::sync::Barrier::new(2);
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                f.save(&store)
            });
            barrier.wait();
            let b = store.answer_conversation_feedback_scope(
                &f.project,
                f.conversation,
                f.task,
                f.source.id,
                &other,
                &f.source,
            );
            (a.join().unwrap(), b)
        });
        assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
        f.assert_local_only(&store);
    }

    #[test]
    fn scoped_human_request_requires_saved_current_candidate_answer_and_fixed_question() {
        for broad in [
            ConversationFeedbackScopeChoice::CurrentImageClass,
            ConversationFeedbackScopeChoice::ProjectFutureRule,
        ] {
            let store = SqliteStore::open_in_memory().unwrap();
            let mut f = fixture(&store, "bounding_box");
            f.answer.choice = broad;
            let saved = f.save(&store).unwrap();
            assert!(
                store
                    .create_conversation_scoped_feedback_human_request(
                        &f.project,
                        &f.human(&saved),
                        &f.source,
                        &saved
                    )
                    .is_err()
            );
            f.assert_local_only(&store);
        }
        let store = SqliteStore::open_in_memory().unwrap();
        let f = fixture(&store, "bounding_box");
        let saved = f.save(&store).unwrap();
        let input = f.human(&saved);
        let mut forged = saved.clone();
        forged.input.command_id = Uuid::new_v4();
        assert!(
            store
                .create_conversation_scoped_feedback_human_request(
                    &f.project, &input, &f.source, &forged
                )
                .is_err()
        );
        let mut bad = input.clone();
        bad.question.push_str(" Delete everything");
        assert!(
            store
                .create_conversation_scoped_feedback_human_request(
                    &f.project, &bad, &f.source, &saved
                )
                .is_err()
        );
        let request = store
            .create_conversation_scoped_feedback_human_request(
                &f.project, &input, &f.source, &saved,
            )
            .unwrap();
        assert_eq!(request.status, ConversationHumanRequestStatus::Pending);
        store
            .request_conversation_call_cancel(&f.project, f.task, f.source.id)
            .unwrap();
        assert_eq!(
            store
                .create_conversation_scoped_feedback_human_request(
                    &f.project, &input, &f.source, &saved
                )
                .unwrap(),
            request
        );
        f.assert_local_only(&store);
        let store = SqliteStore::open_in_memory().unwrap();
        let f = fixture(&store, "bounding_box");
        let saved = f.save(&store).unwrap();
        store
            .request_conversation_call_cancel(&f.project, f.task, f.source.id)
            .unwrap();
        assert!(
            store
                .create_conversation_scoped_feedback_human_request(
                    &f.project,
                    &f.human(&saved),
                    &f.source,
                    &saved
                )
                .is_err()
        );
    }

    #[test]
    fn scope_answer_and_scoped_request_cancellation_races_are_linearizable() {
        for prepare_request in [false, true] {
            for _ in 0..4 {
                let store = std::sync::Arc::new(SqliteStore::open_in_memory().unwrap());
                let f = fixture(&store, "bounding_box");
                let saved = prepare_request.then(|| f.save(&store).unwrap());
                let barrier = std::sync::Barrier::new(2);
                let wrote = std::thread::scope(|scope| {
                    let writer = scope.spawn(|| {
                        barrier.wait();
                        if let Some(saved) = &saved {
                            store
                                .create_conversation_scoped_feedback_human_request(
                                    &f.project,
                                    &f.human(saved),
                                    &f.source,
                                    saved,
                                )
                                .map(|_| ())
                        } else {
                            f.save(&store).map(|_| ())
                        }
                    });
                    barrier.wait();
                    store
                        .request_conversation_call_cancel(&f.project, f.task, f.source.id)
                        .unwrap();
                    writer.join().unwrap().is_ok()
                });
                if let Some(saved) = saved {
                    assert_eq!(f.save(&store).unwrap(), saved);
                    let requests = store
                        .conversation_human_requests(&f.project, f.conversation, f.task)
                        .unwrap();
                    assert_eq!(requests.len(), usize::from(wrote));
                    if wrote {
                        assert_eq!(requests[0].status, ConversationHumanRequestStatus::Pending);
                        assert_eq!(
                            store
                                .create_conversation_scoped_feedback_human_request(
                                    &f.project,
                                    &f.human(&saved),
                                    &f.source,
                                    &saved
                                )
                                .unwrap(),
                            requests[0]
                        );
                    }
                } else {
                    let answer = store
                        .conversation_feedback_scope_answer(&f.project, f.task, f.source.id)
                        .unwrap();
                    assert_eq!(answer.is_some(), wrote);
                    if let Some(answer) = answer {
                        assert_eq!(f.save(&store).unwrap(), answer);
                    }
                }
                f.assert_local_only(&store);
            }
        }
    }

    #[test]
    fn command_identity_cannot_answer_another_call_or_expand_pending_requests() {
        let store = SqliteStore::open_in_memory().unwrap();
        let first = fixture(&store, "bounding_box");
        let mut record = first.record.clone();
        let mut message: ConversationMessage =
            serde_json::from_value(record.context["message"].clone()).unwrap();
        message.input.id = Uuid::new_v4();
        message = store
            .append_conversation_message(&first.project, first.conversation, &message.input)
            .unwrap();
        let call = Uuid::new_v4();
        record.context["message"] = serde_json::to_value(message.clone()).unwrap();
        record.consent.call_id = call;
        record.consent.message_id = message.input.id;
        record.consent.previous_grant_id = Some(first.record.grant.id);
        record.consent.scope_hash = "e".repeat(64);
        record.grant.id = call;
        record.grant.maximum_calls = 2;
        record.grant.scope_hash = record.consent.scope_hash.clone();
        store
            .authorize_conversation_feedback(
                &first.project,
                first.conversation,
                first.task,
                &record,
            )
            .unwrap();
        store
            .reserve_conversation_call(
                &first.project,
                first.task,
                call,
                &record.consent.scope_hash,
                &"f".repeat(64),
            )
            .unwrap();
        let mut evidence = first.source.evidence.clone().unwrap();
        evidence["context"]["subject"] = record.context.clone();
        evidence["context"]["scope_hash"] = json!(record.consent.scope_hash);
        let source = store
            .finish_conversation_call(
                &first.project,
                first.task,
                call,
                ConversationCallStatus::Completed,
                evidence,
            )
            .unwrap();
        let mut input = first.answer.clone();
        input.expected_context_digest =
            conversation_feedback_context_digest(&record.context).unwrap();
        let saved_first = first.save(&store).unwrap();
        assert!(
            store
                .answer_conversation_feedback_scope(
                    &first.project,
                    first.conversation,
                    first.task,
                    call,
                    &input,
                    &source
                )
                .is_err()
        );
        input.command_id = Uuid::new_v4();
        let saved_second = store
            .answer_conversation_feedback_scope(
                &first.project,
                first.conversation,
                first.task,
                call,
                &input,
                &source,
            )
            .unwrap();
        let second = Fixture {
            project: first.project.clone(),
            conversation: first.conversation,
            task: first.task,
            record,
            source,
            answer: input,
        };
        let request = store
            .create_conversation_scoped_feedback_human_request(
                &first.project,
                &first.human(&saved_first),
                &first.source,
                &saved_first,
            )
            .unwrap();
        assert!(
            store
                .create_conversation_scoped_feedback_human_request(
                    &second.project,
                    &second.human(&saved_second),
                    &second.source,
                    &saved_second
                )
                .is_err()
        );
        assert_eq!(
            store
                .conversation_human_requests(&first.project, first.conversation, first.task)
                .unwrap(),
            vec![request]
        );
        assert_eq!(
            store
                .conversation_call_budget(&first.project, first.task)
                .unwrap()
                .unwrap()
                .used_calls,
            2
        );
    }
}
