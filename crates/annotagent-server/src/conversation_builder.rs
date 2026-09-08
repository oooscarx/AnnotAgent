use super::*;
use annotagent_application::{
    ConversationBuilderExecution, ConversationBuilderRepair, PipelineBuilderModelRuntime,
};
use annotagent_storage::ConversationCallGrant;
use chrono::{DateTime, Duration, Utc};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BuilderSelection {
    operation_id: uuid::Uuid,
    schema_id: uuid::Uuid,
    schema_revision: u64,
    model_id: Option<ModelProfileId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repair_request_id: Option<uuid::Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BuilderConsent {
    selection: BuilderSelection,
    previous_grant_id: uuid::Uuid,
    scope_hash: String,
    expires_at: DateTime<Utc>,
    allow_unknown_cost: bool,
    #[serde(default)]
    repair: Option<ConversationBuilderRepair>,
}
fn scope(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    selection: &BuilderSelection,
    previous: Option<uuid::Uuid>,
) -> ApiResult<(PipelineBuilderModelRuntime, Value)> {
    let mut budget = state
        .application
        .conversation_builder_budget(project, conversation, task)
        .map_err(ApiError::bad_request)?;
    if budget.revoked {
        return Err(ApiError::bad_request("Task authorization was revoked"));
    }
    if let Some(previous) = previous {
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
        .current_grant
        .maximum_calls
        .saturating_add(8)
        .min(128);
    if maximum_calls == budget.used_calls {
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
    let hash=annotagent_image_tools::sha256(&serde_json::to_vec(&json!({"contract":"conversation-builder-v1","selection":canonical_selection,"repair":repair,"schema":schema,"model":selected.model,"provider":selected.provider,"config":config,"previous":budget.current_grant.id,"maximum_calls":maximum_calls,"images":0,"dry_runs":0})).map_err(ApiError::internal)?);
    Ok((
        selected.clone(),
        json!({"selection":BuilderSelection { model_id:Some(selected.model.id),..selection.clone() },"repair":repair,"previous_grant_id":budget.current_grant.id,"scope_hash":hash,"model_name":selected.model.display_name,"remote_model":selected.model.remote_model_id,"destination":selected.provider.endpoint_summary(),"used_calls":budget.used_calls,"maximum_calls":maximum_calls,"maximum_builder_calls":maximum_calls.saturating_sub(budget.used_calls).min(16),"image_count":0,"estimated_cost":null,"expires_at":Utc::now()+Duration::minutes(30),"data_scope":if repair.is_some() {"Saved goal, Schema, preserved Draft, scoped human feedback and terminal result metadata, and Registry descriptions. No image pixels."} else {"Saved goal, Schema and registered model/skill descriptions. No image pixels."},"operation":if repair.is_some() {"Repair this exact editable Draft only. No sample inference, publication or dataset Run."} else {"Build an editable Pipeline Draft only. No sample inference, publication or dataset Run."}}),
    ))
}
pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(selection): Query<BuilderSelection>,
) -> ApiResult<Json<Value>> {
    scope(&state, &project, conversation, task, &selection, None).map(|(_, preview)| Json(preview))
}
pub(super) async fn history(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .conversation_builder_history(&project, conversation, task)
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
    {
        return Err(ApiError::bad_request(
            "Repair consent must identify the previewed human request and Draft revision",
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
    let history = state
        .application
        .conversation_builder_history(&project, conversation, task)
        .map_err(ApiError::bad_request)?;
    if let Some(saved) = history["items"].as_array().and_then(|items| {
        items
            .iter()
            .find(|item| item["operation"]["id"] == consent.selection.operation_id.to_string())
    }) {
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
        };
        let hash=annotagent_image_tools::sha256(&serde_json::to_vec(&json!({"execution":execution,"model":selected.safe_selection(),"settings":settings})).map_err(ApiError::internal)?);
        if saved["operation"]["request_hash"] != hash {
            return Err(ApiError::bad_request(
                "Builder operation request key conflicts",
            ));
        }
        return serde_json::from_value(saved["operation"].clone())
            .map(Json)
            .map_err(ApiError::internal);
    }
    let (selected, preview) = scope(
        &state,
        &project,
        conversation,
        task,
        &consent.selection,
        Some(consent.previous_grant_id),
    )?;
    if preview["scope_hash"] != consent.scope_hash
        || preview["previous_grant_id"] != consent.previous_grant_id.to_string()
        || preview["repair"] != serde_json::to_value(&consent.repair).map_err(ApiError::internal)?
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
    state
        .application
        .advance_conversation_builder_authorization(
            &project,
            conversation,
            consent.previous_grant_id,
            &grant,
        )
        .map_err(ApiError::bad_request)?;
    let settings = state.settings.read().await.clone();
    let execution = ConversationBuilderExecution {
        conversation_id: conversation,
        task_id: task,
        schema_id: consent.selection.schema_id,
        schema_revision: consent.selection.schema_revision,
        operation_id: consent.selection.operation_id,
        scope_hash: consent.scope_hash,
        repair: consent.repair,
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
