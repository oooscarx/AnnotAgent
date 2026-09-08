use super::*;
use annotagent_application::{
    ConversationBuilderExecution, ConversationBuilderRepair, PipelineBuilderModelRuntime,
};
use annotagent_storage::ConversationCallGrant;
use chrono::{DateTime, Duration, Utc};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BuilderSelection {
    pub(super) operation_id: uuid::Uuid,
    pub(super) schema_id: uuid::Uuid,
    pub(super) schema_revision: u64,
    pub(super) model_id: Option<ModelProfileId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) repair_request_id: Option<uuid::Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) image_class_review_id: Option<uuid::Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BuilderConsent {
    selection: BuilderSelection,
    previous_grant_id: Option<uuid::Uuid>,
    scope_hash: String,
    expires_at: DateTime<Utc>,
    allow_unknown_cost: bool,
    #[serde(default)]
    repair: Option<ConversationBuilderRepair>,
    #[serde(default)]
    image_class_repair: Option<annotagent_application::ConversationImageClassBuilderRepair>,
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum AuthorizationBase {
    Preview,
    Initial,
    Existing(uuid::Uuid),
}
pub(super) fn scope(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    selection: &BuilderSelection,
    previous: AuthorizationBase,
) -> ApiResult<(PipelineBuilderModelRuntime, Value)> {
    if selection.repair_request_id.is_some() && selection.image_class_review_id.is_some() {
        return Err(ApiError::bad_request("Choose one exact repair source"));
    }
    let mut budget = state
        .application
        .optional_conversation_builder_budget(project, conversation, task)
        .map_err(ApiError::bad_request)?;
    if budget.as_ref().is_some_and(|budget| budget.revoked) {
        return Err(ApiError::bad_request("Task authorization was revoked"));
    }
    if let AuthorizationBase::Existing(previous) = previous {
        let budget = budget
            .as_mut()
            .ok_or_else(|| ApiError::bad_request("Previous authorization not found"))?;
        if budget.current_grant.id != previous && budget.current_grant.id != selection.operation_id
        {
            return Err(ApiError::bad_request(
                "Another authorization replaced this Builder request",
            ));
        }
        budget.current_grant = state
            .application
            .conversation_builder_grant(project, conversation, task, previous)
            .map_err(ApiError::bad_request)?;
    }
    if previous == AuthorizationBase::Initial {
        if budget
            .as_ref()
            .is_some_and(|budget| budget.current_grant.id != selection.operation_id)
        {
            return Err(ApiError::bad_request(
                "Task authorization changed; review the cumulative scope",
            ));
        }
        // A retry after initial grant persistence uses its original zero-grant preview.
        // Existing operation receipts were already handled by launch before reaching scope.
        budget = None;
    }
    let schema = state
        .application
        .conversation_schema_draft(
            project,
            selection.schema_id,
            Some(selection.schema_revision),
        )
        .map_err(ApiError::bad_request)?;
    if schema.task_id != task {
        return Err(ApiError::bad_request("Schema belongs to another task"));
    }
    let selected = state
        .application
        .resolve_pipeline_builder_model(project, selection.model_id)
        .map_err(ApiError::bad_request)?;
    let mut config = selected
        .openai_compatible_config()
        .map_err(ApiError::bad_request)?;
    config.max_retries = 0;
    config.max_output_tokens = config.max_output_tokens.min(4096);
    let maximum_calls = budget
        .as_ref()
        .map_or(0, |budget| budget.current_grant.maximum_calls)
        .saturating_add(8)
        .min(128);
    let used_calls = budget.as_ref().map_or(0, |budget| budget.used_calls);
    let previous_id = budget.as_ref().map(|budget| budget.current_grant.id);
    if maximum_calls == used_calls {
        return Err(ApiError::bad_request(
            "Task cumulative call limit exhausted",
        ));
    }
    let canonical_selection = BuilderSelection {
        model_id: Some(selected.model.id),
        ..selection.clone()
    };
    let repair = selection
        .repair_request_id
        .map(|request| {
            state
                .application
                .conversation_builder_repair(project, conversation, task, request)
        })
        .transpose()
        .map_err(ApiError::bad_request)?;
    let image_class_repair = selection
        .image_class_review_id
        .map(|id| {
            state.application.conversation_image_class_builder_repair(
                project,
                conversation,
                task,
                id,
            )
        })
        .transpose()
        .map_err(ApiError::bad_request)?;
    if image_class_repair.as_ref().is_some_and(|source| {
        source.schema_id != selection.schema_id
            || source.schema_revision != selection.schema_revision
    }) {
        return Err(ApiError::bad_request(
            "Class repair must use the exact sealed sample Schema",
        ));
    }
    let mut context = json!({"contract":"conversation-builder-v1","selection":canonical_selection,"repair":repair,"schema":schema,"model":selected.model,"provider":selected.provider,"config":config,"previous":previous_id,"maximum_calls":maximum_calls,"images":0,"dry_runs":0});
    if image_class_repair.is_some() {
        context["image_class_repair"] = json!(image_class_repair);
    }
    let hash =
        annotagent_image_tools::sha256(&serde_json::to_vec(&context).map_err(ApiError::internal)?);
    Ok((
        selected.clone(),
        json!({"selection":BuilderSelection { model_id:Some(selected.model.id),..selection.clone() },"repair":repair,"image_class_repair":image_class_repair,"previous_grant_id":previous_id,"scope_hash":hash,"model_name":selected.model.display_name,"remote_model":selected.model.remote_model_id,"destination":selected.provider.endpoint_summary(),"used_calls":used_calls,"maximum_calls":maximum_calls,"maximum_builder_calls":maximum_calls.saturating_sub(used_calls).min(16),"image_count":0,"estimated_cost":null,"expires_at":Utc::now()+Duration::minutes(30),"data_scope":if repair.is_some() || image_class_repair.is_some() {"Saved goal, Schema, preserved Draft, scoped human feedback and terminal result metadata, and Registry descriptions. No image pixels."} else {"Saved goal, Schema and registered model/skill descriptions. No image pixels."},"operation":if repair.is_some() || image_class_repair.is_some() {"Repair this exact editable Draft only. No sample inference, publication or dataset Run."} else {"Build an editable Pipeline Draft only. No sample inference, publication or dataset Run."}}),
    ))
}
pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(selection): Query<BuilderSelection>,
) -> ApiResult<Json<Value>> {
    scope(
        &state,
        &project,
        conversation,
        task,
        &selection,
        AuthorizationBase::Preview,
    )
    .and_then(|(_, mut preview)| {
        preview["project_call_limit"] = json!(
            state
                .application
                .project_conversation_call_limit(&project)
                .map_err(ApiError::bad_request)?
        );
        Ok(Json(preview))
    })
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BuilderHistoryQuery {
    operation_id: Option<uuid::Uuid>,
    image_class_review_id: Option<uuid::Uuid>,
}
pub(super) async fn history(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(query): Query<BuilderHistoryQuery>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .conversation_builder_history_scoped(
            &project,
            conversation,
            task,
            query.operation_id,
            query.image_class_review_id,
        )
        .map(Json)
        .map_err(ApiError::bad_request)
}
pub(super) async fn launch(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(consent): Json<BuilderConsent>,
) -> ApiResult<Json<annotagent_storage::ConversationBuilderOperation>> {
    if consent.selection.repair_request_id
        != consent.repair.as_ref().map(|repair| repair.request_id)
        || consent.selection.image_class_review_id
            != consent
                .image_class_repair
                .as_ref()
                .map(|repair| repair.review_id)
        || (consent.repair.is_some() && consent.image_class_repair.is_some())
    {
        return Err(ApiError::bad_request(
            "Repair consent must identify exactly one previewed correction source and Draft revision",
        ));
    }
    if !consent.allow_unknown_cost
        || consent.expires_at <= Utc::now()
        || consent.expires_at > Utc::now() + Duration::minutes(31)
    {
        return Err(ApiError::bad_request(
            "Review and confirm the current bounded Builder authorization",
        ));
    }
    // A repeated POST reads its own persisted operation, never advances the allowance again.
    let saved = state
        .application
        .conversation_builder_operation(
            &project,
            conversation,
            task,
            consent.selection.operation_id,
        )
        .map_err(ApiError::bad_request)?;
    if let Some(saved) = saved {
        let selected = state
            .application
            .resolve_pipeline_builder_model(&project, consent.selection.model_id)
            .map_err(ApiError::bad_request)?;
        let settings = state.settings.read().await.clone();
        let execution = ConversationBuilderExecution {
            conversation_id: conversation,
            task_id: task,
            schema_id: consent.selection.schema_id,
            schema_revision: consent.selection.schema_revision,
            operation_id: consent.selection.operation_id,
            scope_hash: consent.scope_hash.clone(),
            repair: consent.repair.clone(),
            image_class_repair: consent.image_class_repair.clone(),
        };
        let hash=annotagent_image_tools::sha256(&serde_json::to_vec(&json!({"execution":execution,"model":selected.safe_selection(),"settings":settings})).map_err(ApiError::internal)?);
        if saved.request_hash != hash {
            return Err(ApiError::bad_request(
                "Builder operation request key conflicts",
            ));
        }
        return Ok(Json(saved));
    }
    let (selected, preview) = scope(
        &state,
        &project,
        conversation,
        task,
        &consent.selection,
        consent
            .previous_grant_id
            .map_or(AuthorizationBase::Initial, AuthorizationBase::Existing),
    )?;
    if preview["scope_hash"] != consent.scope_hash
        || preview["previous_grant_id"]
            != serde_json::to_value(consent.previous_grant_id).map_err(ApiError::internal)?
        || preview["repair"] != serde_json::to_value(&consent.repair).map_err(ApiError::internal)?
        || preview["image_class_repair"]
            != serde_json::to_value(&consent.image_class_repair).map_err(ApiError::internal)?
    {
        return Err(ApiError::bad_request(
            "Builder authorization changed. Reload saved operation state before retrying.",
        ));
    }
    let credential = resolve_provider_credential(&state, &selected.provider)
        .await?
        .ok_or_else(|| {
            ApiError::bad_request("Provider credential missing; no Builder request sent")
        })?;
    let mut config = selected
        .openai_compatible_config()
        .map_err(ApiError::bad_request)?;
    config.max_retries = 0;
    config.max_output_tokens = config.max_output_tokens.min(4096);
    let provider = OpenAiCompatibleProvider::new_with_api_key(
        config,
        Some(credential.expose_secret().to_owned()),
    )
    .map_err(ApiError::bad_request)?;
    let grant = ConversationCallGrant {
        id: consent.selection.operation_id,
        task_id: task,
        scope_hash: consent.scope_hash.clone(),
        maximum_calls: u32::try_from(preview["maximum_calls"].as_u64().unwrap_or(0))
            .map_err(ApiError::internal)?,
        expires_at: consent.expires_at,
    };
    if let Some(previous) = consent.previous_grant_id {
        state
            .application
            .advance_conversation_builder_authorization(&project, conversation, previous, &grant)
            .map_err(ApiError::bad_request)?;
    } else {
        state
            .application
            .initial_conversation_builder_authorization(&project, conversation, &grant)
            .map_err(ApiError::bad_request)?;
    }
    let settings = state.settings.read().await.clone();
    let execution = ConversationBuilderExecution {
        conversation_id: conversation,
        task_id: task,
        schema_id: consent.selection.schema_id,
        schema_revision: consent.selection.schema_revision,
        operation_id: consent.selection.operation_id,
        scope_hash: consent.scope_hash,
        repair: consent.repair,
        image_class_repair: consent.image_class_repair,
    };
    state
        .application
        .build_conversation_pipeline(
            &project,
            &execution,
            &settings,
            &selected,
            &provider,
            CancellationToken::default(),
        )
        .await
        .map(Json)
        .map_err(ApiError::bad_request)
}
