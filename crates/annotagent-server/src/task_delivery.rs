//! Same-origin protected intake routes; no model invocation on GET or POST.
use super::*;
use annotagent_application::{SaveTaskDeliveryIntent, TaskDeliveryView};

pub(super) async fn current_schema(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    let delivery = state
        .application
        .require_delivery_intake(&project, conversation, task)
        .map_err(ApiError::conversation)?;
    let schema = if delivery.is_some() {
        state
            .application
            .human_conversation_schema_drafts(&project, conversation, task)
            .map_err(ApiError::conversation)?
            .into_iter()
            .find(|s| {
                annotagent_application::require_delivery_schema(delivery.as_ref(), &s.definition)
                    .is_ok()
            })
    } else {
        None
    };
    Ok(Json(json!({"required":delivery.is_some(),"schema":schema})))
}

pub(super) async fn prepare_schema(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<annotagent_application::PrepareDeliverySchema>,
) -> ApiResult<Json<annotagent_storage::ConversationSchemaDraft>> {
    state
        .application
        .prepare_delivery_schema(&project, conversation, task, &input)
        .map(Json)
        .map_err(ApiError::conversation)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ImageQuery {
    source_run_id: Option<annotagent_core::RunId>,
}

pub(super) async fn image(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, image)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        annotagent_core::ImageId,
    )>,
    Query(query): Query<ImageQuery>,
) -> ApiResult<Json<annotagent_application::TaskDeliveryImageView>> {
    state
        .application
        .task_delivery_image(&project, conversation, task, image, query.source_run_id)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn confirm_image(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, image)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        annotagent_core::ImageId,
    )>,
    Json(input): Json<annotagent_storage::DeliveryImageReviewInput>,
) -> ApiResult<Json<annotagent_storage::DeliveryImageReview>> {
    if input.image_id != image {
        return Err(ApiError::conversation(anyhow::anyhow!(
            "Confirmation image does not match the route"
        )));
    }
    state
        .application
        .confirm_task_delivery_image(&project, conversation, task, &input)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn get(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<TaskDeliveryView>> {
    state
        .application
        .task_delivery_intent(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn save(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<SaveTaskDeliveryIntent>,
) -> ApiResult<Json<TaskDeliveryView>> {
    state
        .application
        .save_task_delivery_intent(&project, conversation, task, input)
        .map(Json)
        .map_err(ApiError::conversation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{request, response_json, test_state};
    use annotagent_provider::InMemorySecretStore;
    use axum::http::Method;

    #[tokio::test]
    async fn delivery_image_http_requires_explicit_owned_snapshot_and_restores_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(dir.path()).unwrap());
        let yaml = include_str!(
            "../../../examples/label-pipelines/whole-image-classification/project.yaml"
        );
        app.create_project("TEST-image-review", yaml).unwrap();
        app.create_project("TEST-foreign", yaml).unwrap();
        let source = dir.path().join("TEST-original.png");
        annotagent_image_tools::generate_synthetic_inspection(&source).unwrap();
        app.import_images("TEST-image-review", &source).unwrap();
        let images = app
            .list_project_image_summaries("TEST-image-review")
            .unwrap();
        let image = images[0].image_id;
        let conversation = app
            .create_project_conversation("TEST-image-review")
            .unwrap();
        let sent = app.send_project_conversation_message("TEST-image-review",conversation,&serde_json::from_value(json!({
            "message":{"id":uuid::Uuid::new_v4(),"text":"TEST training delivery","image":null},"task_id":null,
            "schema_revision":app.project_goal("TEST-image-review").unwrap()["revision"],"mode":"plan"
        })).unwrap()).unwrap();
        let service = router(
            test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let root = format!(
            "/api/projects/TEST-image-review/conversations/{conversation}/tasks/{}",
            sent.task_id
        );
        let image_uri = format!("{root}/delivery-images/{image}");
        assert!(
            !request(&service, Method::GET, &image_uri, None)
                .await
                .status()
                .is_success()
        );
        let mut intent = json!({"command_id":uuid::Uuid::new_v4(),"expected_revision":0,"image_ids":[image],
            "label_spec":[{"stable_id":"TEST-target","display_name":"TEST target","aliases":[],"include":"","exclude":""}],
            "training_target":{"annotation_kind":"bounding_box","framework":"ultralytics","export_profile":"ultralytics_yolo_detection","profile_revision":1},
            "split_policy":{"train_percent":80,"seed":0,"preserve_existing":true,"keep_known_groups_together":true}});
        let saved = request(
            &service,
            Method::POST,
            &format!("{root}/delivery-intent"),
            Some(intent.clone()),
        )
        .await;
        assert_eq!(saved.status(), StatusCode::OK);
        let view = response_json(request(&service, Method::GET, &image_uri, None).await).await;
        assert_eq!(view["confirmation_current"], false);
        assert!(view["review"].is_null());
        assert_eq!(view["accepted_objects"], 0);
        let prepare = json!({"command_id":uuid::Uuid::new_v4(),"expected_revision":view["intent_revision"],"expected_sha256":view["intent_sha256"]});
        let prepared = request(
            &service,
            Method::POST,
            &format!("{root}/delivery-schema"),
            Some(prepare.clone()),
        )
        .await;
        assert_eq!(prepared.status(), StatusCode::OK);
        let prepared = response_json(prepared).await;
        assert_eq!(prepared["definition"]["task"]["kind"], "bounding_box");
        assert_eq!(
            prepared["definition"]["task"]["labels"],
            json!(["TEST-target"])
        );
        assert_eq!(
            prepared,
            response_json(
                request(
                    &service,
                    Method::POST,
                    &format!("{root}/delivery-schema"),
                    Some(prepare.clone())
                )
                .await
            )
            .await
        );
        let prepared_schema: annotagent_storage::ConversationSchemaDraft =
            serde_json::from_value(prepared).unwrap();
        let restored_schema = response_json(
            request(
                &service,
                Method::GET,
                &format!("{root}/delivery-schema"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            restored_schema["schema"]["id"],
            prepared_schema.id.to_string()
        );
        assert_eq!(restored_schema["required"], true);
        let current_delivery = app
            .task_delivery_intent("TEST-image-review", conversation, sent.task_id)
            .unwrap();
        annotagent_application::require_delivery_schema(
            current_delivery.saved.as_ref(),
            &prepared_schema.definition,
        )
        .unwrap();
        let mut wrong = prepared_schema.definition.clone();
        wrong.task.kind = annotagent_core::TaskKind::Classification;
        assert!(
            annotagent_application::require_delivery_schema(
                current_delivery.saved.as_ref(),
                &wrong
            )
            .is_err()
        );
        assert!(
            !request(
                &service,
                Method::POST,
                &format!("{root}/delivery-schema").replacen("TEST-image-review", "TEST-foreign", 1),
                Some(prepare.clone())
            )
            .await
            .status()
            .is_success()
        );
        let mut confirm = json!({"command_id":uuid::Uuid::new_v4(),"intent_revision":view["intent_revision"],"intent_sha256":view["intent_sha256"],
            "image_id":image,"source_run_id":null,"expected_snapshot_sha256":view["snapshot"]["sha256"],"expected_review_revision":0,
            "decision":"negative_confirmed","reason":null,"confirmed":false});
        assert!(
            !request(&service, Method::POST, &image_uri, Some(confirm.clone()))
                .await
                .status()
                .is_success()
        );
        confirm["confirmed"] = json!(true);
        let mut forged = confirm.clone();
        forged["expected_snapshot_sha256"] = json!("0".repeat(64));
        assert!(
            !request(&service, Method::POST, &image_uri, Some(forged))
                .await
                .status()
                .is_success()
        );
        let response = request(&service, Method::POST, &image_uri, Some(confirm.clone())).await;
        assert_eq!(response.status(), StatusCode::OK);
        let receipt = response_json(response).await;
        assert_eq!(receipt["revision"], 1);
        assert_eq!(
            receipt,
            response_json(request(&service, Method::POST, &image_uri, Some(confirm.clone())).await)
                .await
        );
        let mut changed = confirm.clone();
        changed["decision"] = json!("excluded");
        changed["reason"] = json!("changed retry");
        assert!(
            !request(&service, Method::POST, &image_uri, Some(changed))
                .await
                .status()
                .is_success()
        );
        let foreign = image_uri.replacen("TEST-image-review", "TEST-foreign", 1);
        assert!(
            !request(&service, Method::GET, &foreign, None)
                .await
                .status()
                .is_success()
        );
        assert!(
            !request(&service, Method::POST, &foreign, Some(confirm.clone()))
                .await
                .status()
                .is_success()
        );
        let wrong_image_uri = format!("{root}/delivery-images/{}", uuid::Uuid::new_v4());
        assert!(
            !request(&service, Method::POST, &wrong_image_uri, Some(confirm))
                .await
                .status()
                .is_success()
        );
        let restored_app = Arc::new(LocalApplication::new(dir.path()).unwrap());
        let restored_service = router(
            test_state(restored_app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let restored =
            response_json(request(&restored_service, Method::GET, &image_uri, None).await).await;
        assert_eq!(restored["confirmation_current"], true);
        assert_eq!(restored["review"], receipt);
        // New delivery semantics invalidate current confirmation; old receipts stay immutable.
        intent["command_id"] = json!(uuid::Uuid::new_v4());
        intent["expected_revision"] = json!(1);
        intent["label_spec"][0]["display_name"] = json!("TEST changed target");
        assert_eq!(
            request(
                &service,
                Method::POST,
                &format!("{root}/delivery-intent"),
                Some(intent)
            )
            .await
            .status(),
            StatusCode::OK
        );
        let current = response_json(request(&service, Method::GET, &image_uri, None).await).await;
        assert_eq!(current["confirmation_current"], false);
        assert!(current["review"].is_null());
        assert!(
            !request(
                &service,
                Method::POST,
                &format!("{root}/delivery-schema"),
                Some(prepare)
            )
            .await
            .status()
            .is_success()
        );
        let latest = app
            .task_delivery_intent("TEST-image-review", conversation, sent.task_id)
            .unwrap();
        assert!(
            response_json(
                request(
                    &service,
                    Method::GET,
                    &format!("{root}/delivery-schema"),
                    None
                )
                .await
            )
            .await["schema"]
                .is_null()
        );
        assert!(
            annotagent_application::require_delivery_schema(
                latest.saved.as_ref(),
                &prepared_schema.definition
            )
            .is_err()
        );
        assert!(
            app.conversation_schema_calls("TEST-image-review", conversation, sent.task_id)
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn delivery_http_restores_missing_slots_and_rejects_scope_and_revision_changes() {
        let dir = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(dir.path()).unwrap());
        let yaml = include_str!(
            "../../../examples/label-pipelines/whole-image-classification/project.yaml"
        );
        app.create_project("TEST-intake", yaml).unwrap();
        app.create_project("TEST-other", yaml).unwrap();
        let conversation = app.create_project_conversation("TEST-intake").unwrap();
        let sent = app.send_project_conversation_message("TEST-intake", conversation, &serde_json::from_value(json!({
            "message":{"id":uuid::Uuid::new_v4(),"text":"TEST training dataset","image":null},
            "task_id":null,"schema_revision":app.project_goal("TEST-intake").unwrap()["revision"],"mode":"plan"
        })).unwrap()).unwrap();
        let service = router(
            test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let uri = format!(
            "/api/projects/TEST-intake/conversations/{conversation}/tasks/{}/delivery-intent",
            sent.task_id
        );
        let read = response_json(request(&service, Method::GET, &uri, None).await).await;
        assert!(read["saved"].is_null());
        assert_eq!(read["missing_slots"].as_array().unwrap().len(), 3);
        let input = json!({"command_id":uuid::Uuid::new_v4(),"expected_revision":0,"image_ids":null,
            "label_spec":[{"stable_id":"ball-id","display_name":"球","aliases":[],"include":"","exclude":""}],
            "training_target":null,"split_policy":{"train_percent":80,"seed":0,"preserve_existing":true,"keep_known_groups_together":true}});
        let response = request(&service, Method::POST, &uri, Some(input.clone())).await;
        assert_eq!(response.status(), StatusCode::OK);
        let saved = response_json(response).await;
        assert_eq!(
            saved["missing_slots"],
            json!(["dataset_scope", "training_target"])
        );
        assert_eq!(saved["execution_authorized"], false);
        let planning_uri = uri.replace("delivery-intent", "schema-preview");
        let blocked = request(&service, Method::GET, &planning_uri, None).await;
        assert!(!blocked.status().is_success());
        assert!(
            response_json(blocked)
                .await
                .to_string()
                .contains("Complete delivery information")
        );
        assert!(
            app.conversation_schema_calls("TEST-intake", conversation, sent.task_id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            saved,
            response_json(request(&service, Method::POST, &uri, Some(input.clone())).await).await
        );
        assert_eq!(
            saved,
            response_json(request(&service, Method::GET, &uri, None).await).await
        );
        let mut changed = input.clone();
        changed["label_spec"][0]["display_name"] = json!("瓶子");
        assert!(
            !request(&service, Method::POST, &uri, Some(changed))
                .await
                .status()
                .is_success()
        );
        let mut foreign_image = input;
        foreign_image["command_id"] = json!(uuid::Uuid::new_v4());
        foreign_image["expected_revision"] = json!(1);
        foreign_image["image_ids"] = json!([uuid::Uuid::new_v4()]);
        assert!(
            !request(&service, Method::POST, &uri, Some(foreign_image))
                .await
                .status()
                .is_success()
        );
        let foreign = uri.replacen("TEST-intake", "TEST-other", 1);
        assert!(
            !request(&service, Method::GET, &foreign, None)
                .await
                .status()
                .is_success()
        );
        assert_eq!(
            saved,
            response_json(request(&service, Method::GET, &uri, None).await).await
        );
    }
}
