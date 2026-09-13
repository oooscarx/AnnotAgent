//! Thin read-only UI projections. All commands use their existing routes.
use super::*;
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct Page {
    cursor: Option<String>,
    limit: Option<u32>,
    #[serde(default)]
    state: annotagent_storage::ConversationTaskLifecycleFilter,
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
        .agent_ui_tasks_filtered(
            &project,
            conversation,
            after,
            page.limit.unwrap_or(50),
            page.state,
        )
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
    let capability = super::mainline_capability::snapshot(&state, &project, conversation, task)?;
    let preset_import = value["mainline"]["formal_source"]["kind"]
        .as_str()
        .is_some_and(|kind| kind == "preset_candidate_import");
    let diagnostics = if preset_import {
        Vec::new()
    } else {
        super::mainline_capability::capability_result_diagnostics(&capability)
    };
    value["mainline"]["capability_readiness"] = capability;
    value["mainline"]["result_diagnostics"]
        .as_array_mut()
        .expect("Application Mainline diagnostics are an array")
        .extend(diagnostics);
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

    #[tokio::test]
    async fn upload_scoped_send_exposes_one_passive_journey_approval_for_three_of_six_images() {
        let temp = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        let project = "TEST-one-approval";
        app.create_project(project,"version: 1\nproject:\n  name: TEST one approval\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let incoming = temp.path().join("TEST-one-approval-images");
        std::fs::create_dir(&incoming).unwrap();
        for value in 1..=6 {
            image::RgbImage::from_pixel(48, 32, image::Rgb([value, value + 10, value + 20]))
                .save(incoming.join(format!("image-{value}.png")))
                .unwrap();
        }
        let imported = app.import_images_with_report(project, &incoming).unwrap();
        assert_eq!(imported.images.len(), 6);
        let conversation = app.create_project_conversation(project).unwrap();
        let now = Utc::now();
        let planner_provider_id = ProviderId::new();
        let planner_reference = CredentialReference {
            provider_id: planner_provider_id,
            source: CredentialSource::SessionOnly,
            locator: "TEST-one-approval-planner".into(),
        };
        app.store()
            .save_provider_profile(&ProviderProfile {
                id: planner_provider_id,
                display_name: "TEST loopback planner".into(),
                preset_id: None,
                adapter: ProviderAdapterKind::OpenAiCompatible,
                base_url: "http://127.0.0.1:9/v1".parse().unwrap(),
                organization: None,
                workspace: None,
                credential_ref: Some(planner_reference.clone()),
                safe_headers: BTreeMap::new(),
                connection_policy: ProviderConnectionPolicy::default(),
                enabled: true,
                health: ProviderHealthSnapshot {
                    status: ProviderHealthStatus::Available,
                    safe_message: Some("TEST loopback only".into()),
                    checked_at: Some(now),
                },
                created_at: now,
                updated_at: now,
            })
            .unwrap();
        let planner = ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id: planner_provider_id,
            display_name: "TEST one approval planner".into(),
            remote_model_id: "TEST-one-approval-planner".into(),
            input_modalities: BTreeSet::from([InputModality::Text]),
            protocol_features: ProtocolFeatures {
                tool_calls: true,
                structured_output: true,
                json_schema: true,
                usage_reporting: true,
                ..ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([ModelCapability::TextGeneration]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing::default(),
            quality_contracts: vec![],
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: true,
            created_at: now,
            updated_at: now,
        };
        app.store().save_model_profile(&planner).unwrap();
        let mut defaults = app.store().get_global_model_defaults().unwrap();
        defaults.pipeline_builder = Some(planner.id);
        app.store().save_global_model_defaults(&defaults).unwrap();
        let secrets = Arc::new(InMemorySecretStore::default());
        secrets
            .put(
                SecretScope {
                    provider_id: planner_reference.provider_id,
                    source: planner_reference.source,
                    locator: planner_reference.locator.clone(),
                },
                SecretValue::new("TEST-only-not-secret").unwrap(),
            )
            .await
            .unwrap();
        let state = test_state(app.clone(), secrets).await;
        let mock_provider = app
            .store()
            .list_provider_profiles()
            .unwrap()
            .into_iter()
            .find(|profile| profile.adapter == ProviderAdapterKind::Mock)
            .unwrap();
        let detector = app
            .store()
            .list_model_profiles(Some(mock_provider.id), false)
            .unwrap()
            .into_iter()
            .find(|model| model.remote_model_id == "mock-detector")
            .unwrap();
        let project_owner = registry_project_id(&state, project).unwrap();
        let service = router(state, None);
        let message_id = uuid::Uuid::new_v4();
        let send = json!({
            "message":{"id":message_id,"text":"请框出桌面上的杯子和瓶子，并交付 YOLO Detection 训练包","image":null},
            "task_images":imported.images.iter().map(|image|json!({"image_id":image.image_id,"sha256":image.content_hash})).collect::<Vec<_>>(),
            "task_id":null,
            "schema_revision":app.project_goal(project).unwrap()["revision"],
            "mode":"execute"
        });
        let send_url = format!("/api/projects/{project}/conversations/{conversation}/send");
        let receipt =
            response_json(request(&service, Method::POST, &send_url, Some(send.clone())).await)
                .await;
        let task = receipt["task_id"].as_str().unwrap();
        let root = format!("/api/projects/{project}/conversations/{conversation}/tasks/{task}");
        let workspace =
            response_json(request(&service, Method::GET, &format!("{root}/workspace"), None).await)
                .await;
        assert_eq!(
            workspace["mainline"]["available_actions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let action = &workspace["mainline"]["available_actions"][0];
        assert_eq!(action["id"], "build_and_test_pipeline");
        assert_eq!(action["url"], format!("{root}/journey-preview"));
        assert_eq!(action["scope"]["images"].as_array().unwrap().len(), 6);
        assert_eq!(action["scope"]["maximum_sample_images"], 3);
        assert!(
            app.conversation_journey_history(project, conversation, task.parse().unwrap())
                .unwrap()
                .is_empty()
        );

        let missing = request(&service, Method::GET, action["url"].as_str().unwrap(), None).await;
        assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
        let missing = response_json(missing).await;
        assert_eq!(missing["code"], "capability_setup_required");
        assert!(missing["setup_requests"].as_array().unwrap().iter().any(
            |request| request["role"] == "visual_inference" && request["status"] == "required"
        ));
        app.store()
            .save_project_model_binding(
                &ProjectModelBinding {
                    id: ModelBindingId::new(),
                    project_id: project_owner,
                    capability: ModelCapability::ObjectDetection,
                    role: ModelBindingRole::PrimaryInference,
                    match_kind: ModelBindingMatch::Role,
                    model_profile_id: detector.id,
                    locked: true,
                    created_at: Utc::now(),
                },
                BindingMutationActor::User,
            )
            .unwrap();
        let preview = response_json(
            request(&service, Method::GET, action["url"].as_str().unwrap(), None).await,
        )
        .await;
        assert_eq!(preview["consent"]["images"].as_array().unwrap().len(), 3);
        assert_eq!(preview["data"]["images"].as_array().unwrap().len(), 3);
        assert!(
            app.conversation_journey_history(project, conversation, task.parse().unwrap())
                .unwrap()
                .is_empty(),
            "GET preview must not persist a consent or dispatch work"
        );

        let mut consent = preview["consent"].clone();
        consent["allow_unknown_cost"] = json!(true);
        consent["schema_proposal"]["allow_unknown_cost"] = json!(true);
        let consent_url = format!("{root}/journey-consents");
        let saved = response_json(
            request(&service, Method::POST, &consent_url, Some(consent.clone())).await,
        )
        .await;
        assert_eq!(saved["consent"]["id"], consent["id"]);
        let execution_url = format!(
            "{root}/journey-consents/{}/execution",
            consent["id"].as_str().unwrap()
        );
        let first_status =
            response_json(request(&service, Method::GET, &execution_url, None).await).await;
        assert!(!first_status["dispatch"].is_null());
        let first_attempt = first_status["dispatch"]["attempt_id"].clone();

        let repeated =
            response_json(request(&service, Method::POST, &consent_url, Some(consent)).await).await;
        assert_eq!(repeated["consent"]["id"], saved["consent"]["id"]);
        let repeated_status =
            response_json(request(&service, Method::GET, &execution_url, None).await).await;
        assert_eq!(repeated_status["dispatch"]["attempt_id"], first_attempt);

        let replay =
            response_json(request(&service, Method::POST, &send_url, Some(send)).await).await;
        assert_eq!(replay, receipt);
        assert_eq!(
            app.conversation_tasks(project, conversation).unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn described_task_accepts_exact_later_upload_receipts_through_delivery_cas() {
        let temp = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        let project = "TEST-describe-before-upload";
        app.create_project(project,"version: 1\nproject:\n  name: TEST describe before upload\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let incoming = temp.path().join("TEST-later-upload");
        std::fs::create_dir(&incoming).unwrap();
        for value in 1..=3 {
            image::RgbImage::from_pixel(32, 24, image::Rgb([value, value + 1, value + 2]))
                .save(incoming.join(format!("later-{value}.png")))
                .unwrap();
        }
        let imported = app.import_images_with_report(project, &incoming).unwrap();
        let conversation = app.create_project_conversation(project).unwrap();
        let service = router(
            test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let send_url = format!("/api/projects/{project}/conversations/{conversation}/send");
        let sent = response_json(
            request(
                &service,
                Method::POST,
                &send_url,
                Some(json!({
                    "message":{"id":uuid::Uuid::new_v4(),"text":"标注杯子，用于 YOLO；图片稍后上传","image":null},
                    "task_id":null,
                    "schema_revision":app.project_goal(project).unwrap()["revision"],
                    "mode":"plan"
                })),
            )
            .await,
        )
        .await;
        let task = sent["task_id"].as_str().unwrap();
        let root = format!("/api/projects/{project}/conversations/{conversation}/tasks/{task}");
        let command = uuid::Uuid::new_v4();
        let task_images = imported
            .images
            .iter()
            .map(|image| json!({"image_id":image.image_id,"sha256":image.content_hash}))
            .collect::<Vec<_>>();
        let attach = json!({
            "command_id":command,"expected_revision":0,"image_ids":null,
            "task_images":task_images,"label_spec":null,"training_target":null,
            "split_policy":{"train_percent":80,"seed":0,"preserve_existing":true,"keep_known_groups_together":true},
            "image_metadata":{}
        });
        let delivery_url = format!("{root}/delivery-intent");
        let first = response_json(
            request(&service, Method::POST, &delivery_url, Some(attach.clone())).await,
        )
        .await;
        let replay = response_json(
            request(&service, Method::POST, &delivery_url, Some(attach.clone())).await,
        )
        .await;
        assert_eq!(replay, first);
        assert_eq!(first["saved"]["revision"], 1);
        assert_eq!(
            first["saved"]["intent"]["dataset_scope"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            first["missing_slots"],
            json!(["label_spec", "training_target"])
        );
        let workspace =
            response_json(request(&service, Method::GET, &format!("{root}/workspace"), None).await)
                .await;
        assert_eq!(
            workspace["mainline"]["available_actions"][0]["id"],
            "build_and_test_pipeline"
        );
        assert_eq!(
            workspace["mainline"]["available_actions"][0]["scope"]["images"]
                .as_array()
                .unwrap()
                .len(),
            3
        );

        let mut reused_command = attach.clone();
        reused_command["task_images"] = json!([task_images[0].clone()]);
        let reused_command =
            request(&service, Method::POST, &delivery_url, Some(reused_command)).await;
        assert_eq!(reused_command.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(reused_command).await["code"],
            "delivery_command_conflict"
        );

        let mut stale = attach.clone();
        stale["command_id"] = json!(uuid::Uuid::new_v4());
        let stale = request(&service, Method::POST, &delivery_url, Some(stale)).await;
        assert_eq!(stale.status(), StatusCode::CONFLICT);
        let stale = response_json(stale).await;
        assert_eq!(stale["code"], "delivery_revision_conflict");
        assert_eq!(stale["expected_revision"], 0);
        assert_eq!(stale["current_revision"], 1);

        let mut changed = attach.clone();
        changed["task_images"][0]["sha256"] = json!("f".repeat(64));
        let changed = request(&service, Method::POST, &delivery_url, Some(changed)).await;
        assert_eq!(changed.status(), StatusCode::BAD_REQUEST);

        let foreign = "TEST-foreign-upload";
        app.create_project(foreign,"version: 1\nproject:\n  name: TEST foreign\ndataset:\n  root: images\nruntime: {}\ntasks: []\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let foreign_source = temp.path().join("TEST-foreign-source");
        std::fs::create_dir(&foreign_source).unwrap();
        image::RgbImage::from_pixel(32, 24, image::Rgb([9, 8, 7]))
            .save(foreign_source.join("foreign.png"))
            .unwrap();
        let foreign_image = app
            .import_images_with_report(foreign, &foreign_source)
            .unwrap()
            .images
            .remove(0);
        let mut cross_project = attach;
        cross_project["command_id"] = json!(uuid::Uuid::new_v4());
        cross_project["expected_revision"] = json!(1);
        cross_project["task_images"] =
            json!([{"image_id":foreign_image.image_id,"sha256":foreign_image.content_hash}]);
        let cross_project =
            request(&service, Method::POST, &delivery_url, Some(cross_project)).await;
        assert_eq!(cross_project.status(), StatusCode::BAD_REQUEST);
    }

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
        let schemas = app
            .human_conversation_schema_drafts("TEST-mainline-task", conversation, sent.task_id)
            .unwrap();
        assert_eq!(schemas.len(), 1);

        // A completed Builder is still only the first half of the saved Journey.
        // The Task view must expose that same consent/sample identity over its
        // registered GET+POST routes instead of offering a new Builder preview.
        let now = Utc::now();
        let provider = ProviderProfile {
            id: ProviderId::new(),
            display_name: "TEST saved Journey provider".into(),
            preset_id: Some("mock".into()),
            adapter: ProviderAdapterKind::Mock,
            base_url: "http://127.0.0.1:8796/v1".parse().unwrap(),
            organization: None,
            workspace: None,
            credential_ref: None,
            safe_headers: BTreeMap::new(),
            connection_policy: ProviderConnectionPolicy::default(),
            enabled: true,
            health: ProviderHealthSnapshot {
                status: ProviderHealthStatus::Available,
                safe_message: Some("TEST only".into()),
                checked_at: Some(now),
            },
            created_at: now,
            updated_at: now,
        };
        let model = ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id: provider.id,
            display_name: "TEST saved Journey visual model".into(),
            remote_model_id: "TEST-not-called-by-read".into(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures::default(),
            task_capabilities: BTreeSet::from([ModelCapability::ImageClassification]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing::default(),
            quality_contracts: vec![],
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        app.store().save_provider_profile(&provider).unwrap();
        app.store().save_model_profile(&model).unwrap();
        let selections = vec![format!("model-profile:{}", model.id)];
        let scope = app
            .conversation_journey_data_scope(
                "TEST-mainline-task",
                conversation,
                sent.task_id,
                schemas[0].id,
                schemas[0].revision,
                &selections,
            )
            .unwrap();
        let consent = annotagent_storage::ConversationJourneyConsent {
            repair_after_answer: None,
            repair: None,
            continue_after_clarification: false,
            schema_proposal: None,
            id: uuid::Uuid::new_v4(),
            task_id: sent.task_id,
            builder_operation_id: uuid::Uuid::new_v4(),
            builder_model_id: Some(model.id),
            previous_grant_id: None,
            sample_operation_id: uuid::Uuid::new_v4(),
            builder_scope_hash: "a".repeat(64),
            schema_id: schemas[0].id,
            schema_revision: schemas[0].revision,
            schema_digest: scope.schema_digest.clone(),
            images: scope.images.clone(),
            allowed_models: scope.models.iter().map(|item| item.scope.clone()).collect(),
            maximum_builder_calls: 8,
            maximum_sample_calls: 12,
            expires_at: now + chrono::Duration::minutes(20),
            allow_unknown_cost: true,
        };
        let owner = stable_project_id(
            app.project_path("TEST-mainline-task")
                .unwrap()
                .parent()
                .unwrap(),
        )
        .to_string();
        app.store()
            .save_conversation_journey(&owner, conversation, &consent)
            .unwrap();
        let mut draft = app
            .create_workflow_draft(
                "TEST-mainline-task",
                &annotagent_application::load_settings(None).unwrap(),
                false,
            )
            .unwrap();
        draft.nodes.push(annotagent_core::WorkflowDraftNode {
            id: "TEST-classifier".into(),
            node_type: "classification.classify".into(),
            kind: WorkflowNodeKind::VisionModel,
            model_profile_binding: Some(annotagent_core::WorkflowModelBinding {
                model_profile_id: model.id,
                locked: true,
            }),
            ..annotagent_core::WorkflowDraftNode::default()
        });
        let draft = app.save_workflow_draft(draft).unwrap();
        let draft = app
            .bind_conversation_schema_to_workflow(
                "TEST-mainline-task",
                &draft.id,
                draft.revision,
                schemas[0].id,
                schemas[0].revision,
            )
            .unwrap();
        app.store()
            .reserve_conversation_builder(
                &owner,
                sent.task_id,
                consent.builder_operation_id,
                &"1".repeat(64),
            )
            .unwrap();
        app.store()
            .settle_conversation_builder(
                &owner,
                sent.task_id,
                consent.builder_operation_id,
                true,
                &json!({
                    "outcome":"draft_ready_for_human_review",
                    "draft_id":draft.id,
                    "draft_revision":draft.revision,
                    "draft_content_hash":draft.content_hash
                }),
            )
            .unwrap();
        let continued =
            response_json(request(&service, Method::GET, &format!("{root}/workspace"), None).await)
                .await;
        let action = &continued["mainline"]["available_actions"][0];
        assert_eq!(action["id"], "test_pipeline_samples");
        assert_eq!(action["state"], "requires_confirmation");
        assert_eq!(
            action["scope"]["sample_operation_id"],
            consent.sample_operation_id.to_string()
        );
        assert_eq!(action["scope"]["draft_id"], draft.id);
        assert_eq!(
            action["url"],
            format!("{root}/journey-consents/{}", consent.id)
        );
        assert_eq!(
            action["execution_url"],
            format!("{root}/journey-consents/{}/execution", consent.id)
        );
        let exact_consent = response_json(
            request(&service, Method::GET, action["url"].as_str().unwrap(), None).await,
        )
        .await;
        assert_eq!(exact_consent["consent"]["id"], consent.id.to_string());
        assert_eq!(
            exact_consent["consent"]["sample_operation_id"],
            consent.sample_operation_id.to_string()
        );
        let execution_url = action["execution_url"].as_str().unwrap();
        let first_execution =
            response_json(request(&service, Method::POST, execution_url, Some(json!({}))).await)
                .await;
        let repeated_execution =
            response_json(request(&service, Method::POST, execution_url, Some(json!({}))).await)
                .await;
        assert_eq!(
            first_execution["record"]["consent"]["builder_operation_id"],
            consent.builder_operation_id.to_string()
        );
        assert_eq!(
            repeated_execution["record"]["consent"]["sample_operation_id"],
            consent.sample_operation_id.to_string()
        );
        for _ in 0..100 {
            let current =
                response_json(request(&service, Method::GET, execution_url, None).await).await;
            if current["dispatch"]["status"] != "running" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(
            app.conversation_builder_history("TEST-mainline-task", conversation, sent.task_id)
                .unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let sample_operations = app
            .store()
            .conversation_sample_operations("TEST-mainline-task", conversation, sent.task_id)
            .unwrap();
        assert!(sample_operations.len() <= 1);
        assert!(
            sample_operations
                .iter()
                .all(|operation| operation.id == consent.sample_operation_id.to_string())
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
            if terminal == annotagent_storage::ConversationCallStatus::InDoubt {
                let workspace = response_json(
                    request(
                        &service,
                        axum::http::Method::GET,
                        &format!(
                            "/api/projects/TEST-stop/conversations/{c}/tasks/{}/workspace",
                            sent.task_id
                        ),
                        None,
                    )
                    .await,
                )
                .await;
                let diagnostic = workspace["mainline"]["result_diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|value| value["code"] == "provider_outcome_unknown")
                    .unwrap();
                assert_eq!(diagnostic["source"]["id"], call.to_string());
                assert_eq!(diagnostic["automatic_retry"], false);
                assert_eq!(diagnostic["preserves_existing_results"], true);
            }
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
