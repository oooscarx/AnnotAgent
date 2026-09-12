//! Thin read-only UI projections. All commands use their existing routes.
use super::*;
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct Page {
    cursor: Option<String>,
    limit: Option<u32>,
}
pub(super) async fn navigation(
    State(state): State<ServerState>,
    Query(page): Query<Page>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .agent_ui_navigation(page.cursor.as_deref(), page.limit.unwrap_or(50) as usize)
        .map(Json)
        .map_err(ApiError::conversation)
}
pub(super) async fn tasks(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
    Query(page): Query<Page>,
) -> ApiResult<Json<Value>> {
    let after = page
        .cursor
        .as_deref()
        .unwrap_or("0")
        .parse()
        .map_err(ApiError::bad_request)?;
    state
        .application
        .agent_ui_tasks(&project, conversation, after, page.limit.unwrap_or(50))
        .map(Json)
        .map_err(ApiError::conversation)
}
pub(super) async fn thread(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(page): Query<Page>,
) -> ApiResult<Json<Value>> {
    let after = page
        .cursor
        .as_deref()
        .unwrap_or("0")
        .parse()
        .map_err(ApiError::bad_request)?;
    state
        .application
        .agent_ui_thread(
            &project,
            conversation,
            task,
            after,
            page.limit.unwrap_or(50),
        )
        .map(Json)
        .map_err(ApiError::conversation)
}
pub(super) async fn snapshot(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    let mut value = state
        .application
        .agent_ui_snapshot(&project, conversation, task)
        .map_err(ApiError::conversation)?;
    value["mainline"]["capability_readiness"] =
        super::mainline_capability::snapshot(&state, &project, conversation, task)?;
    Ok(Json(value))
}

pub(super) async fn advance(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<annotagent_application::AdvanceTaskInput>,
) -> ApiResult<Json<annotagent_application::AdvanceTaskReceipt>> {
    match state
        .application
        .advance_mainline_task(&project, conversation, task, &input)
    {
        Ok(receipt) => Ok(Json(receipt)),
        Err(error)
            if error.to_string().contains("Task changed; reload")
                || error.to_string().contains("older delivery scope") =>
        {
            Err(ApiError {
                status: StatusCode::CONFLICT,
                body: json!({
                    "status":409,"code":"task_revision_conflict","error":error.to_string(),
                    "suggested_action":"reload_task_workspace"
                }),
            })
        }
        Err(error) => Err(ApiError::conversation(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{request, response_json, test_state};
    use annotagent_provider::InMemorySecretStore;
    use axum::http::Method;
    #[test]
    fn contract_examples_decode_with_current_http_dtos() {
        let examples: Value = serde_json::from_str(include_str!(
            "../../../docs/contracts/agent-ui-v1/EXAMPLES.json"
        ))
        .unwrap();
        serde_json::from_value::<annotagent_storage::ConversationSendInput>(
            examples["send"].clone(),
        )
        .unwrap();
        serde_json::from_value::<annotagent_storage::SelectConversationAgentModel>(
            examples["select_agent_model"].clone(),
        )
        .unwrap();
        serde_json::from_value::<annotagent_storage::ConversationMessageInput>(
            examples["stop"].clone(),
        )
        .unwrap();
        serde_json::from_value::<crate::conversation_stop::SelectStop>(
            examples["select_stop"].clone(),
        )
        .unwrap();
        serde_json::from_value::<crate::agent_ui_settings::BudgetPatch>(
            examples["budget_patch"].clone(),
        )
        .unwrap();
        serde_json::from_value::<annotagent_storage::ProjectCallLimitInput>(
            examples["call_limit"].clone(),
        )
        .unwrap();
        let mainline: Value = serde_json::from_str(include_str!(
            "../../../docs/contracts/mainline-v1/EXAMPLES.json"
        ))
        .unwrap();
        let mut formal = mainline["formal_visual_selection"].clone();
        formal.as_object_mut().unwrap().remove("contract_status");
        serde_json::from_value::<annotagent_storage::ConversationSelectionRef>(formal).unwrap();
        serde_json::from_value::<annotagent_storage::ConversationSchemaAuthorization>(
            examples["schema_consent"].clone(),
        )
        .unwrap();
        let schema: Value = serde_json::from_str(include_str!(
            "../../../docs/contracts/agent-ui-v1/SCHEMAS.json"
        ))
        .unwrap();
        assert!(
            schema["$defs"]["SendReceipt"]["properties"]["resolved_agent_model_id"].is_object()
        );
    }

    #[tokio::test]
    async fn task_read_model_advances_only_current_authorized_local_schema_and_replays() {
        let temp = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        app.create_project(
            "TEST-mainline-task",
            "version: 1\nproject:\n  name: TEST mainline task\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: objects\n    kind: bounding_box\n    labels: [target]\n    required: true\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
        )
        .unwrap();
        let incoming = temp.path().join("TEST-mainline-input");
        std::fs::create_dir(&incoming).unwrap();
        image::RgbImage::from_pixel(32, 24, image::Rgb([3, 4, 5]))
            .save(incoming.join("one.png"))
            .unwrap();
        app.import_images("TEST-mainline-task", &incoming).unwrap();
        let image = app
            .list_project_image_summaries("TEST-mainline-task")
            .unwrap()[0]
            .image_id;
        let conversation = app
            .create_project_conversation("TEST-mainline-task")
            .unwrap();
        let sent = app
            .send_project_conversation_message(
                "TEST-mainline-task",
                conversation,
                &serde_json::from_value(json!({
                    "message":{"id":uuid::Uuid::new_v4(),"text":"TEST package","image":null},
                    "task_id":null,
                    "schema_revision":app.project_goal("TEST-mainline-task").unwrap()["revision"],
                    "mode":"plan"
                }))
                .unwrap(),
            )
            .unwrap();
        app.save_task_delivery_intent(
            "TEST-mainline-task",
            conversation,
            sent.task_id,
            serde_json::from_value(json!({
                "command_id":uuid::Uuid::new_v4(),"expected_revision":0,"image_ids":[image],
                "label_spec":[{"stable_id":"target","display_name":"Target","aliases":[],"include":"","exclude":""}],
                "training_target":{"annotation_kind":"bounding_box","framework":"ultralytics","export_profile":"ultralytics_yolo_detection","profile_revision":1},
                "split_policy":{"train_percent":80,"seed":5,"preserve_existing":true,"keep_known_groups_together":true}
            }))
            .unwrap(),
        )
        .unwrap();
        let service = router(
            test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let root = format!(
            "/api/projects/TEST-mainline-task/conversations/{conversation}/tasks/{}",
            sent.task_id
        );
        let before =
            response_json(request(&service, Method::GET, &format!("{root}/workspace"), None).await)
                .await;
        assert_eq!(
            before["mainline"]["available_actions"][0]["id"],
            "prepare_delivery_schema"
        );
        assert_eq!(
            before["mainline"]["available_actions"][0]["state"],
            "authorized"
        );
        assert_eq!(
            before["mainline"]["revision"],
            before["read_model_revision"]
        );
        assert_eq!(before["mainline"]["intake"]["missing_slots"], json!([]));
        assert_eq!(
            before["mainline"]["actions"][0]["scope_revision"],
            before["read_model_revision"]
        );
        assert_eq!(before["mainline"]["actions"][0]["available"], true);
        assert_eq!(
            before["mainline"]["links"]["visual_selections"],
            format!("{root}/visual-selections")
        );
        assert_eq!(before["mainline"]["messages"], json!([]));
        assert_eq!(before["mainline"]["completion"]["status"], "incomplete");
        assert_eq!(before["mainline"]["completion"]["task_completed"], false);
        let command = uuid::Uuid::new_v4();
        let body = json!({
            "command_id":command,
            "expected_read_model_revision":before["read_model_revision"],
            "action_id":"prepare_delivery_schema"
        });
        let first = response_json(
            request(
                &service,
                Method::POST,
                &format!("{root}/advance"),
                Some(body.clone()),
            )
            .await,
        )
        .await;
        assert_eq!(first["replayed"], false);
        assert_eq!(first["result"]["source_request_id"], command.to_string());
        assert_ne!(
            first["workspace"]["read_model_revision"],
            before["read_model_revision"]
        );
        let replay = response_json(
            request(
                &service,
                Method::POST,
                &format!("{root}/advance"),
                Some(body),
            )
            .await,
        )
        .await;
        assert_eq!(replay["replayed"], true);
        assert_eq!(replay["result"]["id"], first["result"]["id"]);
        let stale = request(
            &service,
            Method::POST,
            &format!("{root}/advance"),
            Some(json!({
                "command_id":uuid::Uuid::new_v4(),
                "expected_read_model_revision":before["read_model_revision"],
                "action_id":"prepare_delivery_schema"
            })),
        )
        .await;
        assert_eq!(stale.status(), axum::http::StatusCode::CONFLICT);
        assert_eq!(
            app.human_conversation_schema_drafts("TEST-mainline-task", conversation, sent.task_id,)
                .unwrap()
                .len(),
            1
        );
    }
    #[tokio::test]
    async fn stop_http_trace_keeps_unknown_receipt_and_spent_budget_on_retry() {
        for (terminal, normalized) in [
            (
                annotagent_storage::ConversationCallStatus::InDoubt,
                "outcome_unknown",
            ),
            (
                annotagent_storage::ConversationCallStatus::Failed,
                "interrupted",
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let app = Arc::new(LocalApplication::new(dir.path()).unwrap());
            app.create_project(
                "TEST-stop",
                include_str!(
                    "../../../examples/label-pipelines/whole-image-classification/project.yaml"
                ),
            )
            .unwrap();
            let c = app.create_project_conversation("TEST-stop").unwrap();
            let sent=app.send_project_conversation_message("TEST-stop",c,&serde_json::from_value(json!({"message":{"id":uuid::Uuid::new_v4(),"text":"TEST goal","image":null},"task_id":null,"schema_revision":app.project_goal("TEST-stop").unwrap()["revision"],"mode":"plan"})).unwrap()).unwrap();
            let owner = stable_project_id(app.project_path("TEST-stop").unwrap().parent().unwrap())
                .to_string();
            let call = uuid::Uuid::new_v4();
            let grant = annotagent_storage::ConversationCallGrant {
                id: call,
                task_id: sent.task_id,
                scope_hash: "b".repeat(64),
                maximum_calls: 2,
                expires_at: Utc::now() + chrono::Duration::minutes(5),
            };
            app.store()
                .authorize_conversation_calls(&owner, &grant)
                .unwrap();
            app.store()
                .reserve_conversation_call(
                    &owner,
                    sent.task_id,
                    call,
                    &grant.scope_hash,
                    &"c".repeat(64),
                )
                .unwrap();
            let budget = app
                .store()
                .conversation_task_budget(&owner, sent.task_id)
                .unwrap();
            let service = router(
                test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await,
                None,
            );
            let root = format!("/api/projects/TEST-stop/conversations/{c}/stop-requests");
            let command = json!({"id":uuid::Uuid::new_v4(),"text":"stop","image":null,"reference":{"scope":"stop_request","task_id":sent.task_id}});
            let requested = response_json(
                request(
                    &service,
                    axum::http::Method::POST,
                    &root,
                    Some(command.clone()),
                )
                .await,
            )
            .await;
            assert_eq!(requested["normalized_state"], "stopping");
            app.store()
                .finish_conversation_call(
                    &owner,
                    sent.task_id,
                    call,
                    terminal,
                    json!({"TEST":"remote completion unknown"}),
                )
                .unwrap();
            let settled = response_json(
                request(
                    &service,
                    axum::http::Method::GET,
                    &format!("{}/{}", root, command["id"].as_str().unwrap()),
                    None,
                )
                .await,
            )
            .await;
            assert_eq!(settled["normalized_state"], normalized);
            assert_eq!(settled["resume"]["available"], false);
            let retry = response_json(
                request(&service, axum::http::Method::POST, &root, Some(command)).await,
            )
            .await;
            assert_eq!(settled, retry);
            assert_eq!(
                app.store()
                    .conversation_task_budget(&owner, sent.task_id)
                    .unwrap(),
                budget
            );
            println!(
                "AGENT_UI_TRACE {}",
                json!({"fixture":true,"test":"stop_http_trace_keeps_unknown_receipt_and_spent_budget_on_retry","requested":requested,"settled":settled,"retry":retry,"budget_before":budget,"budget_after":app.store().conversation_task_budget(&owner,sent.task_id).unwrap()})
            );
        }
    }
    #[tokio::test]
    async fn navigation_snapshot_thread_keep_real_ownership_without_execution() {
        let dir = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(dir.path()).unwrap());
        for route in ["TEST-a", "TEST-b"] {
            app.create_project(
                route,
                include_str!(
                    "../../../examples/label-pipelines/whole-image-classification/project.yaml"
                ),
            )
            .unwrap();
        }
        let c = app.create_project_conversation("TEST-a").unwrap();
        let goal = app.project_goal("TEST-a").unwrap();
        let sent=app.send_project_conversation_message("TEST-a",c,&serde_json::from_value(json!({"message":{"id":uuid::Uuid::new_v4(),"text":"TEST goal","image":null},"task_id":null,"schema_revision":goal["revision"],"mode":"plan"})).unwrap()).unwrap();
        let service = router(
            test_state(app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let nav = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/navigation?limit=1",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(nav["items"].as_array().unwrap().len(), 1);
        assert!(nav["next_cursor"].is_string());
        assert!(!nav.to_string().contains(&dir.path().display().to_string()));
        let next = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!(
                    "/api/navigation?limit=1&cursor={}",
                    nav["next_cursor"].as_str().unwrap()
                ),
                None,
            )
            .await,
        )
        .await;
        assert_ne!(
            next["items"][0]["project_owner_id"],
            nav["items"][0]["project_owner_id"]
        );
        let root = format!(
            "/api/projects/TEST-a/conversations/{c}/tasks/{}",
            sent.task_id
        );
        let thread = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("{root}/thread"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(thread["items"][0]["role"], "user");
        let snapshot = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("{root}/workspace"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(snapshot["task"]["input"]["id"], sent.task_id.to_string());
        assert!(snapshot["budget"].is_null());
        assert_eq!(
            request(
                &service,
                axum::http::Method::GET,
                &format!("{}/workspace", root.replace("TEST-a", "TEST-b")),
                None
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
}
