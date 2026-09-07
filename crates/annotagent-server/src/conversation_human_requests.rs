//! Human-only HTTP commands; no inference dispatch or spending authority.
use super::*;
use annotagent_storage::{
    ConversationHumanRequest, ConversationHumanRequestInput, SampleFeedbackRevision,
};
use uuid::Uuid;

pub(super) async fn cancel(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(String, Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<ConversationHumanRequest>> {
    state
        .application
        .cancel_conversation_human_request(&project, conversation, task, id)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn list(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, Uuid, Uuid)>,
) -> ApiResult<Json<Vec<ConversationHumanRequest>>> {
    state
        .application
        .conversation_human_requests(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn create(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, Uuid, Uuid)>,
    Json(input): Json<ConversationHumanRequestInput>,
) -> ApiResult<Json<ConversationHumanRequest>> {
    if input.conversation_id != conversation || input.task_id != task {
        return Err(ApiError::bad_request(
            "Human request body does not match the route task",
        ));
    }
    state
        .application
        .create_conversation_human_request(&project, &input)
        .map(Json)
        .map_err(ApiError::bad_request)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HumanAnswer {
    answer: SampleFeedbackRevision,
}

pub(super) async fn answer(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(String, Uuid, Uuid, Uuid)>,
    Json(input): Json<HumanAnswer>,
) -> ApiResult<Json<ConversationHumanRequest>> {
    // Existing same-origin mutation middleware protects these routes. Answers update
    // Sandbox feedback only; an outbox acknowledgment is not exposed to browsers.
    state
        .application
        .answer_conversation_human_request(&project, conversation, task, id, &input.answer)
        .map_err(ApiError::bad_request)?;
    state
        .application
        .continue_conversation_correction(&project, conversation, task, id)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn resume(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(String, Uuid, Uuid, Uuid)>,
) -> ApiResult<Json<ConversationHumanRequest>> {
    state
        .application
        .continue_conversation_correction(&project, conversation, task, id)
        .map(Json)
        .map_err(ApiError::bad_request)
}
