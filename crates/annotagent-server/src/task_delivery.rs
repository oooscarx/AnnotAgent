//! Same-origin protected intake routes; no model invocation on GET or POST.
use super::*;
use annotagent_application::{SaveTaskDeliveryIntent, TaskDeliveryView};

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
