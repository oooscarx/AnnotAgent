//! Human-only HTTP commands; no inference dispatch or spending authority.
use super::*;
use annotagent_storage::{
    ConversationHumanRequest, ConversationHumanRequestInput, SampleFeedbackRevision,
};
use uuid::Uuid;

pub(super) async fn defer(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(String, Uuid, Uuid, Uuid)>,
    Json(input): Json<annotagent_storage::ConversationHumanDeferral>,
) -> ApiResult<Json<ConversationHumanRequest>> {
    state
        .application
        .set_conversation_human_deferral(&project, conversation, task, id, &input)
        .map(Json)
        .map_err(ApiError::bad_request)
}

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
    journey_consent_id: Option<Uuid>,
}

pub(super) async fn answer(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(String, Uuid, Uuid, Uuid)>,
    Json(input): Json<HumanAnswer>,
) -> ApiResult<Json<Value>> {
    // Existing same-origin mutation middleware protects these routes. Answers update
    // Sandbox feedback only; an outbox acknowledgment is not exposed to browsers.
    if let Some(consent_id) = input.journey_consent_id {
        let consent = state
            .application
            .conversation_journey_consent(&project, conversation, task, consent_id)
            .map_err(ApiError::bad_request)?
            .ok_or_else(|| ApiError::bad_request("Journey consent not found"))?;
        if consent
            .consent
            .repair_after_answer
            .as_ref()
            .map(|pending| pending.id)
            != Some(id)
        {
            return Err(ApiError::bad_request(
                "This answer is not linked to the authorized pending request",
            ));
        }
    }
    state
        .application
        .answer_conversation_human_request_in_journey(
            &project,
            conversation,
            task,
            id,
            &input.answer,
            input.journey_consent_id,
        )
        .map_err(ApiError::bad_request)?;
    let saved = state
        .application
        .continue_conversation_correction(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    let mut result = serde_json::to_value(saved).map_err(ApiError::internal)?;
    if let Some(consent_id) = input.journey_consent_id {
        // Saving the answer succeeded. A continuation failure must not be
        // reported as if the human edit were lost or require rewriting it.
        result["journey_resume"] = match conversation_journey::execute(
            State(state.clone()),
            AxumPath((project, conversation, task, consent_id)),
            Json(conversation_journey::ExecuteJourney {}),
        )
        .await
        {
            Ok(value) => json!({"consent_id":consent_id,"status":value.0}),
            Err(error) => {
                state
                    .application
                    .store()
                    .fail_conversation_answer_delivery(
                        consent_id,
                        error.body["error"]
                            .as_str()
                            .unwrap_or("Continuation admission failed"),
                    )
                    .map_err(ApiError::internal)?;
                json!({"consent_id":consent_id,"error":error.body["error"]})
            }
        };
    }
    Ok(Json(result))
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
