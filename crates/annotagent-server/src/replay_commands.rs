use super::{
    ApiError, ApiResult, AxumPath, Deserialize, Json, Query, Serialize, ServerState, State,
    StatusCode, StorageError, Value, json, parse_run_id,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};
fn epoch() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| uuid::Uuid::new_v4().to_string())
}
fn project_receipt(mut receipt: Value) -> Value {
    if receipt["status"] == "running" && receipt["epoch"] != epoch() {
        receipt["status"] = json!("outcome_unknown");
        receipt["failure"] = json!("executor_restarted");
    }
    receipt.as_object_mut().unwrap().remove("epoch");
    receipt
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplayOwner {
    project_id: String,
    bindings: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExactReplayRequest {
    project_id: String,
    command_id: uuid::Uuid,
    scope_hash: String,
    maximum_model_requests: u64,
    allow_unknown_cost: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    bindings: BTreeMap<String, String>,
}
pub(crate) async fn preview(
    State(state): State<ServerState>,
    AxumPath((run, node)): AxumPath<(String, String)>,
    Query(owner): Query<ReplayOwner>,
) -> ApiResult<Json<Value>> {
    let selections = owner
        .bindings
        .as_deref()
        .map(serde_json::from_str::<BTreeMap<String, String>>)
        .transpose()
        .map_err(ApiError::bad_request)?
        .unwrap_or_default();
    let (preview, _) = state
        .application
        .preview_node_replay_bindings(&owner.project_id, parse_run_id(&run)?, &node, &selections)
        .map_err(ApiError::not_found)?;
    Ok(Json(preview))
}
pub(crate) async fn receipt(
    State(state): State<ServerState>,
    AxumPath((run, node, id)): AxumPath<(String, String, String)>,
    Query(owner): Query<ReplayOwner>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .project_path(&owner.project_id)
        .map_err(ApiError::not_found)?;
    let id = uuid::Uuid::parse_str(&id).map_err(ApiError::bad_request)?;
    let receipt = state
        .application
        .store()
        .replay_command(id, &owner.project_id, &run, &node)
        .map_err(ApiError::bad_request)?
        .ok_or_else(|| ApiError::not_found("Replay command not found"))?;
    Ok(Json(project_receipt(receipt)))
}
pub(crate) async fn execute(
    state: ServerState,
    run: String,
    node: String,
    request: ExactReplayRequest,
) -> ApiResult<Json<Value>> {
    state
        .application
        .project_path(&request.project_id)
        .map_err(ApiError::not_found)?;
    let body = serde_json::to_value(&request).map_err(ApiError::bad_request)?;
    if let Some(old) = state
        .application
        .store()
        .replay_command(request.command_id, &request.project_id, &run, &node)
        .map_err(ApiError::bad_request)?
    {
        if old["request"] != body {
            return Err(ApiError::management(anyhow::anyhow!(
                StorageError::Management {
                    code: "replay_command_conflict".into(),
                    message: "Replay command already has another scope".into()
                }
            )));
        }
        return Ok(Json(project_receipt(old)));
    }
    let id = parse_run_id(&run)?;
    let (preview, overlay) = state
        .application
        .preview_node_replay_bindings(&request.project_id, id, &node, &request.bindings)
        .map_err(ApiError::not_found)?;
    let external = overlay.uses_external_calls();
    if preview["scope_hash"] != request.scope_hash
        || preview["available"] != true
        || (external
            && (!(1..=12).contains(&request.maximum_model_requests) || !request.allow_unknown_cost))
        || (!external && (request.maximum_model_requests != 0 || request.allow_unknown_cost))
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({"code":"replay_scope_not_admitted","admitted":false,"preview":preview}),
        });
    }
    let (receipt, created) = state
        .application
        .store()
        .reserve_replay_command_with_scope(
            request.command_id,
            &request.project_id,
            &run,
            &node,
            &body,
            epoch(),
            Some(&preview),
        )
        .map_err(|e| ApiError::management(e.into()))?;
    if created {
        let mut settings = state.settings.read().await.clone();
        settings.provider.max_retries = 0;
        settings.provider.request_timeout_seconds = 30;
        settings.provider.extra_request_fields.clear();
        settings.provider.custom_headers.clear();
        tokio::spawn(async move {
            let app = state.application.clone();
            let project = request.project_id.clone();
            let selected = request.bindings.clone();
            let expected = request.scope_hash.clone();
            let source_node = node.clone();
            let guard: annotagent_application::ReplayPermissionCheck = Arc::new(move || match app
                .preview_node_replay_bindings(&project, id, &source_node, &selected)
            {
                Ok((current, _))
                    if current["available"] == true && current["scope_hash"] == expected =>
                {
                    Ok(())
                }
                _ => Err(annotagent_core::CoreError::Validation(
                    "Replay binding permission or source scope changed".into(),
                )),
            });
            if guard().is_err() {
                let _ = state.application.store().settle_replay_command(
                    request.command_id,
                    None,
                    Some("scope_changed_before_execution"),
                );
                return;
            }
            if let Some(profile) = overlay.profiles.first() {
                if let Ok(provider) = state
                    .application
                    .store()
                    .get_provider_profile(profile.provider_id)
                {
                    settings.provider.request_timeout_seconds =
                        provider.connection_policy.request_timeout_seconds.min(30);
                    settings.provider.custom_headers = provider.safe_headers;
                    if let Some(org) = provider.organization {
                        settings
                            .provider
                            .custom_headers
                            .insert("OpenAI-Organization".into(), org);
                    }
                    if let Some(workspace) = provider.workspace {
                        settings
                            .provider
                            .custom_headers
                            .insert("OpenAI-Project".into(), workspace);
                    }
                } else {
                    let _ = state.application.store().settle_replay_command(
                        request.command_id,
                        None,
                        Some("current_provider_unavailable"),
                    );
                    return;
                }
            }
            let credential = if overlay.profiles.is_empty() {
                None
            } else if let Ok((_, credential)) =
                super::resolve_runtime_model_profiles(&state, &overlay.profiles, true).await
            {
                credential
            } else {
                let _ = state.application.store().settle_replay_command(
                    request.command_id,
                    None,
                    Some("current_credential_unavailable"),
                );
                return;
            };
            let outcome = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                state.application.replay_run_with_overlay(
                    id,
                    &node,
                    &settings,
                    preview["source_record_hash"].as_str().unwrap_or(""),
                    overlay,
                    credential,
                    request.maximum_model_requests,
                    guard,
                ),
            )
            .await;
            let (result, failure) = match outcome {
                Ok(Ok(report)) => {
                    let mut report = serde_json::to_value(report).unwrap_or(Value::Null);
                    report["execution_bindings"] = preview["current_bindings"].clone();
                    report["scope_hash"] = preview["scope_hash"].clone();
                    (Some(report), None)
                }
                Ok(Err(_)) => (None, Some("sandbox_execution_failed")),
                Err(_) => (None, Some("execution_timeout")),
            };
            let _ = state.application.store().settle_replay_command(
                request.command_id,
                result,
                failure,
            );
        });
    }
    Ok(Json(project_receipt(receipt)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_receipt_old_process_is_unknown_without_mutation() {
        let stored = json!({"status":"running","epoch":"TEST-dead-process","result":null});
        let visible = project_receipt(stored.clone());
        assert_eq!(visible["status"], "outcome_unknown");
        assert_eq!(visible["failure"], "executor_restarted");
        assert!(visible.get("epoch").is_none());
        assert_eq!(stored["status"], "running");
    }
}
