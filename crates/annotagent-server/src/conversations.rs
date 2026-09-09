//! Journal endpoints: persisting a user message never grants inference authority.
use super::*;
use annotagent_storage::{ConversationMessage, ConversationMessageInput};

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct QueuePage {
    #[serde(default)]
    after: i64,
}

pub(super) async fn message_queue(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(page): Query<QueuePage>,
) -> ApiResult<Json<Vec<annotagent_storage::ConversationQueuedMessage>>> {
    state
        .application
        .project_conversation_message_queue(&project, conversation, task, page.after)
        .map(Json)
        .map_err(ApiError::conversation)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CancelQueuedMessage {}

pub(super) async fn cancel_queued_message(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, message)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(_input): Json<CancelQueuedMessage>,
) -> ApiResult<Json<annotagent_storage::ConversationQueuedMessage>> {
    state
        .application
        .cancel_project_queued_message(&project, conversation, task, message)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn agent_model(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
) -> ApiResult<Json<annotagent_storage::ConversationAgentModel>> {
    state
        .application
        .project_conversation_agent_model(&project, conversation)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn select_agent_model(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
    Json(input): Json<annotagent_storage::SelectConversationAgentModel>,
) -> ApiResult<Json<annotagent_storage::ConversationAgentModel>> {
    state
        .application
        .select_project_conversation_agent_model(&project, conversation, &input)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn call_limit(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
) -> ApiResult<Json<annotagent_storage::ProjectCallLimit>> {
    state
        .application
        .project_conversation_call_limit(&project)
        .map(Json)
        .map_err(ApiError::conversation)
}
pub(super) async fn set_call_limit(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
    Json(input): Json<annotagent_storage::ProjectCallLimitInput>,
) -> ApiResult<Json<annotagent_storage::ProjectCallLimit>> {
    state
        .application
        .set_project_conversation_call_limit(&project, &input)
        .map(Json)
        .map_err(ApiError::conversation)
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct MessagePage {
    #[serde(default)]
    after: i64,
    before: Option<i64>,
    #[serde(default)]
    latest: bool,
    #[serde(default)]
    first_goal: bool,
    limit: Option<u32>,
}

pub(super) async fn create(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = state
        .application
        .create_project_conversation(&project)
        .map_err(ApiError::conversation)?;
    Ok(Json(json!({ "conversation_id": id })))
}

pub(super) async fn current(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = state
        .application
        .project_conversation(&project)
        .map_err(ApiError::conversation)?;
    Ok(Json(json!({ "conversation_id": id })))
}

pub(super) async fn messages(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
    Query(page): Query<MessagePage>,
) -> ApiResult<Json<Vec<ConversationMessage>>> {
    if page.first_goal {
        if page.after != 0 || page.before.is_some() || page.latest || page.limit.is_some() {
            return Err(ApiError::bad_request(
                "First goal cannot be combined with message paging",
            ));
        }
        return state
            .application
            .project_conversation_first_goal(&project, conversation)
            .map(|goal| Json(goal.into_iter().collect()))
            .map_err(ApiError::conversation);
    }
    if page.latest || page.before.is_some() {
        if page.after != 0 || (page.latest && page.before.is_some()) {
            return Err(ApiError::bad_request("Select one message paging direction"));
        }
        return state
            .application
            .project_conversation_message_history(
                &project,
                conversation,
                page.before,
                page.limit.unwrap_or(100),
            )
            .map(Json)
            .map_err(ApiError::conversation);
    }
    state
        .application
        .project_conversation_messages(
            &project,
            conversation,
            page.after,
            page.limit.unwrap_or(100),
        )
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn message(
    State(state): State<ServerState>,
    AxumPath((project, conversation, message)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<ConversationMessage>> {
    state
        .application
        .project_conversation_message(&project, conversation, message)
        .map(Json)
        .map_err(ApiError::not_found)
}

pub(super) async fn append(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
    Json(input): Json<ConversationMessageInput>,
) -> ApiResult<Json<ConversationMessage>> {
    state
        .application
        .append_project_conversation_message(&project, conversation, &input)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn send(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
    Json(input): Json<annotagent_storage::ConversationSendInput>,
) -> ApiResult<Json<annotagent_storage::ConversationSendReceipt>> {
    state
        .application
        .send_project_conversation_message(&project, conversation, &input)
        .map(Json)
        .map_err(|error| {
            if matches!(error.downcast_ref::<annotagent_storage::StorageError>(), Some(annotagent_storage::StorageError::StaleConversationAgentModel)) {
                ApiError { status: StatusCode::CONFLICT, body: json!({"error":error.to_string(),"code":"send_model_selection_changed","status":409,"admitted":false}) }
            } else { ApiError::bad_request(error) }
        })
}

pub(super) async fn send_receipt(
    State(state): State<ServerState>,
    AxumPath((project, conversation, message)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    let receipt = state
        .application
        .project_conversation_send_receipt(&project, conversation, message)
        .map_err(ApiError::conversation)?;
    Ok(Json(receipt.map_or(
        Value::Null,
        |(input, receipt)| json!({"input":input,"receipt":receipt}),
    )))
}

pub(super) async fn tasks(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
) -> ApiResult<Json<Vec<annotagent_storage::ConversationTask>>> {
    state
        .application
        .conversation_tasks(&project, conversation)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn selection(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
) -> ApiResult<Json<annotagent_storage::ConversationTaskSelection>> {
    state
        .application
        .conversation_task_selection(&project, conversation)
        .map(Json)
        .map_err(ApiError::conversation)
}
pub(super) async fn select_task(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
    Json(input): Json<annotagent_storage::SelectConversationTask>,
) -> ApiResult<Json<annotagent_storage::ConversationTaskSelection>> {
    state
        .application
        .select_conversation_task(&project, conversation, &input)
        .map(Json)
        .map_err(ApiError::conversation)
}

pub(super) async fn begin_task(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
    Json(input): Json<annotagent_storage::BeginConversationTask>,
) -> ApiResult<Json<annotagent_storage::ConversationTask>> {
    state
        .application
        .begin_conversation_task(&project, conversation, &input)
        .map(Json)
        .map_err(ApiError::conversation)
}
