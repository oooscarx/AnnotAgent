//! Human-only image-class corrections; same-origin mutation protection, no model execution.
use super::*;
use annotagent_storage::{
    ConversationImageClassAnswerInput, ConversationImageClassCreateInput,
    ConversationImageClassReview,
};
use uuid::Uuid;

type Subject = (String, Uuid, Uuid, Uuid);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    target_label: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Empty {}

pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, feedback)): AxumPath<Subject>,
    Query(selection): Query<Selection>,
) -> ApiResult<Json<Value>> {
    let scope = state
        .application
        .conversation_image_class_scope(
            &project,
            conversation,
            task,
            feedback,
            selection.target_label.as_deref(),
        )
        .map_err(ApiError::bad_request)?;
    Ok(Json(
        json!({"scope_digest":scope.digest().map_err(ApiError::internal)?,"scope":scope}),
    ))
}

pub(super) async fn for_feedback(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, feedback)): AxumPath<Subject>,
) -> ApiResult<Json<Option<ConversationImageClassReview>>> {
    state
        .application
        .conversation_image_class_review_for_feedback(&project, conversation, task, feedback)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn get(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<Subject>,
) -> ApiResult<Json<ConversationImageClassReview>> {
    state
        .application
        .conversation_image_class_review(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("Class review not found in this task"))
}

pub(super) async fn create(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, feedback)): AxumPath<Subject>,
    Json(input): Json<ConversationImageClassCreateInput>,
) -> ApiResult<Json<ConversationImageClassReview>> {
    if input.feedback_call_id != feedback {
        return Err(ApiError::bad_request(
            "The class review body does not match its feedback route",
        ));
    }
    state
        .application
        .create_conversation_image_class_review(&project, conversation, task, &input)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn answer(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<Subject>,
    Json(input): Json<ConversationImageClassAnswerInput>,
) -> ApiResult<Json<ConversationImageClassReview>> {
    state
        .application
        .answer_conversation_image_class_review(&project, conversation, task, id, &input)
        .map_err(ApiError::bad_request)?;
    state
        .application
        .continue_conversation_image_class_review(&project, conversation, task, id)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn resume(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<Subject>,
    Json(_): Json<Empty>,
) -> ApiResult<Json<ConversationImageClassReview>> {
    state
        .application
        .continue_conversation_image_class_review(&project, conversation, task, id)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn cancel(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<Subject>,
    Json(_): Json<Empty>,
) -> ApiResult<Json<ConversationImageClassReview>> {
    state
        .application
        .cancel_conversation_image_class_review(&project, conversation, task, id)
        .map(Json)
        .map_err(ApiError::bad_request)
}
