use super::{
    ApiError, ApiResult, AxumPath, Deserialize, Json, Query, Serialize, ServerState, State,
    StatusCode, StorageError, Value, json, parse_run_id,
};
use std::sync::OnceLock;
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
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExactReplayRequest {
    project_id: String,
    command_id: uuid::Uuid,
    scope_hash: String,
    maximum_model_requests: u64,
    allow_unknown_cost: bool,
}
pub(crate) async fn preview(
    State(state): State<ServerState>,
    AxumPath((run, node)): AxumPath<(String, String)>,
    Query(owner): Query<ReplayOwner>,
) -> ApiResult<Json<Value>> {
    let preview = state
        .application
        .preview_node_replay(&owner.project_id, parse_run_id(&run)?, &node)
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
    let preview = state
        .application
        .preview_node_replay(&request.project_id, id, &node)
        .map_err(ApiError::not_found)?;
    if preview["scope_hash"] != request.scope_hash
        || preview["available"] != true
        || request.maximum_model_requests != 0
        || request.allow_unknown_cost
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({"code":"replay_scope_not_admitted","admitted":false,"preview":preview}),
        });
    }
    let (receipt, created) = state
        .application
        .store()
        .reserve_replay_command(
            request.command_id,
            &request.project_id,
            &run,
            &node,
            &body,
            epoch(),
        )
        .map_err(|e| ApiError::management(e.into()))?;
    if created {
        let settings = state.settings.read().await.clone();
        tokio::spawn(async move {
            // Revalidate after durable reservation; never retry a reserved command.
            let current = state
                .application
                .preview_node_replay(&request.project_id, id, &node);
            let outcome = if current
                .as_ref()
                .is_ok_and(|v| v["scope_hash"] == request.scope_hash && v["available"] == true)
            {
                tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    state.application.replay_run_from_node_exact(
                        id,
                        &node,
                        &settings,
                        preview["source_record_hash"].as_str().unwrap_or(""),
                    ),
                )
                .await
            } else {
                let _ = state.application.store().settle_replay_command(
                    request.command_id,
                    None,
                    Some("scope_changed_before_execution"),
                );
                return;
            };
            let (result, failure) = match outcome {
                Ok(Ok(report)) => (serde_json::to_value(report).ok(), None),
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
