//! Joint authorization boundary. Only the explicit execution POST invokes models.
use super::*;
use annotagent_storage::{ConversationJourneyConsent, ConversationJourneyRecord};
use conversation_builder::{AuthorizationBase, BuilderSelection};
use futures::FutureExt;
use std::panic::AssertUnwindSafe;

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
    let execution = state
        .application
        .conversation_journey_execution_status(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    let saved = state
        .application
        .revoke_conversation_journey_consent(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    state
        .application
        .cancel_conversation_schema(
            &project,
            conversation,
            task,
            saved.consent.builder_operation_id,
        )
        .map_err(ApiError::bad_request)?;
    if !execution["sample"].is_null() {
        let _ = sample_operations::cancel_operation(
            State(state),
            AxumPath((project, saved.consent.sample_operation_id.to_string())),
        )
        .await?;
    }
    Ok(Json(saved))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecuteJourney {}

pub(super) async fn status(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .conversation_journey_execution_status(&project, conversation, task, id)
        .map(Json)
        .map_err(ApiError::bad_request)
}

/// Explicit POST advances the saved bounded journey; GET/mount never does. Each
/// child service still owns its receipt, call accounting, permissions and execution.
pub(super) async fn execute(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, id)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(_input): Json<ExecuteJourney>,
) -> ApiResult<Json<Value>> {
    let current = state
        .application
        .conversation_journey_execution_status(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    if !current["sample"].is_null() || current["dispatch"]["status"] == "running" {
        return Ok(Json(current));
    }
    state
        .application
        .require_active_conversation_journey(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    let attempt = uuid::Uuid::new_v4();
    let permit = state.journey_workers.clone().try_acquire_owned().map_err(|_| ApiError {
        status: StatusCode::TOO_MANY_REQUESTS,
        body: json!({"error":"Background journey capacity is full. No new execution was admitted.","code":"journey_capacity_exhausted"}),
    })?;
    if state
        .application
        .claim_conversation_journey_dispatch(&project, conversation, task, id, attempt)
        .map_err(ApiError::bad_request)?
    {
        let worker_state = state.clone();
        let worker_project = project.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let result = AssertUnwindSafe(Box::pin(advance(
                worker_state.clone(),
                worker_project,
                conversation,
                task,
                id,
            )))
            .catch_unwind()
            .await;
            let error = match result {
                Ok(Ok(_)) => None,
                Ok(Err(error)) => Some(error.body["error"].as_str().unwrap_or("Journey execution failed; child receipts remain saved.").to_owned()),
                Err(_) => Some("Journey worker stopped unexpectedly. Saved child receipts remain; no automatic retry was started.".to_owned()),
            };
            if let Err(error) = worker_state
                .application
                .store()
                .finish_conversation_journey_dispatch(id, attempt, error.as_deref())
            {
                eprintln!("could not settle journey dispatch {id}: {error}");
            }
        });
    }
    status(State(state), AxumPath((project, conversation, task, id))).await
}

async fn advance(
    state: ServerState,
    project: String,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    id: uuid::Uuid,
) -> ApiResult<Json<Value>> {
    let current = state
        .application
        .conversation_journey_execution_status(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    if !current["sample"].is_null() {
        return Ok(Json(current));
    }
    let saved = state
        .application
        .require_active_conversation_journey(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    let consent = &saved.consent;
    if current["builder"].is_null() {
        state
            .application
            .validate_conversation_journey_data(&project, conversation, consent)
            .map_err(ApiError::bad_request)?;
        let builder: conversation_builder::BuilderConsent = serde_json::from_value(json!({
            "selection": {
                "operation_id": consent.builder_operation_id,
                "schema_id": consent.schema_id,
                "schema_revision": consent.schema_revision,
                "model_id": consent.builder_model_id
            },
            "previous_grant_id": consent.previous_grant_id,
            "scope_hash": consent.builder_scope_hash,
            "expires_at": consent.expires_at,
            "allow_unknown_cost": consent.allow_unknown_cost
        }))
        .map_err(ApiError::internal)?;
        let _ = Box::pin(conversation_builder::launch(
            State(state.clone()),
            AxumPath((project.clone(), conversation, task)),
            Json(builder),
        ))
        .await?;
    }
    let current = state
        .application
        .conversation_journey_execution_status(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    if !current["sample"].is_null()
        || current["builder"]["status"] != "completed"
        || current["builder"]["evidence"]["outcome"] != "draft_ready_for_human_review"
    {
        // Running/unknown/interrupted planning is never silently restarted.
        return Ok(Json(current));
    }
    state
        .application
        .require_active_conversation_journey(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    let draft_id = current["builder"]["evidence"]["draft_id"]
        .as_str()
        .ok_or_else(|| ApiError::bad_request("Builder did not save an executable Draft"))?;
    let (draft, models) = state
        .application
        .resolved_workflow_draft_model_profiles(draft_id)
        .map_err(ApiError::bad_request)?;
    let fingerprint = guided_sample_fingerprint(&state, &draft, &models)?;
    let execution = DryRunWorkflowRequest {
        image_indices: (0..consent.images.len()).collect(),
        expected_revision: Some(draft.revision),
        authorization_fingerprint: Some(fingerprint.clone()),
    };
    sample_operations::validate_scope(&state, &draft, &models, &execution)?;
    state
        .application
        .seal_conversation_journey_draft(
            &project,
            conversation,
            task,
            id,
            draft_id,
            &fingerprint,
            consent.maximum_sample_calls,
        )
        .map_err(ApiError::bad_request)?;
    // Use the original Builder grant even if a previous sample admission saved its
    // grant and lost the following operation write. Never compute a fresh allowance.
    let budget = sample_operations::conversation_scope(
        &state,
        &project,
        conversation,
        task,
        &draft,
        &fingerprint,
        consent.sample_operation_id,
        Some(consent.builder_operation_id),
    )?;
    let sample: sample_operations::StartSampleRequest = serde_json::from_value(json!({
        "request_id": consent.sample_operation_id,
        "draft_id": draft_id,
        "image_indices": execution.image_indices,
        "expected_revision": draft.revision,
        "authorization_fingerprint": fingerprint,
        "conversation": {
            "conversation_id": conversation,
            "task_id": task,
            "previous_grant_id": consent.builder_operation_id,
            "scope_hash": budget["scope_hash"],
            "expires_at": consent.expires_at,
            "allow_unknown_cost": consent.allow_unknown_cost,
            "human_review": true,
            "journey_consent_id": id
        }
    }))
    .map_err(ApiError::internal)?;
    let _ = sample_operations::start_operation(
        State(state.clone()),
        AxumPath(project.clone()),
        Json(sample),
    )
    .await?;
    status(State(state), AxumPath((project, conversation, task, id))).await
}
