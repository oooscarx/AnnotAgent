//! Joint authorization HTTP boundary. These routes never invoke a Provider.
use super::*;
use annotagent_storage::{ConversationJourneyConsent, ConversationJourneyRecord};
use conversation_builder::{AuthorizationBase, BuilderSelection};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JourneySelection {
    consent_id: uuid::Uuid,
    builder_operation_id: uuid::Uuid,
    sample_operation_id: uuid::Uuid,
    schema_id: uuid::Uuid,
    schema_revision: u64,
    planner_model_id: Option<ModelProfileId>,
    /// JSON array of exact Model Profile / Plugin selection IDs, not model hashes.
    allowed_models: String,
}

pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(selection): Query<JourneySelection>,
) -> ApiResult<Json<Value>> {
    let models: Vec<String> =
        serde_json::from_str(&selection.allowed_models).map_err(ApiError::bad_request)?;
    let data = state
        .application
        .conversation_journey_data_scope(
            &project,
            conversation,
            task,
            selection.schema_id,
            selection.schema_revision,
            &models,
        )
        .map_err(ApiError::bad_request)?;
    let builder_selection = BuilderSelection {
        operation_id: selection.builder_operation_id,
        schema_id: selection.schema_id,
        schema_revision: selection.schema_revision,
        model_id: selection.planner_model_id,
        repair_request_id: None,
    };
    let (model, builder) = conversation_builder::scope(
        &state,
        &project,
        conversation,
        task,
        &builder_selection,
        AuthorizationBase::Preview,
    )?;
    let consent = ConversationJourneyConsent {
        id: selection.consent_id,
        task_id: task,
        builder_operation_id: selection.builder_operation_id,
        builder_model_id: Some(model.model.id),
        previous_grant_id: serde_json::from_value(builder["previous_grant_id"].clone())
            .map_err(ApiError::internal)?,
        sample_operation_id: selection.sample_operation_id,
        builder_scope_hash: builder["scope_hash"]
            .as_str()
            .ok_or_else(|| ApiError::internal("Builder preview omitted its scope"))?
            .into(),
        schema_id: data.schema_id,
        schema_revision: data.schema_revision,
        schema_digest: data.schema_digest.clone(),
        images: data.images.clone(),
        allowed_models: data
            .models
            .iter()
            .map(|model| model.scope.clone())
            .collect(),
        maximum_builder_calls: u32::try_from(
            builder["maximum_builder_calls"]
                .as_u64()
                .ok_or_else(|| ApiError::internal("Builder call limit missing"))?,
        )
        .map_err(ApiError::internal)?,
        maximum_sample_calls: 12,
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(30),
        allow_unknown_cost: false,
    };
    let project_limit = state
        .application
        .project_conversation_call_limit(&project)
        .map_err(ApiError::bad_request)?;
    Ok(Json(
        json!({"consent":consent,"builder":builder,"data":data,"project_call_limit":project_limit,"estimated_cost":null,"operation":"Build one Draft, then test only the listed images with permitted model bindings. No publication, dataset Run or annotation acceptance. Saving this consent alone does not start execution."}),
    ))
}

pub(super) async fn save(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(consent): Json<ConversationJourneyConsent>,
) -> ApiResult<Json<ConversationJourneyRecord>> {
    if consent.task_id != task {
        return Err(ApiError::bad_request(
            "Journey consent belongs to another task",
        ));
    }
    if let Some(saved) = state
        .application
        .conversation_journey_consent(&project, conversation, task, consent.id)
        .map_err(ApiError::bad_request)?
    {
        if saved.consent != consent {
            return Err(ApiError::bad_request(
                "Journey retry changed its original consent",
            ));
        }
        return Ok(Json(saved));
    }
    if !consent.allow_unknown_cost
        || consent.builder_model_id.is_none()
        || consent.maximum_sample_calls != 12
    {
        return Err(ApiError::bad_request(
            "Explicitly confirm the bounded journey and unknown cost",
        ));
    }
    let selection = BuilderSelection {
        operation_id: consent.builder_operation_id,
        schema_id: consent.schema_id,
        schema_revision: consent.schema_revision,
        model_id: consent.builder_model_id,
        repair_request_id: None,
    };
    let (_, builder) = conversation_builder::scope(
        &state,
        &project,
        conversation,
        task,
        &selection,
        consent
            .previous_grant_id
            .map_or(AuthorizationBase::Initial, AuthorizationBase::Existing),
    )?;
    if builder["scope_hash"] != consent.builder_scope_hash
        || builder["previous_grant_id"] != json!(consent.previous_grant_id)
        || builder["maximum_builder_calls"].as_u64()
            != Some(u64::from(consent.maximum_builder_calls))
    {
        return Err(ApiError::bad_request(
            "Journey planning model, prior authorization or call scope changed",
        ));
    }
    state
        .application
        .save_conversation_journey_consent(&project, conversation, &consent)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn get(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<ConversationJourneyRecord>> {
    state
        .application
        .conversation_journey_consent(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("Journey consent not found"))
}

pub(super) async fn revoke(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<ConversationJourneyRecord>> {
    state
        .application
        .revoke_conversation_journey_consent(&project, conversation, task, id)
        .map(Json)
        .map_err(ApiError::bad_request)
}
