//! One authorized text interpretation of a frozen sample-candidate message.
use super::*;
use annotagent_application::{
    ConversationFeedbackContext, ConversationSchemaExecution, PipelineBuilderModelRuntime,
};
use annotagent_storage::{
    ConversationCallGrant, ConversationFeedbackAuthorization,
    ConversationFeedbackAuthorizationRecord,
};
use chrono::{Duration, Utc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    #[serde(rename = "message_id")]
    message: uuid::Uuid,
    #[serde(rename = "call_id")]
    call: uuid::Uuid,
    #[serde(rename = "model_id")]
    model: Option<ModelProfileId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MessageQuery {
    message_id: uuid::Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Execute {}

fn scope(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    selection: &Selection,
    original: Option<&ConversationFeedbackAuthorization>,
) -> ApiResult<(
    PipelineBuilderModelRuntime,
    Value,
    ConversationFeedbackContext,
)> {
    if selection.call.is_nil() || selection.message.is_nil() {
        return Err(ApiError::bad_request(
            "Feedback requires stable message and call identities",
        ));
    }
    let context = state
        .application
        .conversation_feedback_context(project, conversation, task, selection.message)
        .map_err(ApiError::bad_request)?;
    let mut budget = state
        .application
        .optional_conversation_builder_budget(project, conversation, task)
        .map_err(ApiError::bad_request)?;
    if budget.as_ref().is_some_and(|budget| budget.revoked) {
        return Err(ApiError::bad_request("Task authorization was revoked"));
    }
    if let Some(original) = original {
        if let Some(previous) = original.previous_grant_id {
            let budget = budget
                .as_mut()
                .ok_or_else(|| ApiError::bad_request("Previous authorization is missing"))?;
            if budget.current_grant.id != previous && budget.current_grant.id != original.call_id {
                return Err(ApiError::bad_request(
                    "Another authorization replaced this feedback request",
                ));
            }
            budget.current_grant = state
                .application
                .conversation_builder_grant(project, conversation, task, previous)
                .map_err(ApiError::bad_request)?;
        } else {
            if budget
                .as_ref()
                .is_some_and(|budget| budget.current_grant.id != original.call_id)
            {
                return Err(ApiError::bad_request(
                    "Task authorization changed; review the saved feedback scope",
                ));
            }
            budget = None;
        }
    }
    let pending = state
        .application
        .conversation_human_requests(project, conversation, task)
        .map_err(ApiError::bad_request)?;
    if pending.iter().any(|request| {
        request.status == annotagent_storage::ConversationHumanRequestStatus::Pending
    }) {
        return Err(ApiError::bad_request(
            "This task is waiting for a saved human request. Open that request in the canvas; no feedback model call was sent.",
        ));
    }
    let selected = state
        .application
        .resolve_pipeline_builder_model(project, selection.model)
        .map_err(ApiError::bad_request)?;
    let mut config = selected
        .openai_compatible_config()
        .map_err(ApiError::bad_request)?;
    config.max_retries = 0;
    config.max_output_tokens = config.max_output_tokens.min(2048);
    let maximum = budget
        .as_ref()
        .map_or(0, |budget| budget.current_grant.maximum_calls)
        .saturating_add(1)
        .min(128);
    let used = budget.as_ref().map_or(0, |budget| budget.used_calls);
    if used >= maximum {
        return Err(ApiError::bad_request(
            "Task cumulative call limit exhausted; no feedback call was sent",
        ));
    }
    let previous = budget.as_ref().map(|budget| budget.current_grant.id);
    let hash = annotagent_image_tools::sha256(&serde_json::to_vec(&json!({
        "contract":"conversation-feedback-consent-v1", "conversation":conversation,"task":task,
        "call_id":selection.call,"context":context,"model":selected.model,"provider":selected.provider,
        "config":config,"previous_grant_id":previous,"maximum_calls":maximum,
    })).map_err(ApiError::internal)?);
    let consent = ConversationFeedbackAuthorization {
        call_id: selection.call,
        message_id: selection.message,
        model_id: selected.model.id,
        previous_grant_id: previous,
        scope_hash: hash,
        expires_at: Utc::now() + Duration::minutes(30),
        allow_unknown_cost: false,
    };
    let preview = json!({"consent":consent,"model_name":selected.model.display_name,
        "remote_model":selected.model.remote_model_id,"destination":selected.provider.endpoint_summary(),
        "maximum_calls":1,"cumulative_maximum_calls":maximum,"used_calls":used,"image_count":0,
        "maximum_output_tokens":config.max_output_tokens,"estimated_cost":null,
        "data_scope":"This saved message and its exact terminal candidate metadata only. No image pixels; the text model cannot visually verify the result.",
        "operation":"Interpret this candidate feedback only. Does not change annotations, labels, workflows, publish or run images.",
        "project_call_limit":state.application.project_conversation_call_limit(project).map_err(ApiError::bad_request)?,
    });
    Ok((selected, preview, context))
}

pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(selection): Query<Selection>,
) -> ApiResult<Json<Value>> {
    if let Some(record) = state
        .application
        .conversation_feedback_authorization(&project, conversation, task, selection.call)
        .map_err(ApiError::bad_request)?
    {
        if record.consent.message_id != selection.message
            || selection
                .model
                .is_some_and(|id| id != record.consent.model_id)
        {
            return Err(ApiError::bad_request(
                "Saved feedback preview belongs to another message or model",
            ));
        }
        let budget = state
            .application
            .conversation_task_budget(&project, conversation, task)
            .map_err(ApiError::bad_request)?;
        let mut restored = record.summary.clone();
        restored["consent"] = json!(record.consent);
        restored["maximum_calls"] = json!(1);
        restored["cumulative_maximum_calls"] = json!(record.grant.maximum_calls);
        restored["used_calls"] = json!(budget.planning_reserved_calls);
        restored["image_count"] = json!(0);
        restored["estimated_cost"] = Value::Null;
        restored["project_call_limit"] = json!(
            state
                .application
                .project_conversation_call_limit(&project)
                .map_err(ApiError::bad_request)?
        );
        return Ok(Json(restored));
    }
    scope(&state, &project, conversation, task, &selection, None)
        .map(|(_, preview, _)| Json(preview))
}

pub(super) async fn for_message(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(query): Query<MessageQuery>,
) -> ApiResult<Json<Value>> {
    let saved = state
        .application
        .conversation_feedback_for_message(&project, conversation, task, query.message_id)
        .map_err(ApiError::bad_request)?;
    saved.map_or(Ok(Json(Value::Null)), |record| {
        status_value(&state, &project, conversation, task, record.consent.call_id).map(Json)
    })
}

fn status_value(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    call: uuid::Uuid,
) -> ApiResult<Value> {
    state
        .application
        .conversation_feedback_status(project, conversation, task, call)
        .map_err(ApiError::bad_request)
}

pub(super) async fn status(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<Value>> {
    status_value(&state, &project, conversation, task, call).map(Json)
}

pub(super) async fn future_schema(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<annotagent_application::ConversationFutureSchemaView>> {
    state
        .application
        .conversation_future_schema(&project, conversation, task, call)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn save_future_schema(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(request): Json<annotagent_application::ConversationFutureSchemaRequest>,
) -> ApiResult<Json<annotagent_application::ConversationFutureSchemaView>> {
    state
        .application
        .save_conversation_future_schema(&project, conversation, task, call, &request)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn authorize(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(consent): Json<ConversationFeedbackAuthorization>,
) -> ApiResult<Json<Value>> {
    // Lost acknowledgement recovery must precede re-reading credentials, pixels,
    // current model availability or expiry. It can only read exact saved consent.
    if let Some(saved) = state
        .application
        .conversation_feedback_authorization(&project, conversation, task, consent.call_id)
        .map_err(ApiError::bad_request)?
    {
        if saved.consent != consent {
            return Err(ApiError::bad_request(
                "Feedback authorization retry changed the original consent",
            ));
        }
        return status_value(&state, &project, conversation, task, consent.call_id).map(Json);
    }
    if !consent.allow_unknown_cost
        || consent.expires_at <= Utc::now()
        || consent.expires_at > Utc::now() + Duration::minutes(31)
    {
        return Err(ApiError::bad_request(
            "Confirm this bounded text request and its unknown cost before saving authorization",
        ));
    }
    let selection = Selection {
        message: consent.message_id,
        call: consent.call_id,
        model: Some(consent.model_id),
    };
    let (_, preview, context) = scope(
        &state,
        &project,
        conversation,
        task,
        &selection,
        Some(&consent),
    )?;
    if preview["consent"]["scope_hash"] != consent.scope_hash
        || preview["consent"]["previous_grant_id"] != json!(consent.previous_grant_id)
    {
        return Err(ApiError::bad_request(
            "Feedback model, candidate or authorization scope changed. Review again; no model call was sent.",
        ));
    }
    let record = ConversationFeedbackAuthorizationRecord {
        grant: ConversationCallGrant {
            id: consent.call_id,
            task_id: task,
            scope_hash: consent.scope_hash.clone(),
            maximum_calls: u32::try_from(preview["cumulative_maximum_calls"].as_u64().unwrap_or(0))
                .map_err(ApiError::internal)?,
            expires_at: consent.expires_at,
        },
        consent,
        context: serde_json::to_value(context).map_err(ApiError::internal)?,
        summary: json!({"model_name":preview["model_name"],"remote_model":preview["remote_model"],
            "destination":preview["destination"],"data_scope":preview["data_scope"],"operation":preview["operation"],
            "maximum_output_tokens":preview["maximum_output_tokens"]}),
    };
    let saved = state
        .application
        .authorize_conversation_feedback(&project, conversation, task, &record)
        .map_err(ApiError::bad_request)?;
    status_value(&state, &project, conversation, task, saved.consent.call_id).map(Json)
}

pub(super) async fn execute(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(_input): Json<Execute>,
) -> ApiResult<Json<Value>> {
    let saved = state
        .application
        .conversation_feedback_authorization(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?
        .ok_or_else(|| ApiError::not_found("Save explicit feedback authorization first"))?;
    let current = status_value(&state, &project, conversation, task, call)?;
    if !current["receipt"].is_null() || current["cancelled"] == true {
        return Ok(Json(current));
    }
    if saved.consent.expires_at <= Utc::now() {
        return Err(ApiError::bad_request(
            "Saved feedback authorization expired. No model request was sent.",
        ));
    }
    let selection = Selection {
        message: saved.consent.message_id,
        call,
        model: Some(saved.consent.model_id),
    };
    let (selected, preview, context) = scope(
        &state,
        &project,
        conversation,
        task,
        &selection,
        Some(&saved.consent),
    )?;
    if preview["consent"]["scope_hash"] != saved.consent.scope_hash
        || serde_json::to_value(&context).map_err(ApiError::internal)? != saved.context
    {
        return Err(ApiError::bad_request(
            "Feedback scope changed after authorization. Saved message and consent remain available; no model call was sent.",
        ));
    }
    let credential = resolve_provider_credential(&state, &selected.provider).await?
        .ok_or_else(|| ApiError::bad_request("Provider credential is missing. Authorization is saved; no model request was sent."))?;
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
    let permit = state.journey_workers.clone().try_acquire_owned().map_err(|_| ApiError {
        status:StatusCode::TOO_MANY_REQUESTS,
        body:json!({"error":"Background planning capacity is full. Feedback authorization remains saved; no new call was admitted.","code":"journey_capacity_exhausted"}),
    })?;
    // Model/subject/permission may change while the credential lookup is pending.
    let (_, rechecked, _) = scope(
        &state,
        &project,
        conversation,
        task,
        &selection,
        Some(&saved.consent),
    )?;
    if rechecked["consent"]["scope_hash"] != saved.consent.scope_hash {
        return Err(ApiError::bad_request(
            "Feedback scope changed before dispatch; no model call was sent",
        ));
    }
    let execution = ConversationSchemaExecution {
        conversation_id: conversation,
        task_id: task,
        call_id: call,
        remote_model: selected.model.remote_model_id,
        scope_hash: saved.consent.scope_hash,
    };
    let application = state.application.clone();
    let worker_project = project.clone();
    let outcome = tokio::spawn(async move {
        let _permit = permit;
        application
            .execute_conversation_feedback(
                &worker_project,
                &execution,
                &context,
                &provider,
                CancellationToken::default(),
            )
            .await
    })
    .await;
    let status = status_value(&state, &project, conversation, task, call)?;
    // Concurrent identical execution loses ledger admission but observes the same
    // running receipt. Unknown outcomes are displayed, never reissued.
    if !status["receipt"].is_null() || status["cancelled"] == true {
        return Ok(Json(status));
    }
    outcome
        .map_err(|_| {
            ApiError::internal("Feedback worker stopped. Read the saved request before retrying.")
        })?
        .map_err(ApiError::bad_request)?;
    Ok(Json(status))
}

pub(super) async fn human_request(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(_input): Json<Execute>,
) -> ApiResult<Json<Option<annotagent_storage::ConversationHumanRequest>>> {
    state
        .application
        .conversation_feedback_authorization(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?
        .ok_or_else(|| ApiError::not_found("Feedback authorization not found"))?;
    state
        .application
        .prepare_conversation_feedback_request(&project, conversation, task, call)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn answer_scope(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(input): Json<annotagent_storage::ConversationFeedbackScopeAnswerInput>,
) -> ApiResult<Json<annotagent_storage::ConversationFeedbackScopeAnswer>> {
    state
        .application
        .answer_conversation_feedback_scope(&project, conversation, task, call, &input)
        .map(Json)
        .map_err(ApiError::bad_request)
}
