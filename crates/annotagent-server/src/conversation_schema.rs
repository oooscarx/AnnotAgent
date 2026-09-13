//! Explicit consent boundary for one text-only Schema proposal. No publish/start.
use super::*;
use annotagent_application::{
    ConversationSchemaExecution, ConversationSchemaRequestConfig, PipelineBuilderModelRuntime,
};
use annotagent_storage::ConversationCallReceipt;
use chrono::{Duration, Utc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaRevisionQuery {
    revision: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaEdit {
    request_id: uuid::Uuid,
    expected_revision: u64,
    decision: annotagent_application::ConversationSchemaDecision,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HumanSchemaInput {
    request_id: uuid::Uuid,
    decision: annotagent_application::ConversationSchemaDecision,
    clarification: Option<annotagent_storage::SchemaClarificationRef>,
    journey_consent_id: Option<uuid::Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OutputClarificationAnswer {
    command_id: uuid::Uuid,
    expected_schema_revision: String,
    journey_consent_id: uuid::Uuid,
    choice: annotagent_application::SchemaOutputChoice,
}

pub(super) async fn clarification(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<Value>> {
    let clarification = state
        .application
        .schema_clarification(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?;
    let choices = state
        .application
        .schema_clarification_choices(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?;
    let answer = (!choices.is_empty() && clarification.status == "pending").then(|| {
        json!({
            "method":"POST",
            "url":format!("/api/projects/{project}/conversations/{conversation}/tasks/{task}/calls/{call}/clarification/answer"),
            "required_fields":["command_id","expected_schema_revision","journey_consent_id","choice"]
        })
    });
    let mut view = serde_json::to_value(clarification).map_err(ApiError::internal)?;
    view["choices"] = serde_json::to_value(choices).map_err(ApiError::internal)?;
    view["answer"] = json!(answer);
    Ok(Json(view))
}

pub(super) async fn answer_clarification(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(input): Json<OutputClarificationAnswer>,
) -> ApiResult<Json<Value>> {
    let clarification = state
        .application
        .schema_clarification(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?;
    if clarification.expected_schema_revision != input.expected_schema_revision {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({
                "status":409,
                "code":"schema_clarification_revision_conflict",
                "error":"Clarification Schema revision changed; reload before answering",
                "expected_schema_revision":input.expected_schema_revision,
                "current_schema_revision":clarification.expected_schema_revision,
                "admitted":false,
                "suggested_action":"reload_clarification"
            }),
        });
    }
    let choices = state
        .application
        .schema_clarification_choices(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?;
    let selected = choices
        .iter()
        .find(|choice| choice.value == input.choice)
        .ok_or_else(|| ApiError::bad_request("Unknown clarification choice"))?;
    if !selected.supported {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({
                "status":409,
                "code":selected.unsupported_reason_code,
                "error":selected.unsupported_reason,
                "choice":input.choice,
                "admitted":false,
                "suggested_action":"choose_supported_output_or_edit_schema"
            }),
        });
    }
    let journey = state
        .application
        .conversation_journey_consent(&project, conversation, task, input.journey_consent_id)
        .map_err(ApiError::bad_request)?
        .ok_or_else(|| ApiError::bad_request("Journey authorization not found"))?;
    if !journey.consent.continue_after_clarification
        || journey
            .consent
            .schema_proposal
            .as_ref()
            .map(|proposal| proposal.call_id)
            != Some(call)
    {
        return Err(ApiError::bad_request(
            "This answer is not linked to the authorized journey clarification",
        ));
    }
    let saved = state
        .application
        .answer_schema_output_clarification(
            &project,
            conversation,
            task,
            call,
            input.command_id,
            &input.expected_schema_revision,
            input.choice,
        )
        .map_err(|error| ApiError {
            status: StatusCode::CONFLICT,
            body: json!({
                "status":409,
                "code":"schema_clarification_answer_conflict",
                "error":error.to_string(),
                "admitted":false,
                "suggested_action":"reload_clarification"
            }),
        })?;
    let schema_id = saved.id;
    let mut resume = json!({"consent_id":input.journey_consent_id});
    match state.application.queue_conversation_journey_answer(
        &project,
        conversation,
        task,
        input.journey_consent_id,
        schema_id,
    ) {
        Err(error) => resume["error"] = json!(error.to_string()),
        Ok(()) => {
            resume["status"] = match conversation_journey::execute(
                State(state.clone()),
                AxumPath((
                    project.clone(),
                    conversation,
                    task,
                    input.journey_consent_id,
                )),
                Json(conversation_journey::ExecuteJourney {}),
            )
            .await
            {
                Ok(value) => value.0,
                Err(error) => json!({"error":error.body["error"]}),
            };
        }
    }
    let updated = state
        .application
        .schema_clarification(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({
        "clarification":updated,
        "schema":saved,
        "selected_choice":input.choice,
        "journey_resume":resume
    })))
}

pub(super) async fn cancel_clarification(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
    Json(reference): Json<annotagent_storage::SchemaClarificationRef>,
) -> ApiResult<Json<annotagent_storage::SchemaClarification>> {
    if call != reference.call_id {
        return Err(ApiError::bad_request("Clarification identity changed"));
    }
    state
        .application
        .cancel_schema_clarification(&project, conversation, task, &reference)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn human_drafts(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Vec<annotagent_storage::ConversationSchemaDraft>>> {
    state
        .application
        .human_conversation_schema_drafts(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn save_human_draft(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<HumanSchemaInput>,
) -> ApiResult<Json<Value>> {
    if let Some(id) = input.journey_consent_id {
        let saved = state
            .application
            .conversation_journey_consent(&project, conversation, task, id)
            .map_err(ApiError::bad_request)?
            .ok_or_else(|| ApiError::bad_request("Journey authorization not found"))?;
        if !saved.consent.continue_after_clarification
            || saved
                .consent
                .schema_proposal
                .as_ref()
                .map(|proposal| proposal.call_id)
                != input
                    .clarification
                    .as_ref()
                    .map(|reference| reference.call_id)
            || input.clarification.is_none()
        {
            return Err(ApiError::bad_request(
                "This answer is not linked to the authorized journey clarification",
            ));
        }
    }
    let saved = state
        .application
        .save_human_schema_with_clarification(
            &project,
            conversation,
            task,
            input.request_id,
            &input.decision,
            input.clarification.as_ref(),
        )
        .map_err(ApiError::bad_request)?;
    let schema_id = saved.id;
    let mut result = serde_json::to_value(saved).map_err(ApiError::internal)?;
    if let Some(id) = input.journey_consent_id {
        // The answer is already durable. Failure to admit continuation must not
        // turn a saved human answer into an ambiguous failed-save response.
        let queued = state.application.queue_conversation_journey_answer(
            &project,
            conversation,
            task,
            id,
            schema_id,
        );
        if let Err(error) = queued {
            result["journey_resume"] = json!({"consent_id":id,"error":error.to_string()});
            return Ok(Json(result));
        }
        result["journey_resume"] = match conversation_journey::execute(
            State(state),
            AxumPath((project, conversation, task, id)),
            Json(conversation_journey::ExecuteJourney {}),
        )
        .await
        {
            Ok(value) => json!({"consent_id":id,"status":value.0}),
            Err(error) => json!({"consent_id":id,"error":error.body["error"]}),
        };
    }
    Ok(Json(result))
}

pub(super) async fn save_draft(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<annotagent_storage::ConversationSchemaDraft>> {
    state
        .application
        .save_conversation_schema_draft(&project, conversation, task, call)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn draft_for_call(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<Option<annotagent_storage::ConversationSchemaDraft>>> {
    state
        .application
        .conversation_schema_for_call(&project, conversation, task, call)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn read_draft(
    State(state): State<ServerState>,
    AxumPath((project, draft)): AxumPath<(String, uuid::Uuid)>,
    Query(query): Query<SchemaRevisionQuery>,
) -> ApiResult<Json<annotagent_storage::ConversationSchemaDraft>> {
    state
        .application
        .conversation_schema_draft(&project, draft, query.revision)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn edit_draft(
    State(state): State<ServerState>,
    AxumPath((project, draft)): AxumPath<(String, uuid::Uuid)>,
    Json(edit): Json<SchemaEdit>,
) -> ApiResult<Json<annotagent_storage::ConversationSchemaDraft>> {
    state
        .application
        .revise_conversation_schema_draft(
            &project,
            draft,
            edit.request_id,
            edit.expected_revision,
            &edit.decision,
        )
        .map(Json)
        .map_err(ApiError::bad_request)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModelSelection {
    model_id: Option<ModelProfileId>,
}

pub(super) type SchemaConsent = annotagent_storage::ConversationSchemaAuthorization;

fn official_zhipu_chat_endpoint(selected: &PipelineBuilderModelRuntime) -> bool {
    selected
        .provider
        .base_url
        .host_str()
        .is_some_and(|host| host == "open.bigmodel.cn" || host.ends_with(".bigmodel.cn"))
}

fn unsupported_response_mode(message: impl Into<String>) -> ApiError {
    ApiError {
        status: StatusCode::CONFLICT,
        body: json!({
            "status":409,"code":"schema_response_mode_unsupported",
            "error":message.into(),"admitted":false,
            "suggested_action":"update_model_profile_and_preview_again"
        }),
    }
}

pub(super) fn schema_stage_config(
    selected: &PipelineBuilderModelRuntime,
) -> ApiResult<(
    annotagent_provider::OpenAiCompatibleConfig,
    ConversationSchemaRequestConfig,
)> {
    let mut provider = selected
        .openai_compatible_config()
        .map_err(ApiError::bad_request)?;
    provider.max_retries = 0;
    let mode = selected
        .model
        .generation_defaults
        .structured_output_mode
        .as_deref();
    let response_mode = match mode {
        Some("json_object") => {
            if !selected.model.protocol_features.structured_output {
                return Err(unsupported_response_mode(
                    "Model Profile selects json_object without structured-output support",
                ));
            }
            annotagent_provider::OpenAiResponseMode::JsonObject
        }
        Some("json_schema") => {
            if !selected.model.protocol_features.json_schema {
                return Err(unsupported_response_mode(
                    "Model Profile selects json_schema without JSON Schema support",
                ));
            }
            annotagent_provider::OpenAiResponseMode::JsonSchema
        }
        Some("tool" | "native_tool") => {
            if !selected.model.protocol_features.tool_calls {
                return Err(unsupported_response_mode(
                    "Model Profile selects native tools without tool-call support",
                ));
            }
            annotagent_provider::OpenAiResponseMode::NativeTool
        }
        Some(other) => {
            return Err(unsupported_response_mode(format!(
                "Unsupported Schema structured-output mode {other:?}"
            )));
        }
        None if official_zhipu_chat_endpoint(selected)
            && selected.model.protocol_features.structured_output =>
        {
            annotagent_provider::OpenAiResponseMode::JsonObject
        }
        None if selected.model.protocol_features.tool_calls => {
            annotagent_provider::OpenAiResponseMode::NativeTool
        }
        None if selected.model.protocol_features.structured_output => {
            annotagent_provider::OpenAiResponseMode::JsonObject
        }
        None => {
            return Err(unsupported_response_mode(
                "Schema planning needs a declared native-tool or JSON Object capability",
            ));
        }
    };
    provider.response_mode = response_mode;

    // Schema extraction defaults to non-thinking only when the saved capability or
    // the official endpoint dialect defines an exact wire representation.
    let (thinking_parameter, thinking_value) =
        if selected.model.protocol_features.reasoning_controls {
            use annotagent_core::ReasoningWireParameter as Wire;
            let wire = selected.model.generation_defaults.reasoning_wire_parameter;
            match wire {
                Some(Wire::Thinking) => {
                    provider.reasoning_mode = None;
                    provider.extra_request_fields.remove("enable_thinking");
                    provider
                        .extra_request_fields
                        .insert("thinking".to_owned(), json!({"type":"disabled"}));
                    (Some(Wire::Thinking), Some(json!({"type":"disabled"})))
                }
                None if official_zhipu_chat_endpoint(selected) => {
                    provider.reasoning_mode = None;
                    provider.extra_request_fields.remove("enable_thinking");
                    provider
                        .extra_request_fields
                        .insert("thinking".to_owned(), json!({"type":"disabled"}));
                    (Some(Wire::Thinking), Some(json!({"type":"disabled"})))
                }
                Some(Wire::EnableThinking) => {
                    provider.reasoning_mode = None;
                    provider.extra_request_fields.remove("thinking");
                    provider
                        .extra_request_fields
                        .insert("enable_thinking".to_owned(), json!(false));
                    (Some(Wire::EnableThinking), Some(json!(false)))
                }
                Some(Wire::ReasoningEffort)
                    if selected
                        .model
                        .generation_defaults
                        .supported_reasoning_modes
                        .contains("none") =>
                {
                    provider.reasoning_mode = Some("none".to_owned());
                    provider.extra_request_fields.remove("thinking");
                    provider.extra_request_fields.remove("enable_thinking");
                    (Some(Wire::ReasoningEffort), Some(json!("none")))
                }
                Some(Wire::ReasoningEffort) => (
                    Some(Wire::ReasoningEffort),
                    provider.reasoning_mode.clone().map(Value::String),
                ),
                None => (None, None),
            }
        } else {
            (None, None)
        };
    let request = ConversationSchemaRequestConfig {
        maximum_output_tokens: provider.max_output_tokens,
        response_mode,
        thinking_parameter,
        thinking_value,
        retry_of: None,
    };
    Ok((provider, request))
}

pub(super) async fn pending_authorization(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Option<SchemaConsent>>> {
    state
        .application
        .pending_conversation_schema_authorization(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) fn preview_scope(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    model_id: Option<ModelProfileId>,
) -> ApiResult<(PipelineBuilderModelRuntime, Value)> {
    let delivery = state
        .application
        .task_delivery_intent(project, conversation, task)
        .map_err(ApiError::bad_request)?;
    let task_record = state
        .application
        .conversation_tasks(project, conversation)
        .map_err(ApiError::bad_request)?
        .into_iter()
        .find(|item| item.input.id == task)
        .ok_or_else(|| ApiError::not_found("Task not found in this conversation"))?;
    let goal = state
        .application
        .project_goal(project)
        .map_err(ApiError::bad_request)?;
    if goal["revision"].as_str() != Some(&task_record.input.schema_revision) {
        return Err(ApiError::bad_request(
            "Project Schema changed since this task started. No request was sent.",
        ));
    }
    let selected = state
        .application
        .resolve_conversation_message_model(
            project,
            conversation,
            task_record.input.source_message_id,
            model_id,
        )
        .map_err(ApiError::bad_request)?;
    let (config, request_config) = schema_stage_config(&selected)?;
    // Hash complete server-resolved configuration; never include credentials in public output.
    let mut consent_scope = json!({
        "contract":"conversation-schema-consent-v1", "task":task_record,
        "model":selected.model, "provider":selected.provider, "config":config,
        "request_config":request_config,
        "maximum_calls":1, "image_count":0,
    });
    if let Some(saved) = &delivery.saved {
        consent_scope["delivery"] =
            json!({"revision":saved.revision,"sha256":saved.content_sha256});
    }
    let scope_hash = annotagent_image_tools::sha256(
        &serde_json::to_vec(&consent_scope).map_err(ApiError::internal)?,
    );
    let preview = json!({
        "task_id":task,"model_id":selected.model.id,"model_name":selected.model.display_name,
        "remote_model":selected.model.remote_model_id,"destination":selected.provider.endpoint_summary(),
        "scope_hash":scope_hash,"maximum_calls":1,"image_count":0,"estimated_cost":null,
        "maximum_output_tokens":config.max_output_tokens,"expires_at":Utc::now()+Duration::minutes(30),
        "response_mode":request_config.response_mode,
        "thinking":{"parameter":request_config.thinking_parameter,"value":request_config.thinking_value},
        "data_scope":"Saved goal text, saved delivery labels/rules/training target and existing schema definitions only. No image pixels.",
        "operation":"Propose annotation Schema only; does not publish, run the dataset or accept annotations.",
    });
    Ok((selected, preview))
}

pub(super) async fn preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(selection): Query<ModelSelection>,
) -> ApiResult<Json<Value>> {
    for call in state
        .application
        .conversation_schema_calls(&project, conversation, task)
        .map_err(ApiError::bad_request)?
        .into_iter()
        .rev()
    {
        let initial_authorization = state
            .application
            .conversation_schema_authorization(&project, conversation, task, call.id)
            .map_err(ApiError::bad_request)?;
        let retry_authorization = state
            .application
            .conversation_schema_retry(&project, conversation, task, call.id)
            .map_err(ApiError::bad_request)?;
        if initial_authorization.is_none() && retry_authorization.is_none() {
            continue;
        }
        let draft = state
            .application
            .conversation_schema_for_call(&project, conversation, task, call.id)
            .map_err(ApiError::bad_request)?;
        let suggested_action = if draft.is_some() {
            "continue_with_saved_schema"
        } else {
            match call.status {
                annotagent_storage::ConversationCallStatus::Reserved => "poll_existing_call",
                annotagent_storage::ConversationCallStatus::InDoubt => "wait_for_remote_outcome",
                annotagent_storage::ConversationCallStatus::Completed => {
                    "review_schema_retry_preview"
                }
                annotagent_storage::ConversationCallStatus::Failed => "inspect_existing_failure",
            }
        };
        let root = format!("/api/projects/{project}/conversations/{conversation}/tasks/{task}");
        let retry_preview_url = if draft.is_none()
            && call.status == annotagent_storage::ConversationCallStatus::Completed
        {
            Some(selection.model_id.map_or_else(
                || format!("{root}/schema-retry-preview?retry_of={}", call.id),
                |model| {
                    format!(
                        "{root}/schema-retry-preview?retry_of={}&model_id={model}",
                        call.id
                    )
                },
            ))
        } else {
            None
        };
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({
                "status":409,
                "code":"schema_authorization_already_exists",
                "error":"This Task already has an exact Schema authorization. A new preview cannot replace its scope or reset its budget.",
                "admitted":false,
                "existing_call_id":call.id,
                "existing_call_status":call.status,
                "schema_draft_id":draft.as_ref().map(|value|value.id),
                "receipt_url":format!("{root}/calls/{}",call.id),
                "schema_draft_url":draft.as_ref().map(|_|format!("{root}/calls/{}/schema-draft",call.id)),
                "pending_authorization_url":format!("{root}/schema-authorizations/pending"),
                "retry_preview_url":retry_preview_url,
                "suggested_action":suggested_action
            }),
        });
    }
    preview_scope(&state, &project, conversation, task, selection.model_id).and_then(
        |(_, mut preview)| {
            preview["project_call_limit"] = json!(
                state
                    .application
                    .project_conversation_call_limit(&project)
                    .map_err(ApiError::bad_request)?
            );
            Ok(Json(preview))
        },
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaRetrySelection {
    retry_of: uuid::Uuid,
    model_id: Option<ModelProfileId>,
}

fn retryable_source(receipt: &ConversationCallReceipt) -> ApiResult<()> {
    if receipt.status != annotagent_storage::ConversationCallStatus::Completed
        || receipt
            .evidence
            .as_ref()
            .and_then(|value| value.pointer("/decision/Err"))
            .is_none()
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({
                "status":409,"code":"schema_retry_source_not_retryable",
                "error":"Only a settled invalid structured Schema response can be retried with this operation",
                "admitted":false,"suggested_action":"inspect_original_receipt"
            }),
        });
    }
    if receipt
        .evidence
        .as_ref()
        .and_then(|value| value.pointer("/diagnostic/failure_code"))
        .and_then(Value::as_str)
        == Some("provider_filtered")
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({
                "status":409,"code":"schema_retry_requires_new_task_decision",
                "error":"Filtered or refused output is not eligible for this structured-output repair retry",
                "admitted":false,"suggested_action":"review_provider_or_task_policy"
            }),
        });
    }
    Ok(())
}

fn retry_scope(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    retry_of: uuid::Uuid,
    requested_model: Option<ModelProfileId>,
    saved_command: Option<&annotagent_storage::ConversationSchemaRetryAuthorization>,
) -> ApiResult<(
    PipelineBuilderModelRuntime,
    ConversationSchemaRequestConfig,
    Value,
)> {
    let source = state
        .application
        .conversation_call_receipt(project, conversation, task, retry_of)
        .map_err(ApiError::bad_request)?
        .ok_or_else(|| ApiError::not_found("Schema retry source receipt not found"))?;
    retryable_source(&source)?;
    let source_model = if let Some(initial) = state
        .application
        .conversation_schema_authorization(project, conversation, task, retry_of)
        .map_err(ApiError::bad_request)?
    {
        initial.model_id
    } else {
        state
            .application
            .conversation_schema_retry(project, conversation, task, retry_of)
            .map_err(ApiError::bad_request)?
            .map(|retry| retry.model_id)
            .ok_or_else(|| ApiError::bad_request("Schema retry source has no saved model scope"))?
    };
    if requested_model.is_some_and(|model| model != source_model) {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({
                "status":409,"code":"schema_retry_model_changed",
                "error":"Retry keeps the original Model Profile; review a separate task change to use another Provider",
                "admitted":false,"source_model_id":source_model
            }),
        });
    }
    let (selected, base) = preview_scope(state, project, conversation, task, Some(source_model))?;
    let (provider, mut request_config) = schema_stage_config(&selected)?;
    request_config.retry_of = Some(retry_of);
    let budget = state
        .application
        .optional_conversation_builder_budget(project, conversation, task)
        .map_err(ApiError::bad_request)?
        .ok_or_else(|| ApiError::bad_request("Original Schema call has no saved allowance"))?;
    let (previous_grant_id, maximum_calls, expires_at) = if let Some(saved) = saved_command {
        if saved.retry_of != retry_of
            || saved.model_id != source_model
            || budget.current_grant.id != saved.call_id
            || budget.current_grant.scope_hash != saved.scope_hash
            || budget.current_grant.maximum_calls != saved.maximum_calls
            || budget.current_grant.expires_at != saved.expires_at
            || budget.revoked
        {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                body: json!({"status":409,"code":"schema_retry_scope_changed","error":"Saved Schema retry is no longer the current active authorization; no request was sent","admitted":false,"suggested_action":"inspect_retry_receipt"}),
            });
        }
        (
            saved.previous_grant_id,
            saved.maximum_calls,
            saved.expires_at,
        )
    } else {
        (
            budget.current_grant.id,
            budget
                .current_grant
                .maximum_calls
                .max(budget.used_calls.saturating_add(1)),
            Utc::now() + Duration::minutes(30),
        )
    };
    let scope_hash = annotagent_image_tools::sha256(
        &serde_json::to_vec(&json!({
            "contract":"conversation-schema-retry-v1",
            "retry_of":retry_of,
            "source_request_hash":source.request_hash,
            "source_failure":source.evidence.as_ref().and_then(|value|value.get("diagnostic")),
            "base_scope_hash":base["scope_hash"],
            "model":selected.model,
            "provider":selected.provider,
            "provider_config":provider,
            "request_config":request_config,
            "previous_grant_id":previous_grant_id,
            "maximum_calls":maximum_calls,
        }))
        .map_err(ApiError::internal)?,
    );
    let preview = json!({
        "contract":"conversation-schema-retry-v1",
        "retry_of":retry_of,
        "source_status":source.status,
        "source_request_hash":source.request_hash,
        "source_diagnostic":source.evidence.as_ref().and_then(|value|value.get("diagnostic")),
        "model_id":selected.model.id,
        "model_name":selected.model.display_name,
        "destination":selected.provider.endpoint_summary(),
        "previous_grant_id":previous_grant_id,
        "maximum_calls":maximum_calls,
        "new_request_limit":1,
        "scope_hash":scope_hash,
        "maximum_output_tokens":request_config.maximum_output_tokens,
        "response_mode":request_config.response_mode,
        "thinking":{"parameter":request_config.thinking_parameter,"value":request_config.thinking_value},
        "estimated_cost":null,
        "expires_at":expires_at,
        "operation":"Retry this saved Schema interpretation once with the corrected structured-output configuration. No image inference, publication or annotation acceptance."
    });
    Ok((selected, request_config, preview))
}

pub(super) async fn retry_preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(selection): Query<SchemaRetrySelection>,
) -> ApiResult<Json<Value>> {
    retry_scope(
        &state,
        &project,
        conversation,
        task,
        selection.retry_of,
        selection.model_id,
        None,
    )
    .map(|(_, _, preview)| Json(preview))
}

pub(super) async fn retry_receipt(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<Value>> {
    let authorization = state
        .application
        .conversation_schema_retry(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?
        .ok_or_else(|| ApiError::not_found("Schema retry command not found"))?;
    let receipt = state
        .application
        .conversation_call_receipt(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?;
    Ok(Json(
        json!({"authorization":authorization,"receipt":receipt}),
    ))
}

pub(super) async fn retry(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(input): Json<annotagent_storage::ConversationSchemaRetryAuthorization>,
) -> ApiResult<Json<Value>> {
    let saved_command = state
        .application
        .conversation_schema_retry(&project, conversation, task, input.call_id)
        .map_err(ApiError::bad_request)?;
    if let Some(saved) = &saved_command {
        if saved != &input {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                body: json!({"status":409,"code":"schema_retry_command_conflict","error":"Schema retry command changed its saved scope","admitted":false}),
            });
        }
        if let Some(receipt) = state
            .application
            .conversation_call_receipt(&project, conversation, task, input.call_id)
            .map_err(ApiError::bad_request)?
        {
            let valid = receipt
                .evidence
                .as_ref()
                .and_then(|value| value.pointer("/decision/Ok/decision"))
                .is_some();
            let (journey_id, queued) = if valid {
                super::conversation_journey::resume_after_schema_retry(
                    &state,
                    &project,
                    conversation,
                    task,
                    saved.retry_of,
                )?
            } else {
                (None, false)
            };
            return Ok(Json(json!({
                "authorization":saved,"receipt":receipt,"replayed":true,
                "journey_resume":{"journey_id":journey_id,"queued":queued}
            })));
        }
    }
    if !input.allow_unknown_cost {
        return Err(ApiError::bad_request(
            "Confirm unknown Provider cost for this one corrected Schema request",
        ));
    }
    let (selected, request_config, preview) = retry_scope(
        &state,
        &project,
        conversation,
        task,
        input.retry_of,
        Some(input.model_id),
        saved_command.as_ref(),
    )?;
    if preview["scope_hash"] != input.scope_hash
        || preview["previous_grant_id"] != json!(input.previous_grant_id)
        || preview["maximum_calls"] != input.maximum_calls
        || input.expires_at <= Utc::now()
        || input.expires_at > Utc::now() + Duration::minutes(31)
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({"status":409,"code":"schema_retry_scope_changed","error":"Schema retry configuration, model, task or allowance changed; review a fresh preview","admitted":false,"suggested_action":"reload_retry_preview"}),
        });
    }
    let credential = resolve_provider_credential(&state, &selected.provider)
        .await?
        .ok_or_else(|| {
            ApiError::bad_request("Provider credential is missing; no retry was sent")
        })?;
    let (provider_config, rechecked_config) = schema_stage_config(&selected)?;
    if rechecked_config.maximum_output_tokens != request_config.maximum_output_tokens
        || rechecked_config.response_mode != request_config.response_mode
        || rechecked_config.thinking_parameter != request_config.thinking_parameter
        || rechecked_config.thinking_value != request_config.thinking_value
    {
        return Err(ApiError::bad_request(
            "Schema retry configuration changed before dispatch",
        ));
    }
    let attempt_observer = state
        .application
        .task_model_attempt_observer(&project, conversation, task, &selected)
        .map_err(ApiError::bad_request)?;
    let provider = OpenAiCompatibleProvider::new_with_api_key(
        provider_config,
        Some(credential.expose_secret().to_owned()),
    )
    .map_err(ApiError::bad_request)?
    .with_attempt_observer(attempt_observer);
    state
        .application
        .authorize_conversation_schema_retry(&project, conversation, task, &input)
        .map_err(ApiError::bad_request)?;
    let execution = ConversationSchemaExecution {
        conversation_id: conversation,
        task_id: task,
        call_id: input.call_id,
        remote_model: selected.model.remote_model_id,
        scope_hash: input.scope_hash.clone(),
    };
    let receipt = state
        .application
        .execute_conversation_schema_with_config(
            &project,
            &execution,
            &request_config,
            &provider,
            CancellationToken::default(),
        )
        .await
        .map_err(ApiError::bad_request)?;
    let valid = receipt
        .evidence
        .as_ref()
        .and_then(|value| value.pointer("/decision/Ok/decision"))
        .is_some();
    let (journey_id, queued) = if valid {
        super::conversation_journey::resume_after_schema_retry(
            &state,
            &project,
            conversation,
            task,
            input.retry_of,
        )?
    } else {
        (None, false)
    };
    Ok(Json(json!({
        "authorization":input,"receipt":receipt,"replayed":false,
        "journey_resume":{"journey_id":journey_id,"queued":queued}
    })))
}

pub(super) async fn propose(
    state: State<ServerState>,
    path: AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    consent: Json<SchemaConsent>,
) -> ApiResult<Json<ConversationCallReceipt>> {
    Box::pin(propose_owned(state, path, consent, true)).await
}

/// The journey already owns a bounded background worker. Reuse identical consent
/// validation and call admission without consuming a nested worker permit.
pub(super) async fn propose_in_journey(
    state: State<ServerState>,
    path: AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    consent: Json<SchemaConsent>,
) -> ApiResult<Json<ConversationCallReceipt>> {
    Box::pin(propose_owned(state, path, consent, false)).await
}

async fn propose_owned(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Json(consent): Json<SchemaConsent>,
    detach: bool,
) -> ApiResult<Json<ConversationCallReceipt>> {
    if !consent.allow_unknown_cost {
        return Err(ApiError::bad_request(
            "Confirm that actual Provider cost is unknown before sending the bounded text request.",
        ));
    }
    let (selected, preview) =
        preview_scope(&state, &project, conversation, task, Some(consent.model_id))?;
    if preview["scope_hash"].as_str() != Some(consent.scope_hash.as_str())
        || consent.expires_at > Utc::now() + Duration::minutes(31)
    {
        return Err(ApiError::bad_request(
            "Schema authorization scope changed. Review the current model and data scope again; no request was sent.",
        ));
    }
    let credential = resolve_provider_credential(&state, &selected.provider)
        .await?
        .ok_or_else(|| {
            ApiError::bad_request("Provider credential is missing. No Schema request was sent.")
        })?;
    let (config, request_config) = schema_stage_config(&selected)?;
    let attempt_observer = state
        .application
        .task_model_attempt_observer(&project, conversation, task, &selected)
        .map_err(ApiError::bad_request)?;
    let provider = OpenAiCompatibleProvider::new_with_api_key(
        config,
        Some(credential.expose_secret().to_owned()),
    )
    .map_err(ApiError::bad_request)?
    .with_attempt_observer(attempt_observer);
    state
        .application
        .authorize_conversation_schema_request(&project, conversation, task, &consent)
        .map_err(ApiError::bad_request)?;
    let execution = ConversationSchemaExecution {
        conversation_id: conversation,
        task_id: task,
        call_id: consent.call_id,
        remote_model: selected.model.remote_model_id,
        scope_hash: consent.scope_hash,
    };
    if !detach {
        return state
            .application
            .execute_conversation_schema_with_config(
                &project,
                &execution,
                &request_config,
                &provider,
                CancellationToken::default(),
            )
            .await
            .map(Json)
            .map_err(ApiError::bad_request);
    }
    // A dropped HTTP response must not drop an admitted model operation. The
    // existing call receipt/cancellation service remains the execution owner.
    let permit = state.journey_workers.clone().try_acquire_owned().map_err(|_| ApiError {
        status: StatusCode::TOO_MANY_REQUESTS,
        body: json!({"error":"Background planning capacity is full. The original Schema authorization remains saved; no new model call was admitted.","code":"journey_capacity_exhausted"}),
    })?;
    let application = state.application.clone();
    tokio::spawn(async move {
        let _permit = permit;
        application
            .execute_conversation_schema_with_config(
                &project,
                &execution,
                &request_config,
                &provider,
                CancellationToken::default(),
            )
            .await
    })
    .await
    .map_err(|_| {
        ApiError::internal(
            "Schema worker stopped unexpectedly; read the saved call receipt before retrying.",
        )
    })?
    .map(Json)
    .map_err(ApiError::bad_request)
}

pub(super) async fn receipt(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<ConversationCallReceipt>> {
    state
        .application
        .conversation_call_receipt(&project, conversation, task, call)
        .map_err(ApiError::bad_request)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("Schema call receipt not found"))
}

pub(super) async fn history(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Vec<ConversationCallReceipt>>> {
    state
        .application
        .conversation_schema_calls(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}
pub(super) async fn cancel(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task, call)): AxumPath<(
        String,
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
    )>,
) -> ApiResult<Json<annotagent_storage::ConversationCallCancellation>> {
    state
        .application
        .cancel_conversation_schema(&project, conversation, task, call)
        .map(Json)
        .map_err(ApiError::bad_request)
}

pub(super) async fn cancellations(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Vec<annotagent_storage::ConversationCallCancellation>>> {
    state
        .application
        .conversation_schema_cancellations(&project, conversation, task)
        .map(Json)
        .map_err(ApiError::bad_request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::{
        CapabilityDeclarationSource, GenerationDefaults, InputModality, ModelBindingSource,
        ModelCapability, ModelLimits, ModelPricing, ModelProfile, ModelProfileStatus,
        ProtocolFeatures, ProviderAdapterKind, ProviderConnectionPolicy, ProviderHealthSnapshot,
        ProviderId, ProviderProfile, ReasoningWireParameter,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn runtime(endpoint: &str, maximum_output_tokens: u32) -> PipelineBuilderModelRuntime {
        let provider_id = ProviderId::new();
        let now = chrono::Utc::now();
        PipelineBuilderModelRuntime {
            provider: ProviderProfile {
                id: provider_id,
                display_name: "TEST provider".to_owned(),
                preset_id: None,
                adapter: ProviderAdapterKind::OpenAiCompatible,
                base_url: endpoint.parse().unwrap(),
                organization: None,
                workspace: None,
                credential_ref: None,
                safe_headers: BTreeMap::new(),
                connection_policy: ProviderConnectionPolicy::default(),
                enabled: true,
                health: ProviderHealthSnapshot::default(),
                created_at: now,
                updated_at: now,
            },
            model: ModelProfile {
                id: annotagent_core::ModelProfileId::new(),
                revision: 3,
                provider_id,
                display_name: "TEST model".to_owned(),
                remote_model_id: "TEST-remote".to_owned(),
                input_modalities: BTreeSet::from([InputModality::Text]),
                protocol_features: ProtocolFeatures {
                    tool_calls: true,
                    structured_output: true,
                    usage_reporting: true,
                    reasoning_controls: true,
                    ..ProtocolFeatures::default()
                },
                task_capabilities: BTreeSet::from([ModelCapability::TextGeneration]),
                capability_source: CapabilityDeclarationSource::UserDeclared,
                limits: ModelLimits {
                    context_tokens: Some(32_768),
                    maximum_output_tokens: Some(8_192),
                    maximum_images_per_request: None,
                    maximum_image_pixels: None,
                },
                generation_defaults: GenerationDefaults {
                    maximum_output_tokens: Some(u64::from(maximum_output_tokens)),
                    structured_output_mode: Some("json_object".to_owned()),
                    reasoning_mode: Some("enabled".to_owned()),
                    reasoning_wire_parameter: Some(ReasoningWireParameter::Thinking),
                    supported_reasoning_modes: BTreeSet::from([
                        "enabled".to_owned(),
                        "disabled".to_owned(),
                    ]),
                    ..GenerationDefaults::default()
                },
                pricing: ModelPricing::default(),
                quality_contracts: Vec::new(),
                status: ModelProfileStatus::Available,
                enabled: true,
                locked: false,
                created_at: now,
                updated_at: now,
            },
            binding_source: ModelBindingSource::GlobalDefault,
            locked: false,
        }
    }

    #[test]
    fn schema_stage_preserves_explicit_caps_and_uses_zhipu_json_object_thinking_wire() {
        for cap in [1_024, 2_048, 4_096] {
            let selected = runtime("https://open.bigmodel.cn/api/paas/v4", cap);
            let (provider, request) = schema_stage_config(&selected).unwrap();
            assert_eq!(provider.max_output_tokens, cap);
            assert_eq!(request.maximum_output_tokens, cap);
            assert_eq!(
                request.response_mode,
                annotagent_provider::OpenAiResponseMode::JsonObject
            );
            assert_eq!(
                request.thinking_parameter,
                Some(ReasoningWireParameter::Thinking)
            );
            assert_eq!(request.thinking_value, Some(json!({"type":"disabled"})));
            assert_eq!(
                provider.extra_request_fields["thinking"],
                json!({"type":"disabled"})
            );
            assert!(provider.reasoning_mode.is_none());
        }
    }

    #[test]
    fn schema_stage_rejects_output_mode_without_saved_capability() {
        let mut selected = runtime("https://provider.invalid/v1", 4_096);
        selected.model.protocol_features.structured_output = false;
        assert!(schema_stage_config(&selected).is_err());
    }
}
