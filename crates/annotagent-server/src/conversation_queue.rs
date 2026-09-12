//! Explicit bounded semantic planning for a saved supplement. No automatic dispatch.
use super::*;
use annotagent_application::{ConversationSchemaExecution, PipelineBuilderModelRuntime};
use annotagent_storage::{ConversationCallGrant, QueuedPlanningAuthorization};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Consent {
    call_id: Uuid,
    model_id: ModelProfileId,
    scope_hash: String,
    request_hash: String,
    previous_grant_id: Option<Uuid>,
    maximum_calls: u32,
    expires_at: DateTime<Utc>,
    allow_unknown_cost: bool,
}

pub(super) async fn authorization(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, message)): AxumPath<(String, Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<Option<Consent>>> {
    let queued = state
        .application
        .queued_conversation_message(&project, conversation, task, message)
        .map_err(ApiError::bad_request)?;
    let Some(call) = queued.planning_call_id else {
        return Ok(Json(None));
    };
    let saved = state
        .application
        .queued_planning_authorization(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?
        .ok_or_else(|| ApiError::internal("Queued authorization is missing"))?;
    Ok(Json(Some(Consent {
        call_id: call,
        model_id: saved.model_id,
        scope_hash: saved.grant.scope_hash,
        request_hash: saved.request_hash,
        previous_grant_id: saved.previous_grant_id,
        maximum_calls: saved.grant.maximum_calls,
        expires_at: saved.grant.expires_at,
        allow_unknown_cost: true,
    })))
}

fn admission_error(error: anyhow::Error) -> ApiError {
    if let Some(annotagent_storage::StorageError::ConversationContract { code, .. }) =
        error.downcast_ref::<annotagent_storage::StorageError>()
        && matches!(
            *code,
            "human_input_pending" | "schema_clarification_pending"
        )
    {
        return ApiError {
            status: StatusCode::CONFLICT,
            body: json!({"status":409,"code":code,"error":error.to_string(),"admitted":false,"suggested_action":"answer_human_then_retry_same_command"}),
        };
    }
    ApiError::bad_request(error)
}

fn scope(
    state: &ServerState,
    project: &str,
    conversation: Uuid,
    task: Uuid,
    message: Uuid,
    model: Option<ModelProfileId>,
    saved: Option<&QueuedPlanningAuthorization>,
) -> ApiResult<(PipelineBuilderModelRuntime, Value)> {
    let queued = state
        .application
        .queued_conversation_message(project, conversation, task, message)
        .map_err(ApiError::bad_request)?;
    if queued.cancelled_at.is_some() {
        return Err(ApiError::bad_request("Queued instruction is cancelled"));
    }
    state
        .application
        .check_queued_schema_admission(
            project,
            conversation,
            task,
            queued.planning_call_id.unwrap_or_default(),
        )
        .map_err(admission_error)?;
    let frozen = queued
        .receipt
        .agent_model
        .as_ref()
        .and_then(|v| v.model_profile_id);
    if model.zip(frozen).is_some_and(|(a, b)| a != b) {
        return Err(ApiError::bad_request("Use the Agent model frozen at Send"));
    }
    let selected = state
        .application
        .resolve_conversation_message_model(project, conversation, message, model)
        .map_err(ApiError::bad_request)?;
    let (config, request_config) = super::conversation_schema::schema_stage_config(&selected)?;
    let request_hash = state
        .application
        .queued_schema_request_hash(
            project,
            conversation,
            task,
            message,
            &selected.model.remote_model_id,
            &request_config,
        )
        .map_err(ApiError::bad_request)?;
    let budget = state
        .application
        .optional_conversation_builder_budget(project, conversation, task)
        .map_err(ApiError::bad_request)?;
    let (previous, maximum) = if let Some(saved) = saved {
        (saved.previous_grant_id, saved.grant.maximum_calls)
    } else {
        (
            budget.as_ref().map(|v| v.current_grant.id),
            budget.as_ref().map_or(1, |v| {
                v.current_grant
                    .maximum_calls
                    .max(v.used_calls.saturating_add(1))
            }),
        )
    };
    let scope_hash=annotagent_image_tools::sha256(&serde_json::to_vec(&json!({"contract":"queued-schema-consent-v1","request_hash":request_hash,"model":selected.model,"provider":selected.provider,"config":config,"request_config":request_config,"previous_grant_id":previous,"maximum_calls":maximum})).map_err(ApiError::internal)?);
    let preview = json!({
        "message_id":message,"task_id":task,"model_id":selected.model.id,"model_name":selected.model.display_name,
        "destination":selected.provider.endpoint_summary(),"scope_hash":scope_hash,"request_hash":request_hash,
        "previous_grant_id":previous,"maximum_calls":maximum,"used_calls":budget.map_or(0,|v|v.used_calls),
        "new_request_limit":1,"image_count":0,"estimated_cost":null,"expires_at":Utc::now()+Duration::minutes(30),
        "maximum_output_tokens":request_config.maximum_output_tokens,"response_mode":request_config.response_mode,
        "thinking":{"parameter":request_config.thinking_parameter,"value":request_config.thinking_value},
        "data_scope":"Original goal and this saved supplement, plus existing Project Schema. No image pixels or other conversation history.",
        "operation":"Propose a new semantic Schema Draft; does not modify the Workflow, publish, infer on images or accept annotations."
    });
    Ok((selected, preview))
}

pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, message)): AxumPath<(String, Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<Value>> {
    scope(&state, &project, conversation, task, message, None, None)
        .map(|(_, preview)| Json(preview))
}

pub(super) async fn propose(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, message)): AxumPath<(String, Uuid, Uuid, Uuid)>,
    Json(consent): Json<Consent>,
) -> ApiResult<Json<annotagent_storage::ConversationCallReceipt>> {
    if !consent.allow_unknown_cost {
        return Err(ApiError::bad_request(
            "Confirm unknown Provider cost before this text-only planning call",
        ));
    }
    let input = QueuedPlanningAuthorization {
        conversation_id: conversation,
        message_id: message,
        previous_grant_id: consent.previous_grant_id,
        grant: ConversationCallGrant {
            id: consent.call_id,
            task_id: task,
            scope_hash: consent.scope_hash.clone(),
            maximum_calls: consent.maximum_calls,
            expires_at: consent.expires_at,
        },
        model_id: consent.model_id,
        request_hash: consent.request_hash.clone(),
    };
    let saved = state
        .application
        .queued_planning_authorization(&project, conversation, task, consent.call_id)
        .map_err(ApiError::bad_request)?;
    if let Some(saved) = &saved {
        if saved != &input {
            return Err(ApiError::bad_request(
                "Queued authorization retry changed its saved scope",
            ));
        }
        if let Some(receipt) = state
            .application
            .conversation_call_receipt(&project, conversation, task, consent.call_id)
            .map_err(ApiError::bad_request)?
        {
            // Recovery must not resolve credentials, extend a grant or call a model.
            return Ok(Json(receipt));
        }
    }
    let (selected, preview) = scope(
        &state,
        &project,
        conversation,
        task,
        message,
        Some(consent.model_id),
        saved.as_ref(),
    )?;
    if preview["scope_hash"].as_str() != Some(&consent.scope_hash)
        || preview["request_hash"].as_str() != Some(&consent.request_hash)
        || preview["maximum_calls"].as_u64() != Some(u64::from(consent.maximum_calls))
        || serde_json::to_value(consent.previous_grant_id).map_err(ApiError::internal)?
            != preview["previous_grant_id"]
        || consent.expires_at > Utc::now() + Duration::minutes(31)
        || consent.expires_at <= Utc::now()
    {
        return Err(ApiError::bad_request(
            "Queued planning scope or budget changed; review the preview again",
        ));
    }
    let credential = resolve_provider_credential(&state, &selected.provider)
        .await?
        .ok_or_else(|| {
            ApiError::bad_request("Provider credential is missing; no request was sent")
        })?;
    let (config, request_config) = super::conversation_schema::schema_stage_config(&selected)?;
    let attempt_observer = state
        .application
        .task_model_attempt_observer(&project, conversation, task, &selected)
        .map_err(admission_error)?;
    let provider = OpenAiCompatibleProvider::new_with_api_key(
        config,
        Some(credential.expose_secret().to_owned()),
    )
    .map_err(ApiError::bad_request)?
    .with_attempt_observer(attempt_observer);
    state
        .application
        .authorize_queued_schema(&project, &input)
        .map_err(admission_error)?;
    let permit=state.journey_workers.clone().try_acquire_owned().map_err(|_|ApiError{status:StatusCode::TOO_MANY_REQUESTS,body:json!({"error":"Planning capacity full. Authorization is saved; retry the same request later."})})?;
    let application = state.application.clone();
    let execution = ConversationSchemaExecution {
        conversation_id: conversation,
        task_id: task,
        call_id: consent.call_id,
        remote_model: selected.model.remote_model_id,
        scope_hash: consent.scope_hash,
    };
    tokio::spawn(async move {
        let _permit = permit;
        application
            .execute_conversation_schema_with_config(
                &project,
                &execution,
                &request_config,
                &provider,
                CancellationToken::default(),
            )
            .await
    })
    .await
    .map_err(|_| ApiError::internal("Worker interrupted; inspect the saved call before retrying"))?
    .map(Json)
    .map_err(admission_error)
}
