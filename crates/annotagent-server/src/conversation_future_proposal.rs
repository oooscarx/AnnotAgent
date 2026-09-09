//! Explicit one-call consent for a future-only Schema suggestion, not an apply command.
use super::*;
use annotagent_application::{ConversationSchemaExecution, PipelineBuilderModelRuntime};
use annotagent_storage::{
    ConversationCallGrant, ConversationFeedbackAuthorization,
    ConversationFutureSchemaProposalAuthorizationRecord,
};
use chrono::{Duration, Utc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    call_id: uuid::Uuid,
    model_id: Option<ModelProfileId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Execute {
    call_id: uuid::Uuid,
}

type OwnedPath = (String, uuid::Uuid, uuid::Uuid, uuid::Uuid);

fn scope(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    feedback: uuid::Uuid,
    selection: &Selection,
    original: Option<&ConversationFeedbackAuthorization>,
) -> ApiResult<(PipelineBuilderModelRuntime, Value, Value)> {
    if selection.call_id.is_nil() {
        return Err(ApiError::bad_request(
            "Future rule proposal requires a stable call identity",
        ));
    }
    let context = state
        .application
        .future_schema_proposal_context(project, conversation, task, feedback)
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
                .ok_or_else(|| ApiError::bad_request("Previous authorization missing"))?;
            if budget.current_grant.id != previous && budget.current_grant.id != original.call_id {
                return Err(ApiError::bad_request(
                    "Another authorization replaced this future rule request",
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
                    "Task authorization changed; review the future rule scope",
                ));
            }
            budget = None;
        }
    }
    let selected = state
        .application
        .resolve_conversation_agent_model(project, conversation, selection.model_id)
        .map_err(ApiError::bad_request)?;
    let mut config = selected
        .openai_compatible_config()
        .map_err(ApiError::bad_request)?;
    config.max_retries = 0;
    config.max_output_tokens = config.max_output_tokens.min(2048);
    let maximum = budget
        .as_ref()
        .map_or(0, |value| value.current_grant.maximum_calls)
        .checked_add(1)
        .filter(|limit| *limit <= 128)
        .ok_or_else(|| ApiError::bad_request("The cumulative task authorization limit is reached; this task cannot authorize another future-rule call."))?;
    let used = budget.as_ref().map_or(0, |value| value.used_calls);
    if used >= maximum {
        return Err(ApiError::bad_request(
            "The cumulative task call limit is exhausted; no future rule request was sent",
        ));
    }
    let previous = budget.as_ref().map(|value| value.current_grant.id);
    let hash=annotagent_image_tools::sha256(&serde_json::to_vec(&json!({
        "contract":"conversation-future-schema-consent-v1","conversation":conversation,"task":task,
        "call_id":selection.call_id,"context":context,"model":selected.model,"provider":selected.provider,
        "config":config,"previous_grant_id":previous,"maximum_calls":maximum,
    })).map_err(ApiError::internal)?);
    let message = context["feedback"]["message"]["input"]["id"]
        .as_str()
        .ok_or_else(|| ApiError::bad_request("Saved feedback message identity missing"))?
        .parse()
        .map_err(ApiError::bad_request)?;
    let consent = ConversationFeedbackAuthorization {
        call_id: selection.call_id,
        message_id: message,
        model_id: selected.model.id,
        previous_grant_id: previous,
        scope_hash: hash,
        expires_at: Utc::now() + Duration::minutes(30),
        allow_unknown_cost: false,
    };
    let preview = json!({"consent":consent,"source":context["source"],"model_name":selected.model.display_name,
        "remote_model":selected.model.remote_model_id,"destination":selected.provider.endpoint_summary(),
        "maximum_calls":1,"cumulative_maximum_calls":maximum,"used_calls":used,"image_count":0,"maximum_output_tokens":config.max_output_tokens,"estimated_cost":null,
        "data_scope":"The saved feedback message, explicit future-rule scope, exact tested Schema and terminal candidate metadata only. No image pixels.",
        "operation":"Suggest a future-only Schema change for human review. Does not save a Schema, change historical annotations, publish or run images.",
        "project_call_limit":state.application.project_conversation_call_limit(project).map_err(ApiError::bad_request)?});
    Ok((selected, preview, context))
}

fn status(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    feedback: uuid::Uuid,
) -> ApiResult<Value> {
    Ok(state
        .application
        .future_schema_proposal_status(project, conversation, task, feedback)
        .map_err(ApiError::bad_request)?
        .unwrap_or(Value::Null))
}

pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, feedback)): AxumPath<OwnedPath>,
    Query(selection): Query<Selection>,
) -> ApiResult<Json<Value>> {
    if let Some(saved) = state
        .application
        .future_schema_proposal_for_feedback(&project, conversation, task, feedback)
        .map_err(ApiError::bad_request)?
    {
        if selection.call_id != saved.consent.call_id
            || selection
                .model_id
                .is_some_and(|model| model != saved.consent.model_id)
        {
            return Err(ApiError::bad_request(
                "This future-rule source already has a different saved proposal. Restore that proposal first.",
            ));
        }
        let budget = state
            .application
            .conversation_task_budget(&project, conversation, task)
            .map_err(ApiError::bad_request)?;
        let mut restored = saved.summary;
        restored["consent"] = json!(saved.consent);
        restored["source"] = json!(saved.source);
        restored["maximum_calls"] = json!(1);
        restored["cumulative_maximum_calls"] = json!(saved.grant.maximum_calls);
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
    scope(
        &state,
        &project,
        conversation,
        task,
        feedback,
        &selection,
        None,
    )
    .map(|(_, preview, _)| Json(preview))
}

pub(super) async fn get(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, feedback)): AxumPath<OwnedPath>,
) -> ApiResult<Json<Value>> {
    status(&state, &project, conversation, task, feedback).map(Json)
}

pub(super) async fn authorize(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, feedback)): AxumPath<OwnedPath>,
    Json(consent): Json<ConversationFeedbackAuthorization>,
) -> ApiResult<Json<Value>> {
    if let Some(saved) = state
        .application
        .future_schema_proposal_for_feedback(&project, conversation, task, feedback)
        .map_err(ApiError::bad_request)?
    {
        if saved.consent != consent {
            return Err(ApiError::bad_request(
                "The future rule authorization retry differs from its saved consent",
            ));
        }
        return status(&state, &project, conversation, task, feedback).map(Json);
    }
    if !consent.allow_unknown_cost
        || consent.expires_at <= Utc::now()
        || consent.expires_at > Utc::now() + Duration::minutes(31)
    {
        return Err(ApiError::bad_request(
            "Confirm one bounded future-rule text request and its unknown cost before authorizing it",
        ));
    }
    let selection = Selection {
        call_id: consent.call_id,
        model_id: Some(consent.model_id),
    };
    let (_, preview, context) = scope(
        &state,
        &project,
        conversation,
        task,
        feedback,
        &selection,
        Some(&consent),
    )?;
    if preview["consent"]["scope_hash"] != consent.scope_hash
        || preview["consent"]["previous_grant_id"] != json!(consent.previous_grant_id)
        || preview["consent"]["message_id"] != json!(consent.message_id)
    {
        return Err(ApiError::bad_request(
            "The model, tested rules or authorization scope changed. No model call was sent.",
        ));
    }
    let record = ConversationFutureSchemaProposalAuthorizationRecord {
        grant: ConversationCallGrant {
            id: consent.call_id,
            task_id: task,
            scope_hash: consent.scope_hash.clone(),
            maximum_calls: u32::try_from(preview["cumulative_maximum_calls"].as_u64().unwrap_or(0))
                .map_err(ApiError::internal)?,
            expires_at: consent.expires_at,
        },
        consent,
        source: serde_json::from_value(context["source"].clone()).map_err(ApiError::internal)?,
        context,
        summary: json!({"model_name":preview["model_name"],"remote_model":preview["remote_model"],"destination":preview["destination"],"data_scope":preview["data_scope"],"operation":preview["operation"],"maximum_output_tokens":preview["maximum_output_tokens"]}),
    };
    state
        .application
        .authorize_future_schema_proposal(&project, conversation, task, &record)
        .map_err(ApiError::bad_request)?;
    status(&state, &project, conversation, task, feedback).map(Json)
}

pub(super) async fn execute(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, feedback)): AxumPath<OwnedPath>,
    Json(input): Json<Execute>,
) -> ApiResult<Json<Value>> {
    let saved = state
        .application
        .future_schema_proposal_for_feedback(&project, conversation, task, feedback)
        .map_err(ApiError::bad_request)?
        .ok_or_else(|| ApiError::not_found("Save explicit future rule authorization first"))?;
    if input.call_id != saved.consent.call_id {
        return Err(ApiError::bad_request(
            "Execution does not match the saved future-rule call",
        ));
    }
    let current = status(&state, &project, conversation, task, feedback)?;
    if !current["receipt"].is_null() || current["cancelled"] == true {
        return Ok(Json(current));
    }
    if saved.consent.expires_at <= Utc::now() {
        return Err(ApiError::bad_request(
            "The saved future rule authorization expired. No request was sent.",
        ));
    }
    let selection = Selection {
        call_id: input.call_id,
        model_id: Some(saved.consent.model_id),
    };
    let (selected, preview, context) = scope(
        &state,
        &project,
        conversation,
        task,
        feedback,
        &selection,
        Some(&saved.consent),
    )?;
    if preview["consent"]["scope_hash"] != saved.consent.scope_hash || context != saved.context {
        return Err(ApiError::bad_request(
            "The future rule scope changed after authorization. No request was sent.",
        ));
    }
    let credential=resolve_provider_credential(&state,&selected.provider).await?.ok_or_else(||ApiError::bad_request("Provider credential is missing. The saved authorization is retained; no request was sent."))?;
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
    let permit=state.journey_workers.clone().try_acquire_owned().map_err(|_|ApiError{status:StatusCode::TOO_MANY_REQUESTS,body:json!({"error":"Background planning capacity is full. Future rule authorization remains saved; no call was admitted.","code":"journey_capacity_exhausted"})})?;
    let (_, rechecked, _) = scope(
        &state,
        &project,
        conversation,
        task,
        feedback,
        &selection,
        Some(&saved.consent),
    )?;
    if rechecked["consent"]["scope_hash"] != saved.consent.scope_hash {
        return Err(ApiError::bad_request(
            "Future rule scope changed before dispatch; no model call was sent",
        ));
    }
    let execution = ConversationSchemaExecution {
        conversation_id: conversation,
        task_id: task,
        call_id: input.call_id,
        remote_model: selected.model.remote_model_id,
        scope_hash: saved.consent.scope_hash,
    };
    let app = state.application.clone();
    let worker_project = project.clone();
    let outcome = tokio::spawn(async move {
        let _permit = permit;
        app.execute_future_schema_proposal(
            &worker_project,
            &execution,
            feedback,
            &provider,
            CancellationToken::default(),
        )
        .await
    })
    .await;
    let restored = status(&state, &project, conversation, task, feedback)?;
    if !restored["receipt"].is_null() || restored["cancelled"] == true {
        return Ok(Json(restored));
    }
    outcome
        .map_err(|_| {
            ApiError::internal(
                "Future rule worker stopped. Restore its saved receipt before retrying.",
            )
        })?
        .map_err(ApiError::bad_request)?;
    Ok(Json(restored))
}
