use super::*;
use crate::*;
use chrono::{Duration, Utc};
use serde_json::json;

fn fixture(
    store: &SqliteStore,
) -> (
    crate::conversation_future_schema::tests::Fixture,
    ConversationFutureSchemaProposalAuthorizationRecord,
) {
    let saved = crate::conversation_future_schema::tests::fixture(store);
    let source = ConversationFutureSchemaProposalSource {
        feedback_call_id: saved.input.feedback_call_id,
        scope_answer_command_id: saved.input.scope_answer_command_id,
        context_digest: saved.input.context_digest.clone(),
        base_schema_id: saved.input.base_schema_id,
        base_schema_revision: saved.input.base_schema_revision,
    };
    let call = Uuid::new_v4();
    let expiry = Utc::now() + Duration::minutes(5);
    let scope = "d".repeat(64);
    let record = ConversationFutureSchemaProposalAuthorizationRecord {
        consent: ConversationFeedbackAuthorization {
            call_id: call,
            message_id: saved.source.record.consent.message_id,
            model_id: annotagent_core::ModelProfileId::new(),
            previous_grant_id: Some(saved.source.record.grant.id),
            scope_hash: scope.clone(),
            expires_at: expiry,
            allow_unknown_cost: true,
        },
        grant: ConversationCallGrant {
            id: call,
            task_id: saved.source.task,
            scope_hash: scope,
            maximum_calls: saved.source.record.grant.maximum_calls + 1,
            expires_at: expiry,
        },
        context: json!({"source":source,"base_schema":saved.base,"feedback":saved.source.record.context,"scope":"future_tasks_only"}),
        source,
        summary: json!({"remote_model":"TEST future schema","model_name":"TEST","destination":"TEST offline","maximum_output_tokens":2048}),
    };
    (saved, record)
}

#[test]
fn authorization_persists_once_without_inference_or_schema_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST-future-proposal.db");
    let store = SqliteStore::open(&path).unwrap();
    let (saved, record) = fixture(&store);
    let f = &saved.source;
    let before = store.conversation_task_budget(&f.project, f.task).unwrap();
    assert_eq!(
        store
            .authorize_conversation_future_schema_proposal(
                &f.project,
                f.conversation,
                f.task,
                &record
            )
            .unwrap(),
        record
    );
    assert_eq!(
        store
            .conversation_task_budget(&f.project, f.task)
            .unwrap()
            .planning_reserved_calls,
        before.planning_reserved_calls
    );
    assert!(
        store
            .future_schema_draft(&f.project, f.task, record.source.feedback_call_id)
            .unwrap()
            .is_none()
    );
    drop(store);
    let store = SqliteStore::open(&path).unwrap();
    assert_eq!(
        store
            .conversation_future_schema_proposal_for_feedback(
                &f.project,
                f.task,
                record.source.feedback_call_id
            )
            .unwrap(),
        Some(record.clone())
    );
    assert_eq!(
        store
            .authorize_conversation_future_schema_proposal(
                &f.project,
                f.conversation,
                f.task,
                &record
            )
            .unwrap(),
        record
    );
}

fn proposal_response() -> annotagent_core::ModelResponse {
    serde_json::from_value(json!({"content":null,"tool_calls":[{"id":"TEST-proposal-tool","name":"propose_future_annotation_schema","arguments":{"goal":"TEST only cups including occluded cups","decision":"draft","kind":"bounding_box","labels":["cup"],"multi_label":false,"attributes":{},"boundary_rules":["TEST include occluded cups"],"rationale":"TEST future tasks only"}}],"usage":annotagent_core::TokenUsage::known(10,5,annotagent_core::UsageSource::Mock),"request_id":"TEST-request","provider_metadata":{}})).unwrap()
}

fn complete(
    store: &SqliteStore,
    saved: &crate::conversation_future_schema::tests::Fixture,
    record: &ConversationFutureSchemaProposalAuthorizationRecord,
    response: &annotagent_core::ModelResponse,
) -> ConversationFutureSchemaInput {
    let f = &saved.source;
    store
        .authorize_conversation_future_schema_proposal(&f.project, f.conversation, f.task, record)
        .unwrap();
    assert_eq!(
        store
            .reserve_conversation_call(
                &f.project,
                f.task,
                record.consent.call_id,
                &record.consent.scope_hash,
                &"f".repeat(64)
            )
            .unwrap(),
        ConversationCallAdmission::Admitted
    );
    store.finish_conversation_call(&f.project,f.task,record.consent.call_id,ConversationCallStatus::Completed,json!({"phase":"future_schema_patch_text","context":{"subject":record.context,"scope_hash":record.consent.scope_hash},"response":response,"proposal":{"Ok":{"untrusted":"do not consume cached parse"}}})).unwrap();
    let mut input = saved.input.clone();
    input.proposal_call_id = Some(record.consent.call_id);
    input.proposal_digest = Some(annotagent_image_tools::sha256(
        &serde_json::to_vec(&json!({"context":record.context,"response":response})).unwrap(),
    ));
    input
}

#[test]
fn actual_model_proposal_can_only_be_materialized_by_explicit_human_confirmation() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (saved, record) = fixture(&store);
    let f = &saved.source;
    let input = complete(&store, &saved, &record, &proposal_response());
    assert!(
        store
            .future_schema_draft(&f.project, f.task, input.feedback_call_id)
            .unwrap()
            .is_none()
    );
    let before = store.conversation_task_budget(&f.project, f.task).unwrap();
    let result = store
        .create_future_schema_draft(&f.project, f.conversation, f.task, &input)
        .unwrap();
    assert_ne!(result.schema_id, saved.base.id);
    assert_eq!(result.input, input);
    assert_eq!(
        store.conversation_task_budget(&f.project, f.task).unwrap(),
        before
    );
    store
        .request_conversation_call_cancel(&f.project, f.task, record.consent.call_id)
        .unwrap();
    assert_eq!(
        store
            .create_future_schema_draft(&f.project, f.conversation, f.task, &input)
            .unwrap(),
        result
    );
}

#[test]
fn authorization_rejects_changed_source_identity_and_blockers_atomically() {
    for variant in 0..15 {
        let store = SqliteStore::open_in_memory().unwrap();
        let (saved, mut record) = fixture(&store);
        let f = &saved.source;
        match variant {
            0 => record.consent.message_id = Uuid::new_v4(),
            1 => record.source.context_digest = "0".repeat(64),
            2 => record.source.scope_answer_command_id = Uuid::new_v4(),
            3 => record.source.base_schema_id = Uuid::new_v4(),
            4 => record.context["scope"] = json!("all_existing_runs"),
            5 => record.context["feedback"]["candidate"]["outcome"]["id"] = json!("guessed"),
            6 => record.grant.maximum_calls += 1,
            7 => {
                record.consent.expires_at = Utc::now() - Duration::seconds(1);
                record.grant.expires_at = record.consent.expires_at;
            }
            8 => {
                store
                    .request_conversation_call_cancel(&f.project, f.task, f.source.id)
                    .unwrap();
            }
            9 => {
                store
                    .request_conversation_call_cancel(&f.project, f.task, record.consent.call_id)
                    .unwrap();
            }
            10 => {
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
            11 => {
                store
                    .with_connection(|db| {
                        db.execute("DELETE FROM sample_scope_seals", [])?;
                        Ok(())
                    })
                    .unwrap();
            }
            12 => {
                store.with_connection(|db|{db.execute("INSERT INTO conversation_human_requests(id,task_id,request_json,status,created_at) VALUES(?1,?2,'{}','pending',?3)",params![Uuid::new_v4().to_string(),f.task.to_string(),Utc::now().to_rfc3339()])?;Ok(())}).unwrap();
            }
            13 => {
                store
                    .create_future_schema_draft(&f.project, f.conversation, f.task, &saved.input)
                    .unwrap();
            }
            _ => {
                store.with_connection(|db|{db.execute_batch("CREATE TRIGGER TEST_reject_proposal BEFORE INSERT ON conversation_future_schema_proposal_authorizations BEGIN SELECT RAISE(ABORT,'TEST rollback'); END;")?;Ok(())}).unwrap();
            }
        }
        let before = store.conversation_call_budget(&f.project, f.task).unwrap();
        assert!(
            store
                .authorize_conversation_future_schema_proposal(
                    &f.project,
                    f.conversation,
                    f.task,
                    &record
                )
                .is_err(),
            "accepted variant {variant}"
        );
        assert_eq!(
            store.conversation_call_budget(&f.project, f.task).unwrap(),
            before,
            "changed grant for {variant}"
        );
        assert!(
            store
                .conversation_future_schema_proposal_authorization(
                    &f.project,
                    f.task,
                    record.consent.call_id
                )
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn historical_authorization_retry_never_restores_old_current_grant_or_live_context() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (saved, record) = fixture(&store);
    let f = &saved.source;
    store
        .authorize_conversation_future_schema_proposal(&f.project, f.conversation, f.task, &record)
        .unwrap();
    let later = ConversationCallGrant {
        id: Uuid::new_v4(),
        task_id: f.task,
        scope_hash: "e".repeat(64),
        maximum_calls: record.grant.maximum_calls + 1,
        expires_at: Utc::now() + Duration::minutes(6),
    };
    store
        .advance_conversation_authorization(&f.project, record.grant.id, &later)
        .unwrap();
    store
        .request_conversation_call_cancel(&f.project, f.task, f.source.id)
        .unwrap();
    store
        .with_connection(|db| {
            db.execute(
                "UPDATE images SET sha256='TEST changed' WHERE project_id=?1",
                [&f.project],
            )?;
            Ok(())
        })
        .unwrap();
    let before = store.conversation_call_budget(&f.project, f.task).unwrap();
    assert_eq!(
        store
            .authorize_conversation_future_schema_proposal(
                &f.project,
                f.conversation,
                f.task,
                &record
            )
            .unwrap(),
        record
    );
    assert_eq!(
        store.conversation_call_budget(&f.project, f.task).unwrap(),
        before
    );
    let mut changed = record.clone();
    changed.summary["destination"] = json!("changed remote host");
    assert!(
        store
            .authorize_conversation_future_schema_proposal(
                &f.project,
                f.conversation,
                f.task,
                &changed
            )
            .is_err()
    );
    assert!(
        store
            .reserve_conversation_call(
                &f.project,
                f.task,
                record.consent.call_id,
                &record.consent.scope_hash,
                &"f".repeat(64)
            )
            .is_err()
    );
}

#[test]
fn source_changes_between_authorization_and_admission_never_spend() {
    for variant in 0..4 {
        let store = SqliteStore::open_in_memory().unwrap();
        let (saved, record) = fixture(&store);
        let f = &saved.source;
        store
            .authorize_conversation_future_schema_proposal(
                &f.project,
                f.conversation,
                f.task,
                &record,
            )
            .unwrap();
        match variant {
            0 => {
                store
                    .request_conversation_call_cancel(&f.project, f.task, f.source.id)
                    .unwrap();
            }
            1 => {
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
            2 => {
                store
                    .create_future_schema_draft(&f.project, f.conversation, f.task, &saved.input)
                    .unwrap();
            }
            _ => {
                store
                    .request_conversation_call_cancel(&f.project, f.task, record.consent.call_id)
                    .unwrap();
            }
        }
        let before = store.conversation_call_budget(&f.project, f.task).unwrap();
        assert!(
            store
                .reserve_conversation_call(
                    &f.project,
                    f.task,
                    record.consent.call_id,
                    &record.consent.scope_hash,
                    &"f".repeat(64)
                )
                .is_err(),
            "admitted {variant}"
        );
        assert_eq!(
            store.conversation_call_budget(&f.project, f.task).unwrap(),
            before
        );
    }
}

#[test]
fn raw_proposal_protocol_rejects_unsupported_or_injected_fields() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (saved, _) = fixture(&store);
    for variant in 0..15 {
        let mut response = serde_json::to_value(proposal_response()).unwrap();
        match variant {
            0 => response["content"] = json!("do this now"),
            1 => {
                let duplicate = response["tool_calls"][0].clone();
                response["tool_calls"]
                    .as_array_mut()
                    .unwrap()
                    .push(duplicate);
            }
            2 => response["tool_calls"][0]["name"] = json!("apply_schema"),
            3 => response["tool_calls"][0]["arguments"]["decision"] = json!("delete"),
            4 => response["tool_calls"][0]["arguments"]["kind"] = json!("segmentation"),
            5 => response["tool_calls"][0]["arguments"]["task_id"] = json!(Uuid::new_v4()),
            6 => response["tool_calls"][0]["arguments"]["scope"] = json!("all_runs"),
            7 => response["tool_calls"][0]["arguments"]["coordinates"] = json!([1, 2, 3, 4]),
            8 => response["tool_calls"][0]["arguments"]["human_verified"] = json!(true),
            9 => response["tool_calls"][0]["arguments"]["goal"] = json!("a".repeat(4001)),
            10 => response["tool_calls"][0]["arguments"]["labels"] = json!(["cup", "cup"]),
            11 => {
                response["tool_calls"][0]["arguments"]["attributes"] =
                    json!({"occluded":{"type":"boolean","required":false,"coordinates":[]}});
            }
            12 => {
                response["tool_calls"][0]["arguments"] = json!({"goal":"TEST","decision":"clarify","question":"TEST?","rationale":"TEST","labels":["cup"]});
            }
            13 => {
                response["tool_calls"][0]["arguments"] =
                    json!({"goal":"TEST","decision":"clarify","question":" ","rationale":"TEST"});
            }
            _ => response["tool_calls"] = json!([]),
        }
        assert!(
            validate_response(&response, &saved.base.definition).is_err(),
            "accepted {variant}"
        );
    }
}

#[test]
fn provenance_rejects_receipt_tampering_cancellation_or_digest_conflicts_without_fork() {
    for variant in 0..10 {
        let store = SqliteStore::open_in_memory().unwrap();
        let (saved, record) = fixture(&store);
        let f = &saved.source;
        let mut input = complete(&store, &saved, &record, &proposal_response());
        match variant {
            0 => input.proposal_digest = None,
            1 => input.proposal_call_id = None,
            2 => input.proposal_digest = Some("0".repeat(64)),
            3 => input.proposal_call_id = Some(Uuid::new_v4()),
            4 => {
                store
                    .request_conversation_call_cancel(&f.project, f.task, record.consent.call_id)
                    .unwrap();
            }
            _ => store
                .with_connection(|db| {
                    let receipt =
                        crate::conversation_calls::receipt(db, record.consent.call_id)?.unwrap();
                    let mut evidence = receipt.evidence.unwrap();
                    match variant {
                        5 => evidence["phase"] = json!("schema_text"),
                        6 => evidence["context"]["subject"]["scope"] = json!("current_image"),
                        7 => evidence["context"]["scope_hash"] = json!("0".repeat(64)),
                        8 => evidence["cancelled"] = json!(true),
                        _ => {
                            evidence["response"]["tool_calls"][0]["arguments"]["goal"] =
                                json!("TEST changed response");
                        }
                    }
                    db.execute(
                        "UPDATE conversation_model_calls SET evidence_json=?2 WHERE id=?1",
                        params![
                            record.consent.call_id.to_string(),
                            serde_json::to_string(&evidence)?
                        ],
                    )?;
                    Ok(())
                })
                .unwrap(),
        }
        let before = store.conversation_task_budget(&f.project, f.task).unwrap();
        assert!(
            store
                .create_future_schema_draft(&f.project, f.conversation, f.task, &input)
                .is_err(),
            "accepted {variant}"
        );
        assert!(
            store
                .future_schema_draft(&f.project, f.task, input.feedback_call_id)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store.conversation_task_budget(&f.project, f.task).unwrap(),
            before
        );
        assert_eq!(
            store
                .human_conversation_schema_drafts(&f.project, f.task)
                .unwrap(),
            vec![saved.base]
        );
    }
}

#[test]
fn completed_clarification_can_be_answered_locally_and_historical_retry_preserves_later_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST-proposal-replay.db");
    let store = SqliteStore::open(&path).unwrap();
    let (saved, record) = fixture(&store);
    let f = &saved.source;
    let mut response = proposal_response();
    response.tool_calls[0].arguments = json!({"goal":"TEST cups only","decision":"clarify","question":"TEST visible or inferred occluded extent?","rationale":"TEST needs a human boundary rule"});
    let input = complete(&store, &saved, &record, &response);
    let result = store
        .create_future_schema_draft(&f.project, f.conversation, f.task, &input)
        .unwrap();
    let mut edited = input.definition.clone();
    edited
        .boundary_rules
        .push("TEST human edit after confirmation".into());
    let later = store
        .revise_conversation_schema_draft(&f.project, result.schema_id, Uuid::new_v4(), 1, &edited)
        .unwrap();
    store
        .request_conversation_call_cancel(&f.project, f.task, record.consent.call_id)
        .unwrap();
    store
        .with_connection(|db| {
            db.execute(
                "UPDATE images SET sha256='TEST changed' WHERE project_id=?1",
                [&f.project],
            )?;
            Ok(())
        })
        .unwrap();
    drop(store);
    let store = SqliteStore::open(&path).unwrap();
    assert_eq!(
        store
            .create_future_schema_draft(&f.project, f.conversation, f.task, &input)
            .unwrap(),
        result
    );
    assert_eq!(
        store
            .conversation_schema_draft(&f.project, result.schema_id, None)
            .unwrap(),
        later
    );
    let mut changed = input.clone();
    changed.proposal_digest = Some("0".repeat(64));
    assert!(
        store
            .create_future_schema_draft(&f.project, f.conversation, f.task, &changed)
            .is_err()
    );
}

#[test]
fn concurrent_authorization_for_one_source_only_adds_one_call() {
    let store = std::sync::Arc::new(SqliteStore::open_in_memory().unwrap());
    let (saved, record) = fixture(&store);
    let f = &saved.source;
    let mut other = record.clone();
    other.consent.call_id = Uuid::new_v4();
    other.grant.id = other.consent.call_id;
    let start = std::sync::Barrier::new(3);
    let results = std::thread::scope(|threads| {
        let first = threads.spawn(|| {
            start.wait();
            store.authorize_conversation_future_schema_proposal(
                &f.project,
                f.conversation,
                f.task,
                &record,
            )
        });
        let second = threads.spawn(|| {
            start.wait();
            store.authorize_conversation_future_schema_proposal(
                &f.project,
                f.conversation,
                f.task,
                &other,
            )
        });
        start.wait();
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let current = store
        .conversation_call_budget(&f.project, f.task)
        .unwrap()
        .unwrap();
    assert_eq!(
        current.current_grant.maximum_calls,
        record.grant.maximum_calls
    );
    assert_eq!(current.used_calls, 1);
}

#[test]
fn foreign_task_cannot_read_cancel_or_claim_a_saved_proposal() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (saved, record) = fixture(&store);
    let f = &saved.source;
    store
        .authorize_conversation_future_schema_proposal(&f.project, f.conversation, f.task, &record)
        .unwrap();
    let message = ConversationMessageInput {
        id: Uuid::new_v4(),
        text: "TEST independent goal".into(),
        image: None,
        reference: None,
    };
    store
        .append_conversation_message(&f.project, f.conversation, &message)
        .unwrap();
    let other = Uuid::new_v4();
    store
        .begin_conversation_task(
            &f.project,
            f.conversation,
            &BeginConversationTask {
                id: other,
                source_message_id: message.id,
                schema_revision: "a".repeat(64),
            },
        )
        .unwrap();
    assert!(
        store
            .conversation_future_schema_proposal_authorization(
                &f.project,
                other,
                record.consent.call_id
            )
            .is_err()
    );
    assert!(
        store
            .conversation_future_schema_proposal_for_feedback(
                &f.project,
                other,
                record.source.feedback_call_id
            )
            .is_err()
    );
    assert!(
        store
            .request_conversation_call_cancel(&f.project, other, record.consent.call_id)
            .is_err()
    );
    assert!(
        store
            .conversation_future_schema_proposal_authorization(
                "TEST foreign owner",
                f.task,
                record.consent.call_id
            )
            .is_err()
    );
    assert_eq!(
        store
            .reserve_conversation_call(
                &f.project,
                f.task,
                record.consent.call_id,
                &record.consent.scope_hash,
                &"f".repeat(64)
            )
            .unwrap(),
        ConversationCallAdmission::Admitted
    );
}

#[test]
fn concurrent_identical_authorizations_restore_one_record_without_double_increment() {
    let store = std::sync::Arc::new(SqliteStore::open_in_memory().unwrap());
    let (saved, record) = fixture(&store);
    let f = &saved.source;
    let start = std::sync::Barrier::new(3);
    std::thread::scope(|threads| {
        let first = threads.spawn(|| {
            start.wait();
            store.authorize_conversation_future_schema_proposal(
                &f.project,
                f.conversation,
                f.task,
                &record,
            )
        });
        let second = threads.spawn(|| {
            start.wait();
            store.authorize_conversation_future_schema_proposal(
                &f.project,
                f.conversation,
                f.task,
                &record,
            )
        });
        start.wait();
        assert_eq!(first.join().unwrap().unwrap(), record);
        assert_eq!(second.join().unwrap().unwrap(), record);
    });
    let budget = store
        .conversation_call_budget(&f.project, f.task)
        .unwrap()
        .unwrap();
    assert_eq!(budget.current_grant, record.grant);
    assert_eq!(budget.used_calls, 1);
}

#[test]
fn proposal_cancellation_and_human_confirmation_have_one_durable_order() {
    for _ in 0..4 {
        let store = std::sync::Arc::new(SqliteStore::open_in_memory().unwrap());
        let (saved, record) = fixture(&store);
        let f = &saved.source;
        let input = complete(&store, &saved, &record, &proposal_response());
        let start = std::sync::Barrier::new(3);
        let result = std::thread::scope(|threads| {
            let confirm = threads.spawn(|| {
                start.wait();
                store.create_future_schema_draft(&f.project, f.conversation, f.task, &input)
            });
            let cancel = threads.spawn(|| {
                start.wait();
                store.request_conversation_call_cancel(&f.project, f.task, record.consent.call_id)
            });
            start.wait();
            cancel.join().unwrap().unwrap();
            confirm.join().unwrap()
        });
        let persisted = store
            .future_schema_draft(&f.project, f.task, input.feedback_call_id)
            .unwrap();
        if let Ok(record) = result {
            assert_eq!(persisted, Some(record.clone()));
            assert_eq!(
                store
                    .create_future_schema_draft(&f.project, f.conversation, f.task, &input)
                    .unwrap(),
                record
            );
        } else {
            assert!(persisted.is_none());
            assert!(
                store
                    .create_future_schema_draft(&f.project, f.conversation, f.task, &input)
                    .is_err()
            );
        }
        assert_eq!(
            store
                .conversation_call_budget(&f.project, f.task)
                .unwrap()
                .unwrap()
                .used_calls,
            2
        );
    }
}
