//! HTTP lifecycle adapter around the existing Sample Test service.
use super::*;
use annotagent_application::SampleExecutionControl;
use annotagent_storage::SampleOperation;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartSampleRequest {
    request_id: String,
    draft_id: String,
    image_indices: Vec<usize>,
    expected_revision: u64,
    authorization_fingerprint: String,
}

pub(super) fn validate_scope(
    state: &ServerState,
    draft: &WorkflowDraft,
    models: &[ModelProfileSnapshot],
    input: &DryRunWorkflowRequest,
) -> ApiResult<()> {
    if input.expected_revision != Some(draft.revision)
        || input.authorization_fingerprint.as_deref()
            != Some(guided_sample_fingerprint(state, draft, models)?.as_str())
    {
        return Err(ApiError::bad_request(
            "The plan, images, goal or model scope changed after authorization. Review the updated sample scope.",
        ));
    }
    reject_unresolved_registry_model_nodes(draft)?;
    if draft.label_pipeline.is_none()
        || draft
            .nodes
            .iter()
            .any(|node| node.model_binding.is_some() && node.model_profile_binding.is_none())
    {
        return Err(ApiError::bad_request(
            "This plan contains bindings not yet supported by bounded guided sampling. No inference was started.",
        ));
    }
    let count = state
        .application
        .get_project(&draft.project_id)
        .map_err(ApiError::bad_request)?
        .image_count
        .min(3);
    if count == 0 || input.image_indices != (0..count).collect::<Vec<_>>() {
        return Err(ApiError::bad_request(
            "Guided sampling requires an explicit selection of 1–3 images.",
        ));
    }
    Ok(())
}

fn owned(state: &ServerState, project_id: &str, id: &str) -> ApiResult<SampleOperation> {
    let operation = state
        .application
        .store()
        .sample_operation(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Sample task was not found"))?;
    if operation.project_id != project_id {
        return Err(ApiError::not_found(
            "Sample task does not belong to this Project",
        ));
    }
    Ok(operation)
}

pub(super) async fn get_operation(
    State(state): State<ServerState>,
    AxumPath((project_id, id)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(owned(&state, &project_id, &id)?)))
}

pub(super) async fn cancel_operation(
    State(state): State<ServerState>,
    AxumPath((project_id, id)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    owned(&state, &project_id, &id)?;
    state
        .application
        .store()
        .cancel_sample_operation(&id, &project_id)
        .map_err(ApiError::internal)?;
    if let Some(token) = state.sample_cancellations.read().await.get(&id) {
        token.cancel();
    }
    Ok(Json(json!(owned(&state, &project_id, &id)?)))
}

pub(super) async fn start_operation(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(input): Json<StartSampleRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let request_id = uuid::Uuid::parse_str(&input.request_id)
        .map_err(|_| ApiError::bad_request("A UUID sample request_id is required"))?
        .to_string();
    let execution = DryRunWorkflowRequest {
        image_indices: input.image_indices,
        expected_revision: Some(input.expected_revision),
        authorization_fingerprint: Some(input.authorization_fingerprint.clone()),
    };
    let now = chrono::Utc::now().to_rfc3339();
    let operation = SampleOperation {
        id: request_id,
        project_id: project_id.clone(),
        draft_id: input.draft_id,
        authorization_fingerprint: input.authorization_fingerprint,
        request: serde_json::to_value(&execution).map_err(ApiError::internal)?,
        status: "queued".into(),
        error: None,
        created_at: now.clone(),
        updated_at: now,
    };
    // Retrying an identical request reads its receipt, even when it has completed or stopped.
    if state
        .application
        .store()
        .sample_operation(&operation.id)
        .map_err(ApiError::internal)?
        .is_some()
    {
        state
            .application
            .store()
            .reserve_sample_operation(&operation)
            .map_err(ApiError::bad_request)?;
        return Ok((
            StatusCode::OK,
            Json(json!(owned(&state, &project_id, &operation.id)?)),
        ));
    }
    let (draft, models) = state
        .application
        .resolved_workflow_draft_model_profiles(&operation.draft_id)
        .map_err(ApiError::bad_request)?;
    if draft.project_id != project_id {
        return Err(ApiError::not_found(
            "This plan does not belong to this Project",
        ));
    }
    validate_scope(&state, &draft, &models, &execution)?;
    let mut settings = state.settings.read().await.clone();
    settings.budget.max_requests = Some(12);
    let (provider, credential) =
        resolve_runtime_model_profiles(&state, &models, workflow_uses_model(&draft)).await?;
    let (latest, models) = state
        .application
        .resolved_workflow_draft_model_profiles(&operation.draft_id)
        .map_err(ApiError::bad_request)?;
    validate_scope(&state, &latest, &models, &execution)?;
    if !state
        .application
        .store()
        .reserve_sample_operation(&operation)
        .map_err(ApiError::bad_request)?
    {
        return Ok((
            StatusCode::OK,
            Json(json!(owned(&state, &project_id, &operation.id)?)),
        ));
    }
    let cancellation = CancellationToken::new();
    state
        .sample_cancellations
        .write()
        .await
        .insert(operation.id.clone(), cancellation.clone());
    let response = operation.clone();
    tokio::spawn(async move {
        let id = operation.id.clone();
        let execution_state = state.clone();
        let execution_id = id.clone();
        let result = tokio::spawn(async move {
            let state = execution_state;
            let id = execution_id;
            if !state.application.store().start_sample_operation(&id)? {
                return Err(anyhow!("Sample task was stopped before execution"));
            }
            let scope_state = state.clone();
            let scope_draft_id = operation.draft_id.clone();
            let scope_input = execution.clone();
            // Preparation changes status/revision; later checks protect the captured scope.
            let baseline = latest;
            let initial_check = std::sync::atomic::AtomicBool::new(true);
            let control = SampleExecutionControl {
                id: id.clone(),
                cancellation,
                check_scope: Some(Arc::new(move || {
                    let (draft, models) = scope_state
                        .application
                        .resolved_workflow_draft_model_profiles(&scope_draft_id)?;
                    let checked = if initial_check.swap(false, std::sync::atomic::Ordering::SeqCst)
                    {
                        &draft
                    } else {
                        &baseline
                    };
                    validate_scope(&scope_state, checked, &models, &scope_input).map_err(|error| {
                        anyhow!(
                            error.body["error"]
                                .as_str()
                                .unwrap_or("Sample scope validation failed")
                                .to_owned()
                        )
                    })
                })),
            };
            state
                .application
                .dry_run_workflow_samples_controlled(
                    &operation.draft_id,
                    &settings,
                    &execution.image_indices,
                    &provider,
                    credential.as_deref(),
                    control,
                )
                .await?;
            Ok::<_, anyhow::Error>(())
        })
        .await;
        let error = match result {
            Ok(result) => result.err().map(|error| error.to_string()),
            Err(_) => Some(
                "Sample worker stopped unexpectedly. No automatic retry was started.".to_owned(),
            ),
        };
        if let Err(error) = state
            .application
            .store()
            .finish_sample_operation(&id, error.as_deref())
        {
            eprintln!("could not finish sample operation {id}: {error}");
        }
        state.sample_cancellations.write().await.remove(&id);
    });
    Ok((StatusCode::ACCEPTED, Json(json!(response))))
}
