//! Safe Settings projection and revision-checked future budget updates.
use super::*;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct View {
    view: Option<String>,
}
pub(super) fn revision(settings: &Settings) -> String {
    annotagent_image_tools::sha256(&serde_json::to_vec(settings).expect("Settings serialize"))
}
pub(super) fn conflict(current: &str) -> ApiError {
    ApiError {
        status: StatusCode::CONFLICT,
        body: json!({"error":"Settings changed; reload before updating future budgets","status":409,"code":"settings_revision_conflict","current_revision":current,"suggested_action":"reload_settings"}),
    }
}
pub(super) async fn safe(state: &ServerState) -> Json<Value> {
    let settings = state.settings.read().await.clone();
    Json(json!({"revision":revision(&settings),"sections":{
        "general":{"storage":"browser_preferences","fields":["theme","language","density","font_size"]},
        "providers":{"list_url":"/api/providers","credential_configured":state.api_key.read().await.is_some(),"probe_on_read":false},
        "agent_models":{"profiles_url":"/api/model-profiles","defaults_url":"/api/agent-model-bindings","effective":"next_request"},
        "vision_plugins":{"plugins_url":"/api/plugins","instances_url":"/api/model-instances","bundles_url":"/api/model-bundles"},
        "data_privacy":{"workspace_id":stable_project_id(state.application.workspace()),"storage_scope":"local_workspace","cleanup_requires_preview":true,"outbound_authorization":"exact_operation_scope"},
        "usage_budget":{"future_run_budget":settings.budget,"pricing":settings.pricing,"scope":"future_runs_only","unknown_cost":null,"task_budget":"existing_task_ledger"}
    }}))
}
pub(super) async fn get(
    State(state): State<ServerState>,
    Query(view): Query<View>,
) -> ApiResult<Json<Value>> {
    match view.view.as_deref() {
        Some("agent-ui") => Ok(safe(&state).await),
        None => Ok(super::get_settings(State(state)).await),
        _ => Err(ApiError::bad_request("Unknown settings view")),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BudgetPatch {
    expected_revision: String,
    budget: annotagent_core::Budget,
}
pub(super) async fn patch(
    State(state): State<ServerState>,
    Json(input): Json<BudgetPatch>,
) -> ApiResult<Json<Value>> {
    let _write = state.settings_writes.lock().await;
    let mut settings = state.settings.read().await.clone();
    let current = revision(&settings);
    if input.expected_revision != current {
        return Err(conflict(&current));
    }
    if input
        .budget
        .max_cost
        .is_some_and(|cost| cost.is_sign_negative())
    {
        return Err(ApiError::bad_request(
            "Budget max_cost must not be negative",
        ));
    }
    settings.budget = input.budget;
    validate_settings(&settings).map_err(ApiError::bad_request)?;
    persist_settings(&state.settings_path, &settings).map_err(ApiError::internal)?;
    *state.settings.write().await = settings;
    *state.settings_persisted.write().await = true;
    Ok(safe(&state).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{request, response_json, test_state};
    use annotagent_provider::InMemorySecretStore;
    #[tokio::test]
    async fn safe_settings_are_passive_and_budget_patch_rejects_stale_revision() {
        let dir = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(dir.path()).unwrap());
        let service = router(
            test_state(app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let initial = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/settings?view=agent-ui",
                None,
            )
            .await,
        )
        .await;
        assert!(initial["revision"].is_string());
        assert!(initial["sections"]["providers"].is_object());
        assert!(
            !initial
                .to_string()
                .contains(&dir.path().display().to_string())
        );
        assert!(initial.get("provider").is_none());
        let input = json!({"expected_revision":initial["revision"],"budget":{"max_requests":7}});
        let saved = response_json(
            request(
                &service,
                axum::http::Method::PATCH,
                "/api/settings",
                Some(input.clone()),
            )
            .await,
        )
        .await;
        assert_eq!(
            saved["sections"]["usage_budget"]["future_run_budget"]["max_requests"],
            7
        );
        let stale = request(
            &service,
            axum::http::Method::PATCH,
            "/api/settings",
            Some(input),
        )
        .await;
        assert_eq!(stale.status(), StatusCode::CONFLICT);
        let error = response_json(stale).await;
        assert_eq!(error["code"], "settings_revision_conflict");
        assert_eq!(error["current_revision"], saved["revision"]);
    }
}
