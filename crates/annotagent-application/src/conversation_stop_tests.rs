use super::*;
use annotagent_storage::{BeginConversationTask, ConversationCallGrant, ConversationSelectionRef};
use chrono::{Duration, Utc};
use tokio_util::sync::CancellationToken;

fn app() -> (tempfile::TempDir, LocalApplication, Uuid) {
    let temp = tempfile::tempdir().unwrap();
    let app = LocalApplication::new(temp.path()).unwrap();
    let yaml = "version: 1\nproject:\n  name: TEST stop\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n";
    app.create_project("TEST-stop", yaml).unwrap();
    app.create_project("TEST-foreign", yaml).unwrap();
    let conversation = app.create_project_conversation("TEST-stop").unwrap();
    (temp, app, conversation)
}
fn stop(task_id: Option<Uuid>) -> ConversationMessageInput {
    ConversationMessageInput {
        id: Uuid::new_v4(),
        text: "停止".into(),
        image: None,
        reference: Some(ConversationSelectionRef::StopRequest { task_id }),
    }
}

#[test]
fn stop_uses_stable_owner_and_cannot_be_reinterpreted_as_a_goal() {
    let (temp, app, conversation) = app();
    let input = stop(None);
    assert!(
        app.append_project_conversation_message("TEST-stop", conversation, &input)
            .is_err()
    );
    assert!(
        app.begin_conversation_stop("TEST-foreign", conversation, &input)
            .is_err()
    );
    let saved = app
        .begin_conversation_stop("TEST-stop", conversation, &input)
        .unwrap();
    assert_eq!(saved.status, ConversationStopStatus::NoActiveWork);
    assert!(
        app.conversation_stop_request("TEST-foreign", conversation, input.id)
            .is_err()
    );
    assert!(
        app.begin_conversation_task(
            "TEST-stop",
            conversation,
            &BeginConversationTask {
                id: Uuid::new_v4(),
                source_message_id: input.id,
                schema_revision: app.project_goal("TEST-stop").unwrap()["revision"]
                    .as_str()
                    .unwrap()
                    .into(),
            }
        )
        .is_err()
    );
    assert!(
        app.conversation_tasks("TEST-stop", conversation)
            .unwrap()
            .is_empty()
    );
    drop(app);
    let app = LocalApplication::new(temp.path()).unwrap();
    assert_eq!(
        app.begin_conversation_stop("TEST-stop", conversation, &input)
            .unwrap(),
        saved
    );
    assert!(
        app.conversation_stop_observation("TEST-stop", conversation, input.id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn reading_stop_does_not_signal_and_receipt_does_not_claim_remote_success() {
    let (_temp, app, conversation) = app();
    let goal = ConversationMessageInput {
        id: Uuid::new_v4(),
        text: "TEST classify cups".into(),
        image: None,
        reference: None,
    };
    app.append_project_conversation_message("TEST-stop", conversation, &goal)
        .unwrap();
    let task = Uuid::new_v4();
    app.begin_conversation_task(
        "TEST-stop",
        conversation,
        &BeginConversationTask {
            id: task,
            source_message_id: goal.id,
            schema_revision: app.project_goal("TEST-stop").unwrap()["revision"]
                .as_str()
                .unwrap()
                .into(),
        },
    )
    .unwrap();
    let owner = app.conversation_project_identity("TEST-stop").unwrap();
    let call = Uuid::new_v4();
    app.store
        .authorize_conversation_calls(
            &owner,
            &ConversationCallGrant {
                id: call,
                task_id: task,
                scope_hash: "b".repeat(64),
                maximum_calls: 2,
                expires_at: Utc::now() + Duration::minutes(1),
            },
        )
        .unwrap();
    app.store
        .reserve_conversation_call(&owner, task, call, &"b".repeat(64), &"c".repeat(64))
        .unwrap();
    let before = app.store.conversation_task_budget(&owner, task).unwrap();
    let token = CancellationToken::new();
    app.conversation_cancellations
        .lock()
        .unwrap()
        .insert(call, token.clone());
    let input = stop(Some(task));
    let saved = app
        .begin_conversation_stop("TEST-stop", conversation, &input)
        .unwrap();
    assert_eq!(saved.selected_target.as_ref().unwrap().id, call.to_string());
    assert_eq!(
        app.conversation_stop_observation("TEST-stop", conversation, input.id)
            .unwrap()
            .unwrap()
            .state,
        "cancel_pending"
    );
    assert!(!token.is_cancelled());
    assert!(
        app.signal_conversation_stop("TEST-stop", conversation, input.id)
            .unwrap()
            .is_empty()
    );
    assert!(token.is_cancelled());
    app.store
        .finish_conversation_call(
            &owner,
            task,
            call,
            ConversationCallStatus::InDoubt,
            serde_json::json!({"error":"TEST remote completion unknown"}),
        )
        .unwrap();
    assert_eq!(
        app.conversation_stop_observation("TEST-stop", conversation, input.id)
            .unwrap()
            .unwrap()
            .state,
        "unknown"
    );
    assert_eq!(
        app.store.conversation_task_budget(&owner, task).unwrap(),
        before
    );
    assert_eq!(
        app.begin_conversation_stop("TEST-stop", conversation, &input)
            .unwrap(),
        saved
    );
}
