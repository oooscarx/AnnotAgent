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
    #[serde(default)]
    schema_id: uuid::Uuid,
    #[serde(default)]
    schema_revision: u64,
    schema_call_id: Option<uuid::Uuid>,
    planner_model_id: Option<ModelProfileId>,
    repair_request_id: Option<uuid::Uuid>,
    pending_request_id: Option<uuid::Uuid>,
    /// JSON array of exact Model Profile / Plugin selection IDs, not model hashes.
    allowed_models: String,
}

pub(super) async fn history(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    let items = state
        .application
        .conversation_journey_history(&project, conversation, task)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"items":items,"limit":50})))
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
    if let Some(call_id) = selection.schema_call_id {
        if selection.repair_request_id.is_some()
            || selection.pending_request_id.is_some()
            || !selection.schema_id.is_nil()
            || selection.schema_revision != 0
            || state
                .application
                .optional_conversation_builder_budget(&project, conversation, task)
                .map_err(ApiError::bad_request)?
                .is_some()
        {
            return Err(ApiError::bad_request(
                "Initial journey requires an unexecuted goal; existing Schema work keeps its original authorization",
            ));
        }
        let (model, mut builder) = conversation_schema::preview_scope(
            &state,
            &project,
            conversation,
            task,
            selection.planner_model_id,
        )?;
        let expires_at = chrono::Utc::now() + chrono::Duration::minutes(30);
        let proposal = annotagent_storage::ConversationSchemaAuthorization {
            call_id,
            model_id: model.model.id,
            scope_hash: builder["scope_hash"]
                .as_str()
                .ok_or_else(|| ApiError::internal("Schema scope missing"))?
                .into(),
            expires_at,
            allow_unknown_cost: false,
        };
        let consent = ConversationJourneyConsent {
            repair_after_answer: None,
            repair: None,
            continue_after_clarification: true,
            schema_proposal: Some(proposal.clone()),
            id: selection.consent_id,
            task_id: task,
            builder_operation_id: selection.builder_operation_id,
            builder_model_id: Some(model.model.id),
            previous_grant_id: None,
            sample_operation_id: selection.sample_operation_id,
            builder_scope_hash: proposal.scope_hash,
            schema_id: data.schema_id,
            schema_revision: data.schema_revision,
            schema_digest: data.schema_digest.clone(),
            images: data.images.clone(),
            allowed_models: data
                .models
                .iter()
                .map(|model| model.scope.clone())
                .collect(),
            maximum_builder_calls: 8,
            maximum_sample_calls: 12,
            expires_at,
            allow_unknown_cost: false,
        };
        builder["maximum_builder_calls"] = json!(8);
        builder["maximum_calls"] = json!(9);
        let project_limit = state
            .application
            .project_conversation_call_limit(&project)
            .map_err(ApiError::bad_request)?;
        return Ok(Json(
            json!({"consent":consent,"builder":builder,"data":data,"project_call_limit":project_limit,"estimated_cost":null,"operation":"One text-only Schema proposal, then one bounded Builder and sample test using only the listed images/models. Clarification or invalid Schema stops before image inference. No publish or annotation acceptance."}),
        ));
    }
    if selection.pending_request_id.is_some() && selection.repair_request_id.is_some() {
        return Err(ApiError::bad_request(
            "Choose one pending answer or completed correction",
        ));
    }
    let pending = selection
        .pending_request_id
        .map(|id| {
            let request = state
                .application
                .conversation_human_requests(&project, conversation, task)
                .map_err(ApiError::bad_request)?
                .into_iter()
                .find(|item| item.input.id == id)
                .ok_or_else(|| {
                    ApiError::bad_request("Pending human request not found in this task")
                })?;
            if request.status != annotagent_storage::ConversationHumanRequestStatus::Pending
                || request.deferred
            {
                return Err(ApiError::bad_request(
                    "Preauthorization requires an active pending request",
                ));
            }
            Ok(request.input)
        })
        .transpose()?;
    let builder_selection = BuilderSelection {
        operation_id: selection.builder_operation_id,
        schema_id: selection.schema_id,
        schema_revision: selection.schema_revision,
        model_id: selection.planner_model_id,
        repair_request_id: selection.repair_request_id,
        image_class_review_id: None,
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
        repair_after_answer: pending,
        repair: serde_json::from_value(builder["repair"].clone()).map_err(ApiError::internal)?,
        continue_after_clarification: false,
        schema_proposal: None,
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
    if let Some(proposal) = &consent.schema_proposal {
        let (_, preview) = conversation_schema::preview_scope(
            &state,
            &project,
            conversation,
            task,
            Some(proposal.model_id),
        )?;
        if preview["scope_hash"] != proposal.scope_hash
            || consent.builder_scope_hash != proposal.scope_hash
        {
            return Err(ApiError::bad_request(
                "Initial planning model or goal scope changed",
            ));
        }
        return state
            .application
            .save_conversation_journey_consent(&project, conversation, &consent)
            .map(Json)
            .map_err(ApiError::bad_request);
    }
    let selection = BuilderSelection {
        operation_id: consent.builder_operation_id,
        schema_id: consent.schema_id,
        schema_revision: consent.schema_revision,
        model_id: consent.builder_model_id,
        repair_request_id: consent.repair.as_ref().map(|repair| repair.request_id),
        image_class_review_id: None,
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
        || builder["repair"] != json!(consent.repair)
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
    if let Some(proposal) = &saved.consent.schema_proposal {
        state
            .application
            .cancel_conversation_schema(&project, conversation, task, proposal.call_id)
            .map_err(ApiError::bad_request)?;
    }
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
    let saved = state
        .application
        .require_active_conversation_journey(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    if let Some(pending) = saved.effective_consent().repair_after_answer.as_ref() {
        let request = state
            .application
            .conversation_human_requests(&project, conversation, task)
            .map_err(ApiError::bad_request)?
            .into_iter()
            .find(|item| item.input.id == pending.id)
            .ok_or_else(|| ApiError::bad_request("Authorized human request is unavailable"))?;
        if request.status == annotagent_storage::ConversationHumanRequestStatus::Pending
            && !request.deferred
        {
            return Ok(Json(current));
        }
        let selection = BuilderSelection {
            operation_id: saved.consent.builder_operation_id,
            schema_id: saved.consent.schema_id,
            schema_revision: saved.consent.schema_revision,
            model_id: saved.consent.builder_model_id,
            repair_request_id: Some(pending.id),
            image_class_review_id: None,
        };
        let (_, preview) = conversation_builder::scope(
            &state,
            &project,
            conversation,
            task,
            &selection,
            saved
                .consent
                .previous_grant_id
                .map_or(AuthorizationBase::Initial, AuthorizationBase::Existing),
        )?;
        if preview["maximum_builder_calls"].as_u64()
            != Some(u64::from(saved.consent.maximum_builder_calls))
            || preview["previous_grant_id"] != json!(saved.consent.previous_grant_id)
        {
            return Err(ApiError::bad_request(
                "Answer continuation call authorization changed",
            ));
        }
        let mut resolved = saved.consent.clone();
        resolved.repair_after_answer = None;
        resolved.repair =
            serde_json::from_value(preview["repair"].clone()).map_err(ApiError::internal)?;
        resolved.builder_scope_hash = preview["scope_hash"]
            .as_str()
            .ok_or_else(|| ApiError::internal("Resolved Builder scope missing"))?
            .into();
        state
            .application
            .resolve_answer_journey_repair(&project, conversation, &resolved)
            .map_err(ApiError::bad_request)?;
    }
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
            loop {
                let result = AssertUnwindSafe(Box::pin(advance(
                    worker_state.clone(),
                    worker_project.clone(),
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
                match worker_state
                    .application
                    .store()
                    .finish_conversation_journey_dispatch(id, attempt, error.as_deref())
                {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(error) => {
                        eprintln!("could not settle journey dispatch {id}: {error}");
                        break;
                    }
                }
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
    let mut saved = state
        .application
        .require_active_conversation_journey(&project, conversation, task, id)
        .map_err(ApiError::bad_request)?;
    if saved.resolved_consent.is_none() {
        if let Some(proposal) = saved.consent.schema_proposal.clone() {
            state
                .application
                .validate_conversation_journey_data(&project, conversation, &saved.consent)
                .map_err(ApiError::bad_request)?;
            let (_, preview) = conversation_schema::preview_scope(
                &state,
                &project,
                conversation,
                task,
                Some(proposal.model_id),
            )?;
            if preview["scope_hash"] != proposal.scope_hash {
                return Err(ApiError::bad_request(
                    "Initial goal or planning model changed",
                ));
            }
            let receipt = match state
                .application
                .conversation_call_receipt(&project, conversation, task, proposal.call_id)
                .map_err(ApiError::bad_request)?
            {
                Some(receipt) => receipt,
                None => {
                    Box::pin(conversation_schema::propose_in_journey(
                        State(state.clone()),
                        AxumPath((project.clone(), conversation, task)),
                        Json(proposal.clone()),
                    ))
                    .await?
                    .0
                }
            };
            if receipt.status != annotagent_storage::ConversationCallStatus::Completed {
                return status(State(state), AxumPath((project, conversation, task, id))).await;
            }
            let decision = serde_json::to_value(&receipt).map_err(ApiError::internal)?["evidence"]
                ["decision"]["Ok"]["decision"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            let schema = match decision.as_str() {
                "draft" => state
                    .application
                    .save_conversation_schema_draft(&project, conversation, task, proposal.call_id)
                    .map_err(ApiError::bad_request)?,
                "clarify" if saved.consent.continue_after_clarification => {
                    let question = state
                        .application
                        .schema_clarification(&project, conversation, task, proposal.call_id)
                        .map_err(ApiError::bad_request)?;
                    let Some(schema_id) = question
                        .schema_draft_id
                        .filter(|_| question.status == "applied")
                    else {
                        return status(State(state), AxumPath((project, conversation, task, id)))
                            .await;
                    };
                    let schema = state
                        .application
                        .conversation_schema_draft(&project, schema_id, None)
                        .map_err(ApiError::bad_request)?;
                    if schema.task_id != task || schema.revision != 1 {
                        return Err(ApiError::bad_request(
                            "Clarification labels changed after the answer. Review a new authorization.",
                        ));
                    }
                    schema
                }
                _ => {
                    return status(State(state), AxumPath((project, conversation, task, id))).await;
                }
            };
            let (_, builder) = conversation_builder::scope(
                &state,
                &project,
                conversation,
                task,
                &BuilderSelection {
                    operation_id: saved.consent.builder_operation_id,
                    schema_id: schema.id,
                    schema_revision: schema.revision,
                    model_id: saved.consent.builder_model_id,
                    repair_request_id: None,
                    image_class_review_id: None,
                },
                AuthorizationBase::Existing(proposal.call_id),
            )?;
            let mut resolved = saved.consent.clone();
            resolved.schema_proposal = None;
            resolved.schema_id = schema.id;
            resolved.schema_revision = schema.revision;
            resolved.schema_digest = annotagent_image_tools::sha256(
                &serde_json::to_vec(&schema.definition).map_err(ApiError::internal)?,
            );
            resolved.builder_scope_hash = builder["scope_hash"]
                .as_str()
                .ok_or_else(|| ApiError::internal("Builder scope missing"))?
                .into();
            resolved.previous_grant_id = Some(proposal.call_id);
            saved = state
                .application
                .resolve_initial_journey_schema(&project, conversation, &resolved)
                .map_err(ApiError::bad_request)?;
        }
    }
    let consent = saved.effective_consent();
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
                "model_id": consent.builder_model_id,
                "repair_request_id": consent.repair.as_ref().map(|repair| repair.request_id)
            },
            "repair": consent.repair,
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
