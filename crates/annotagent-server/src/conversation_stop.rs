//! Explicit control commands. Reading their journal never dispatches work or cancellation.
use super::{ApiError, ApiResult, ServerState};
use annotagent_storage::{
    ConversationMessageInput, ConversationStopRequest, ConversationStopTargetRef,
};
use axum::{
    Json,
    extract::{Path as AxumPath, State},
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelectStop {
    target: ConversationStopTargetRef,
}

fn view(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    record: ConversationStopRequest,
    dispatch_error: Option<String>,
) -> ApiResult<Json<Value>> {
    let observation = state
        .application
        .conversation_stop_observation(project, conversation, record.message.input.id)
        .map_err(ApiError::bad_request)?;
    let mut response = serde_json::to_value(record).map_err(ApiError::internal)?;
    response["observation"] = json!(observation);
    response["normalized_state"] = json!(match observation.as_ref().map(|v| v.state) {
        Some("cancel_pending") => Some("stopping"),
        Some("unknown") => Some("outcome_unknown"),
        Some("cancelled") => Some("interrupted"),
        // "finished" does not prove success; keep the terminal outcome unspecified.
        Some("finished") => None,
        _ => Some("idle"),
    });
    response["resume"] = json!({"available":false,"reason":"A stop receipt is not a resumable checkpoint. Use the exact paused Run/Batch or saved HumanRequest; never automatically resend an unknown Provider call."});
    response["dispatch_error"] = dispatch_error.map_or(Value::Null, Value::String);
    Ok(Json(response))
}

async fn signal(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    message: uuid::Uuid,
) -> Option<String> {
    match state.application.signal_conversation_stop(project, conversation, message) {
        Ok(samples) => {
            let cancellations = state.sample_cancellations.read().await;
            for sample in samples {
                if let Some(token) = cancellations.get(&sample) { token.cancel(); }
            }
            None
        }
        Err(_) => Some("The stop request is saved, but signaling the local worker failed. Retry this same stop command; no new target will be selected.".into()),
    }
}

pub(super) async fn begin(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
    Json(input): Json<ConversationMessageInput>,
) -> ApiResult<Json<Value>> {
    let record = state
        .application
        .begin_conversation_stop(&project, conversation, &input)
        .map_err(ApiError::bad_request)?;
    let dispatch_error = signal(&state, &project, conversation, input.id).await;
    view(&state, &project, conversation, record, dispatch_error)
}

pub(super) async fn select(
    State(state): State<ServerState>,
    AxumPath((project, conversation, message)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<SelectStop>,
) -> ApiResult<Json<Value>> {
    let record = state
        .application
        .select_conversation_stop(&project, conversation, message, &input.target)
        .map_err(ApiError::bad_request)?;
    let dispatch_error = signal(&state, &project, conversation, message).await;
    view(&state, &project, conversation, record, dispatch_error)
}

pub(super) async fn get(
    State(state): State<ServerState>,
    AxumPath((project, conversation, message)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    match state
        .application
        .conversation_stop_request(&project, conversation, message)
        .map_err(ApiError::bad_request)?
    {
        Some(record) => view(&state, &project, conversation, record, None),
        None => Ok(Json(Value::Null)),
    }
}
