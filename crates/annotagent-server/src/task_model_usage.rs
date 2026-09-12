//! Passive, owner-scoped physical Provider-attempt usage projections.
use super::*;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct UsagePage {
    cursor: i64,
    limit: u32,
}

impl Default for UsagePage {
    fn default() -> Self {
        Self {
            cursor: 0,
            limit: 50,
        }
    }
}

pub(super) async fn list(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(page): Query<UsagePage>,
) -> ApiResult<Json<Value>> {
    if page.cursor < 0 || !(1..=100).contains(&page.limit) {
        return Err(ApiError::bad_request(
            "attempt cursor must be nonnegative and limit must be 1..100",
        ));
    }
    state
        .application
        .task_model_usage(&project, conversation, task, page.cursor, page.limit)
        .map(Json)
        .map_err(|_| ApiError::not_found("Task usage not found"))
}

pub(super) async fn get(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, attempt)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<annotagent_storage::TaskModelAttempt>> {
    state
        .application
        .task_model_attempt(&project, conversation, task, attempt)
        .map(Json)
        .map_err(|_| ApiError::not_found("Task model attempt not found"))
}
