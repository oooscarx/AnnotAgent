//! TempDir-only integration: image class decisions reuse saved Sandbox evidence and one draft.
use super::*;
use annotagent_storage::{
    ConversationImageClassAction as Action, ConversationImageClassAnswerInput as Answer,
    ConversationImageClassCreateInput as Create, ConversationImageClassStatus as Status,
};

async fn class_fixture() -> (Fixture, TestProvider, Uuid) {
    let fixture = fixture_with_schema_and_class_member(false, true, true);
    fixture
        .app
        .store
        .finish_sample_operation(&fixture.sample.id, None)
        .unwrap();
    let (fixture, provider, source) = scope_fixture_from(fixture).await;
    fixture
        .app
        .answer_conversation_feedback_scope(
            PROJECT,
            fixture.conversation,
            fixture.task.id,
            source.consent.call_id,
            &scope_input(&source, &json!({"scope":"current_image_class"})),
        )
        .unwrap();
    (fixture, provider, source.consent.call_id)
}

#[tokio::test]
async fn decimal_candidate_feedback_restores_and_opens_class_review_without_scope_drift() {
    // These are the original decimals from the browser regression, not rounded fixtures.
    let f = fixture_with_schema_class_and_box(false, true, true, Some([0.12, 0.2, 0.16, 0.22]));
    f.app
        .store
        .finish_sample_operation(&f.sample.id, None)
        .unwrap();
    let original = feedback_authorization(&f);
    f.app
        .authorize_conversation_feedback(PROJECT, f.conversation, f.task.id, &original)
        .unwrap();
    let saved = f
        .app
        .conversation_feedback_authorization(
            PROJECT,
            f.conversation,
            f.task.id,
            original.consent.call_id,
        )
        .unwrap()
        .unwrap();
    assert_ne!(
        saved.context, original.context,
        "The test must exercise the persisted JSON floating-point drift"
    );
    let restored: ConversationFeedbackContext =
        serde_json::from_value(saved.context.clone()).unwrap();
    assert_eq!(restored, f.context());
    let mut provider = provider();
    provider.arguments = json!({"decision":"clarify_scope","question":"Which scope?","rationale":"TEST exact subject only"});
    let execution = ConversationSchemaExecution {
        call_id: saved.consent.call_id,
        ..f.execution()
    };
    f.app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &restored,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    f.app
        .execute_conversation_feedback(
            PROJECT,
            &execution,
            &restored,
            &provider,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    f.app
        .answer_conversation_feedback_scope(
            PROJECT,
            f.conversation,
            f.task.id,
            saved.consent.call_id,
            &scope_input(&saved, &json!({"scope":"current_image_class"})),
        )
        .unwrap();
    let input = create_input(&f, saved.consent.call_id);
    let review = f
        .app
        .create_conversation_image_class_review(PROJECT, f.conversation, f.task.id, &input)
        .unwrap();
    let answer = answer_input(&review);
    f.app
        .answer_conversation_image_class_review(
            PROJECT,
            f.conversation,
            f.task.id,
            review.id,
            &answer,
        )
        .unwrap();
    assert_eq!(
        f.app
            .continue_conversation_image_class_review(PROJECT, f.conversation, f.task.id, review.id)
            .unwrap()
            .status,
        Status::Applied
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}

fn create_input(f: &Fixture, call: Uuid) -> Create {
    let scope = f
        .app
        .conversation_image_class_scope(PROJECT, f.conversation, f.task.id, call, None)
        .unwrap();
    assert_eq!(scope.target_label, "cup");
    assert_eq!(scope.members.len(), 2);
    Create {
        id: Uuid::new_v4(),
        feedback_call_id: call,
        target_label: None,
        expected_scope_digest: scope.digest().unwrap(),
    }
}

fn answer_input(review: &annotagent_storage::ConversationImageClassReview) -> Answer {
    let actions = review
        .scope
        .members
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            if index == 0 {
                Action::Edit {
                    outcome_id: candidate.outcome.id.clone(),
                    source_artifact_id: candidate.source_artifact_id.0,
                    corrected_label: "cup".into(),
                    corrected_value: serde_json::from_value(
                        json!({"kind":"bounding_box","rect":[0.12,0.12,0.1,0.1]}),
                    )
                    .unwrap(),
                }
            } else {
                Action::Exclude {
                    outcome_id: candidate.outcome.id.clone(),
                    source_artifact_id: candidate.source_artifact_id.0,
                }
            }
        })
        .collect();
    Answer {
        command_id: Uuid::new_v4(),
        expected_scope_digest: review.scope_digest.clone(),
        actions,
    }
}

#[tokio::test]
async fn image_class_answer_restores_exact_revisions_and_one_repair_draft_without_inference() {
    let (f, provider, call) = class_fixture().await;
    let old_goal = f.app.project_goal(PROJECT).unwrap();
    let old_plan = f.app.store.get_workflow_draft(&f.sample.draft_id).unwrap();
    let old_budget = f
        .app
        .conversation_task_budget(PROJECT, f.conversation, f.task.id)
        .unwrap();
    let input = create_input(&f, call);
    assert!(
        f.app
            .conversation_image_class_review_for_feedback(PROJECT, f.conversation, f.task.id, call)
            .unwrap()
            .is_none()
    );
    let review = f
        .app
        .create_conversation_image_class_review(PROJECT, f.conversation, f.task.id, &input)
        .unwrap();
    assert_eq!(review.status, Status::Pending);
    assert!(
        f.app
            .store
            .sample_feedback(&f.sample.id, &f.human.image_id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        f.app
            .create_conversation_image_class_review(PROJECT, f.conversation, f.task.id, &input)
            .unwrap(),
        review
    );
    let answer = answer_input(&review);
    let saved = f
        .app
        .answer_conversation_image_class_review(
            PROJECT,
            f.conversation,
            f.task.id,
            review.id,
            &answer,
        )
        .unwrap();
    assert_eq!(saved.status, Status::Answered);
    assert_eq!(saved.revisions.len(), 2);
    assert!(saved.repair_draft_id.is_none());
    assert_eq!(
        saved.revisions[1].reason,
        SampleFeedbackReason::ExcludeTarget
    );
    assert!(saved.revisions[1].corrected_value.is_none());
    assert_eq!(
        f.app
            .answer_conversation_image_class_review(
                PROJECT,
                f.conversation,
                f.task.id,
                review.id,
                &answer
            )
            .unwrap(),
        saved
    );
    // A new Application instance recovers the durable answer into exactly one editable copy.
    let recovered = LocalApplication::new(f.temporary.path()).unwrap();
    let complete = recovered
        .conversation_image_class_review(PROJECT, f.conversation, f.task.id, review.id)
        .unwrap()
        .unwrap();
    assert_eq!(complete.status, Status::Applied);
    assert_eq!(
        complete.repair_draft_id,
        Some(review.resume_checkpoint_ref.to_string())
    );
    let evidence = recovered
        .store
        .sample_plan_evidence(complete.repair_draft_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    let exact: Vec<SampleFeedbackRevision> =
        serde_json::from_value(evidence["feedback"].clone()).unwrap();
    assert_eq!(exact, saved.revisions);
    let copied = recovered
        .store
        .get_workflow_draft(complete.repair_draft_id.as_ref().unwrap())
        .unwrap();
    assert_eq!(
        recovered
            .continue_conversation_image_class_review(PROJECT, f.conversation, f.task.id, review.id)
            .unwrap(),
        complete
    );
    assert_eq!(
        recovered
            .store
            .get_workflow_draft(complete.repair_draft_id.as_ref().unwrap())
            .unwrap(),
        copied
    );
    assert_eq!(
        recovered
            .store
            .get_workflow_draft(&f.sample.draft_id)
            .unwrap(),
        old_plan
    );
    assert_eq!(
        recovered
            .store
            .get_workflow_sample_test_by_id(&f.sample.id)
            .unwrap(),
        Some(f.sample)
    );
    assert_eq!(recovered.project_goal(PROJECT).unwrap(), old_goal);
    assert_eq!(
        recovered
            .conversation_task_budget(PROJECT, f.conversation, f.task.id)
            .unwrap(),
        old_budget
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn class_review_rejects_changed_pixels_but_preserves_saved_answers_for_resume() {
    let (f, provider, call) = class_fixture().await;
    let input = create_input(&f, call);
    let image = f
        .app
        .project_image_path(
            PROJECT,
            ImageId(Uuid::parse_str(&f.human.image_id).unwrap()),
        )
        .unwrap();
    let bytes = std::fs::read(&image).unwrap();
    std::fs::write(&image, b"TEST changed pixels").unwrap();
    assert!(
        f.app
            .create_conversation_image_class_review(PROJECT, f.conversation, f.task.id, &input)
            .is_err()
    );
    std::fs::write(&image, &bytes).unwrap();
    let review = f
        .app
        .create_conversation_image_class_review(PROJECT, f.conversation, f.task.id, &input)
        .unwrap();
    let answer = answer_input(&review);
    std::fs::write(&image, b"TEST changed pixels").unwrap();
    assert!(
        f.app
            .answer_conversation_image_class_review(
                PROJECT,
                f.conversation,
                f.task.id,
                review.id,
                &answer
            )
            .is_err()
    );
    assert!(
        f.app
            .store
            .sample_feedback(&f.sample.id, &f.human.image_id)
            .unwrap()
            .is_empty()
    );
    std::fs::write(&image, &bytes).unwrap();
    let saved = f
        .app
        .answer_conversation_image_class_review(
            PROJECT,
            f.conversation,
            f.task.id,
            review.id,
            &answer,
        )
        .unwrap();
    std::fs::write(&image, b"TEST changed pixels").unwrap();
    assert_eq!(
        f.app
            .answer_conversation_image_class_review(
                PROJECT,
                f.conversation,
                f.task.id,
                review.id,
                &answer
            )
            .unwrap(),
        saved
    );
    let interrupted = f
        .app
        .continue_conversation_image_class_review(PROJECT, f.conversation, f.task.id, review.id)
        .unwrap();
    assert_eq!(interrupted.status, Status::Answered);
    assert!(interrupted.resume_error.is_some());
    assert_eq!(interrupted.revisions, saved.revisions);
    assert!(interrupted.repair_draft_id.is_none());
    std::fs::write(&image, &bytes).unwrap();
    let complete = f
        .app
        .continue_conversation_image_class_review(PROJECT, f.conversation, f.task.id, review.id)
        .unwrap();
    assert_eq!(complete.status, Status::Applied);
    assert!(complete.resume_error.is_none());
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn class_review_foreign_scope_and_cancel_never_write_partial_feedback() {
    let (f, _, call) = class_fixture().await;
    let input = create_input(&f, call);
    let review = f
        .app
        .create_conversation_image_class_review(PROJECT, f.conversation, f.task.id, &input)
        .unwrap();
    assert!(
        f.app
            .conversation_image_class_review(PROJECT, Uuid::new_v4(), f.task.id, review.id)
            .is_err()
    );
    let mut forged = answer_input(&review);
    if let Action::Exclude {
        source_artifact_id, ..
    } = &mut forged.actions[1]
    {
        *source_artifact_id = Uuid::new_v4();
    }
    assert!(
        f.app
            .answer_conversation_image_class_review(
                PROJECT,
                f.conversation,
                f.task.id,
                review.id,
                &forged
            )
            .is_err()
    );
    assert!(
        f.app
            .store
            .sample_feedback(&f.sample.id, &f.human.image_id)
            .unwrap()
            .is_empty()
    );
    let cancelled = f
        .app
        .cancel_conversation_image_class_review(PROJECT, f.conversation, f.task.id, review.id)
        .unwrap();
    assert_eq!(cancelled.status, Status::Cancelled);
    assert!(
        f.app
            .answer_conversation_image_class_review(
                PROJECT,
                f.conversation,
                f.task.id,
                review.id,
                &answer_input(&review)
            )
            .is_err()
    );
    assert!(
        f.app
            .store
            .sample_feedback(&f.sample.id, &f.human.image_id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        f.app
            .create_conversation_image_class_review(PROJECT, f.conversation, f.task.id, &input)
            .unwrap(),
        cancelled
    );
}
