//! Explicit consent boundary for one text-only Schema proposal. No publish/start.
use super::*;
use annotagent_application::{ConversationSchemaExecution, PipelineBuilderModelRuntime};
use annotagent_storage::{ConversationCallGrant, ConversationCallReceipt};
use chrono::{DateTime, Duration, Utc};

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
) -> ApiResult<Json<annotagent_storage::ConversationSchemaDraft>> {
    state
        .application
        .save_human_conversation_schema_draft(
            &project,
            conversation,
            task,
            input.request_id,
            &input.decision,
        )
        .map(Json)
        .map_err(ApiError::bad_request)
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaConsent {
    call_id: uuid::Uuid,
    model_id: ModelProfileId,
    scope_hash: String,
    expires_at: DateTime<Utc>,
    allow_unknown_cost: bool,
}

fn preview_scope(
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
        .resolve_pipeline_builder_model(project, model_id)
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
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(consent): Json<SchemaConsent>,
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
    let grant = ConversationCallGrant {
        id: consent.call_id,
        task_id: task,
        scope_hash: consent.scope_hash.clone(),
        maximum_calls: 1,
        expires_at: consent.expires_at,
    };
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
        .authorize_conversation_task_calls(&project, conversation, &grant)
        .map_err(ApiError::bad_request)?;
    let execution = ConversationSchemaExecution {
        conversation_id: conversation,
        task_id: task,
        call_id: consent.call_id,
        remote_model: selected.model.remote_model_id,
        scope_hash: consent.scope_hash,
    };
    state
        .application
        .execute_conversation_schema(
            &project,
            &execution,
            &provider,
            CancellationToken::default(),
        )
        .await
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
