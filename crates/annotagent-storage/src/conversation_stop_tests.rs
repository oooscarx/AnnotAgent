use super::*;
use crate::{
    BeginConversationTask, ConversationCallAdmission, ConversationCallGrant,
    ConversationCallStatus, SampleOperation,
};
use serde_json::json;

fn setup(store: &SqliteStore) -> (String, Uuid, Uuid) {
    let project = Uuid::new_v4().to_string();
    let conversation = store.create_conversation(&project).unwrap();
    let message = ConversationMessageInput {
        id: Uuid::new_v4(),
        text: "TEST goal".into(),
        image: None,
        reference: None,
    };
    store
        .append_conversation_message(&project, conversation, &message)
        .unwrap();
    let task = Uuid::new_v4();
    store
        .begin_conversation_task(
            &project,
            conversation,
            &BeginConversationTask {
                id: task,
                source_message_id: message.id,
                schema_revision: "a".repeat(64),
            },
        )
        .unwrap();
    (project, conversation, task)
}
fn command(task: Option<Uuid>) -> ConversationMessageInput {
    ConversationMessageInput {
        id: Uuid::new_v4(),
        text: "停止".into(),
        image: None,
        reference: Some(ConversationSelectionRef::StopRequest { task_id: task }),
    }
}
fn grant(store: &SqliteStore, project: &str, task: Uuid) -> ConversationCallGrant {
    let grant = ConversationCallGrant {
        id: Uuid::new_v4(),
        task_id: task,
        scope_hash: "a".repeat(64),
        maximum_calls: 8,
        expires_at: Utc::now() + chrono::Duration::minutes(5),
    };
    store.authorize_conversation_calls(project, &grant).unwrap();
    grant
}

#[test]
fn noop_is_frozen_and_stop_is_never_a_goal() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let input = command(Some(task));
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &input)
        .unwrap();
    assert_eq!(saved.status, ConversationStopStatus::NoActiveWork);
    assert!(saved.targets.is_empty());
    assert!(
        store
            .begin_conversation_task(
                &project,
                conversation,
                &BeginConversationTask {
                    id: Uuid::new_v4(),
                    source_message_id: input.id,
                    schema_revision: "a".repeat(64)
                }
            )
            .is_err()
    );
    grant(&store, &project, task);
    assert_eq!(
        store
            .begin_conversation_stop(&project, "TEST", conversation, &input)
            .unwrap(),
        saved
    );
    let mut changed = input.clone();
    changed.text = "stop".into();
    assert!(
        store
            .begin_conversation_stop(&project, "TEST", conversation, &changed)
            .is_err()
    );
}

#[test]
fn pending_grant_stops_random_children_and_late_sample_without_spending() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let grant = grant(&store, &project, task);
    let result = store
        .begin_conversation_stop(&project, "TEST", conversation, &command(Some(task)))
        .unwrap();
    assert_eq!(result.targets.len(), 1);
    assert_eq!(
        result.targets[0].kind,
        ConversationStopTargetKind::Authorization
    );
    assert_eq!(result.status, ConversationStopStatus::CancelRequested);
    assert!(
        store
            .reserve_conversation_call(
                &project,
                task,
                Uuid::new_v4(),
                &grant.scope_hash,
                &"b".repeat(64)
            )
            .is_err()
    );
    assert!(
        store
            .reserve_conversation_builder(&project, task, grant.id, &"b".repeat(64))
            .is_err()
    );
    let op = SampleOperation {
        id: grant.id.to_string(),
        project_id: "TEST".into(),
        draft_id: "draft".into(),
        authorization_fingerprint: "a".repeat(64),
        request: json!({"conversation":{"conversation_id":conversation,"task_id":task}}),
        status: "queued".into(),
        error: None,
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    };
    assert!(store.reserve_sample_operation(&op).is_err());
    assert_eq!(
        store
            .conversation_task_budget(&project, task)
            .unwrap()
            .planning_reserved_calls,
        0
    );
}

#[test]
fn multiple_targets_require_exact_selection_and_finished_never_redirects() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let grant = grant(&store, &project, task);
    let call = Uuid::new_v4();
    assert_eq!(
        store
            .reserve_conversation_call(&project, task, call, &grant.scope_hash, &"b".repeat(64))
            .unwrap(),
        ConversationCallAdmission::Admitted
    );
    let builder = Uuid::new_v4();
    store
        .reserve_conversation_builder(&project, task, builder, &"c".repeat(64))
        .unwrap();
    let input = command(Some(task));
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &input)
        .unwrap();
    assert_eq!(saved.status, ConversationStopStatus::NeedsSelection);
    assert_eq!(saved.targets.len(), 2);
    assert!(
        store
            .conversation_call_cancellations(&project, task)
            .unwrap()
            .is_empty()
    );
    let target = saved
        .targets
        .iter()
        .find(|target| target.id == call.to_string())
        .unwrap()
        .reference();
    store
        .finish_conversation_call(
            &project,
            task,
            call,
            ConversationCallStatus::Completed,
            json!({}),
        )
        .unwrap();
    let done = store
        .select_conversation_stop(&project, "TEST", conversation, input.id, &target)
        .unwrap();
    assert_eq!(done.status, ConversationStopStatus::Finished);
    assert!(
        store
            .conversation_call_cancellations(&project, task)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .select_conversation_stop(
                &project,
                "TEST",
                conversation,
                input.id,
                &saved
                    .targets
                    .iter()
                    .find(|target| target.id == builder.to_string())
                    .unwrap()
                    .reference()
            )
            .is_err()
    );
}

#[test]
fn exact_ownership_text_and_reference_fields_fail_closed() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let (foreign, other, foreign_task) = setup(&store);
    assert!(
        store
            .begin_conversation_stop(&foreign, "TEST", conversation, &command(Some(task)))
            .is_err()
    );
    assert!(
        store
            .begin_conversation_stop(&project, "TEST", conversation, &command(Some(foreign_task)))
            .is_err()
    );
    for text in ["please stop", "stop sign", "停止这个框", "STOP!", ""] {
        let mut input = command(None);
        input.text = text.into();
        assert!(
            store
                .begin_conversation_stop(&project, "TEST", conversation, &input)
                .is_err()
        );
        assert!(
            store
                .conversation_message(&project, conversation, input.id)
                .unwrap()
                .is_none()
        );
    }
    let mut padded = command(Some(task));
    padded.text = format!("{}stop", " ".repeat(65_537));
    assert!(
        store
            .begin_conversation_stop(&project, "TEST", conversation, &padded)
            .is_err()
    );
    assert!(
        serde_json::from_value::<ConversationSelectionRef>(
            json!({"scope":"stop_request","task_id":task,"image_id":"injected"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<ConversationStopTargetRef>(
            json!({"kind":"call","id":"x","task_id":task,"all":true})
        )
        .is_err()
    );
    let mut input = command(Some(task));
    input.image = Some(crate::ConversationImageRef {
        image_id: "injected".into(),
        sha256: "hash".into(),
    });
    assert!(
        store
            .begin_conversation_stop(&project, "TEST", conversation, &input)
            .is_err()
    );
    let input = command(None);
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &input)
        .unwrap();
    assert!(
        store
            .conversation_stop_request(&foreign, other, input.id)
            .is_err()
    );
    assert_eq!(
        store
            .conversation_stop_request(&project, conversation, input.id)
            .unwrap(),
        Some(saved)
    );
}

#[test]
fn schema_pending_authorization_is_typed_and_stopped_before_reservation() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let input = crate::ConversationSchemaAuthorization {
        call_id: Uuid::new_v4(),
        model_id: annotagent_core::ModelProfileId::new(),
        scope_hash: "a".repeat(64),
        expires_at: Utc::now() + chrono::Duration::minutes(5),
        allow_unknown_cost: true,
    };
    store
        .authorize_conversation_schema(&project, task, &input)
        .unwrap();
    let stopped = store
        .begin_conversation_stop(&project, "TEST", conversation, &command(Some(task)))
        .unwrap();
    assert_eq!(stopped.targets[0].kind, ConversationStopTargetKind::Call);
    assert_eq!(stopped.targets[0].id, input.call_id.to_string());
    assert!(
        store
            .reserve_conversation_call(
                &project,
                task,
                input.call_id,
                &input.scope_hash,
                &"b".repeat(64)
            )
            .is_err()
    );
    assert_eq!(
        store
            .conversation_task_budget(&project, task)
            .unwrap()
            .planning_reserved_calls,
        0
    );
}

#[test]
fn stopping_one_snapshot_target_is_atomic_under_conflicting_concurrent_selection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST-stop.db");
    let store = SqliteStore::open(&path).unwrap();
    let (project, conversation, task) = setup(&store);
    for _ in 0..2 {
        store
            .reserve_conversation_builder(&project, task, Uuid::new_v4(), &"a".repeat(64))
            .unwrap();
    }
    let input = command(Some(task));
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &input)
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let mut threads = Vec::new();
    for target in &saved.targets {
        let store = SqliteStore::open(&path).unwrap();
        let project = project.clone();
        let barrier = barrier.clone();
        let target = target.reference();
        let message = input.id;
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            store.select_conversation_stop(&project, "TEST", conversation, message, &target)
        }));
    }
    let results = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        store
            .conversation_call_cancellations(&project, task)
            .unwrap()
            .len(),
        1
    );
    let reopened = SqliteStore::open(&path).unwrap();
    let restored = reopened
        .conversation_stop_request(&project, conversation, input.id)
        .unwrap()
        .unwrap();
    assert_eq!(restored.status, ConversationStopStatus::CancelRequested);
    let winner = restored.selected_target.as_ref().unwrap();
    assert_eq!(
        reopened
            .select_conversation_stop(&project, "TEST", conversation, input.id, winner)
            .unwrap(),
        restored
    );
    assert_eq!(
        reopened
            .begin_conversation_stop(&project, "TEST", conversation, &input)
            .unwrap(),
        restored
    );
}

#[test]
fn cancellation_failure_rolls_back_message_snapshot_and_all_side_effects() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    grant(&store, &project, task);
    let input = command(Some(task));
    store.with_connection(|db|{db.execute_batch("CREATE TRIGGER TEST_fail_stop BEFORE INSERT ON conversation_call_cancellations BEGIN SELECT RAISE(ABORT,'TEST stop rollback'); END;")?;Ok(())}).unwrap();
    assert!(
        store
            .begin_conversation_stop(&project, "TEST", conversation, &input)
            .is_err()
    );
    assert!(
        store
            .conversation_message(&project, conversation, input.id)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .conversation_stop_request(&project, conversation, input.id)
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .conversation_call_cancellations(&project, task)
            .unwrap()
            .is_empty()
    );
    store
        .with_connection(|db| {
            db.execute_batch("DROP TRIGGER TEST_fail_stop;")?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        store
            .begin_conversation_stop(&project, "TEST", conversation, &input)
            .unwrap()
            .status,
        ConversationStopStatus::CancelRequested
    );
}

#[test]
fn untyped_snapshot_remains_stoppable_after_random_child_admission() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let grant = grant(&store, &project, task);
    store
        .reserve_conversation_builder(&project, task, Uuid::new_v4(), &"c".repeat(64))
        .unwrap();
    let input = command(Some(task));
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &input)
        .unwrap();
    let target = saved
        .targets
        .iter()
        .find(|t| t.kind == ConversationStopTargetKind::Authorization)
        .unwrap()
        .reference();
    let child = Uuid::new_v4();
    store
        .reserve_conversation_call(&project, task, child, &grant.scope_hash, &"b".repeat(64))
        .unwrap();
    let selected = store
        .select_conversation_stop(&project, "TEST", conversation, input.id, &target)
        .unwrap();
    assert_eq!(selected.status, ConversationStopStatus::CancelRequested);
    assert!(
        store
            .reserve_conversation_call(
                &project,
                task,
                Uuid::new_v4(),
                &grant.scope_hash,
                &"d".repeat(64)
            )
            .is_err()
    );
    assert!(matches!(
        store
            .reserve_conversation_call(&project, task, child, &grant.scope_hash, &"b".repeat(64))
            .unwrap(),
        ConversationCallAdmission::Existing(_)
    ));
    assert_eq!(
        store
            .conversation_task_budget(&project, task)
            .unwrap()
            .planning_reserved_calls,
        1
    );
}

#[test]
fn journey_parent_covers_admission_windows_and_children_deduplicate() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, consent, _) = crate::conversation_journey::tests::setup(&store);
    store
        .save_conversation_journey(&project, conversation, &consent)
        .unwrap();
    let grant = ConversationCallGrant {
        id: consent.builder_operation_id,
        task_id: consent.task_id,
        scope_hash: consent.builder_scope_hash.clone(),
        maximum_calls: 8,
        expires_at: consent.expires_at,
    };
    store
        .authorize_conversation_calls(&project, &grant)
        .unwrap();
    store
        .reserve_conversation_builder(
            &project,
            consent.task_id,
            consent.builder_operation_id,
            &"a".repeat(64),
        )
        .unwrap();
    store
        .reserve_conversation_call(
            &project,
            consent.task_id,
            Uuid::new_v4(),
            &grant.scope_hash,
            &"b".repeat(64),
        )
        .unwrap();
    let stopped = store
        .begin_conversation_stop(
            &project,
            "TEST",
            conversation,
            &command(Some(consent.task_id)),
        )
        .unwrap();
    assert_eq!(stopped.targets.len(), 1);
    assert_eq!(stopped.targets[0].kind, ConversationStopTargetKind::Journey);
    assert!(
        store
            .conversation_journey(&project, conversation, consent.task_id, consent.id)
            .unwrap()
            .unwrap()
            .revoked
    );
    assert!(
        store
            .reserve_conversation_call(
                &project,
                consent.task_id,
                Uuid::new_v4(),
                &grant.scope_hash,
                &"c".repeat(64)
            )
            .is_err()
    );
    let sample = SampleOperation {
        id: consent.sample_operation_id.to_string(),
        project_id: "TEST".into(),
        draft_id: "draft".into(),
        authorization_fingerprint: "a".repeat(64),
        request: json!({"conversation":{"conversation_id":conversation,"task_id":consent.task_id}}),
        status: "queued".into(),
        error: None,
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    };
    assert!(store.reserve_sample_operation(&sample).is_err());
}

#[test]
fn frozen_call_selection_does_not_disappear_when_a_journey_attaches_later() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, mut consent, _) = crate::conversation_journey::tests::setup(&store);
    let task = consent.task_id;
    let grant = grant(&store, &project, task);
    store
        .reserve_conversation_call(&project, task, grant.id, &grant.scope_hash, &"b".repeat(64))
        .unwrap();
    store
        .reserve_conversation_builder(&project, task, Uuid::new_v4(), &"a".repeat(64))
        .unwrap();
    let input = command(Some(task));
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &input)
        .unwrap();
    let target = saved
        .targets
        .iter()
        .find(|v| v.kind == ConversationStopTargetKind::Call)
        .unwrap()
        .reference();
    // This is an isolated persisted relationship fixture; normal Application
    // initial-consent validation is covered by its existing Journey tests.
    consent.schema_proposal = Some(crate::ConversationSchemaAuthorization {
        call_id: grant.id,
        model_id: consent.builder_model_id.unwrap(),
        scope_hash: grant.scope_hash.clone(),
        expires_at: grant.expires_at,
        allow_unknown_cost: true,
    });
    store.with_connection(|db|{db.execute("INSERT INTO conversation_journey_consents(id,task_id,builder_operation_id,sample_operation_id,input_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![consent.id.to_string(),task.to_string(),consent.builder_operation_id.to_string(),consent.sample_operation_id.to_string(),serde_json::to_string(&consent)?,Utc::now().to_rfc3339()])?;Ok(())}).unwrap();
    let selected = store
        .select_conversation_stop(&project, "TEST", conversation, input.id, &target)
        .unwrap();
    assert_eq!(selected.status, ConversationStopStatus::CancelRequested);
    assert!(
        !store
            .conversation_journey(&project, conversation, task, consent.id)
            .unwrap()
            .unwrap()
            .revoked
    );
    assert_eq!(
        store
            .conversation_call_cancellations(&project, task)
            .unwrap()
            .iter()
            .map(|v| v.call_id)
            .collect::<Vec<_>>(),
        vec![grant.id]
    );
}

#[test]
fn target_discovery_is_not_limited_by_history_and_conversation_scope_is_owned() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let active = Uuid::new_v4();
    store
        .reserve_conversation_builder(&project, task, active, &"a".repeat(64))
        .unwrap();
    for _ in 0..35 {
        let id = Uuid::new_v4();
        store
            .reserve_conversation_builder(&project, task, id, &"a".repeat(64))
            .unwrap();
        store
            .settle_conversation_builder(&project, task, id, true, &json!({}))
            .unwrap();
    }
    assert!(
        !store
            .conversation_builder_history(&project, task)
            .unwrap()
            .iter()
            .any(|v| v.id == active)
    );
    let (foreign, _, foreign_task) = setup(&store);
    store
        .reserve_conversation_builder(&foreign, foreign_task, Uuid::new_v4(), &"a".repeat(64))
        .unwrap();
    let stopped = store
        .begin_conversation_stop(&project, "TEST", conversation, &command(None))
        .unwrap();
    assert_eq!(stopped.targets.len(), 1);
    assert_eq!(stopped.targets[0].id, active.to_string());
    assert!(
        store
            .conversation_call_cancellations(&foreign, foreign_task)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn prebatch_processing_stop_is_durable_and_does_not_change_receipt_or_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST-processing-stop.db");
    let store = SqliteStore::open(&path).unwrap();
    let (project, conversation, task) = setup(&store);
    let id = Uuid::new_v4().to_string();
    let state = json!({"id":id,"phase":"published_start_failed","authorization":{"conversation":{"conversation_id":conversation,"task_id":task}}});
    store
        .reserve_processing_operation(&id, "TEST", &json!({"id":id}), &state)
        .unwrap();
    let budget = store.conversation_task_budget(&project, task).unwrap();
    let stop = store
        .begin_conversation_stop(&project, "TEST", conversation, &command(Some(task)))
        .unwrap();
    assert_eq!(stop.targets.len(), 1);
    assert_eq!(stop.targets[0].kind, ConversationStopTargetKind::Processing);
    assert!(store.processing_stop_requested(&id).unwrap());
    assert_eq!(store.processing_operation(&id).unwrap(), Some(state));
    assert_eq!(
        store.conversation_task_budget(&project, task).unwrap(),
        budget
    );
    assert!(
        SqliteStore::open(&path)
            .unwrap()
            .processing_stop_requested(&id)
            .unwrap()
    );
    let batch = batch(id.parse().unwrap());
    assert!(
        store
            .create_batch(
                batch.clone(),
                &[(annotagent_core::ImageId::new(), "TEST.png".into())]
            )
            .is_err()
    );
    assert!(store.get_batch(batch.id).is_err());
    assert!(store.reserve_batch_model_call(batch.id).is_err());
    assert_eq!(
        store.conversation_task_budget(&project, task).unwrap(),
        budget
    );
}

fn batch(id: Uuid) -> annotagent_core::BatchRecord {
    let now = Utc::now();
    annotagent_core::BatchRecord {
        id: annotagent_core::BatchId(id),
        project_id: "TEST".into(),
        project_path: "TEST/project.yaml".into(),
        provider: "TEST".into(),
        status: annotagent_core::BatchStatus::Pending,
        max_concurrency: 1,
        workflow_version: "TEST@1".into(),
        workflow_snapshot: json!({"guided_processing":true}),
        project_snapshot: json!({}),
        budget_limits: annotagent_core::BatchBudgetLimits {
            max_request_count: Some(4),
            ..Default::default()
        },
        budget_ledger: annotagent_core::BatchBudgetLedger::default(),
        lease_owner: None,
        lease_expires_at: None,
        event_sequence: 0,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn failed_prebatch_confirmation_remains_an_exact_stoppable_authorization() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let id = Uuid::new_v4();
    let state = json!({"id":id,"phase":"failed","authorization":{"conversation":{"conversation_id":conversation,"task_id":task}}});
    store
        .reserve_processing_operation(&id.to_string(), "TEST", &json!({"id":id}), &state)
        .unwrap();
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &command(Some(task)))
        .unwrap();
    assert_eq!(saved.status, ConversationStopStatus::CancelRequested);
    assert_eq!(saved.targets[0].state, "failed");
    assert!(store.processing_stop_requested(&id.to_string()).unwrap());
    assert!(
        store
            .create_batch(
                batch(id),
                &[(annotagent_core::ImageId::new(), "TEST.png".into())]
            )
            .is_err()
    );
    assert_eq!(
        store.processing_operation(&id.to_string()).unwrap(),
        Some(state)
    );
}

#[test]
fn operation_call_observation_follows_the_original_grant_after_advance() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let first = grant(&store, &project, task);
    let call = Uuid::new_v4();
    store
        .reserve_conversation_call(&project, task, call, &first.scope_hash, &"b".repeat(64))
        .unwrap();
    assert_eq!(
        store
            .conversation_operation_call_state(&project, conversation, task, &first.id.to_string())
            .unwrap(),
        ConversationOperationCallState {
            reserved: 1,
            in_doubt: 0
        }
    );
    store
        .finish_conversation_call(
            &project,
            task,
            call,
            ConversationCallStatus::InDoubt,
            json!({"error":"TEST unknown remote outcome"}),
        )
        .unwrap();
    let next = ConversationCallGrant {
        id: Uuid::new_v4(),
        task_id: task,
        scope_hash: "c".repeat(64),
        maximum_calls: 9,
        expires_at: first.expires_at,
    };
    store
        .advance_conversation_authorization(&project, first.id, &next)
        .unwrap();
    store
        .reserve_conversation_call(
            &project,
            task,
            Uuid::new_v4(),
            &next.scope_hash,
            &"d".repeat(64),
        )
        .unwrap();
    assert_eq!(
        store
            .conversation_operation_call_state(&project, conversation, task, &first.id.to_string())
            .unwrap(),
        ConversationOperationCallState {
            reserved: 0,
            in_doubt: 1
        }
    );
    assert_eq!(
        store
            .conversation_operation_call_state(&project, conversation, task, &next.id.to_string())
            .unwrap(),
        ConversationOperationCallState {
            reserved: 1,
            in_doubt: 0
        }
    );
    let (foreign, other, other_task) = setup(&store);
    assert!(
        store
            .conversation_operation_call_state(&foreign, other, other_task, &first.id.to_string())
            .is_err()
    );
    assert!(
        store
            .conversation_operation_call_state(&project, other, task, &first.id.to_string())
            .is_err()
    );
    assert_eq!(
        store
            .conversation_operation_call_state(
                &project,
                conversation,
                task,
                &Uuid::new_v4().to_string()
            )
            .unwrap(),
        ConversationOperationCallState::default()
    );
}

fn publication_fixture(
    store: &SqliteStore,
) -> (
    String,
    Uuid,
    Uuid,
    String,
    annotagent_core::WorkflowDraft,
    annotagent_core::WorkflowSnapshot,
) {
    let (owner, conversation, task) = setup(store);
    let now = Utc::now();
    let draft:annotagent_core::WorkflowDraft=serde_json::from_value(json!({
        "schema_version":2,"id":Uuid::new_v4().to_string(),"project_id":"TEST","name":"TEST publication fence","status":"editing","revision":1,"content_hash":"","nodes":[],"edges":[],"enabled_skills":{},"resource_versions":{},"runtime_policies":{},"allow_unvalidated_commit":true,"geometry_risk_acceptance":null,"annotation_schema":null,"label_pipeline":null,"created_at":now,"updated_at":now
    })).unwrap();
    store.save_workflow_draft(&draft).unwrap();
    let draft = store.get_workflow_draft(&draft.id).unwrap();
    let snapshot = annotagent_core::WorkflowSnapshot {
        schema_version: 2,
        draft: Some(draft.clone()),
        ..Default::default()
    };
    let sample = Uuid::new_v4().to_string();
    // Boundary fixture: production Application also validates the actual sample report.
    store.with_connection(|db|{db.execute("INSERT INTO workflow_sample_tests(id,draft_id,project_id,draft_revision,request_revision,draft_content_hash,image_set_hash,model_snapshot_hash,status,input_json,model_bindings_json,report_json,started_at,completed_at) VALUES(?1,?2,'TEST',?3,?3,?4,'TEST-images','TEST-models','passed','[]','{}','{}',?5,?5)",params![sample,draft.id,i64::try_from(draft.revision).unwrap(),draft.content_hash,now.to_rfc3339()])?;Ok(())}).unwrap();
    store
        .save_sample_scope_seal(
            &sample,
            &json!({"project_schema_hash":"a".repeat(64),"models":[]}),
        )
        .unwrap();
    let id = Uuid::new_v4().to_string();
    let request = json!({"request_id":id,"expected_revision":draft.revision,"selection":{"draft_id":draft.id,"sample_test_id":sample},"authorization_fingerprint":"b".repeat(64)});
    let authorization = json!({"project_id":"TEST","draft_id":draft.id,"sample_test_id":sample,"revision":draft.revision,"draft_content_hash":draft.content_hash,"project_schema_hash":"a".repeat(64),"models":[],"authorization_fingerprint":"b".repeat(64),"conversation":{"conversation_id":conversation,"task_id":task}});
    let state = json!({"id":id,"project_id":"TEST","draft_id":draft.id,"sample_test_id":sample,"phase":"publishing","authorization":authorization,"request":request});
    store
        .reserve_processing_operation(&id, "TEST", &request, &state)
        .unwrap();
    (owner, conversation, task, id, draft, snapshot)
}

#[test]
fn processing_publication_fence_is_checked_inside_the_publication_write() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (owner, conversation, task, id, draft, snapshot) = publication_fixture(&store);
    assert!(
        !store.processing_stop_requested(&id).unwrap(),
        "HTTP precheck passed before the race"
    );
    let saved = store
        .begin_conversation_stop(&owner, "TEST", conversation, &command(Some(task)))
        .unwrap();
    assert_eq!(saved.status, ConversationStopStatus::CancelRequested);
    let before = store.get_workflow_draft(&draft.id).unwrap();
    let hash = annotagent_image_tools::sha256(&snapshot.content_hash_material().unwrap());
    assert!(
        store
            .publish_workflow_draft_for_processing(&draft, hash, snapshot, &id)
            .is_err()
    );
    assert_eq!(store.get_workflow_draft(&draft.id).unwrap(), before);
    assert!(
        store
            .list_published_workflow_versions(Some("TEST"))
            .unwrap()
            .is_empty()
    );
    assert!(
        !store
            .with_connection(|db| Ok(db.query_row(
                "SELECT EXISTS(SELECT 1 FROM project_workflow_defaults WHERE project_id='TEST')",
                [],
                |r| r.get::<_, bool>(0)
            )?))
            .unwrap()
    );
}

#[test]
fn processing_publication_requires_exact_saved_scope_and_preserves_earlier_versions() {
    for changed in ["none", "project", "revision", "sample", "models", "seal"] {
        let store = SqliteStore::open_in_memory().unwrap();
        let (owner, conversation, task, id, draft, snapshot) = publication_fixture(&store);
        let mut state = store.processing_operation(&id).unwrap().unwrap();
        match changed {
            "project" => state["authorization"]["project_id"] = json!("foreign"),
            "revision" => state["authorization"]["revision"] = json!(draft.revision + 1),
            "sample" => state["authorization"]["sample_test_id"] = json!(Uuid::new_v4()),
            "models" => state["authorization"]["models"] = json!([{"injected":true}]),
            "seal" => {
                store
                    .with_connection(|db| {
                        db.execute(
                            "UPDATE sample_scope_seals SET scope_json=?2 WHERE sample_test_id=?1",
                            params![
                                state["sample_test_id"].as_str().unwrap(),
                                json!({"project_schema_hash":"changed","models":[]}).to_string()
                            ],
                        )?;
                        Ok(())
                    })
                    .unwrap();
            }
            _ => {}
        }
        store.update_processing_operation(&id, &state).unwrap();
        let hash = annotagent_image_tools::sha256(&snapshot.content_hash_material().unwrap());
        let result = store.publish_workflow_draft_for_processing(&draft, hash, snapshot, &id);
        if changed == "none" {
            let published = result.unwrap();
            assert_eq!(published.source_draft_id, draft.id);
            store
                .begin_conversation_stop(&owner, "TEST", conversation, &command(Some(task)))
                .unwrap();
            assert_eq!(
                store
                    .list_published_workflow_versions(Some("TEST"))
                    .unwrap(),
                vec![published]
            );
            assert_eq!(
                store.get_workflow_draft(&draft.id).unwrap().status,
                annotagent_core::WorkflowDraftStatus::Published
            );
        } else {
            assert!(result.is_err(), "changed {changed} authorization must fail");
            assert_eq!(store.get_workflow_draft(&draft.id).unwrap(), draft);
            assert!(
                store
                    .list_published_workflow_versions(Some("TEST"))
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn processing_publication_sql_failure_rolls_back_version_draft_and_default() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (_, _, _, id, draft, snapshot) = publication_fixture(&store);
    store.with_connection(|db|{db.execute_batch("CREATE TRIGGER TEST_fail_publication BEFORE INSERT ON project_workflow_defaults BEGIN SELECT RAISE(ABORT,'TEST rollback publication'); END;")?;Ok(())}).unwrap();
    let hash = annotagent_image_tools::sha256(&snapshot.content_hash_material().unwrap());
    assert!(
        store
            .publish_workflow_draft_for_processing(&draft, hash, snapshot, &id)
            .is_err()
    );
    assert_eq!(store.get_workflow_draft(&draft.id).unwrap(), draft);
    assert!(
        store
            .list_published_workflow_versions(Some("TEST"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn allocated_batch_is_found_without_late_batch_id_receipt_and_cannot_spend_after_stop() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, task) = setup(&store);
    let id = Uuid::new_v4();
    let state = json!({"id":id,"phase":"published","authorization":{"conversation":{"conversation_id":conversation,"task_id":task}}});
    store
        .reserve_processing_operation(&id.to_string(), "TEST", &json!({"id":id}), &state)
        .unwrap();
    let batch = store
        .create_batch(
            batch(id),
            &[(annotagent_core::ImageId::new(), "TEST.png".into())],
        )
        .unwrap();
    store.reserve_batch_model_call(batch.id).unwrap();
    let budget = store.conversation_task_budget(&project, task).unwrap();
    let input = command(Some(task));
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &input)
        .unwrap();
    assert_eq!(saved.selected_target.as_ref().unwrap().id, id.to_string());
    assert_eq!(saved.status, ConversationStopStatus::CancelRequested);
    let stopped = store.get_batch(batch.id).unwrap();
    assert_eq!(stopped.status, annotagent_core::BatchStatus::Cancelled);
    assert!(store.reserve_batch_model_call(batch.id).is_err());
    assert_eq!(
        store.conversation_task_budget(&project, task).unwrap(),
        budget
    );
    assert_eq!(
        store
            .begin_conversation_stop(&project, "TEST", conversation, &input)
            .unwrap(),
        saved
    );
    assert_eq!(store.get_batch(batch.id).unwrap(), stopped);
    let (foreign, foreign_conversation, foreign_task) = setup(&store);
    assert!(
        !store
            .conversation_stop_target_requested(
                &foreign,
                foreign_conversation,
                foreign_task,
                ConversationStopTargetKind::Processing,
                &id.to_string()
            )
            .unwrap()
    );
}

#[test]
fn shared_schema_call_is_a_separate_choice_and_one_journey_stop_preserves_sibling() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, mut one, _) = crate::conversation_journey::tests::setup(&store);
    let task = one.task_id;
    let grant = grant(&store, &project, task);
    store
        .reserve_conversation_call(&project, task, grant.id, &grant.scope_hash, &"b".repeat(64))
        .unwrap();
    one.schema_proposal = Some(crate::ConversationSchemaAuthorization {
        call_id: grant.id,
        model_id: one.builder_model_id.unwrap(),
        scope_hash: grant.scope_hash.clone(),
        expires_at: grant.expires_at,
        allow_unknown_cost: true,
    });
    let mut two = one.clone();
    two.id = Uuid::new_v4();
    two.builder_operation_id = Uuid::new_v4();
    two.sample_operation_id = Uuid::new_v4();
    for consent in [&one, &two] {
        store.with_connection(|db|{db.execute("INSERT INTO conversation_journey_consents(id,task_id,builder_operation_id,sample_operation_id,input_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![consent.id.to_string(),task.to_string(),consent.builder_operation_id.to_string(),consent.sample_operation_id.to_string(),serde_json::to_string(consent)?,Utc::now().to_rfc3339()])?;Ok(())}).unwrap();
    }
    let input = command(Some(task));
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &input)
        .unwrap();
    assert_eq!(saved.targets.len(), 3);
    let call = saved
        .targets
        .iter()
        .find(|t| t.kind == ConversationStopTargetKind::Call)
        .unwrap();
    assert_eq!(call.parent_journey_ids.len(), 2);
    let target = saved
        .targets
        .iter()
        .find(|t| t.id == one.id.to_string())
        .unwrap()
        .reference();
    store
        .select_conversation_stop(&project, "TEST", conversation, input.id, &target)
        .unwrap();
    assert!(
        store
            .conversation_journey(&project, conversation, task, one.id)
            .unwrap()
            .unwrap()
            .revoked
    );
    assert!(
        !store
            .conversation_journey(&project, conversation, task, two.id)
            .unwrap()
            .unwrap()
            .revoked
    );
    assert!(
        !store
            .conversation_call_cancellations(&project, task)
            .unwrap()
            .iter()
            .any(|v| v.call_id == grant.id)
    );
}

#[test]
fn conversation_scope_discovers_all_owned_tasks_without_choosing_latest() {
    let store = SqliteStore::open_in_memory().unwrap();
    let (project, conversation, first) = setup(&store);
    grant(&store, &project, first);
    let message = ConversationMessageInput {
        id: Uuid::new_v4(),
        text: "TEST second goal".into(),
        image: None,
        reference: None,
    };
    store
        .append_conversation_message(&project, conversation, &message)
        .unwrap();
    let second = Uuid::new_v4();
    store
        .begin_conversation_task(
            &project,
            conversation,
            &BeginConversationTask {
                id: second,
                source_message_id: message.id,
                schema_revision: "a".repeat(64),
            },
        )
        .unwrap();
    grant(&store, &project, second);
    let saved = store
        .begin_conversation_stop(&project, "TEST", conversation, &command(None))
        .unwrap();
    assert_eq!(saved.targets.len(), 2);
    assert_eq!(saved.status, ConversationStopStatus::NeedsSelection);
    assert!(saved.selected_target.is_none());
    assert!(
        store
            .conversation_call_cancellations(&project, first)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .conversation_call_cancellations(&project, second)
            .unwrap()
            .is_empty()
    );
    let target = saved
        .targets
        .iter()
        .find(|t| t.task_id == first)
        .unwrap()
        .reference();
    store
        .select_conversation_stop(
            &project,
            "TEST",
            conversation,
            saved.message.input.id,
            &target,
        )
        .unwrap();
    assert_eq!(
        store
            .conversation_call_cancellations(&project, first)
            .unwrap()
            .len(),
        1
    );
    assert!(
        store
            .conversation_call_cancellations(&project, second)
            .unwrap()
            .is_empty()
    );
}
