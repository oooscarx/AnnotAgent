//! Offline application integration checks; all projects and images live in `TempDir`.
#[path = "conversation_future_proposal_tests.rs"]
mod future_proposal;
#[path = "conversation_image_class_tests.rs"]
mod image_class;
use super::*;
use crate::conversation_feedback_intent::{
    ConversationFeedbackDecision, ConversationFeedbackReason,
};
use annotagent_core::{
    CoreResult, ModelCapabilities, ModelRequest, ModelToolCall, TokenUsage, UsageSource,
};
use annotagent_storage::{
    BeginConversationTask, ConversationCallGrant, ConversationCallStatus,
    ConversationHumanRequestInput, ConversationHumanRequestStatus, ConversationImageRef,
    ConversationMessageInput, SampleFeedbackReason, SampleFeedbackRevision, SampleOperation,
    WorkflowSampleTest,
};
use std::sync::Mutex;

const PROJECT: &str = "feedback-test";
const PROJECT_YAML: &str = "version: 1\nproject:\n  name: TEST candidate feedback\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n";

struct Fixture {
    temporary: tempfile::TempDir,
    app: LocalApplication,
    conversation: Uuid,
    task: BeginConversationTask,
    message: ConversationMessageInput,
    goal: ConversationMessageInput,
    sample: WorkflowSampleTest,
    human: ConversationHumanRequestInput,
}

fn fixture(duplicate_candidate_id: bool) -> Fixture {
    fixture_with_schema(duplicate_candidate_id, false)
}

fn fixture_with_schema(duplicate_candidate_id: bool, sealed_schema: bool) -> Fixture {
    fixture_with_schema_and_class_member(duplicate_candidate_id, sealed_schema, false)
}

fn fixture_with_schema_and_class_member(
    duplicate_candidate_id: bool,
    sealed_schema: bool,
    same_class: bool,
) -> Fixture {
    fixture_with_schema_class_and_box(duplicate_candidate_id, sealed_schema, same_class, None)
}

fn fixture_with_schema_class_and_box(
    duplicate_candidate_id: bool,
    sealed_schema: bool,
    same_class: bool,
    rect: Option<[f32; 4]>,
) -> Fixture {
    let temporary = tempfile::tempdir().unwrap();
    let app = LocalApplication::new(temporary.path()).unwrap();
    app.create_project(PROJECT, PROJECT_YAML).unwrap();
    let staging = temporary.path().join("TEST-import");
    std::fs::create_dir(&staging).unwrap();
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/robocup/images/synthetic-robocup.png"),
        staging.join("TEST.png"),
    )
    .unwrap();
    app.import_images(PROJECT, &staging).unwrap();
    let image = app.list_project_image_summaries(PROJECT).unwrap().remove(0);
    let conversation = app.create_project_conversation(PROJECT).unwrap();
    let goal = ConversationMessageInput {
        id: Uuid::new_v4(),
        text: "TEST find the cup".into(),
        image: None,
        reference: None,
    };
    app.append_project_conversation_message(PROJECT, conversation, &goal)
        .unwrap();
    let task = BeginConversationTask {
        id: Uuid::new_v4(),
        source_message_id: goal.id,
        schema_revision: app.project_goal(PROJECT).unwrap()["revision"]
            .as_str()
            .unwrap()
            .into(),
    };
    app.begin_conversation_task(PROJECT, conversation, &task)
        .unwrap();
    let (mut sample, mut human) = crate::conversation_human_requests::tests::fixture();
    sample.project_id = PROJECT.into();
    let mut baseline: annotagent_core::WorkflowDraft = serde_json::from_value(json!({
        "id":sample.draft_id,"project_id":PROJECT,"name":"TEST unchanged baseline",
        "status":"editing","nodes":[],"created_at":chrono::Utc::now(),"updated_at":chrono::Utc::now(),
    }))
    .unwrap();
    if sealed_schema {
        let schema = app.save_human_conversation_schema_draft(PROJECT, conversation, task.id, Uuid::new_v4(), &serde_json::from_value(json!({"decision":"draft","kind":"bounding_box","labels":["cup"],"multi_label":false,"attributes":{},"boundary_rules":["TEST original rule"],"rationale":"TEST human baseline"})).unwrap()).unwrap();
        baseline.annotation_schema = Some(annotagent_core::WorkflowSchemaBinding {
            schema_draft_id: schema.id.to_string(),
            revision: schema.revision,
            goal: schema.definition.goal,
            task: schema.definition.task,
            boundary_rules: schema.definition.boundary_rules,
        });
    }
    app.store.save_workflow_draft(&baseline).unwrap();
    let baseline = app.store.get_workflow_draft(&sample.draft_id).unwrap();
    sample.draft_revision = baseline.revision;
    sample.draft_content_hash = baseline.content_hash;
    sample.inputs[0].image_id = image.image_id.to_string();
    sample.inputs[0].content_hash = image.content_hash.clone();
    if let Some(rect) = rect {
        sample.report.samples[0].projection.final_candidates[0]
            .outcome
            .value =
            Some(serde_json::from_value(json!({"kind":"bounding_box","rect":rect})).unwrap());
    }
    let selected = sample.report.samples[0].projection.final_candidates[0].clone();
    let mut unrelated = selected.clone();
    unrelated.source_artifact_id = annotagent_core::ArtifactId(Uuid::new_v4());
    if !duplicate_candidate_id {
        unrelated.outcome.id = "TEST unrelated first candidate".into();
    }
    unrelated.outcome.label = "TEST unrelated label".into();
    sample.report.samples[0]
        .projection
        .final_candidates
        .insert(0, unrelated.clone());
    sample.report.samples[0].outcomes = vec![unrelated.outcome, selected.outcome.clone()];
    if same_class {
        let mut second = selected.clone();
        second.outcome.id = "TEST second cup".into();
        second.source_artifact_id = annotagent_core::ArtifactId(Uuid::new_v4());
        sample.report.samples[0]
            .outcomes
            .push(second.outcome.clone());
        sample.report.samples[0]
            .projection
            .final_candidates
            .push(second);
    }
    app.store.save_workflow_sample_test(&sample).unwrap();
    app.store
        .reserve_sample_operation_sealed(
            &SampleOperation {
                id: sample.id.clone(),
                project_id: PROJECT.into(),
                draft_id: sample.draft_id.clone(),
                authorization_fingerprint: "TEST fixture operation".into(),
                request: json!({"conversation":{"conversation_id":conversation,"task_id":task.id}}),
                status: "queued".into(),
                error: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
            sealed_schema
                .then(|| json!({"annotation_schema":baseline.annotation_schema}))
                .as_ref(),
        )
        .unwrap();
    let message = ConversationMessageInput {
        id: Uuid::new_v4(),
        text: "this box is too big".into(),
        image: Some(ConversationImageRef {
            image_id: image.image_id.to_string(),
            sha256: image.content_hash.clone(),
        }),
        reference: Some(ConversationSelectionRef::SampleCandidate {
            task_id: task.id,
            project_schema_revision: task.schema_revision.clone(),
            draft_id: sample.draft_id.clone(),
            draft_revision: sample.draft_revision,
            sample_test_id: sample.id.clone(),
            candidate_id: selected.outcome.id.clone(),
            source_artifact_id: selected.source_artifact_id.0,
        }),
    };
    app.append_project_conversation_message(PROJECT, conversation, &message)
        .unwrap();
    human.conversation_id = conversation;
    human.task_id = task.id;
    human.image_id = image.image_id.to_string();
    human.content_hash = image.content_hash;
    human.outcome_id = selected.outcome.id;
    Fixture {
        temporary,
        app,
        conversation,
        task,
        message,
        goal,
        sample,
        human,
    }
}

impl Fixture {
    fn context(&self) -> ConversationFeedbackContext {
        self.app
            .conversation_feedback_context(
                PROJECT,
                self.conversation,
                self.task.id,
                self.message.id,
            )
            .unwrap()
    }

    fn execution(&self) -> ConversationSchemaExecution {
        ConversationSchemaExecution {
            conversation_id: self.conversation,
            task_id: self.task.id,
            call_id: Uuid::new_v4(),
            remote_model: "TEST offline text model".into(),
            scope_hash: "f".repeat(64),
        }
    }

    fn authorize(&self, execution: &ConversationSchemaExecution) {
        self.app
            .store
            .authorize_conversation_calls(
                &self.app.conversation_project_identity(PROJECT).unwrap(),
                &ConversationCallGrant {
                    id: Uuid::new_v4(),
                    task_id: self.task.id,
                    scope_hash: execution.scope_hash.clone(),
                    maximum_calls: 2,
                    expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
                },
            )
            .unwrap();
    }

    fn assert_no_inference_or_feedback(&self, provider: &TestProvider) {
        assert!(provider.requests.lock().unwrap().is_empty());
        let owner = self.app.conversation_project_identity(PROJECT).unwrap();
        assert!(
            self.app
                .store
                .conversation_call_history(&owner, self.task.id)
                .unwrap()
                .is_empty()
        );
        assert!(
            self.app
                .store
                .sample_feedback(&self.sample.id, &self.human.image_id)
                .unwrap()
                .is_empty()
        );
    }
}

struct TestProvider {
    requests: Mutex<Vec<ModelRequest>>,
    cancel_after_response: bool,
    arguments: serde_json::Value,
}

#[async_trait::async_trait]
impl VisionModelProvider for TestProvider {
    fn name(&self) -> &str {
        "TEST offline candidate feedback"
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            vision: false,
            tool_calls: true,
            json_schema: true,
            usage_reporting: true,
            multi_image: false,
        }
    }

    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<ModelResponse> {
        self.requests.lock().unwrap().push(request);
        if self.cancel_after_response {
            cancellation.cancel();
        }
        Ok(ModelResponse {
            content: None,
            tool_calls: vec![ModelToolCall {
                id: "TEST one proposal".into(),
                name: "propose_candidate_feedback".into(),
                arguments: self.arguments.clone(),
            }],
            usage: TokenUsage::known(120, 80, UsageSource::Mock),
            request_id: Some("TEST feedback usage receipt".into()),
            provider_metadata: std::collections::BTreeMap::default(),
        })
    }
}

fn provider() -> TestProvider {
    TestProvider {
        requests: Mutex::new(Vec::new()),
        cancel_after_response: false,
        arguments: json!({
            "decision":"request_correction","reason":"poor_boundary",
            "question":"Please correct the selected box boundary.",
            "rationale":"The saved message says the box is too big; its pixels were not inspected.",
        }),
    }
}

fn feedback_authorization(
    fixture: &Fixture,
) -> annotagent_storage::ConversationFeedbackAuthorizationRecord {
    let call = Uuid::new_v4();
    let expiry = chrono::Utc::now() + chrono::Duration::minutes(20);
    annotagent_storage::ConversationFeedbackAuthorizationRecord {
        consent: annotagent_storage::ConversationFeedbackAuthorization {
            call_id: call,
            message_id: fixture.message.id,
            model_id: annotagent_core::ModelProfileId(Uuid::new_v4()),
            previous_grant_id: None,
            scope_hash: "f".repeat(64),
            expires_at: expiry,
            allow_unknown_cost: true,
        },
        context: serde_json::to_value(fixture.context()).unwrap(),
        summary: json!({"model_name":"TEST feedback","remote_model":"TEST offline text model","destination":"TEST offline provider"}),
        grant: ConversationCallGrant {
            id: call,
            task_id: fixture.task.id,
            scope_hash: "f".repeat(64),
            maximum_calls: 1,
            expires_at: expiry,
        },
    }
}

#[test]
fn persisted_feedback_context_comparison_keeps_full_identity_and_unknown_field_checks() {
    let f = fixture_with_schema_class_and_box(false, false, false, Some([0.12, 0.2, 0.16, 0.22]));
    let context = f.context();
    let original = serde_json::to_value(&context).unwrap();
    let persisted: serde_json::Value =
        serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    assert_ne!(original, persisted);
    assert!(context.matches_saved_context(&original).unwrap());
    assert!(context.matches_saved_context(&persisted).unwrap());
    for (path, replacement) in [
        ("/message/input/text", json!("TEST changed intent")),
        ("/message/input/id", json!(Uuid::new_v4())),
        (
            "/message/input/reference/source_artifact_id",
            json!(Uuid::new_v4()),
        ),
        ("/message/input/image/sha256", json!("0".repeat(64))),
        ("/candidate/source_artifact_id", json!(Uuid::new_v4())),
        ("/candidate/outcome/id", json!("TEST different candidate")),
        ("/candidate/outcome/label", json!("bottle")),
        ("/candidate/outcome/confidence", json!(0.5)),
        ("/candidate/outcome/value/rect/0", json!(0.13)),
        ("/candidate/lineage_id", json!("TEST different lineage")),
        (
            "/candidate/geometry",
            json!("TEST changed geometry evidence"),
        ),
        (
            "/candidate/localization",
            json!("TEST changed localization evidence"),
        ),
        (
            "/candidate/final_status",
            json!("TEST changed status evidence"),
        ),
        ("/expected_feedback_sequence", json!(1)),
        ("/sample_content_hash", json!("0".repeat(64))),
        ("/pixels_supplied", json!(true)),
    ] {
        let mut changed = persisted.clone();
        *changed.pointer_mut(path).unwrap() = replacement;
        assert!(
            !context.matches_saved_context(&changed).unwrap_or(false),
            "Changed field was accepted: {path}"
        );
    }
    for path in [
        "",
        "/message",
        "/candidate",
        "/candidate/outcome",
        "/candidate/outcome/value",
    ] {
        let mut changed = persisted.clone();
        changed
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unsupported_authority".into(), json!("delete all"));
        assert!(
            !context.matches_saved_context(&changed).unwrap_or(false),
            "Unknown field was accepted at {path}"
        );
    }
    let execution = f.execution();
    let envelope = json!({"contract":"conversation-feedback-v1","subject":persisted,"remote_model":execution.remote_model,"scope_hash":execution.scope_hash});
    assert!(
        context
            .matches_saved_envelope(&envelope, &execution)
            .unwrap()
    );
    for key in ["contract", "remote_model", "scope_hash"] {
        let mut changed = envelope.clone();
        changed[key] = json!("TEST substituted scope");
        assert!(
            !context
                .matches_saved_envelope(&changed, &execution)
                .unwrap()
        );
    }
    let mut changed = envelope;
    changed["unsupported_authority"] = json!(true);
    assert!(
        !context
            .matches_saved_envelope(&changed, &execution)
            .unwrap()
    );
}

#[test]
fn authorization_restores_original_message_and_scope_without_inference_after_restart() {
    let fixture = fixture(false);
    let record = feedback_authorization(&fixture);
    let call = record.consent.call_id;
    fixture
        .app
        .authorize_conversation_feedback(PROJECT, fixture.conversation, fixture.task.id, &record)
        .unwrap();
    let status = fixture
        .app
        .conversation_feedback_status(PROJECT, fixture.conversation, fixture.task.id, call)
        .unwrap();
    assert!(status["receipt"].is_null());
    assert!(status["decision"].is_null());
    assert_eq!(status["authorization"]["context"], record.context);
    let app = LocalApplication::new(fixture.temporary.path()).unwrap();
    assert_eq!(
        app.conversation_feedback_status(PROJECT, fixture.conversation, fixture.task.id, call)
            .unwrap(),
        status
    );
    assert_eq!(
        app.conversation_feedback_for_message(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            fixture.message.id
        )
        .unwrap(),
        Some(record.clone())
    );
    assert!(
        app.conversation_feedback_authorization(PROJECT, Uuid::new_v4(), fixture.task.id, call)
            .is_err()
    );
    let mut conflict = record.clone();
    conflict.context["pixels_supplied"] = json!(true);
    assert!(
        app.authorize_conversation_feedback(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            &conflict
        )
        .is_err()
    );
    let image = fixture
        .app
        .project_image_path(
            PROJECT,
            ImageId(Uuid::parse_str(&fixture.human.image_id).unwrap()),
        )
        .unwrap();
    std::fs::write(image, b"TEST changed original image").unwrap();
    assert_eq!(
        app.authorize_conversation_feedback(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            &record
        )
        .unwrap(),
        record
    );
    assert_eq!(
        app.conversation_feedback_status(PROJECT, fixture.conversation, fixture.task.id, call)
            .unwrap(),
        status
    );
    assert!(
        app.conversation_schema_calls(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn interrupted_feedback_restores_unknown_receipt_with_original_subject_and_budget() {
    let fixture = fixture(false);
    let record = feedback_authorization(&fixture);
    let call = record.consent.call_id;
    fixture
        .app
        .authorize_conversation_feedback(PROJECT, fixture.conversation, fixture.task.id, &record)
        .unwrap();
    let owner = fixture.app.conversation_project_identity(PROJECT).unwrap();
    fixture
        .app
        .store
        .reserve_conversation_call(
            &owner,
            fixture.task.id,
            call,
            &record.consent.scope_hash,
            &"c".repeat(64),
        )
        .unwrap();
    let app = LocalApplication::new(fixture.temporary.path()).unwrap();
    let status = app
        .conversation_feedback_status(PROJECT, fixture.conversation, fixture.task.id, call)
        .unwrap();
    assert_eq!(status["receipt"]["status"], "in_doubt");
    assert!(status["decision"].is_null());
    assert!(status["error"].as_str().unwrap().contains("unknown"));
    assert_eq!(status["authorization"]["context"], record.context);
    assert_eq!(status["authorization"]["summary"], record.summary);
    assert_eq!(
        app.conversation_builder_budget(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .used_calls,
        1
    );
    assert_eq!(
        app.conversation_schema_calls(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn saved_feedback_status_separates_valid_decision_from_human_answer_and_cancellation() {
    let fixture = fixture(false);
    let record = feedback_authorization(&fixture);
    let call = record.consent.call_id;
    fixture
        .app
        .authorize_conversation_feedback(PROJECT, fixture.conversation, fixture.task.id, &record)
        .unwrap();
    let mut execution = fixture.execution();
    execution.call_id = call;
    let provider = provider();
    fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &fixture.context(),
            &provider,
            CancellationToken::default(),
        )
        .await
        .unwrap();
    let status = fixture
        .app
        .conversation_feedback_status(PROJECT, fixture.conversation, fixture.task.id, call)
        .unwrap();
    assert_eq!(status["decision"]["Ok"]["reason"], "poor_boundary");
    assert_eq!(status["receipt"]["status"], "completed");
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert!(
        fixture
            .app
            .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .is_empty()
    );
    fixture
        .app
        .cancel_conversation_schema(PROJECT, fixture.conversation, fixture.task.id, call)
        .unwrap();
    let stopped = fixture
        .app
        .conversation_feedback_status(PROJECT, fixture.conversation, fixture.task.id, call)
        .unwrap();
    assert_eq!(stopped["cancelled"], true);
    assert!(
        stopped["decision"]["Err"]
            .as_str()
            .unwrap()
            .contains("cancelled")
    );
    assert!(
        fixture
            .app
            .prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call
            )
            .is_err()
    );
}

#[test]
fn frozen_message_resolves_exact_candidate_and_never_guesses_from_current_context() {
    let fixture = fixture(false);
    let expected = fixture.sample.report.samples[0].projection.final_candidates[1].clone();
    let context = fixture.context();
    assert_eq!(context.message.input, fixture.message);
    assert_eq!(context.candidate, expected);
    assert_ne!(context.candidate.outcome.label, "TEST unrelated label");
    assert!(!context.pixels_supplied);
    assert_eq!(context.expected_feedback_sequence, 0);
    let digest = context.digest().unwrap();
    let latest = ConversationMessageInput {
        id: Uuid::new_v4(),
        text: "TEST remove this other thing".into(),
        image: fixture.message.image.clone(),
        reference: None,
    };
    fixture
        .app
        .append_project_conversation_message(PROJECT, fixture.conversation, &latest)
        .unwrap();
    assert_eq!(fixture.context(), context);
    assert_eq!(fixture.context().digest().unwrap(), digest);
    for message in [fixture.goal.id, latest.id, Uuid::new_v4()] {
        assert!(
            fixture
                .app
                .conversation_feedback_context(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    message
                )
                .is_err()
        );
    }
}

#[test]
fn artifact_identity_disambiguates_identical_outcome_ids() {
    let fixture = fixture(true);
    let selected = &fixture.sample.report.samples[0].projection.final_candidates[1];
    let context = fixture.context();
    assert_eq!(
        context.candidate.source_artifact_id,
        selected.source_artifact_id
    );
    assert_eq!(&context.candidate, selected);
}

#[tokio::test]
async fn foreign_wrong_task_and_changed_pixels_fail_before_inference() {
    let fixture = fixture(false);
    let provider = provider();
    let context = fixture.context();
    let execution = fixture.execution();
    fixture.authorize(&execution);
    fixture
        .app
        .create_project("foreign-test", PROJECT_YAML)
        .unwrap();
    for (project, task) in [("foreign-test", fixture.task.id), (PROJECT, Uuid::new_v4())] {
        assert!(
            fixture
                .app
                .conversation_feedback_context(
                    project,
                    fixture.conversation,
                    task,
                    fixture.message.id
                )
                .is_err()
        );
        let mut wrong = execution.clone();
        wrong.task_id = task;
        assert!(
            fixture
                .app
                .execute_conversation_feedback(
                    project,
                    &wrong,
                    &context,
                    &provider,
                    CancellationToken::new()
                )
                .await
                .is_err()
        );
    }
    let image = fixture
        .app
        .project_image_path(
            PROJECT,
            ImageId(Uuid::parse_str(&fixture.human.image_id).unwrap()),
        )
        .unwrap();
    std::fs::write(image, b"TEST changed image bytes").unwrap();
    assert!(
        fixture
            .app
            .conversation_feedback_context(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                fixture.message.id
            )
            .is_err()
    );
    assert!(
        fixture
            .app
            .execute_conversation_feedback(
                PROJECT,
                &execution,
                &context,
                &provider,
                CancellationToken::new()
            )
            .await
            .is_err()
    );
    fixture.assert_no_inference_or_feedback(&provider);
}

#[tokio::test]
async fn changed_feedback_sequence_invalidates_frozen_context_without_spending() {
    let fixture = fixture(false);
    let provider = provider();
    let before = fixture.context();
    let execution = fixture.execution();
    fixture.authorize(&execution);
    let feedback = SampleFeedbackRevision {
        revision_id: Uuid::new_v4().to_string(),
        sample_test_id: fixture.sample.id.clone(),
        image_id: fixture.human.image_id.clone(),
        sequence: 1,
        reason: SampleFeedbackReason::CannotJudge,
        outcome_id: Some(fixture.human.outcome_id.clone()),
        corrected_value: None,
        corrected_label: None,
        addition_id: None,
        note: "TEST another window added feedback".into(),
        created_at: chrono::Utc::now(),
    };
    fixture.app.store.save_sample_feedback(&feedback).unwrap();
    let after = fixture.context();
    assert_eq!(after.expected_feedback_sequence, 1);
    assert_ne!(after.digest().unwrap(), before.digest().unwrap());
    assert!(
        fixture
            .app
            .execute_conversation_feedback(
                PROJECT,
                &execution,
                &before,
                &provider,
                CancellationToken::new()
            )
            .await
            .is_err()
    );
    assert!(provider.requests.lock().unwrap().is_empty());
    assert_eq!(
        fixture
            .app
            .conversation_builder_budget(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .used_calls,
        0
    );
    assert_eq!(
        fixture
            .app
            .store
            .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
            .unwrap(),
        vec![feedback]
    );
}

#[tokio::test]
async fn authorized_interpretation_records_one_call_and_replays_after_image_change_and_restart() {
    let mut fixture = fixture(false);
    let provider = provider();
    let context = fixture.context();
    let execution = fixture.execution();
    assert!(
        fixture
            .app
            .execute_conversation_feedback(
                PROJECT,
                &execution,
                &context,
                &provider,
                CancellationToken::new()
            )
            .await
            .is_err()
    );
    fixture.assert_no_inference_or_feedback(&provider);
    fixture.authorize(&execution);
    let baseline = fixture
        .app
        .store
        .get_workflow_draft(&fixture.sample.draft_id)
        .unwrap();
    let project_before = std::fs::read(fixture.app.project_path(PROJECT).unwrap()).unwrap();
    let cancellation = CancellationToken::new();
    let result = fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &context,
            &provider,
            cancellation.clone(),
        )
        .await
        .unwrap();
    assert!(
        !cancellation.is_cancelled(),
        "Settling a successful call must not cancel the caller's token"
    );
    assert_eq!(result.receipt.status, ConversationCallStatus::Completed);
    assert!(matches!(
        &result.decision,
        Ok(ConversationFeedbackDecision::RequestCorrection {
            reason: ConversationFeedbackReason::PoorBoundary,
            ..
        })
    ));
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    {
        let requests = provider.requests.lock().unwrap();
        assert!(requests[0].images.is_empty());
        let sent: serde_json::Value =
            serde_json::from_str(&requests[0].messages[1].content).unwrap();
        assert_eq!(
            sent["saved_candidate_context"],
            serde_json::to_value(&context).unwrap()
        );
    }
    let evidence = result.receipt.evidence.as_ref().unwrap();
    assert_eq!(evidence["phase"], "feedback_text");
    assert_eq!(
        evidence["response"]["request_id"],
        "TEST feedback usage receipt"
    );
    assert_eq!(evidence["response"]["usage"]["total_tokens"], 200);
    assert_eq!(
        evidence["context"]["subject"],
        serde_json::to_value(&context).unwrap()
    );
    let image = fixture
        .app
        .project_image_path(
            PROJECT,
            ImageId(Uuid::parse_str(&fixture.human.image_id).unwrap()),
        )
        .unwrap();
    std::fs::write(image, b"TEST changed pixels after completion").unwrap();
    drop(fixture.app);
    fixture.app = LocalApplication::new(fixture.temporary.path()).unwrap();
    assert!(
        fixture
            .app
            .conversation_feedback_context(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                fixture.message.id
            )
            .is_err()
    );
    let replay = fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &context,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&replay).unwrap(),
        serde_json::to_value(&result).unwrap()
    );
    let reread = fixture
        .app
        .read_conversation_feedback(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            execution.call_id,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(reread).unwrap(),
        serde_json::to_value(result).unwrap()
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert_eq!(
        fixture
            .app
            .conversation_builder_budget(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .used_calls,
        1
    );
    assert_eq!(
        fixture
            .app
            .store
            .get_workflow_sample_test_by_id(&fixture.sample.id)
            .unwrap()
            .unwrap(),
        fixture.sample
    );
    assert_eq!(
        fixture.app.store.get_workflow_draft(&baseline.id).unwrap(),
        baseline
    );
    assert!(
        fixture
            .app
            .store
            .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .app
            .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read(fixture.app.project_path(PROJECT).unwrap()).unwrap(),
        project_before
    );
}

#[tokio::test]
async fn explicit_request_preparation_is_idempotent_and_never_submits_feedback_or_repairs() {
    let fixture = fixture(false);
    let provider = provider();
    let context = fixture.context();
    let execution = fixture.execution();
    fixture.authorize(&execution);
    let baseline = fixture
        .app
        .store
        .get_workflow_draft(&fixture.sample.draft_id)
        .unwrap();
    fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &context,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let first = fixture
        .app
        .prepare_conversation_feedback_request(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            execution.call_id,
        )
        .unwrap()
        .unwrap();
    assert_eq!(first.status, ConversationHumanRequestStatus::Pending);
    assert_eq!(first.input.outcome_id, fixture.human.outcome_id);
    assert_eq!(first.input.content_hash, fixture.human.content_hash);
    assert_eq!(first.input.expected_feedback_sequence, 0);
    assert_eq!(first.input.reason_code, "poor_boundary");
    assert!(first.answer.is_none());
    assert!(first.resume_draft_id.is_none());
    let repeated = fixture
        .app
        .prepare_conversation_feedback_request(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            execution.call_id,
        )
        .unwrap()
        .unwrap();
    assert_eq!(repeated, first);
    assert_eq!(
        fixture
            .app
            .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap(),
        vec![first.clone()]
    );
    assert!(
        fixture
            .app
            .store
            .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .app
            .store
            .sample_plan_evidence(&first.input.resume_checkpoint_ref.to_string())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fixture.app.store.get_workflow_draft(&baseline.id).unwrap(),
        baseline
    );
    assert_eq!(
        fixture
            .app
            .store
            .get_workflow_sample_test_by_id(&fixture.sample.id)
            .unwrap()
            .unwrap(),
        fixture.sample
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn ambiguous_removal_creates_no_human_correction_and_cancellation_cannot_reactivate_proposal()
{
    let fixture = fixture(false);
    let mut provider = provider();
    provider.arguments = json!({"decision":"clarify_scope","question":"What do you mean by remove this?","rationale":"The saved message does not establish a false positive or deletion scope."});
    let mut message = fixture.message.clone();
    message.id = Uuid::new_v4();
    message.text = "remove this".into();
    fixture
        .app
        .append_project_conversation_message(PROJECT, fixture.conversation, &message)
        .unwrap();
    let context = fixture
        .app
        .conversation_feedback_context(PROJECT, fixture.conversation, fixture.task.id, message.id)
        .unwrap();
    let execution = fixture.execution();
    fixture.authorize(&execution);
    let result = fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &context,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(matches!(
        result.decision,
        Ok(ConversationFeedbackDecision::ClarifyScope { .. })
    ));
    assert!(
        fixture
            .app
            .prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                execution.call_id
            )
            .unwrap()
            .is_none()
    );

    provider.cancel_after_response = true;
    provider.arguments = json!({"decision":"request_correction","reason":"poor_boundary","question":"Please correct the selected boundary.","rationale":"The message says the selected box is too big."});
    let context = fixture.context();
    let cancelled_execution = fixture.execution();
    let result = fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &cancelled_execution,
            &context,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(result.decision.as_ref().unwrap_err().contains("cancelled"));
    let evidence = result.receipt.evidence.as_ref().unwrap();
    assert_eq!(evidence["cancelled"], true);
    assert_eq!(evidence["response"]["usage"]["total_tokens"], 200);
    assert!(
        fixture
            .app
            .prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                cancelled_execution.call_id
            )
            .is_err()
    );
    let replay = fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &cancelled_execution,
            &context,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(replay.decision.unwrap_err().contains("cancelled"));
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    assert!(
        fixture
            .app
            .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .app
            .store
            .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn pending_and_deferred_human_work_blocks_interpretation_without_changing_request() {
    let fixture = fixture(false);
    let provider = provider();
    let context = fixture.context();
    let execution = fixture.execution();
    fixture.authorize(&execution);
    let saved = fixture
        .app
        .create_conversation_human_request(PROJECT, &fixture.human)
        .unwrap();
    assert_eq!(saved.status, ConversationHumanRequestStatus::Pending);
    for deferred in [false, true] {
        let expected = if deferred {
            fixture
                .app
                .set_conversation_human_deferral(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    fixture.human.id,
                    &annotagent_storage::ConversationHumanDeferral {
                        command_id: Uuid::new_v4(),
                        expected_revision: 0,
                        deferred: true,
                    },
                )
                .unwrap()
        } else {
            saved.clone()
        };
        let error = fixture
            .app
            .execute_conversation_feedback(
                PROJECT,
                &execution,
                &context,
                &provider,
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("waiting for human input"),
            "{error}"
        );
        assert_eq!(
            fixture
                .app
                .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
                .unwrap(),
            vec![expected]
        );
        fixture.assert_no_inference_or_feedback(&provider);
    }
    assert_eq!(
        fixture
            .app
            .conversation_builder_budget(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .used_calls,
        0
    );
}

#[tokio::test]
async fn predictable_request_identity_cannot_restore_a_conflicting_question_or_subject() {
    for change_subject in [false, true] {
        let fixture = fixture(false);
        let provider = provider();
        let context = fixture.context();
        let execution = fixture.execution();
        fixture.authorize(&execution);
        fixture
            .app
            .execute_conversation_feedback(
                PROJECT,
                &execution,
                &context,
                &provider,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let mut conflicting = fixture.human.clone();
        conflicting.id = Uuid::new_v5(&execution.call_id, b"feedback-human-request-v1");
        conflicting.resume_checkpoint_ref = Uuid::new_v5(&conflicting.id, b"prepared-repair-draft");
        conflicting.reason_code = "poor_boundary".into();
        conflicting.question = "Please correct the selected box boundary.".into();
        if change_subject {
            conflicting.outcome_id = fixture.sample.report.samples[0].projection.final_candidates
                [0]
            .outcome
            .id
            .clone();
        } else {
            conflicting.question = "TEST conflicting question under a predictable ID".into();
        }
        let saved = fixture
            .app
            .create_conversation_human_request(PROJECT, &conflicting)
            .unwrap();
        assert!(
            fixture
                .app
                .prepare_conversation_feedback_request(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    execution.call_id,
                )
                .is_err(),
            "A reused request ID with changed frozen input must conflict"
        );
        assert_eq!(
            fixture
                .app
                .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
                .unwrap(),
            vec![saved]
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
        assert!(
            fixture
                .app
                .store
                .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
                .unwrap()
                .is_empty()
        );
        assert!(
            fixture
                .app
                .store
                .sample_plan_evidence(&conflicting.resume_checkpoint_ref.to_string())
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn concurrent_preparation_admits_at_most_one_pending_request_per_sample_image() {
    let fixture = fixture(false);
    let provider = provider();
    let first_context = fixture.context();
    let first = fixture.execution();
    fixture.authorize(&first);
    fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &first,
            &first_context,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let mut second_message = fixture.message.clone();
    second_message.id = Uuid::new_v4();
    second_message.text = "this selected box extends too far beyond the cup".into();
    fixture
        .app
        .append_project_conversation_message(PROJECT, fixture.conversation, &second_message)
        .unwrap();
    let second_context = fixture
        .app
        .conversation_feedback_context(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            second_message.id,
        )
        .unwrap();
    let second = fixture.execution();
    fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &second,
            &second_context,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let barrier = std::sync::Barrier::new(3);
    let results = std::thread::scope(|threads| {
        let left = threads.spawn(|| {
            barrier.wait();
            fixture.app.prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                first.call_id,
            )
        });
        let right = threads.spawn(|| {
            barrier.wait();
            fixture.app.prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                second.call_id,
            )
        });
        barrier.wait();
        [left.join().unwrap(), right.join().unwrap()]
    });
    let saved = fixture
        .app
        .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
        .unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].status, ConversationHumanRequestStatus::Pending);
    assert_eq!(saved[0].input.image_id, fixture.human.image_id);
    assert_eq!(saved[0].input.expected_feedback_sequence, 0);
    assert!(results.iter().any(|result| matches!(result, Ok(Some(_)))));
    for result in results {
        if let Ok(Some(request)) = result {
            assert_eq!(request, saved[0]);
        }
    }
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    assert!(
        fixture
            .app
            .store
            .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
            .unwrap()
            .is_empty()
    );
}

struct GateProvider {
    inner: TestProvider,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

async fn scope_fixture() -> (
    Fixture,
    TestProvider,
    annotagent_storage::ConversationFeedbackAuthorizationRecord,
) {
    scope_fixture_from(fixture(false)).await
}

async fn scope_fixture_from(
    fixture: Fixture,
) -> (
    Fixture,
    TestProvider,
    annotagent_storage::ConversationFeedbackAuthorizationRecord,
) {
    let record = feedback_authorization(&fixture);
    fixture
        .app
        .authorize_conversation_feedback(PROJECT, fixture.conversation, fixture.task.id, &record)
        .unwrap();
    let mut provider = provider();
    provider.arguments = json!({"decision":"clarify_scope","question":"Which scope do you mean?","rationale":"This message is not permission to remove a category."});
    let execution = ConversationSchemaExecution {
        call_id: record.consent.call_id,
        ..fixture.execution()
    };
    fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &fixture.context(),
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    (fixture, provider, record)
}

fn scope_input(
    record: &annotagent_storage::ConversationFeedbackAuthorizationRecord,
    choice: &serde_json::Value,
) -> annotagent_storage::ConversationFeedbackScopeAnswerInput {
    serde_json::from_value(json!({"command_id":Uuid::new_v4(),"expected_context_digest":annotagent_storage::conversation_feedback_context_digest(&record.context).unwrap(),"choice":choice})).unwrap()
}

#[tokio::test]
async fn future_schema_is_an_independent_human_draft_and_restores_after_later_edits() {
    for kind in ["classification", "bounding_box"] {
        let (fixture, provider, source) =
            scope_fixture_from(fixture_with_schema(false, true)).await;
        let app = &fixture.app;
        let call = source.consent.call_id;
        let answer = scope_input(&source, &json!({"scope":"project_future_rule"}));
        app.answer_conversation_feedback_scope(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            call,
            &answer,
        )
        .unwrap();
        let preview = app
            .conversation_future_schema(PROJECT, fixture.conversation, fixture.task.id, call)
            .unwrap();
        assert!(preview.record.is_none());
        let owner = app.conversation_project_identity(PROJECT).unwrap();
        let calls = app
            .store
            .conversation_call_history(&owner, fixture.task.id)
            .unwrap();
        let budget = app
            .conversation_task_budget(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap();
        let old_draft = app
            .store
            .get_workflow_draft(&fixture.sample.draft_id)
            .unwrap();
        let goal = app.project_goal(PROJECT).unwrap();
        let request: crate::ConversationFutureSchemaRequest = serde_json::from_value(json!({
            "command_id":Uuid::new_v4(),"expected_scope_answer_command_id":answer.command_id,
            "expected_context_digest":answer.expected_context_digest,
            "base_schema_id":preview.base_schema.id,"base_schema_revision":preview.base_schema.revision,
            "goal":"TEST future images distinguish cups and plates, excluding bottles",
            "decision":{"decision":"draft","kind":kind,"labels":["cup","plate"],"multi_label":false,"attributes":{},"boundary_rules":["TEST exclude bottles"],"rationale":"TEST explicit human future intent"}
        })).unwrap();
        let result = app
            .save_conversation_future_schema(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call,
                &request,
            )
            .unwrap();
        let schema = result.schema.unwrap();
        assert_ne!(schema.id, preview.base_schema.id);
        assert_eq!(schema.revision, 1);
        assert_eq!(
            schema.definition.task.id,
            preview.base_schema.definition.task.id
        );
        assert_eq!(schema.definition.goal, request.goal);
        assert_eq!(
            app.conversation_schema_draft(PROJECT, preview.base_schema.id, None)
                .unwrap(),
            preview.base_schema
        );
        assert_eq!(
            app.store
                .get_workflow_draft(&fixture.sample.draft_id)
                .unwrap(),
            old_draft
        );
        assert_eq!(
            app.store
                .get_workflow_sample_test_by_id(&fixture.sample.id)
                .unwrap()
                .unwrap(),
            fixture.sample
        );
        assert_eq!(app.project_goal(PROJECT).unwrap(), goal);
        assert_eq!(
            app.store
                .conversation_call_history(&owner, fixture.task.id)
                .unwrap(),
            calls
        );
        assert_eq!(
            app.conversation_task_budget(PROJECT, fixture.conversation, fixture.task.id)
                .unwrap(),
            budget
        );
        assert!(
            app.conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
                .unwrap()
                .is_empty()
        );
        assert!(
            app.store
                .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
                .unwrap()
                .is_empty()
        );
        let changed: crate::ConversationSchemaDecision = serde_json::from_value(json!({"decision":"draft","kind":kind,"labels":["cup"],"multi_label":false,"attributes":{},"boundary_rules":["TEST subsequent human revision"],"rationale":"TEST preserve later edit"})).unwrap();
        let edited = app
            .revise_conversation_schema_draft(PROJECT, schema.id, Uuid::new_v4(), 1, &changed)
            .unwrap();
        app.cancel_conversation_schema(PROJECT, fixture.conversation, fixture.task.id, call)
            .unwrap();
        let reopened = LocalApplication::new(fixture.temporary.path()).unwrap();
        let restored = reopened
            .save_conversation_future_schema(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call,
                &request,
            )
            .unwrap();
        assert_eq!(restored.schema.unwrap(), edited);
        assert_eq!(restored.record, result.record);
        let mut conflict = request.clone();
        conflict.goal = "TEST conflicting retry".into();
        assert!(
            reopened
                .save_conversation_future_schema(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    call,
                    &conflict
                )
                .is_err()
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn future_schema_preview_requires_explicit_scope_exact_sealed_base_and_live_source() {
    for failure in ["scope", "unsealed", "cancelled", "stale", "foreign"] {
        let (fixture, provider, source) =
            scope_fixture_from(fixture_with_schema(false, failure != "unsealed")).await;
        let app = &fixture.app;
        let call = source.consent.call_id;
        let answer = scope_input(
            &source,
            &json!({"scope":if failure == "scope" {"current_image_class"} else {"project_future_rule"}}),
        );
        app.answer_conversation_feedback_scope(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            call,
            &answer,
        )
        .unwrap();
        if failure == "cancelled" {
            app.cancel_conversation_schema(PROJECT, fixture.conversation, fixture.task.id, call)
                .unwrap();
        }
        if failure == "stale" {
            let base = app
                .conversation_future_schema(PROJECT, fixture.conversation, fixture.task.id, call)
                .unwrap()
                .base_schema;
            let changed = serde_json::from_value(json!({"decision":"draft","kind":"bounding_box","labels":["plate"],"multi_label":false,"attributes":{},"boundary_rules":[],"rationale":"TEST newer untested semantics"})).unwrap();
            app.revise_conversation_schema_draft(
                PROJECT,
                base.id,
                Uuid::new_v4(),
                base.revision,
                &changed,
            )
            .unwrap();
        }
        let conversation = if failure == "foreign" {
            Uuid::new_v4()
        } else {
            fixture.conversation
        };
        assert!(
            app.conversation_future_schema(PROJECT, conversation, fixture.task.id, call)
                .is_err(),
            "{failure}"
        );
        let owner = app.conversation_project_identity(PROJECT).unwrap();
        assert!(
            app.store
                .future_schema_draft(&owner, fixture.task.id, call)
                .unwrap()
                .is_none()
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    }
}

#[test]
fn builder_history_keeps_schema_identity_and_revision_paired_with_saved_operation() {
    let fixture = fixture_with_schema(false, true);
    let owner = fixture.app.conversation_project_identity(PROJECT).unwrap();
    let binding = fixture
        .app
        .store
        .get_workflow_draft(&fixture.sample.draft_id)
        .unwrap()
        .annotation_schema
        .unwrap();
    let schema_id = Uuid::parse_str(&binding.schema_draft_id).unwrap();
    let operation = Uuid::new_v4();
    fixture
        .app
        .store
        .reserve_conversation_builder_with_schema(
            &owner,
            fixture.task.id,
            operation,
            &"a".repeat(64),
            schema_id,
            binding.revision,
        )
        .unwrap();
    let reserved = fixture
        .app
        .conversation_builder_history(PROJECT, fixture.conversation, fixture.task.id)
        .unwrap();
    assert_eq!(reserved["items"][0]["schema_id"], schema_id.to_string());
    assert_eq!(reserved["items"][0]["schema_revision"], binding.revision);
    assert_eq!(reserved["items"][0]["operation"]["status"], "reserved");
    assert!(
        reserved["items"][0]["session"].is_null(),
        "The identity must survive even before the seed session exists"
    );
    fixture
        .app
        .store
        .settle_conversation_builder(
            &owner,
            fixture.task.id,
            operation,
            false,
            &json!({"error":"TEST guard interrupted before seed persistence"}),
        )
        .unwrap();
    let history = fixture
        .app
        .conversation_builder_history(PROJECT, fixture.conversation, fixture.task.id)
        .unwrap();
    assert_eq!(history["items"][0]["schema_id"], schema_id.to_string());
    assert_eq!(history["items"][0]["schema_revision"], 1);
}

#[tokio::test]
async fn wider_scope_answers_are_saved_intent_not_implicit_rules_or_bulk_edits() {
    for scope in ["current_image_class", "project_future_rule"] {
        let (fixture, provider, record) = scope_fixture().await;
        let call = record.consent.call_id;
        let input = scope_input(&record, &json!({"scope":scope}));
        let before = fixture.app.project_goal(PROJECT).unwrap();
        let owner = fixture.app.conversation_project_identity(PROJECT).unwrap();
        let calls = fixture
            .app
            .store
            .conversation_call_history(&owner, fixture.task.id)
            .unwrap();
        let answer = fixture
            .app
            .answer_conversation_feedback_scope(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call,
                &input,
            )
            .unwrap();
        assert_eq!(
            fixture
                .app
                .answer_conversation_feedback_scope(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    call,
                    &input
                )
                .unwrap(),
            answer
        );
        assert!(
            fixture
                .app
                .prepare_conversation_feedback_request(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    call
                )
                .unwrap()
                .is_none()
        );
        assert!(
            fixture
                .app
                .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(fixture.app.project_goal(PROJECT).unwrap(), before);
        assert_eq!(
            fixture
                .app
                .store
                .conversation_call_history(&owner, fixture.task.id)
                .unwrap(),
            calls
        );
        assert_eq!(
            fixture
                .app
                .store
                .get_workflow_draft(&fixture.sample.draft_id)
                .unwrap()
                .revision,
            fixture.sample.draft_revision
        );
        assert!(
            fixture
                .app
                .store
                .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
                .unwrap()
                .is_empty()
        );
        let changed = scope_input(
            &record,
            &json!({"scope":"current_candidate","reason":"wrong_target"}),
        );
        assert!(
            fixture
                .app
                .answer_conversation_feedback_scope(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    call,
                    &changed
                )
                .is_err()
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn scope_answer_replay_is_historical_but_fresh_answer_and_request_revalidate_pixels() {
    for saved_first in [false, true] {
        let (fixture, provider, record) = scope_fixture().await;
        let call = record.consent.call_id;
        let input = scope_input(
            &record,
            &json!({"scope":"current_candidate","reason":"poor_boundary"}),
        );
        if saved_first {
            fixture
                .app
                .answer_conversation_feedback_scope(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    call,
                    &input,
                )
                .unwrap();
        }
        let image = fixture
            .app
            .project_image_path(
                PROJECT,
                ImageId(Uuid::parse_str(&fixture.human.image_id).unwrap()),
            )
            .unwrap();
        std::fs::write(image, b"TEST replaced pixels, never the real user image").unwrap();
        let answer = fixture.app.answer_conversation_feedback_scope(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            call,
            &input,
        );
        assert_eq!(answer.is_ok(), saved_first);
        if saved_first {
            assert!(
                fixture
                    .app
                    .prepare_conversation_feedback_request(
                        PROJECT,
                        fixture.conversation,
                        fixture.task.id,
                        call
                    )
                    .is_err()
            );
        }
        assert!(
            fixture
                .app
                .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
                .unwrap()
                .is_empty()
        );
        assert!(
            fixture
                .app
                .store
                .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn scope_answer_owner_and_cancellation_cannot_be_bypassed_by_local_replay() {
    for saved_first in [false, true] {
        let (fixture, provider, record) = scope_fixture().await;
        let call = record.consent.call_id;
        let input = scope_input(
            &record,
            &json!({"scope":"current_candidate","reason":"wrong_target"}),
        );
        assert!(
            fixture
                .app
                .answer_conversation_feedback_scope(
                    PROJECT,
                    Uuid::new_v4(),
                    fixture.task.id,
                    call,
                    &input
                )
                .is_err()
        );
        assert!(
            fixture
                .app
                .answer_conversation_feedback_scope(
                    PROJECT,
                    fixture.conversation,
                    Uuid::new_v4(),
                    call,
                    &input
                )
                .is_err()
        );
        if saved_first {
            fixture
                .app
                .answer_conversation_feedback_scope(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    call,
                    &input,
                )
                .unwrap();
        }
        fixture
            .app
            .cancel_conversation_schema(PROJECT, fixture.conversation, fixture.task.id, call)
            .unwrap();
        let restored = fixture.app.answer_conversation_feedback_scope(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            call,
            &input,
        );
        assert_eq!(restored.is_ok(), saved_first);
        assert!(
            fixture
                .app
                .prepare_conversation_feedback_request(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    call
                )
                .is_err()
        );
        assert!(
            fixture
                .app
                .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn scope_answer_persists_without_applying_feedback_and_opens_only_the_frozen_candidate() {
    use annotagent_storage::{
        ConversationFeedbackCorrectionReason as Reason, ConversationFeedbackScopeAnswerInput,
        ConversationFeedbackScopeChoice,
    };
    let fixture = fixture(false);
    let record = feedback_authorization(&fixture);
    let call = record.consent.call_id;
    fixture
        .app
        .authorize_conversation_feedback(PROJECT, fixture.conversation, fixture.task.id, &record)
        .unwrap();
    let mut provider = provider();
    provider.arguments = json!({"decision":"clarify_scope","question":"Which scope do you mean?","rationale":"The saved text does not authorize removal."});
    let execution = ConversationSchemaExecution {
        call_id: call,
        ..fixture.execution()
    };
    let result = fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &fixture.context(),
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let input = ConversationFeedbackScopeAnswerInput {
        command_id: Uuid::new_v4(),
        expected_context_digest: annotagent_storage::conversation_feedback_context_digest(
            &record.context,
        )
        .unwrap(),
        choice: ConversationFeedbackScopeChoice::CurrentCandidate {
            reason: Reason::PoorBoundary,
        },
    };
    assert!(
        fixture
            .app
            .prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call
            )
            .unwrap()
            .is_none()
    );
    let answer = fixture
        .app
        .answer_conversation_feedback_scope(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            call,
            &input,
        )
        .unwrap();
    assert_eq!(answer.input, input);
    assert!(
        fixture
            .app
            .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .app
            .store
            .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
            .unwrap()
            .is_empty()
    );
    let status = fixture
        .app
        .conversation_feedback_status(PROJECT, fixture.conversation, fixture.task.id, call)
        .unwrap();
    assert_eq!(status["scope_answer"], json!(answer));
    assert_eq!(status["decision"]["Ok"]["decision"], "clarify_scope");
    assert_eq!(status["receipt"], json!(result.receipt));
    let reopened = LocalApplication::new(fixture.temporary.path()).unwrap();
    assert_eq!(
        reopened
            .answer_conversation_feedback_scope(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call,
                &input
            )
            .unwrap(),
        answer
    );
    let human = reopened
        .prepare_conversation_feedback_request(PROJECT, fixture.conversation, fixture.task.id, call)
        .unwrap()
        .unwrap();
    assert_eq!(human.input.outcome_id, fixture.human.outcome_id);
    assert_eq!(human.input.question, Reason::PoorBoundary.question());
    assert_eq!(human.input.reason_code, "poor_boundary");
    assert_eq!(human.status, ConversationHumanRequestStatus::Pending);
    assert_eq!(
        reopened
            .prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call
            )
            .unwrap(),
        Some(human)
    );
    assert!(
        reopened
            .store
            .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    assert_eq!(
        reopened
            .conversation_builder_budget(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .used_calls,
        1
    );
    assert_eq!(
        reopened
            .store
            .get_workflow_draft(&fixture.sample.draft_id)
            .unwrap()
            .revision,
        fixture.sample.draft_revision
    );
}

#[tokio::test]
async fn saved_request_replay_survives_cancellation_and_changed_pixels_for_both_decisions() {
    for clarify in [false, true] {
        let fixture = fixture(false);
        let record = feedback_authorization(&fixture);
        let call = record.consent.call_id;
        fixture
            .app
            .authorize_conversation_feedback(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                &record,
            )
            .unwrap();
        let mut provider = provider();
        if clarify {
            provider.arguments = json!({"decision":"clarify_scope","question":"Which scope do you mean?","rationale":"TEST ambiguous saved text"});
        }
        let execution = ConversationSchemaExecution {
            call_id: call,
            ..fixture.execution()
        };
        let completed = fixture
            .app
            .execute_conversation_feedback(
                PROJECT,
                &execution,
                &fixture.context(),
                &provider,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        if clarify {
            let input = scope_input(
                &record,
                &json!({"scope":"current_candidate","reason":"wrong_target"}),
            );
            fixture
                .app
                .answer_conversation_feedback_scope(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    call,
                    &input,
                )
                .unwrap();
        }
        let request = fixture
            .app
            .prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call,
            )
            .unwrap()
            .unwrap();
        assert_eq!(request.status, ConversationHumanRequestStatus::Pending);
        fixture
            .app
            .cancel_conversation_schema(PROJECT, fixture.conversation, fixture.task.id, call)
            .unwrap();
        let image = fixture
            .app
            .project_image_path(
                PROJECT,
                ImageId(Uuid::parse_str(&fixture.human.image_id).unwrap()),
            )
            .unwrap();
        std::fs::write(image, b"TEST changed pixels after a saved request").unwrap();
        let reopened = LocalApplication::new(fixture.temporary.path()).unwrap();
        assert_eq!(
            reopened
                .prepare_conversation_feedback_request(
                    PROJECT,
                    fixture.conversation,
                    fixture.task.id,
                    call
                )
                .unwrap(),
            Some(request.clone())
        );
        assert_eq!(
            reopened
                .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
                .unwrap(),
            vec![request]
        );
        let result = reopened
            .read_conversation_feedback(PROJECT, fixture.conversation, fixture.task.id, call)
            .unwrap()
            .unwrap();
        assert!(
            result.decision.is_err(),
            "historical request recovery must not reactivate the proposal"
        );
        assert_eq!(result.receipt, completed.receipt);
        assert!(
            reopened
                .store
                .sample_feedback(&fixture.sample.id, &fixture.human.image_id)
                .unwrap()
                .is_empty()
        );
        let owner = reopened.conversation_project_identity(PROJECT).unwrap();
        assert!(
            reopened
                .store
                .pending_conversation_resumes(&owner, fixture.conversation, fixture.task.id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            reopened
                .conversation_builder_budget(PROJECT, fixture.conversation, fixture.task.id)
                .unwrap()
                .used_calls,
            1
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn cancelled_clarification_without_saved_scope_does_not_become_a_successful_noop() {
    let (fixture, provider, record) = scope_fixture().await;
    let call = record.consent.call_id;
    assert!(
        fixture
            .app
            .prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call
            )
            .unwrap()
            .is_none()
    );
    fixture
        .app
        .cancel_conversation_schema(PROJECT, fixture.conversation, fixture.task.id, call)
        .unwrap();
    assert!(
        fixture
            .app
            .prepare_conversation_feedback_request(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                call
            )
            .is_err()
    );
    assert!(
        fixture
            .app
            .conversation_human_requests(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}

#[async_trait::async_trait]
impl VisionModelProvider for GateProvider {
    fn name(&self) -> &str {
        "TEST gated offline feedback"
    }

    fn capabilities(&self) -> ModelCapabilities {
        self.inner.capabilities()
    }

    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<ModelResponse> {
        let response = self.inner.complete(request, cancellation).await?;
        self.started.notify_one();
        self.release.notified().await;
        Ok(response)
    }
}

#[tokio::test]
async fn overlapping_replay_does_not_cancel_or_abandon_the_single_admitted_call() {
    let fixture = fixture(false);
    let provider = GateProvider {
        inner: provider(),
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    };
    let context = fixture.context();
    let execution = fixture.execution();
    fixture.authorize(&execution);
    let first_token = CancellationToken::new();
    let first = fixture.app.execute_conversation_feedback(
        PROJECT,
        &execution,
        &context,
        &provider,
        first_token.clone(),
    );
    let duplicate = async {
        provider.started.notified().await;
        let result = fixture
            .app
            .execute_conversation_feedback(
                PROJECT,
                &execution,
                &context,
                &provider,
                CancellationToken::new(),
            )
            .await;
        assert!(
            result.is_err(),
            "An overlapping request must not dispatch again"
        );
        assert!(!first_token.is_cancelled());
        let receipt = fixture
            .app
            .conversation_call_receipt(
                PROJECT,
                fixture.conversation,
                fixture.task.id,
                execution.call_id,
            )
            .unwrap()
            .unwrap();
        assert_eq!(receipt.status, ConversationCallStatus::Reserved);
        provider.release.notify_one();
    };
    let (completed, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(first, duplicate)
    })
    .await
    .expect("Offline replay coordination did not finish");
    let completed = completed.unwrap();
    assert_eq!(completed.receipt.status, ConversationCallStatus::Completed);
    assert!(completed.decision.is_ok());
    assert!(!first_token.is_cancelled());
    let replay = fixture
        .app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &context,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&replay).unwrap(),
        serde_json::to_value(&completed).unwrap()
    );
    assert_eq!(provider.inner.requests.lock().unwrap().len(), 1);
    assert_eq!(
        fixture
            .app
            .conversation_builder_budget(PROJECT, fixture.conversation, fixture.task.id)
            .unwrap()
            .used_calls,
        1
    );
}
