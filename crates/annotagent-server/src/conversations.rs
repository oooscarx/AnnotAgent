//! Journal endpoints: persisting a user message never grants inference authority.
use super::*;
use annotagent_storage::{ConversationMessage, ConversationMessageInput};

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct MessagePage {
    #[serde(default)]
    after: i64,
    limit: Option<u32>,
}

pub(super) async fn create(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = state
        .application
        .create_project_conversation(&project)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({ "conversation_id": id })))
}

pub(super) async fn current(
    State(state): State<ServerState>,
    AxumPath(project): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = state
        .application
        .project_conversation(&project)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({ "conversation_id": id })))
}

pub(super) async fn messages(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
    Query(page): Query<MessagePage>,
) -> ApiResult<Json<Vec<ConversationMessage>>> {
    state
        .application
        .project_conversation_messages(
            &project,
            conversation,
            page.after,
            page.limit.unwrap_or(100),
        )
        .map(Json)
        .map_err(ApiError::bad_request)
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
        .map_err(ApiError::bad_request)
}

pub(super) async fn tasks(
    State(state): State<ServerState>,
    AxumPath((project, conversation)): AxumPath<(String, uuid::Uuid)>,
) -> ApiResult<Json<Vec<annotagent_storage::ConversationTask>>> {
    state
        .application
        .conversation_tasks(&project, conversation)
        .map(Json)
        .map_err(ApiError::bad_request)
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
        .map_err(ApiError::bad_request)
}
