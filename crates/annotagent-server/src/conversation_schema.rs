//! Explicit consent boundary for one text-only Schema proposal. No publish/start.
use super::*;
use annotagent_application::{ConversationSchemaExecution, PipelineBuilderModelRuntime};
use annotagent_storage::ConversationCallReceipt;
use chrono::{Duration, Utc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaRevisionQuery {
    revision: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaEdit {
    request_id: uuid::Uuid,
    expected_revision: u64,
    decision: annotagent_application::ConversationSchemaDecision,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HumanSchemaInput {
    request_id: uuid::Uuid,
    decision: annotagent_application::ConversationSchemaDecision,
    clarification: Option<annotagent_storage::SchemaClarificationRef>,
    journey_consent_id: Option<uuid::Uuid>,
}

pub(super) async fn clarification(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<annotagent_storage::SchemaClarification>> {
    state
        .application
        .schema_clarification(&project, conversation, task, call)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn cancel_clarification(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(reference): Json<annotagent_storage::SchemaClarificationRef>,
) -> ApiResult<Json<annotagent_storage::SchemaClarification>> {
    if call != reference.call_id {
        return Err(ApiError::bad_request("Clarification identity changed"));
    }
    state
        .application
        .cancel_schema_clarification(&project, conversation, task, &reference)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn human_drafts(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Vec<annotagent_storage::ConversationSchemaDraft>>> {
    state
        .application
        .human_conversation_schema_drafts(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn save_human_draft(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<HumanSchemaInput>,
) -> ApiResult<Json<Value>> {
    if let Some(id) = input.journey_consent_id {
        let saved = state
            .application
            .conversation_journey_consent(&project, conversation, task, id)
            .map_err(ApiError::bad_request)?
            .ok_or_else(|| ApiError::bad_request("Journey authorization not found"))?;
        if !saved.consent.continue_after_clarification
            || saved
                .consent
                .schema_proposal
                .as_ref()
                .map(|proposal| proposal.call_id)
                != input
                    .clarification
                    .as_ref()
                    .map(|reference| reference.call_id)
            || input.clarification.is_none()
        {
            return Err(ApiError::bad_request(
                "This answer is not linked to the authorized journey clarification",
            ));
        }
    }
    let saved = state
        .application
        .save_human_schema_with_clarification(
            &project,
            conversation,
            task,
            input.request_id,
            &input.decision,
            input.clarification.as_ref(),
        )
        .map_err(ApiError::bad_request)?;
    let schema_id = saved.id;
    let mut result = serde_json::to_value(saved).map_err(ApiError::internal)?;
    if let Some(id) = input.journey_consent_id {
        // The answer is already durable. Failure to admit continuation must not
        // turn a saved human answer into an ambiguous failed-save response.
        let queued = state.application.queue_conversation_journey_answer(
            &project,
            conversation,
            task,
            id,
            schema_id,
        );
        if let Err(error) = queued {
            result["journey_resume"] = json!({"consent_id":id,"error":error.to_string()});
            return Ok(Json(result));
        }
        result["journey_resume"] = match conversation_journey::execute(
            State(state),
            AxumPath((project, conversation, task, id)),
            Json(conversation_journey::ExecuteJourney {}),
        )
        .await
        {
            Ok(value) => json!({"consent_id":id,"status":value.0}),
            Err(error) => json!({"consent_id":id,"error":error.body["error"]}),
        };
    }
    Ok(Json(result))
}

pub(super) async fn save_draft(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<annotagent_storage::ConversationSchemaDraft>> {
    state
        .application
        .save_conversation_schema_draft(&project, conversation, task, call)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn draft_for_call(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<Option<annotagent_storage::ConversationSchemaDraft>>> {
    state
        .application
        .conversation_schema_for_call(&project, conversation, task, call)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn read_draft(
    State(state): State<ServerState>,
    AxumPath((project, draft)): AxumPath<(String, uuid::Uuid)>,
    Query(query): Query<SchemaRevisionQuery>,
) -> ApiResult<Json<annotagent_storage::ConversationSchemaDraft>> {
    state
        .application
        .conversation_schema_draft(&project, draft, query.revision)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn edit_draft(
    State(state): State<ServerState>,
    AxumPath((project, draft)): AxumPath<(String, uuid::Uuid)>,
    Json(edit): Json<SchemaEdit>,
) -> ApiResult<Json<annotagent_storage::ConversationSchemaDraft>> {
    state
        .application
        .revise_conversation_schema_draft(
            &project,
            draft,
            edit.request_id,
            edit.expected_revision,
            &edit.decision,
        )
        .map(Json)
        .map_err(ApiError::bad_request)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModelSelection {
    model_id: Option<ModelProfileId>,
}

pub(super) type SchemaConsent = annotagent_storage::ConversationSchemaAuthorization;

pub(super) async fn pending_authorization(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Option<SchemaConsent>>> {
    state
        .application
        .pending_conversation_schema_authorization(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) fn preview_scope(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    model_id: Option<ModelProfileId>,
) -> ApiResult<(PipelineBuilderModelRuntime, Value)> {
    let task_record = state
        .application
        .conversation_tasks(project, conversation)
        .map_err(ApiError::bad_request)?
        .into_iter()
        .find(|item| item.input.id == task)
        .ok_or_else(|| ApiError::not_found("Task not found in this conversation"))?;
    let goal = state
        .application
        .project_goal(project)
        .map_err(ApiError::bad_request)?;
    if goal["revision"].as_str() != Some(&task_record.input.schema_revision) {
        return Err(ApiError::bad_request(
            "Project Schema changed since this task started. No request was sent.",
        ));
    }
    let selected = state
        .application
        .resolve_conversation_agent_model(project, conversation, model_id)
        .map_err(ApiError::bad_request)?;
    let mut config = selected
        .openai_compatible_config()
        .map_err(ApiError::bad_request)?;
    config.max_retries = 0;
    config.max_output_tokens = config.max_output_tokens.min(2048);
    // Hash complete server-resolved configuration; never include credentials in public output.
    let scope_hash = annotagent_image_tools::sha256(
        &serde_json::to_vec(&json!({
            "contract":"conversation-schema-consent-v1", "task":task_record,
            "model":selected.model, "provider":selected.provider, "config":config,
            "maximum_calls":1, "image_count":0,
        }))
        .map_err(ApiError::internal)?,
    );
    let preview = json!({
        "task_id":task,"model_id":selected.model.id,"model_name":selected.model.display_name,
        "remote_model":selected.model.remote_model_id,"destination":selected.provider.endpoint_summary(),
        "scope_hash":scope_hash,"maximum_calls":1,"image_count":0,"estimated_cost":null,
        "maximum_output_tokens":config.max_output_tokens,"expires_at":Utc::now()+Duration::minutes(30),
        "data_scope":"Saved goal text and existing label/schema definitions only. No image pixels.",
        "operation":"Propose annotation Schema only; does not publish, run the dataset or accept annotations.",
    });
    Ok((selected, preview))
}

pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(selection): Query<ModelSelection>,
) -> ApiResult<Json<Value>> {
    preview_scope(&state, &project, conversation, task, selection.model_id).and_then(
        |(_, mut preview)| {
            preview["project_call_limit"] = json!(
                state
                    .application
                    .project_conversation_call_limit(&project)
                    .map_err(ApiError::bad_request)?
            );
            Ok(Json(preview))
        },
    )
}

pub(super) async fn propose(
    state: State<ServerState>,
    path: AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    consent: Json<SchemaConsent>,
) -> ApiResult<Json<ConversationCallReceipt>> {
    Box::pin(propose_owned(state, path, consent, true)).await
}

/// The journey already owns a bounded background worker. Reuse identical consent
/// validation and call admission without consuming a nested worker permit.
pub(super) async fn propose_in_journey(
    state: State<ServerState>,
    path: AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    consent: Json<SchemaConsent>,
) -> ApiResult<Json<ConversationCallReceipt>> {
    Box::pin(propose_owned(state, path, consent, false)).await
}

async fn propose_owned(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(consent): Json<SchemaConsent>,
    detach: bool,
) -> ApiResult<Json<ConversationCallReceipt>> {
    if !consent.allow_unknown_cost {
        return Err(ApiError::bad_request(
            "Confirm that actual Provider cost is unknown before sending the bounded text request.",
        ));
    }
    let (selected, preview) =
        preview_scope(&state, &project, conversation, task, Some(consent.model_id))?;
    if preview["scope_hash"].as_str() != Some(consent.scope_hash.as_str())
        || consent.expires_at > Utc::now() + Duration::minutes(31)
    {
        return Err(ApiError::bad_request(
            "Schema authorization scope changed. Review the current model and data scope again; no request was sent.",
        ));
    }
    let credential = resolve_provider_credential(&state, &selected.provider)
        .await?
        .ok_or_else(|| {
            ApiError::bad_request("Provider credential is missing. No Schema request was sent.")
        })?;
    let mut config = selected
        .openai_compatible_config()
        .map_err(ApiError::bad_request)?;
    config.max_retries = 0;
    config.max_output_tokens = config.max_output_tokens.min(2048);
    let provider = OpenAiCompatibleProvider::new_with_api_key(
        config,
        Some(credential.expose_secret().to_owned()),
    )
    .map_err(ApiError::bad_request)?;
    state
        .application
        .authorize_conversation_schema_request(&project, conversation, task, &consent)
        .map_err(ApiError::bad_request)?;
    let execution = ConversationSchemaExecution {
        conversation_id: conversation,
        task_id: task,
        call_id: consent.call_id,
        remote_model: selected.model.remote_model_id,
        scope_hash: consent.scope_hash,
    };
    if !detach {
        return state
            .application
            .execute_conversation_schema(
                &project,
                &execution,
                &provider,
                CancellationToken::default(),
            )
            .await
            .map(Json)
            .map_err(ApiError::bad_request);
    }
    // A dropped HTTP response must not drop an admitted model operation. The
    // existing call receipt/cancellation service remains the execution owner.
    let permit = state.journey_workers.clone().try_acquire_owned().map_err(|_| ApiError {
        status: StatusCode::TOO_MANY_REQUESTS,
        body: json!({"error":"Background planning capacity is full. The original Schema authorization remains saved; no new model call was admitted.","code":"journey_capacity_exhausted"}),
    })?;
    let application = state.application.clone();
    tokio::spawn(async move {
        let _permit = permit;
        application
            .execute_conversation_schema(
                &project,
                &execution,
                &provider,
                CancellationToken::default(),
            )
            .await
    })
    .await
    .map_err(|_| {
        ApiError::internal(
            "Schema worker stopped unexpectedly; read the saved call receipt before retrying.",
        )
    })?
    .map(Json)
    .map_err(ApiError::bad_request)
}

pub(super) async fn receipt(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<ConversationCallReceipt>> {
    state
        .application
        .conversation_call_receipt(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("Schema call receipt not found"))
}

pub(super) async fn history(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Vec<ConversationCallReceipt>>> {
    state
        .application
        .conversation_schema_calls(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}
pub(super) async fn cancel(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<annotagent_storage::ConversationCallCancellation>> {
    state
        .application
        .cancel_conversation_schema(&project, conversation, task, call)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn cancellations(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Vec<annotagent_storage::ConversationCallCancellation>>> {
    state
        .application
        .conversation_schema_cancellations(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}
