use super::*;
use crate::*;
use serde_json::json;

fn fixture(
    store: &SqliteStore,
    kind: &str,
    count: usize,
) -> crate::conversation_feedback_scope::tests::Fixture {
    let mut f = crate::conversation_feedback_scope::tests::fixture(store, kind);
    let mut sample = store
        .get_workflow_sample_test_by_id("TEST-scoped-sample")
        .unwrap()
        .unwrap();
    let draft:annotagent_core::WorkflowDraft=serde_json::from_value(json!({"id":sample.draft_id,"project_id":sample.project_id,"name":"TEST original plan","status":"editing","nodes":[],"created_at":chrono::Utc::now(),"updated_at":chrono::Utc::now()})).unwrap();
    store.save_workflow_draft(&draft).unwrap();
    let draft = store.get_workflow_draft(&draft.id).unwrap();
    sample.draft_content_hash = draft.content_hash;
    let artifact: Uuid =
        serde_json::from_value(f.record.context["candidate"]["source_artifact_id"].clone())
            .unwrap();
    let first_id = f.record.context["candidate"]["outcome"]["id"]
        .as_str()
        .unwrap();
    let candidates=(0..count).map(|i|serde_json::from_value::<FinalCandidateProjection>(json!({"source_artifact_id":if i==0 {artifact}else{Uuid::new_v4()},"source_artifact_ref":format!("TEST-terminal-{i}"),"lineage_id":format!("TEST-lineage-{i}"),"outcome":{"id":if i==0 {first_id.to_owned()}else{format!("TEST-final-{i}")},"label":"cup","confidence":0.9,"status":"needs_review","value":if kind=="classification" {json!({"kind":kind,"labels":["cup","bottle"]})} else {json!({"kind":kind,"rect":[0.1,0.1,0.2,0.2]})}},"localization":"TEST","geometry":"TEST","final_status":"TEST"})).unwrap()).collect::<Vec<_>>();
    sample.report.samples[0].outcomes = candidates.iter().map(|c| c.outcome.clone()).collect();
    sample.report.samples[0].projection.final_candidates = candidates.clone();
    f.record.context["candidate"] = serde_json::to_value(&candidates[0]).unwrap();
    f.record.context["sample_content_hash"] = json!(sample.draft_content_hash);
    f.source.evidence.as_mut().unwrap()["context"]["subject"] = f.record.context.clone();
    f.answer.expected_context_digest =
        conversation_feedback_context_digest(&f.record.context).unwrap();
    f.answer.choice = ConversationFeedbackScopeChoice::CurrentImageClass;
    store
        .with_connection(|db| {
            db.execute(
                "UPDATE workflow_sample_tests SET report_json=?2,draft_content_hash=?3 WHERE id=?1",
                params![
                    sample.id,
                    serde_json::to_string(&sample.report)?,
                    sample.draft_content_hash
                ],
            )?;
            db.execute(
                "UPDATE sample_operations SET status='succeeded' WHERE id=?1",
                [&sample.id],
            )?;
            db.execute(
                "UPDATE conversation_feedback_authorizations SET record_json=?2 WHERE call_id=?1",
                params![f.source.id.to_string(), serde_json::to_string(&f.record)?],
            )?;
            db.execute(
                "UPDATE conversation_model_calls SET evidence_json=?2 WHERE id=?1",
                params![
                    f.source.id.to_string(),
                    serde_json::to_string(&f.source.evidence)?
                ],
            )?;
            Ok(())
        })
        .unwrap();
    store
        .answer_conversation_feedback_scope(
            &f.project,
            f.conversation,
            f.task,
            f.source.id,
            &f.answer,
            &f.source,
        )
        .unwrap();
    f
}

#[test]
fn class_review_freezes_members_and_saves_one_atomic_human_answer() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f = fixture(&store, "bounding_box", 1);
    let scope = store
        .conversation_image_class_scope(&f.project, f.conversation, f.task, f.source.id, None)
        .unwrap();
    assert_eq!(scope.members.len(), 1);
    let input = ConversationImageClassCreateInput {
        id: Uuid::new_v4(),
        feedback_call_id: f.source.id,
        target_label: None,
        expected_scope_digest: scope.digest().unwrap(),
    };
    let record = store
        .create_conversation_image_class_review(&f.project, f.conversation, f.task, &input)
        .unwrap();
    assert!(
        store
            .sample_feedback(&scope.sample_test_id, &scope.image_id)
            .unwrap()
            .is_empty()
    );
    let member = &scope.members[0];
    let answer = ConversationImageClassAnswerInput {
        command_id: Uuid::new_v4(),
        expected_scope_digest: record.scope_digest.clone(),
        actions: vec![ConversationImageClassAction::Exclude {
            outcome_id: member.outcome.id.clone(),
            source_artifact_id: member.source_artifact_id.0,
        }],
    };
    let saved = store
        .answer_conversation_image_class_review(
            &f.project,
            f.conversation,
            f.task,
            input.id,
            &answer,
        )
        .unwrap();
    assert_eq!(saved.status, ConversationImageClassStatus::Answered);
    assert_eq!(saved.revisions.len(), 1);
    assert_eq!(
        saved.revisions[0].reason,
        SampleFeedbackReason::ExcludeTarget
    );
    assert!(saved.revisions[0].corrected_value.is_none());
    assert_eq!(
        store
            .answer_conversation_image_class_review(
                &f.project,
                f.conversation,
                f.task,
                input.id,
                &answer
            )
            .unwrap(),
        saved
    );
    assert_eq!(
        store
            .sample_feedback(&scope.sample_test_id, &scope.image_id)
            .unwrap()
            .len(),
        1
    );
    let _ = json!({"scope":scope});
}

fn create(
    store: &SqliteStore,
    f: &crate::conversation_feedback_scope::tests::Fixture,
    target: Option<&str>,
) -> ConversationImageClassReview {
    let scope = store
        .conversation_image_class_scope(&f.project, f.conversation, f.task, f.source.id, target)
        .unwrap();
    let input = ConversationImageClassCreateInput {
        id: Uuid::new_v4(),
        feedback_call_id: f.source.id,
        target_label: target.map(str::to_owned),
        expected_scope_digest: scope.digest().unwrap(),
    };
    store
        .create_conversation_image_class_review(&f.project, f.conversation, f.task, &input)
        .unwrap()
}
fn keep(review: &ConversationImageClassReview) -> ConversationImageClassAnswerInput {
    ConversationImageClassAnswerInput {
        command_id: Uuid::new_v4(),
        expected_scope_digest: review.scope_digest.clone(),
        actions: review
            .scope
            .members
            .iter()
            .map(|m| ConversationImageClassAction::Keep {
                outcome_id: m.outcome.id.clone(),
                source_artifact_id: m.source_artifact_id.0,
            })
            .collect(),
    }
}
fn align_baseline(store: &SqliteStore, f: &mut crate::conversation_feedback_scope::tests::Fixture) {
    let image = f.record.context["message"]["input"]["image"]["image_id"]
        .as_str()
        .unwrap();
    let sequence = store
        .sample_feedback("TEST-scoped-sample", image)
        .unwrap()
        .last()
        .map_or(0, |r| r.sequence);
    f.record.context["expected_feedback_sequence"] = json!(sequence);
    f.source.evidence.as_mut().unwrap()["context"]["subject"] = f.record.context.clone();
    f.answer.expected_context_digest =
        conversation_feedback_context_digest(&f.record.context).unwrap();
    store
        .with_connection(|db| {
            db.execute(
                "UPDATE conversation_feedback_authorizations SET record_json=?2 WHERE call_id=?1",
                params![f.source.id.to_string(), serde_json::to_string(&f.record)?],
            )?;
            db.execute(
                "UPDATE conversation_model_calls SET evidence_json=?2 WHERE id=?1",
                params![
                    f.source.id.to_string(),
                    serde_json::to_string(&f.source.evidence)?
                ],
            )?;
            db.execute(
                "UPDATE conversation_feedback_scope_answers SET input_json=?2 WHERE call_id=?1",
                params![f.source.id.to_string(), serde_json::to_string(&f.answer)?],
            )?;
            Ok(())
        })
        .unwrap();
}
fn baseline_revision(
    f: &crate::conversation_feedback_scope::tests::Fixture,
    outcome: &str,
    sequence: u64,
) -> SampleFeedbackRevision {
    SampleFeedbackRevision {
        revision_id: Uuid::new_v4().to_string(),
        sample_test_id: "TEST-scoped-sample".into(),
        image_id: f.record.context["message"]["input"]["image"]["image_id"]
            .as_str()
            .unwrap()
            .into(),
        sequence,
        reason: SampleFeedbackReason::PoorBoundary,
        outcome_id: Some(outcome.into()),
        corrected_value: None,
        corrected_label: None,
        addition_id: None,
        note: "TEST earlier human correction".into(),
        created_at: chrono::Utc::now(),
    }
}

#[test]
fn class_baseline_keeps_prior_geometry_and_excludes_prior_relabels_and_rejections() {
    let store = SqliteStore::open_in_memory().unwrap();
    let mut f = fixture(&store, "bounding_box", 3);
    let anchor = f.record.context["candidate"]["outcome"]["id"]
        .as_str()
        .unwrap();
    let value: VisionArtifactValue =
        serde_json::from_value(json!({"kind":"bounding_box","rect":[0.2,0.2,0.1,0.1]})).unwrap();
    let mut first = baseline_revision(&f, anchor, 1);
    first.corrected_value = Some(value.clone());
    store.save_sample_feedback(&first).unwrap();
    let mut other = baseline_revision(&f, "TEST-final-1", 2);
    other.corrected_label = Some("bottle".into());
    store.save_sample_feedback(&other).unwrap();
    let mut excluded = baseline_revision(&f, "TEST-final-2", 3);
    excluded.reason = SampleFeedbackReason::ExcludeTarget;
    store.save_sample_feedback(&excluded).unwrap();
    align_baseline(&store, &mut f);
    let review = create(&store, &f, None);
    assert_eq!(review.scope.members.len(), 1);
    assert_eq!(review.scope.baseline_feedback.len(), 3);
    let saved = store
        .answer_conversation_image_class_review(
            &f.project,
            f.conversation,
            f.task,
            review.id,
            &keep(&review),
        )
        .unwrap();
    assert_eq!(saved.revisions[0].reason, SampleFeedbackReason::Correct);
    assert!(saved.revisions[0].corrected_value.is_none());
    let all = store
        .sample_feedback(&review.scope.sample_test_id, &review.scope.image_id)
        .unwrap();
    assert_eq!(
        conversation_image_class_effective_outcome(&review.scope.members[0], &all)
            .unwrap()
            .value,
        Some(value)
    );
    assert_eq!(all[1].corrected_label.as_deref(), Some("bottle"));
    assert_eq!(all[2].reason, SampleFeedbackReason::ExcludeTarget);
    let applied = store
        .resume_conversation_image_class_review(&f.project, f.conversation, f.task, review.id)
        .unwrap();
    let evidence = store
        .sample_plan_evidence(applied.repair_draft_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_value::<Vec<SampleFeedbackRevision>>(evidence["feedback"].clone())
            .unwrap(),
        vec![first, saved.revisions[0].clone()]
    );
}

#[test]
fn classification_scope_uses_exact_effective_tokens_and_preserves_every_other_token() {
    let store = SqliteStore::open_in_memory().unwrap();
    let mut f = fixture(&store, "classification", 2);
    let first = f.record.context["candidate"]["outcome"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut correction = baseline_revision(&f, &first, 1);
    correction.corrected_value = Some(
        serde_json::from_value(json!({"kind":"classification","labels":["cup","bottle","plate"]}))
            .unwrap(),
    );
    correction.corrected_label = Some("not a token display".into());
    store.save_sample_feedback(&correction).unwrap();
    align_baseline(&store, &mut f);
    assert!(
        store
            .conversation_image_class_scope(&f.project, f.conversation, f.task, f.source.id, None)
            .is_err()
    );
    assert!(
        store
            .conversation_image_class_scope(
                &f.project,
                f.conversation,
                f.task,
                f.source.id,
                Some("cu")
            )
            .is_err()
    );
    assert!(
        store
            .conversation_image_class_scope(
                &f.project,
                f.conversation,
                f.task,
                f.source.id,
                Some("not a token display")
            )
            .is_err()
    );
    let review = create(&store, &f, Some("cup"));
    assert_eq!(review.scope.members.len(), 2);
    let mut answer = keep(&review);
    answer.actions = review
        .scope
        .members
        .iter()
        .map(|m| ConversationImageClassAction::Exclude {
            outcome_id: m.outcome.id.clone(),
            source_artifact_id: m.source_artifact_id.0,
        })
        .collect();
    let saved = store
        .answer_conversation_image_class_review(
            &f.project,
            f.conversation,
            f.task,
            review.id,
            &answer,
        )
        .unwrap();
    for revision in &saved.revisions {
        assert_eq!(revision.reason, SampleFeedbackReason::WrongTarget);
        let Some(VisionArtifactValue::Classification { labels }) = &revision.corrected_value else {
            panic!("classification lost")
        };
        assert!(!labels.iter().any(|l| l.as_str() == "cup"));
        assert!(labels.iter().any(|l| l.as_str() == "bottle"));
        if revision.outcome_id.as_deref() == Some(first.as_str()) {
            assert_eq!(
                labels
                    .iter()
                    .map(annotagent_core::LabelId::as_str)
                    .collect::<Vec<_>>(),
                vec!["bottle", "plate"]
            );
        }
    }
}

#[test]
fn classification_last_token_is_explicit_exclusion_and_replacement_does_not_merge_tokens() {
    for replace in [false, true] {
        let store = SqliteStore::open_in_memory().unwrap();
        let mut f = fixture(&store, "classification", 1);
        let outcome = f.record.context["candidate"]["outcome"]["id"]
            .as_str()
            .unwrap();
        let mut prior = baseline_revision(&f, outcome, 1);
        prior.corrected_value = Some(
            serde_json::from_value(json!({"kind":"classification","labels":["cup"]})).unwrap(),
        );
        store.save_sample_feedback(&prior).unwrap();
        align_baseline(&store, &mut f);
        let review = create(&store, &f, Some("cup"));
        let member = &review.scope.members[0];
        let mut answer = keep(&review);
        answer.actions[0] = if replace {
            ConversationImageClassAction::ReplaceLabel {
                outcome_id: member.outcome.id.clone(),
                source_artifact_id: member.source_artifact_id.0,
                replacement: "bottle".into(),
            }
        } else {
            ConversationImageClassAction::Exclude {
                outcome_id: member.outcome.id.clone(),
                source_artifact_id: member.source_artifact_id.0,
            }
        };
        let saved = store
            .answer_conversation_image_class_review(
                &f.project,
                f.conversation,
                f.task,
                review.id,
                &answer,
            )
            .unwrap();
        if replace {
            assert_eq!(saved.revisions[0].reason, SampleFeedbackReason::WrongTarget);
            assert_eq!(
                saved.revisions[0].corrected_label.as_deref(),
                Some("bottle")
            );
        } else {
            assert_eq!(
                saved.revisions[0].reason,
                SampleFeedbackReason::ExcludeTarget
            );
            assert!(saved.revisions[0].corrected_value.is_none());
            assert!(saved.revisions[0].corrected_label.is_none());
        }
    }
}

#[test]
fn class_answer_rejects_foreign_partial_duplicate_invalid_geometry_and_rolls_back_all_rows() {
    for variant in 0..10 {
        let store = SqliteStore::open_in_memory().unwrap();
        let f = fixture(&store, "bounding_box", 3);
        let review = create(&store, &f, None);
        let mut answer = keep(&review);
        if variant == 5 {
            assert!(serde_json::from_value::<ConversationImageClassAction>(json!({"action":"edit","outcome_id":review.scope.members[0].outcome.id,"source_artifact_id":review.scope.members[0].source_artifact_id,"corrected_label":"cup","corrected_value":{"kind":"bounding_box","rect":[0.1,0.1,0.0,0.2]}})).is_err());
            assert!(
                store
                    .sample_feedback(&review.scope.sample_test_id, &review.scope.image_id)
                    .unwrap()
                    .is_empty()
            );
            continue;
        }
        match variant {
            0 => {
                answer.actions.pop();
            }
            1 => answer.actions[1] = answer.actions[0].clone(),
            2 => answer.expected_scope_digest = "0".repeat(64),
            3 => answer.command_id = Uuid::nil(),
            4 => {
                let m = &review.scope.members[0];
                answer.actions[0] = ConversationImageClassAction::Keep {
                    outcome_id: m.outcome.id.clone(),
                    source_artifact_id: Uuid::new_v4(),
                };
            }
            5 | 6 => {
                let m = &review.scope.members[0];
                answer.actions[0] = ConversationImageClassAction::Edit {
                    outcome_id: m.outcome.id.clone(),
                    source_artifact_id: m.source_artifact_id.0,
                    corrected_value: serde_json::from_value(if variant == 5 {
                        json!({"kind":"bounding_box","rect":[0.1,0.1,0.0,0.2]})
                    } else {
                        json!({"kind":"classification","labels":["cup"]})
                    })
                    .unwrap(),
                    corrected_label: "cup".into(),
                };
            }
            7 => {
                store
                    .with_connection(|db| {
                        db.execute(
                            "UPDATE images SET sha256='TEST changed' WHERE project_id=?1",
                            [&f.project],
                        )?;
                        Ok(())
                    })
                    .unwrap();
            }
            8 => {
                let existing = baseline_revision(&f, &review.scope.members[0].outcome.id, 1);
                store.save_sample_feedback(&existing).unwrap();
            }
            _ => {
                store.with_connection(|db|{db.execute_batch("CREATE TRIGGER TEST_mid_answer_failure BEFORE INSERT ON sample_feedback_revisions WHEN NEW.sequence=2 BEGIN SELECT RAISE(ABORT,'TEST rollback all members'); END;")?;Ok(())}).unwrap();
            }
        }
        let before = store
            .sample_feedback(&review.scope.sample_test_id, &review.scope.image_id)
            .unwrap();
        assert!(
            store
                .answer_conversation_image_class_review(
                    &f.project,
                    f.conversation,
                    f.task,
                    review.id,
                    &answer
                )
                .is_err(),
            "accepted {variant}"
        );
        assert_eq!(
            store
                .sample_feedback(&review.scope.sample_test_id, &review.scope.image_id)
                .unwrap(),
            before
        );
        assert_eq!(
            store
                .conversation_image_class_review(&f.project, f.conversation, f.task, review.id)
                .unwrap()
                .unwrap()
                .status,
            ConversationImageClassStatus::Pending
        );
    }
}

#[test]
fn pending_class_blocks_model_spend_and_single_request_but_source_cancel_does_not_revoke_it() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f = fixture(&store, "bounding_box", 2);
    let review = create(&store, &f, None);
    let before = store.conversation_call_budget(&f.project, f.task).unwrap();
    assert!(
        store
            .with_connection(
                |db| crate::conversation_calls::require_call_admission_clear(
                    db,
                    f.task,
                    Uuid::new_v4()
                )
            )
            .is_err()
    );
    let input = ConversationHumanRequestInput {
        id: Uuid::new_v4(),
        task_id: f.task,
        conversation_id: f.conversation,
        sample_test_id: review.scope.sample_test_id.clone(),
        image_id: review.scope.image_id.clone(),
        content_hash: review.scope.content_hash.clone(),
        outcome_id: review.scope.members[0].outcome.id.clone(),
        expected_feedback_sequence: 0,
        reason_code: "poor_boundary".into(),
        question: "TEST correction".into(),
        resume_checkpoint_ref: Uuid::new_v4(),
    };
    assert!(
        store
            .create_conversation_human_request(&f.project, &input)
            .is_err()
    );
    store
        .request_conversation_call_cancel(&f.project, f.task, f.source.id)
        .unwrap();
    let saved = store
        .answer_conversation_image_class_review(
            &f.project,
            f.conversation,
            f.task,
            review.id,
            &keep(&review),
        )
        .unwrap();
    assert_eq!(saved.status, ConversationImageClassStatus::Answered);
    assert_eq!(
        store.conversation_call_budget(&f.project, f.task).unwrap(),
        before
    );
}

#[test]
fn class_review_restores_exact_history_and_one_repair_draft_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST-image-class.db");
    let store = SqliteStore::open(&path).unwrap();
    let f = fixture(&store, "bounding_box", 3);
    let review = create(&store, &f, None);
    let answer = keep(&review);
    let original = store.get_workflow_draft(&review.scope.draft_id).unwrap();
    let sample = store
        .get_workflow_sample_test_by_id(&review.scope.sample_test_id)
        .unwrap();
    let budget = store.conversation_task_budget(&f.project, f.task).unwrap();
    let answered = store
        .answer_conversation_image_class_review(
            &f.project,
            f.conversation,
            f.task,
            review.id,
            &answer,
        )
        .unwrap();
    assert_eq!(
        store
            .pending_conversation_image_class_resumes()
            .unwrap()
            .len(),
        1
    );
    store
        .record_conversation_image_class_resume_failure(
            &f.project,
            f.conversation,
            f.task,
            review.id,
            "TEST lost local continuation",
        )
        .unwrap();
    drop(store);
    let store = SqliteStore::open(&path).unwrap();
    let applied = store
        .resume_conversation_image_class_review(&f.project, f.conversation, f.task, review.id)
        .unwrap();
    assert_eq!(applied.status, ConversationImageClassStatus::Applied);
    assert!(applied.resume_error.is_none());
    let draft_id = applied.repair_draft_id.as_ref().unwrap();
    let evidence = store.sample_plan_evidence(draft_id).unwrap().unwrap();
    assert_eq!(
        evidence["feedback"],
        serde_json::to_value(&answered.revisions).unwrap()
    );
    let mut draft = store.get_workflow_draft(draft_id).unwrap();
    draft.name = "TEST later human edit".into();
    store.save_workflow_draft(&draft).unwrap();
    store
        .with_connection(|db| {
            db.execute(
                "UPDATE images SET sha256='TEST changed later' WHERE project_id=?1",
                [&f.project],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        store
            .resume_conversation_image_class_review(&f.project, f.conversation, f.task, review.id)
            .unwrap(),
        applied
    );
    assert_eq!(
        store
            .create_conversation_image_class_review(
                &f.project,
                f.conversation,
                f.task,
                &review.input
            )
            .unwrap(),
        applied
    );
    assert_eq!(
        store
            .answer_conversation_image_class_review(
                &f.project,
                f.conversation,
                f.task,
                review.id,
                &answer
            )
            .unwrap(),
        applied
    );
    assert_eq!(
        store.get_workflow_draft(draft_id).unwrap().name,
        "TEST later human edit"
    );
    assert_eq!(store.get_workflow_draft(&original.id).unwrap(), original);
    assert_eq!(
        store
            .get_workflow_sample_test_by_id(&review.scope.sample_test_id)
            .unwrap(),
        sample
    );
    assert_eq!(
        store.conversation_task_budget(&f.project, f.task).unwrap(),
        budget
    );
    assert!(
        store
            .pending_conversation_image_class_resumes()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn duplicate_answers_and_cancellation_have_a_single_durable_order() {
    for cancel in [false, true] {
        let store = std::sync::Arc::new(SqliteStore::open_in_memory().unwrap());
        let f = fixture(&store, "bounding_box", 2);
        let review = create(&store, &f, None);
        let answer = keep(&review);
        let barrier = std::sync::Barrier::new(3);
        let result = std::thread::scope(|threads| {
            let first = threads.spawn(|| {
                barrier.wait();
                store.answer_conversation_image_class_review(
                    &f.project,
                    f.conversation,
                    f.task,
                    review.id,
                    &answer,
                )
            });
            let second = threads.spawn(|| {
                barrier.wait();
                if cancel {
                    store.cancel_conversation_image_class_review(
                        &f.project,
                        f.conversation,
                        f.task,
                        review.id,
                    )
                } else {
                    store.answer_conversation_image_class_review(
                        &f.project,
                        f.conversation,
                        f.task,
                        review.id,
                        &answer,
                    )
                }
            });
            barrier.wait();
            [first.join().unwrap(), second.join().unwrap()]
        });
        let current = store
            .conversation_image_class_review(&f.project, f.conversation, f.task, review.id)
            .unwrap()
            .unwrap();
        let revisions = store
            .sample_feedback(&review.scope.sample_test_id, &review.scope.image_id)
            .unwrap();
        if cancel {
            assert!(result.iter().any(Result::is_ok));
            assert!(matches!(
                (current.status, revisions.len()),
                (ConversationImageClassStatus::Answered, 2)
                    | (ConversationImageClassStatus::Cancelled, 0)
            ));
        } else {
            assert!(result.iter().all(Result::is_ok));
            assert_eq!(revisions.len(), 2);
        }
    }
}

#[test]
fn class_contract_rejects_unknown_actions_and_exclusions_with_geometry() {
    let id = Uuid::new_v4();
    for value in [
        json!({"action":"exclude","outcome_id":"TEST","source_artifact_id":id,"corrected_value":{"kind":"bounding_box","rect":[0.1,0.1,0.2,0.2]}}),
        json!({"action":"delete_all","outcome_id":"TEST","source_artifact_id":id}),
        json!({"action":"keep","outcome_id":"TEST","source_artifact_id":id,"HumanVerified":true}),
    ] {
        assert!(serde_json::from_value::<ConversationImageClassAction>(value).is_err());
    }
}

#[test]
fn class_scope_rejects_foreign_changed_ambiguous_oversized_or_cancelled_sources() {
    for variant in 0..9 {
        let store = SqliteStore::open_in_memory().unwrap();
        let f = fixture(&store, "bounding_box", if variant == 0 { 257 } else { 2 });
        let mut owner = f.project.clone();
        let mut conversation = f.conversation;
        let mut task = f.task;
        match variant {
            0 => {}
            1 => owner = "TEST foreign owner".into(),
            2 => conversation = Uuid::new_v4(),
            3 => task = Uuid::new_v4(),
            4 => {
                store
                    .request_conversation_call_cancel(&f.project, f.task, f.source.id)
                    .unwrap();
            }
            5 => {
                store
                    .with_connection(|db| {
                        db.execute(
                            "UPDATE images SET sha256='TEST changed' WHERE project_id=?1",
                            [&f.project],
                        )?;
                        Ok(())
                    })
                    .unwrap();
            }
            6 => {
                let mut test = store
                    .get_workflow_sample_test_by_id("TEST-scoped-sample")
                    .unwrap()
                    .unwrap();
                let duplicate = test.report.samples[0].projection.final_candidates[0]
                    .outcome
                    .id
                    .clone();
                test.report.samples[0].projection.final_candidates[1]
                    .outcome
                    .id = duplicate;
                store.with_connection(|db|{db.execute("UPDATE workflow_sample_tests SET report_json=?1 WHERE id='TEST-scoped-sample'",[serde_json::to_string(&test.report)?])?;Ok(())}).unwrap();
            }
            7 => {
                store
                    .with_connection(|db| {
                        db.execute(
                            "UPDATE workflow_drafts SET revision=revision+1 WHERE id='draft-1'",
                            [],
                        )?;
                        Ok(())
                    })
                    .unwrap();
            }
            _ => {
                let mut answer = f.answer.clone();
                answer.choice = ConversationFeedbackScopeChoice::ProjectFutureRule;
                store.with_connection(|db|{db.execute("UPDATE conversation_feedback_scope_answers SET input_json=?1 WHERE call_id=?2",params![serde_json::to_string(&answer)?,f.source.id.to_string()])?;Ok(())}).unwrap();
            }
        }
        assert!(
            store
                .conversation_image_class_scope(&owner, conversation, task, f.source.id, None)
                .is_err(),
            "accepted {variant}"
        );
        let count = store
            .with_connection(|db| {
                Ok(db.query_row(
                    "SELECT COUNT(*) FROM conversation_image_class_reviews",
                    [],
                    |r| r.get::<_, i64>(0),
                )?)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[test]
fn class_creation_exact_retry_and_reverse_single_pending_exclusion_are_atomic() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f = fixture(&store, "bounding_box", 2);
    let scope = store
        .conversation_image_class_scope(&f.project, f.conversation, f.task, f.source.id, None)
        .unwrap();
    let input = ConversationImageClassCreateInput {
        id: Uuid::new_v4(),
        feedback_call_id: f.source.id,
        target_label: None,
        expected_scope_digest: scope.digest().unwrap(),
    };
    let single = ConversationHumanRequestInput {
        id: Uuid::new_v4(),
        task_id: f.task,
        conversation_id: f.conversation,
        sample_test_id: scope.sample_test_id.clone(),
        image_id: scope.image_id.clone(),
        content_hash: scope.content_hash.clone(),
        outcome_id: scope.members[0].outcome.id.clone(),
        expected_feedback_sequence: 0,
        reason_code: "poor_boundary".into(),
        question: "TEST pending single request".into(),
        resume_checkpoint_ref: Uuid::new_v4(),
    };
    store
        .create_conversation_human_request(&f.project, &single)
        .unwrap();
    assert!(
        store
            .create_conversation_image_class_review(&f.project, f.conversation, f.task, &input)
            .is_err()
    );
    store
        .close_conversation_human_request(&f.project, single.id, false)
        .unwrap();
    let review = store
        .create_conversation_image_class_review(&f.project, f.conversation, f.task, &input)
        .unwrap();
    store
        .request_conversation_call_cancel(&f.project, f.task, f.source.id)
        .unwrap();
    assert_eq!(
        store
            .create_conversation_image_class_review(&f.project, f.conversation, f.task, &input)
            .unwrap(),
        review
    );
    let mut changed = input.clone();
    changed.target_label = Some("cup".into());
    assert!(
        store
            .create_conversation_image_class_review(&f.project, f.conversation, f.task, &changed)
            .is_err()
    );
    let cancelled = store
        .cancel_conversation_image_class_review(&f.project, f.conversation, f.task, review.id)
        .unwrap();
    assert_eq!(
        store
            .cancel_conversation_image_class_review(&f.project, f.conversation, f.task, review.id)
            .unwrap(),
        cancelled
    );
    assert!(
        store
            .answer_conversation_image_class_review(
                &f.project,
                f.conversation,
                f.task,
                review.id,
                &keep(&review)
            )
            .is_err()
    );
    assert!(
        store
            .conversation_image_class_review("TEST foreign", f.conversation, f.task, review.id)
            .is_err()
    );
}

#[test]
fn class_snapshot_never_expands_to_relabelled_other_members_or_intermediate_results() {
    let store = SqliteStore::open_in_memory().unwrap();
    let f = fixture(&store, "bounding_box", 3);
    let mut test = store
        .get_workflow_sample_test_by_id("TEST-scoped-sample")
        .unwrap()
        .unwrap();
    test.report.samples[0].projection.final_candidates[1]
        .outcome
        .label = "cupboard".into();
    test.report.samples[0].projection.final_candidates[2]
        .outcome
        .label = "bottle".into();
    test.report.samples[0].outcomes = test.report.samples[0]
        .projection
        .final_candidates
        .iter()
        .map(|c| c.outcome.clone())
        .collect();
    let mut intermediate = test.report.samples[0].outcomes[0].clone();
    intermediate.id = "TEST-intermediate".into();
    test.report.samples[0].outcomes.push(intermediate);
    store
        .with_connection(|db| {
            db.execute(
                "UPDATE workflow_sample_tests SET report_json=?1 WHERE id='TEST-scoped-sample'",
                [serde_json::to_string(&test.report)?],
            )?;
            Ok(())
        })
        .unwrap();
    let review = create(&store, &f, None);
    assert_eq!(review.scope.members.len(), 1);
    let mut foreign = keep(&review);
    foreign.actions[0] = ConversationImageClassAction::Keep {
        outcome_id: "TEST-intermediate".into(),
        source_artifact_id: Uuid::new_v4(),
    };
    assert!(
        store
            .answer_conversation_image_class_review(
                &f.project,
                f.conversation,
                f.task,
                review.id,
                &foreign
            )
            .is_err()
    );
    let mut prior = baseline_revision(&f, "TEST-final-2", 1);
    prior.corrected_label = Some("cup".into());
    store.save_sample_feedback(&prior).unwrap();
    assert_eq!(
        store
            .conversation_image_class_review(&f.project, f.conversation, f.task, review.id)
            .unwrap()
            .unwrap()
            .scope
            .members
            .len(),
        1
    );
    assert!(
        store
            .answer_conversation_image_class_review(
                &f.project,
                f.conversation,
                f.task,
                review.id,
                &keep(&review)
            )
            .is_err()
    );
}
