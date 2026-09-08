//! One confirmation composes immutable publication and the existing Dataset coordinator.
use super::*;
use annotagent_application::{ConfirmedBatchScope, PublicationApproval};
use annotagent_storage::WorkflowSampleTestInput;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcessingSelection {
    draft_id: String,
    sample_test_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConfirmProcessing {
    request_id: String,
    selection: ProcessingSelection,
    expected_revision: u64,
    authorization_fingerprint: String,
}

fn scope(
    state: &ServerState,
    project: &str,
    selection: &ProcessingSelection,
    settings: &Settings,
) -> ApiResult<Value> {
    let (draft, models) = state
        .application
        .resolved_workflow_draft_model_profiles(&selection.draft_id)
        .map_err(ApiError::bad_request)?;
    if draft.project_id != project {
        return Err(ApiError::not_found("This plan belongs to another Project"));
    }
    #[cfg(not(test))]
    if workflow_draft_uses_mock(&draft) {
        return Err(ApiError::bad_request(
            "Mock models cannot be used for live processing",
        ));
    }
    reject_unresolved_registry_model_nodes(&draft)?;
    if draft.label_pipeline.is_none() || !guided_other_bindings(&draft).is_empty() {
        return Err(ApiError::bad_request(
            "This plan needs a verified Registry or installed plugin model before guided processing",
        ));
    }
    let sample = state
        .application
        .store()
        .get_workflow_sample_test_by_id(&selection.sample_test_id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Sample Test was not found"))?;
    if sample.project_id != project
        || sample.draft_id != draft.id
        || sample.draft_revision != draft.revision
        || sample.draft_content_hash != draft.content_hash
    {
        return Err(ApiError::bad_request(
            "The plan changed after this sample. Test the updated plan before processing.",
        ));
    }
    if !sample.status.allows_publication()
        || !sample.report.validation.valid
        || sample.report.summary.failed_count != 0
    {
        return Err(ApiError::bad_request(
            "This sample has execution failures or an invalid plan. Resolve them before processing.",
        ));
    }
    let images = state
        .application
        .list_project_image_summaries(project)
        .map_err(ApiError::bad_request)?;
    let inputs = images
        .iter()
        .map(|image| WorkflowSampleTestInput {
            image_id: image.image_id.to_string(),
            content_hash: image.content_hash.clone(),
        })
        .collect::<Vec<_>>();
    let schema_hash = state
        .application
        .project_execution_schema_hash(project)
        .map_err(ApiError::bad_request)?;
    let sealed = state.application.store().sample_scope_seal(&sample.id).map_err(ApiError::internal)?.ok_or_else(|| ApiError::bad_request("This older sample has no authorization snapshot. Run a new bounded sample test before processing."))?;
    let native_models = guided_native_models(state, &draft)?;
    let current_seal = guided_sample_seal(state, &draft, &models)?;
    if sealed != current_seal {
        return Err(ApiError::bad_request(
            "Images, model connection or Project definition changed after the sample. A new sample and authorization are required.",
        ));
    }
    let count = selection.limit.unwrap_or(inputs.len());
    if count == 0 || count > inputs.len() {
        return Err(ApiError::bad_request(
            "Choose an available image range greater than zero",
        ));
    }
    let maximum = settings
        .budget
        .max_requests
        .unwrap_or(u64::MAX)
        .min(u64::try_from(count).unwrap_or(u64::MAX).saturating_mul(12));
    if maximum == 0 {
        return Err(ApiError::bad_request(
            "The configured model-call budget is zero",
        ));
    }
    let sample_feedback = sample
        .inputs
        .iter()
        .map(|image| {
            state
                .application
                .store()
                .sample_feedback(&sample.id, &image.image_id)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::internal)?;
    let feedback_count = sample_feedback.iter().map(Vec::len).sum::<usize>();
    let conversation = state
        .application
        .conversation_processing_context(project, &draft, &sample.id)
        .map_err(ApiError::bad_request)?;
    let mut value = json!({
        "project_id":project,"draft_id":draft.id,"sample_test_id":sample.id,"revision":draft.revision,"draft_content_hash":draft.content_hash,
        "goal":state.application.project_goal(project).map_err(ApiError::bad_request)?,
        "plan_name":draft.name,"project_schema_hash":schema_hash,"image_count":count,"available_images":inputs.len(),"images":inputs.iter().take(count).collect::<Vec<_>>(),
        "models":models,"maximum_model_calls":maximum,"estimated_cost":null,
        "sample_feedback":sample_feedback,"sample_feedback_count":feedback_count,
        "sample_images_are_sandbox_only":true,"review_policy":"Uncertain results stay in Review. This action does not accept every output.",
    });
    if let Some(context) = conversation {
        value["goal"] = json!({"goal": context.schema.definition.goal});
        value["conversation"] = json!(context);
    }
    add_native_scope(&mut value, &native_models);
    if !native_models.is_empty() {
        value["native_models"] = json!(native_model_descriptions(&native_models));
    }
    value["authorization_fingerprint"] = json!(annotagent_image_tools::sha256(
        &serde_json::to_vec(&json!({"scope":value,"settings":settings}))
            .map_err(ApiError::internal)?
    ));
    Ok(value)
}

pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
    Query(selection): Query<ProcessingSelection>,
) -> ApiResult<Json<Value>> {
    let settings = state.settings.read().await.clone();
    Ok(Json(scope(&state, &project, &selection, &settings)?))
}

pub(super) async fn conversation_history(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Vec<Value>>> {
    state
        .application
        .conversation_processing_history(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}

fn owned(state: &ServerState, project: &str, id: &str) -> ApiResult<Value> {
    let value = state
        .application
        .store()
        .processing_operation(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Processing confirmation was not found"))?;
    if value["project_id"].as_str() != Some(project) {
        return Err(ApiError::not_found(
            "Processing confirmation belongs to another Project",
        ));
    }
    Ok(value)
}

pub(super) async fn get(
    State(state): State<ServerState>,
    AxumPath((project, id)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(owned(&state, &project, &id)?))
}

pub(super) async fn confirm(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
    Json(mut input): Json<ConfirmProcessing>,
) -> ApiResult<Json<Value>> {
    input.request_id = uuid::Uuid::parse_str(&input.request_id)
        .map_err(ApiError::bad_request)?
        .to_string();
    // Serialize short composition steps, never inference. Durable IDs also cover response loss/restart.
    let _guard = state.processing_gate.lock().await;
    let request = serde_json::to_value(&input).map_err(ApiError::internal)?;
    let existing = state
        .application
        .store()
        .processing_operation(&input.request_id)
        .map_err(ApiError::internal)?;
    if let Some(value) = &existing {
        state
            .application
            .store()
            .reserve_processing_operation(&input.request_id, &project, &request, value)
            .map_err(ApiError::bad_request)?;
        if value["phase"] == "started" {
            return Ok(Json(owned(&state, &project, &input.request_id)?));
        }
    }
    let settings = state.settings.read().await.clone();
    let authorization = scope(&state, &project, &input.selection, &settings)?;
    if authorization["revision"].as_u64() != Some(input.expected_revision)
        || authorization["authorization_fingerprint"].as_str()
            != Some(input.authorization_fingerprint.as_str())
    {
        return Err(ApiError::bad_request(
            "Processing scope changed. Review its images, models and budget again; nothing new was started.",
        ));
    }
    let mut receipt = existing.unwrap_or_else(|| json!({"id":input.request_id,"project_id":project,"draft_id":input.selection.draft_id,"sample_test_id":input.selection.sample_test_id,"phase":"confirmed","authorization":authorization,"request":request}));
    state
        .application
        .store()
        .reserve_processing_operation(&input.request_id, &project, &request, &receipt)
        .map_err(ApiError::bad_request)?;
    let result = execute_confirmation(
        &state,
        &project,
        &input,
        &authorization,
        &settings,
        &mut receipt,
    )
    .await;
    if let Err(error) = result {
        receipt["phase"] = json!(if receipt.get("workflow_id").is_some() {
            "published_start_failed"
        } else {
            "failed"
        });
        receipt["error"] = error.body["error"].clone();
    }
    state
        .application
        .store()
        .update_processing_operation(&input.request_id, &receipt)
        .map_err(ApiError::internal)?;
    Ok(Json(receipt))
}

async fn execute_confirmation(
    state: &ServerState,
    project: &str,
    input: &ConfirmProcessing,
    authorization: &Value,
    settings: &Settings,
    receipt: &mut Value,
) -> ApiResult<()> {
    let batch_id = parse_batch_id(&input.request_id)?;
    if let Ok(batch) = state.application.store().get_batch(batch_id) {
        if batch.project_id != project {
            return Err(ApiError::bad_request("Batch ownership mismatch"));
        }
        // A crash after Batch creation must not create another Batch or silently restart one.
        receipt["batch_id"] = json!(batch.id);
        receipt["phase"] = json!("started");
        receipt["error"] = Value::Null;
        return Ok(());
    }
    let project_summary = state
        .application
        .get_project(project)
        .map_err(ApiError::bad_request)?;
    if project_summary.active_batch.is_some() || project_summary.active_run.is_some() {
        return Err(ApiError::bad_request(
            "This Project already has active processing. Continue or stop that task first.",
        ));
    }
    let published = state
        .application
        .store()
        .list_published_workflow_versions(Some(project))
        .map_err(ApiError::internal)?
        .into_iter()
        .find(|version| {
            version.source_draft_id == input.selection.draft_id
                && version.draft.revision == input.expected_revision
                && serde_json::to_value(&version.snapshot.model_profiles)
                    .is_ok_and(|models| models == authorization["models"])
                && serde_json::to_value(&version.snapshot.plugin_models).is_ok_and(|models| {
                    models
                        == authorization
                            .get("plugin_models")
                            .cloned()
                            .unwrap_or_else(|| json!([]))
                })
                && version.draft.content_hash
                    == authorization["draft_content_hash"]
                        .as_str()
                        .unwrap_or_default()
        });
    let published = if let Some(version) = published {
        version
    } else {
        receipt["phase"] = json!("publishing");
        state
            .application
            .store()
            .update_processing_operation(&input.request_id, receipt)
            .map_err(ApiError::internal)?;
        let approval = PublicationApproval {
            sample_test_id: input.selection.sample_test_id.clone(),
            revision: input.expected_revision,
            models: serde_json::from_value(authorization["models"].clone())
                .map_err(ApiError::internal)?,
            plugin_models: serde_json::from_value(
                authorization
                    .get("plugin_models")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            )
            .map_err(ApiError::internal)?,
            project_schema_hash: authorization["project_schema_hash"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        };
        state
            .application
            .publish_workflow_with_approval(&input.selection.draft_id, settings, Some(&approval))
            .map_err(ApiError::bad_request)?
    };
    receipt["workflow_id"] = json!(published.workflow_id);
    receipt["version"] = json!(published.version);
    receipt["phase"] = json!("published");
    state
        .application
        .store()
        .update_processing_operation(&input.request_id, receipt)
        .map_err(ApiError::internal)?;
    let (provider, credential) =
        resolve_published_runtime_provider(state, &published.workflow_id, published.version)
            .await?;
    let mut execution_settings = settings.clone();
    execution_settings.budget.max_requests = authorization["maximum_model_calls"].as_u64();
    let confirmed = ConfirmedBatchScope {
        id: batch_id,
        settings: execution_settings,
        images: serde_json::from_value(authorization["images"].clone())
            .map_err(ApiError::internal)?,
        project_schema_hash: authorization["project_schema_hash"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
    };
    let path = state
        .application
        .project_path(project)
        .map_err(ApiError::bad_request)?;
    let batch = DatasetCoordinator::new(state.application.as_ref())
        .create_confirmed(
            &path,
            &provider,
            None,
            Some(confirmed.images.len()),
            Some((&published.workflow_id, published.version)),
            Some(&confirmed),
        )
        .map_err(ApiError::bad_request)?;
    receipt["batch_id"] = json!(batch.id);
    receipt["phase"] = json!("started");
    receipt["error"] = Value::Null;
    state
        .application
        .store()
        .update_processing_operation(&input.request_id, receipt)
        .map_err(ApiError::internal)?;
    let application = state.application.clone();
    tokio::spawn(async move {
        let _ = DatasetCoordinator::new(application.as_ref())
            .execute(batch.id, credential)
            .await;
    });
    Ok(())
}
