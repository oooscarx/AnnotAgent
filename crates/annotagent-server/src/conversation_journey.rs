//! Joint authorization boundary. Only the explicit execution POST invokes models.
use super::*;
use annotagent_storage::{ConversationJourneyConsent, ConversationJourneyRecord};
use conversation_builder::{AuthorizationBase, BuilderSelection};
use futures::FutureExt;
use std::panic::AssertUnwindSafe;

/// Only explicit answer intents that never reached a worker claim are replayed.
/// Application startup already recovered local Sandbox checkpoint delivery.
pub(super) async fn recover_answers(state: ServerState) {
    match state
        .application
        .store()
        .queued_conversation_journey_dispatches()
    {
        Ok(items) => {
            for (project, conversation, task, id) in items {
                spawn_queued_journey(state.clone(), project, conversation, task, id);
            }
        }
        Err(error) => {
            eprintln!("could not read queued Journey execution intents: {error}");
        }
    }
    loop {
        let deliveries = match state
            .application
            .store()
            .pending_conversation_answer_deliveries()
        {
            Ok(items) => items,
            Err(error) => {
                eprintln!("could not read pending answer deliveries: {error}");
                return;
            }
        };
        if deliveries.is_empty() {
            return;
        }
        for (project, conversation, task, id) in deliveries {
            let result = execute(
                State(state.clone()),
                AxumPath((project, conversation, task, id)),
                Json(ExecuteJourney {}),
            )
            .await;
            let failure = match result {
                Err(error) => Some(error.body["error"].as_str().unwrap_or("Continuation recovery admission failed").to_owned()),
                Ok(value) if value.0["dispatch"].is_null() => Some("No continuation worker was admitted. Inspect saved state before explicit retry.".into()),
                Ok(_) => None,
            };
            if let Some(error) = failure {
                if let Err(save_error) = state
                    .application
                    .store()
                    .fail_conversation_answer_delivery(id, &error)
                {
                    eprintln!("could not settle answer delivery {id}: {save_error}");
                    return;
                }
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JourneySelection {
    #[serde(default)]
    consent_id: uuid::Uuid,
    #[serde(default)]
    builder_operation_id: uuid::Uuid,
    #[serde(default)]
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
    #[serde(default)]
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

fn automatic_visual_models(readiness: &Value) -> ApiResult<Vec<String>> {
    let candidates = readiness["candidates"]
        .as_array()
        .ok_or_else(|| ApiError::internal("Capability readiness omitted candidates"))?;
    let is_ready_visual = |candidate: &&Value| {
        candidate["readiness"] == "ready"
            && candidate["roles"]
                .as_array()
                .is_some_and(|roles| roles.iter().any(|role| role.as_str() != Some("agent")))
    };
    let local_refiners = candidates
        .iter()
        .filter(|candidate| {
            candidate["candidate_type"] == "model_instance"
                && candidate["readiness"] == "ready"
                && candidate["production_eligible"] != false
                && candidate["capabilities"]
                    .as_array()
                    .is_some_and(|capabilities| {
                        capabilities
                            .iter()
                            .any(|capability| capability == "prompted_segmentation")
                    })
        })
        .filter_map(|candidate| candidate["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let exact_scope = candidates
        .iter()
        .filter(is_ready_visual)
        .filter(|candidate| candidate["allowed_by_current_scope"] == true)
        .filter_map(|candidate| candidate["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    if !exact_scope.is_empty() {
        let mut selected = exact_scope;
        for id in &local_refiners {
            if !selected.contains(id) {
                selected.push(id.clone());
            }
        }
        return Ok(selected);
    }
    let bound = candidates
        .iter()
        .filter(is_ready_visual)
        .filter(|candidate| {
            candidate["project_bindings"]
                .as_array()
                .is_some_and(|bindings| !bindings.is_empty())
        })
        .collect::<Vec<_>>();
    let primary = bound
        .iter()
        .filter(|candidate| {
            candidate["project_bindings"]
                .as_array()
                .is_some_and(|bindings| {
                    bindings
                        .iter()
                        .any(|binding| binding["role"] == "primary_inference")
                })
        })
        .collect::<Vec<_>>();
    let selected = if primary.len() == 1 {
        Some(*primary[0])
    } else if primary.is_empty() && bound.len() == 1 {
        Some(bound[0])
    } else {
        None
    };
    let mut selected = selected
        .and_then(|candidate| candidate["id"].as_str())
        .map(|id| vec![id.to_owned()])
        .ok_or_else(|| ApiError {
            status: StatusCode::BAD_REQUEST,
            body: json!({
                "error":"Choose one ready Project-bound primary inference model before approving the automatic Sample scope.",
                "status":StatusCode::BAD_REQUEST.as_u16(),
                "code":"capability_setup_required",
                "suggested_action":"configure_project_model_binding",
                "registry_revision":readiness["registry_revision"],
                "setup_requests":readiness["setup_requests"],
                "eligible_project_model_ids":bound.iter().filter_map(|candidate|candidate["id"].as_str()).collect::<Vec<_>>()
            }),
        })?;
    for id in local_refiners {
        if !selected.contains(&id) {
            selected.push(id);
        }
    }
    Ok(selected)
}

pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(mut selection): Query<JourneySelection>,
) -> ApiResult<Json<Value>> {
    if selection.consent_id.is_nil()
        && selection.builder_operation_id.is_nil()
        && selection.sample_operation_id.is_nil()
        && selection.allowed_models.is_empty()
    {
        let readiness = super::mainline_capability::snapshot(&state, &project, conversation, task)?;
        let allowed = automatic_visual_models(&readiness)?;
        let schema_calls = state
            .application
            .conversation_schema_calls(&project, conversation, task)
            .map_err(ApiError::bad_request)?;
        let mut saved_schema = None;
        for call in schema_calls.into_iter().rev() {
            if let Some(schema) = state
                .application
                .conversation_schema_for_call(&project, conversation, task, call.id)
                .map_err(ApiError::bad_request)?
            {
                saved_schema = Some(schema);
                break;
            }
        }
        let identity = saved_schema.as_ref().map(|schema| schema.id);
        let stable = |purpose: &str| automatic_journey_id(task, identity, purpose);
        selection.consent_id = stable("consent");
        selection.builder_operation_id = stable("builder");
        selection.sample_operation_id = stable("sample");
        if let Some(schema) = saved_schema {
            // A completed, persisted Schema is evidence, not a reason to create
            // another paid Schema request. Continue under a new cumulative
            // Builder authorization derived by `scope` below.
            selection.schema_id = schema.id;
            selection.schema_revision = schema.revision;
            selection.schema_call_id = None;
        } else {
            selection.schema_call_id = Some(stable("schema"));
        }
        selection.allowed_models = serde_json::to_string(&allowed).map_err(ApiError::internal)?;
    }
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
        queued_message_id: None,
        source_draft_id: None,
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
        json!({"consent":consent,"builder":builder,"data":data,"project_call_limit":project_limit,"estimated_cost":null,"operation":"Approve one bounded Schema, Builder and Sample continuation using only the listed images/models/calls. Saving the exact consent persists its execution intent. No publication, dataset Run or annotation acceptance."}),
    ))
}

fn automatic_journey_id(task: uuid::Uuid, schema: Option<uuid::Uuid>, purpose: &str) -> uuid::Uuid {
    let scope = schema.map_or_else(
        || match purpose {
            "consent" => "annotagent-p0-journey-consent-v1".to_owned(),
            "builder" => "annotagent-p0-builder-operation-v1".to_owned(),
            "sample" => "annotagent-p0-sample-operation-v1".to_owned(),
            "schema" => "annotagent-p0-schema-call-v1".to_owned(),
            purpose => format!("annotagent-p0-initial-journey-v1:{purpose}"),
        },
        |schema| format!("annotagent-p0-schema-continuation-v1:{schema}:{purpose}"),
    );
    uuid::Uuid::new_v5(&task, scope.as_bytes())
}

fn start_newly_approved_journey(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    saved: &ConversationJourneyRecord,
) -> ApiResult<()> {
    if saved.effective_consent().repair_after_answer.is_some() {
        return Ok(());
    }
    let current = state
        .application
        .conversation_journey_execution_status(project, conversation, task, saved.consent.id)
        .map_err(ApiError::bad_request)?;
    if current["dispatch"].is_null() {
        let queue_id = uuid::Uuid::new_v4();
        if state
            .application
            .queue_conversation_journey_dispatch(
                project,
                conversation,
                task,
                saved.consent.id,
                queue_id,
            )
            .map_err(ApiError::bad_request)?
        {
            spawn_queued_journey(
                state.clone(),
                project.to_owned(),
                conversation,
                task,
                saved.consent.id,
            );
        }
    }
    Ok(())
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
        start_newly_approved_journey(&state, &project, conversation, task, &saved)?;
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
        let saved = state
            .application
            .save_conversation_journey_consent(&project, conversation, &consent)
            .map_err(ApiError::bad_request)?;
        start_newly_approved_journey(&state, &project, conversation, task, &saved)?;
        return Ok(Json(saved));
    }
    let selection = BuilderSelection {
        operation_id: consent.builder_operation_id,
        schema_id: consent.schema_id,
        schema_revision: consent.schema_revision,
        model_id: consent.builder_model_id,
        repair_request_id: consent.repair.as_ref().map(|repair| repair.request_id),
        image_class_review_id: None,
        queued_message_id: None,
        source_draft_id: None,
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
    let saved = state
        .application
        .save_conversation_journey_consent(&project, conversation, &consent)
        .map_err(ApiError::bad_request)?;
    start_newly_approved_journey(&state, &project, conversation, task, &saved)?;
    Ok(Json(saved))
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
    if !current["sample"].is_null()
        || matches!(
            current["dispatch"]["status"].as_str(),
            Some("queued" | "running")
        )
        || (current["dispatch"]["status"] == "settled" && !current["dispatch"]["error"].is_null())
    {
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
            queued_message_id: None,
            source_draft_id: None,
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
    let queue_id = uuid::Uuid::new_v4();
    if state
        .application
        .queue_conversation_journey_dispatch(&project, conversation, task, id, queue_id)
        .map_err(ApiError::bad_request)?
    {
        spawn_queued_journey(state.clone(), project.clone(), conversation, task, id);
    }
    status(State(state), AxumPath((project, conversation, task, id))).await
}

fn child_waits_for_commit(current: &Value) -> bool {
    current["sample"].is_null()
        && (["reserved", "running"].contains(&current["schema"]["status"].as_str().unwrap_or(""))
            || ["reserved", "running"]
                .contains(&current["builder"]["status"].as_str().unwrap_or("")))
}

fn committed_child_failure(current: &Value) -> Option<String> {
    if !current["sample"].is_null() {
        return None;
    }
    if let Some(status @ ("failed" | "in_doubt" | "interrupted" | "cancelled")) =
        current["schema"]["status"].as_str()
    {
        return Some(format!(
            "Schema request ended as {status}; no Builder or Sample was started. Inspect the saved request receipt."
        ));
    }
    if let Some(status @ ("failed" | "in_doubt" | "interrupted" | "cancelled")) =
        current["builder"]["status"].as_str()
    {
        return Some(format!(
            "Builder ended as {status}; no Sample was started. Inspect the saved Builder receipt."
        ));
    }
    if current["builder"]["status"] == "completed"
        && current["builder"]["evidence"]["outcome"] != "draft_ready_for_human_review"
    {
        return Some(
            "Builder completed without an executable Draft; no Sample was started. Inspect the saved Builder receipt."
                .to_owned(),
        );
    }
    None
}

fn spawn_queued_journey(
    state: ServerState,
    project: String,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    id: uuid::Uuid,
) {
    tokio::spawn(async move {
        let Ok(permit) = state.journey_workers.clone().acquire_owned().await else {
            return;
        };
        let _permit = permit;
        let attempt = uuid::Uuid::new_v4();
        match state
            .application
            .claim_queued_conversation_journey_dispatch(id, attempt)
        {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                eprintln!("could not claim queued Journey {id}: {error}");
                return;
            }
        }
        loop {
            let result = AssertUnwindSafe(Box::pin(advance(
                state.clone(),
                project.clone(),
                conversation,
                task,
                id,
            )))
            .catch_unwind()
            .await;
            let error = match result {
                Ok(Ok(Json(current))) if child_waits_for_commit(&current) => {
                    match state
                        .application
                        .store()
                        .touch_conversation_journey_dispatch(id, attempt)
                    {
                        Ok(true) => {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            continue;
                        }
                        Ok(false) => return,
                        Err(error) => {
                            eprintln!("could not heartbeat Journey dispatch {id}: {error}");
                            return;
                        }
                    }
                }
                Ok(Ok(Json(current))) => committed_child_failure(&current),
                Ok(Err(error)) => Some(
                    error.body["error"]
                        .as_str()
                        .unwrap_or("Journey execution failed; child receipts remain saved.")
                        .to_owned(),
                ),
                Err(_) => Some("Journey worker stopped unexpectedly. Saved child receipts remain; no automatic retry was started.".to_owned()),
            };
            match state
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

pub(super) fn resume_after_schema_retry(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    original_call: uuid::Uuid,
) -> ApiResult<(Option<uuid::Uuid>, bool)> {
    let Some(id) = state
        .application
        .conversation_journey_id_for_schema_call(project, conversation, task, original_call)
        .map_err(ApiError::bad_request)?
    else {
        return Ok((None, false));
    };
    let queue_id = uuid::Uuid::new_v4();
    let queued = state
        .application
        .queue_conversation_journey_dispatch(project, conversation, task, id, queue_id)
        .map_err(ApiError::bad_request)?;
    if queued {
        spawn_queued_journey(state.clone(), project.to_owned(), conversation, task, id);
    }
    Ok((Some(id), queued))
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
            let retry = state
                .application
                .latest_conversation_schema_retry(&project, conversation, task, proposal.call_id)
                .map_err(ApiError::bad_request)?;
            let schema_call_id = retry
                .as_ref()
                .map_or(proposal.call_id, |value| value.call_id);
            let receipt = if let Some(receipt) = state
                .application
                .conversation_call_receipt(&project, conversation, task, schema_call_id)
                .map_err(ApiError::bad_request)?
            {
                receipt
            } else if retry.is_some() {
                // The explicit retry command owns this successor request. Journey
                // recovery observes its durable receipt and never dispatches it.
                return status(State(state), AxumPath((project, conversation, task, id))).await;
            } else {
                // Recheck the text request scope at first admission. Once
                // this exact call has a durable receipt, never treat the
                // later clarification answer/delivery revision as a reason
                // to resend it. Builder and Sample retain their own exact
                // image/model/budget validation below.
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
                Box::pin(conversation_schema::propose_in_journey(
                    State(state.clone()),
                    AxumPath((project.clone(), conversation, task)),
                    Json(proposal.clone()),
                ))
                .await?
                .0
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
                    .save_conversation_schema_draft(&project, conversation, task, schema_call_id)
                    .map_err(ApiError::bad_request)?,
                "clarify" if saved.consent.continue_after_clarification => {
                    let question = state
                        .application
                        .schema_clarification(&project, conversation, task, schema_call_id)
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
            let delivery = state
                .application
                .task_delivery_intent(&project, conversation, task)
                .map_err(ApiError::bad_request)?;
            if delivery.missing_slots.is_empty()
                && delivery.blockers.is_empty()
                && let Some(delivery) = delivery.saved
            {
                let already_prepared = state
                    .application
                    .human_conversation_schema_drafts(&project, conversation, task)
                    .map_err(ApiError::bad_request)?
                    .iter()
                    .any(|draft| {
                        annotagent_application::require_delivery_schema(
                            Some(&delivery),
                            &draft.definition,
                        )
                        .is_ok()
                    });
                if !already_prepared {
                    state
                        .application
                        .prepare_delivery_schema(
                            &project,
                            conversation,
                            task,
                            &annotagent_application::PrepareDeliverySchema {
                                command_id: uuid::Uuid::new_v5(
                                    &id,
                                    b"annotagent-p0-delivery-schema-v1",
                                ),
                                expected_revision: delivery.revision,
                                expected_sha256: delivery.content_sha256,
                            },
                        )
                        .map_err(ApiError::bad_request)?;
                }
            }
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
                    queued_message_id: None,
                    source_draft_id: None,
                },
                AuthorizationBase::Existing(schema_call_id),
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
            resolved.previous_grant_id = Some(schema_call_id);
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
        image_indices: sample_operations::journey_image_indices(&state, &project, &consent.images)?,
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

#[cfg(test)]
mod tests {
    use super::{
        automatic_journey_id, automatic_visual_models, child_waits_for_commit,
        committed_child_failure,
    };
    use serde_json::json;

    #[test]
    fn existing_in_flight_child_keeps_the_same_journey_worker_alive() {
        for current in [
            json!({"schema":{"status":"reserved"},"builder":null,"sample":null}),
            json!({"schema":null,"builder":{"status":"reserved"},"sample":null}),
            json!({"schema":null,"builder":{"status":"running"},"sample":null}),
        ] {
            assert!(child_waits_for_commit(&current));
            assert!(committed_child_failure(&current).is_none());
        }
        assert!(!child_waits_for_commit(&json!({
            "schema":null,
            "builder":{"status":"completed","evidence":{"outcome":"draft_ready_for_human_review"}},
            "sample":{"id":"TEST-same-sample-operation"}
        })));
    }

    #[test]
    fn completed_schema_continuation_has_stable_distinct_operation_ids() {
        let task = uuid::Uuid::new_v4();
        let schema = uuid::Uuid::new_v4();
        let initial = automatic_journey_id(task, None, "consent");
        assert_eq!(
            initial,
            uuid::Uuid::new_v5(&task, b"annotagent-p0-journey-consent-v1")
        );
        let continuation = automatic_journey_id(task, Some(schema), "consent");
        assert_ne!(initial, continuation);
        assert_eq!(
            continuation,
            automatic_journey_id(task, Some(schema), "consent")
        );
        assert_ne!(
            automatic_journey_id(task, Some(schema), "builder"),
            automatic_journey_id(task, Some(schema), "sample")
        );
    }

    #[test]
    fn unknown_or_invalid_child_result_stops_without_fictional_sample() {
        for (current, expected) in [
            (
                json!({"schema":{"status":"in_doubt"},"builder":null,"sample":null}),
                "Schema request ended as in_doubt",
            ),
            (
                json!({"schema":null,"builder":{"status":"interrupted"},"sample":null}),
                "Builder ended as interrupted",
            ),
            (
                json!({"schema":null,"builder":{"status":"completed","evidence":{"outcome":"failed"}},"sample":null}),
                "Builder completed without an executable Draft",
            ),
        ] {
            assert!(!child_waits_for_commit(&current));
            assert!(
                committed_child_failure(&current)
                    .unwrap()
                    .starts_with(expected)
            );
        }
    }

    #[test]
    fn automatic_scope_uses_one_project_primary_and_ready_local_refiners() {
        let readiness = json!({
            "registry_revision":"TEST-registry",
            "setup_requests":[{"id":"TEST-setup"}],
            "candidates":[
                {"id":"model-profile:one","readiness":"ready","roles":["vision_language"],"allowed_by_current_scope":false,
                 "project_bindings":[{"role":"primary_inference"}]},
                {"id":"model-instance:segment","candidate_type":"model_instance","readiness":"ready","production_eligible":true,
                 "capabilities":["prompted_segmentation"],"roles":["visual","segmentation"],"allowed_by_current_scope":false,"project_bindings":[]},
                {"id":"model-profile:other-provider","readiness":"ready","roles":["detection"],"allowed_by_current_scope":false,
                 "project_bindings":[]}
            ]
        });
        assert_eq!(
            automatic_visual_models(&readiness).unwrap(),
            vec!["model-profile:one", "model-instance:segment"]
        );
        let no_binding = json!({
            "registry_revision":"TEST-registry",
            "setup_requests":[{"id":"TEST-setup"}],
            "candidates":[
                {"id":"model-profile:one","readiness":"ready","roles":["vision_language"],"allowed_by_current_scope":false,"project_bindings":[]},
                {"id":"model-profile:other-provider","readiness":"ready","roles":["detection"],"allowed_by_current_scope":false,"project_bindings":[]}
            ]
        });
        let error = automatic_visual_models(&no_binding).unwrap_err();
        assert_eq!(error.body["code"], "capability_setup_required");
        assert_eq!(error.body["eligible_project_model_ids"], json!([]));

        let frozen = json!({
            "candidates":[
                {"id":"model-profile:frozen","readiness":"ready","roles":["detection"],"allowed_by_current_scope":true,"project_bindings":[]},
                {"id":"model-instance:segment","candidate_type":"model_instance","readiness":"ready","production_eligible":true,
                 "capabilities":["prompted_segmentation"],"roles":["visual","segmentation"],"allowed_by_current_scope":false,"project_bindings":[]},
                {"id":"model-profile:new","readiness":"ready","roles":["vision_language"],"allowed_by_current_scope":false,"project_bindings":[{"role":"primary_inference"}]}
            ]
        });
        assert_eq!(
            automatic_visual_models(&frozen).unwrap(),
            vec!["model-profile:frozen", "model-instance:segment"]
        );
    }
}
