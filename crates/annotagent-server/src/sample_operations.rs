//! HTTP lifecycle adapter around the existing Sample Test service.
use super::*;
use annotagent_application::SampleExecutionControl;
use annotagent_storage::SampleOperation;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartSampleRequest {
    request_id: String,
    draft_id: String,
    image_indices: Vec<usize>,
    expected_revision: u64,
    authorization_fingerprint: String,
    #[serde(default)]
    conversation: Option<ConversationSampleConsent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConversationSampleConsent {
    conversation_id: uuid::Uuid,
    task_id: uuid::Uuid,
    previous_grant_id: uuid::Uuid,
    scope_hash: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    allow_unknown_cost: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    human_review: bool,
}

#[derive(Deserialize)]
pub(super) struct ConversationSampleSelection {
    draft_id: String,
    request_id: uuid::Uuid,
}

#[allow(clippy::too_many_arguments)]
fn conversation_scope(
    state: &ServerState,
    project: &str,
    conversation: uuid::Uuid,
    task: uuid::Uuid,
    draft: &WorkflowDraft,
    fingerprint: &str,
    request: uuid::Uuid,
    previous: Option<uuid::Uuid>,
) -> ApiResult<Value> {
    let mut budget = state
        .application
        .conversation_builder_budget(project, conversation, task)
        .map_err(ApiError::bad_request)?;
    if budget.revoked {
        return Err(ApiError::bad_request("Task authorization was revoked"));
    }
    if let Some(previous) = previous {
        if budget.current_grant.id != previous && budget.current_grant.id != request {
            return Err(ApiError::bad_request(
                "Another authorization replaced this sample request",
            ));
        }
        budget.current_grant = state
            .application
            .conversation_builder_grant(project, conversation, task, previous)
            .map_err(ApiError::bad_request)?;
    }
    let binding = draft.annotation_schema.as_ref().ok_or_else(|| {
        ApiError::bad_request("Bind a saved conversation Schema revision before testing")
    })?;
    let schema = state
        .application
        .conversation_schema_draft(
            project,
            uuid::Uuid::parse_str(&binding.schema_draft_id).map_err(ApiError::bad_request)?,
            Some(binding.revision),
        )
        .map_err(ApiError::bad_request)?;
    if schema.task_id != task || draft.project_id != project {
        return Err(ApiError::bad_request(
            "Sample Draft belongs to another task or Project",
        ));
    }
    let maximum_calls = budget
        .current_grant
        .maximum_calls
        .saturating_add(12)
        .min(128);
    if budget.used_calls >= maximum_calls {
        return Err(ApiError::bad_request("Task call allowance exhausted"));
    }
    let scope_hash = annotagent_image_tools::sha256(&serde_json::to_vec(&json!({"phase":"conversation_samples_v1","project":project,"conversation":conversation,"task":task,"draft_id":draft.id,"fingerprint":fingerprint,"request_id":request,"previous_grant_id":budget.current_grant.id,"maximum_calls":maximum_calls})).map_err(ApiError::internal)?);
    Ok(
        json!({"scope_hash":scope_hash,"previous_grant_id":budget.current_grant.id,"maximum_calls":maximum_calls,"used_calls":budget.used_calls,"expires_at":chrono::Utc::now()+chrono::Duration::minutes(30),"estimated_cost":null}),
    )
}

pub(super) async fn conversation_preview(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
    Query(selection): Query<ConversationSampleSelection>,
) -> ApiResult<Json<Value>> {
    let (draft, models) = state
        .application
        .resolved_workflow_draft_model_profiles(&selection.draft_id)
        .map_err(ApiError::bad_request)?;
    let fingerprint = guided_sample_fingerprint(&state, &draft, &models)?;
    let budget = conversation_scope(
        &state,
        &project,
        conversation,
        task,
        &draft,
        &fingerprint,
        selection.request_id,
        None,
    )?;
    let Json(mut preview) =
        preview_workflow_samples(State(state), AxumPath(selection.draft_id)).await?;
    preview["conversation_budget"] = budget;
    preview["request_id"] = json!(selection.request_id);
    Ok(Json(preview))
}

pub(super) async fn conversation_history(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, uuid::Uuid, uuid::Uuid)>,
) -> ApiResult<Json<Value>> {
    state
        .application
        .conversation_builder_budget(&project, conversation, task)
        .map_err(ApiError::bad_request)?;
    let items = state
        .application
        .store()
        .conversation_sample_operations(&project, conversation, task)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"items":items})))
}

pub(super) fn validate_scope(
    state: &ServerState,
    draft: &WorkflowDraft,
    models: &[ModelProfileSnapshot],
    input: &DryRunWorkflowRequest,
) -> ApiResult<()> {
    if input.expected_revision != Some(draft.revision)
        || input.authorization_fingerprint.as_deref()
            != Some(guided_sample_fingerprint(state, draft, models)?.as_str())
    {
        return Err(ApiError::bad_request(
            "The plan, images, goal or model scope changed after authorization. Review the updated sample scope.",
        ));
    }
    reject_unresolved_registry_model_nodes(draft)?;
    if draft.label_pipeline.is_none() || !guided_other_bindings(draft).is_empty() {
        return Err(ApiError::bad_request(
            "This plan contains bindings not yet supported by bounded guided sampling. No inference was started.",
        ));
    }
    let count = state
        .application
        .get_project(&draft.project_id)
        .map_err(ApiError::bad_request)?
        .image_count
        .min(3);
    if count == 0 || input.image_indices != (0..count).collect::<Vec<_>>() {
        return Err(ApiError::bad_request(
            "Guided sampling requires an explicit selection of 1–3 images.",
        ));
    }
    Ok(())
}

fn owned(state: &ServerState, project_id: &str, id: &str) -> ApiResult<SampleOperation> {
    let operation = state
        .application
        .store()
        .sample_operation(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("Sample task was not found"))?;
    if operation.project_id != project_id {
        return Err(ApiError::not_found(
            "Sample task does not belong to this Project",
        ));
    }
    Ok(operation)
}

pub(super) async fn get_operation(
    State(state): State<ServerState>,
    AxumPath((project_id, id)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let mut value = json!(owned(&state, &project_id, &id)?);
    value["assistance"] = json!(
        state
            .application
            .store()
            .sample_assistance_status(&id)
            .map_err(ApiError::internal)?
    );
    Ok(Json(value))
}

pub(super) async fn cancel_operation(
    State(state): State<ServerState>,
    AxumPath((project_id, id)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    owned(&state, &project_id, &id)?;
    state
        .application
        .store()
        .cancel_sample_operation(&id, &project_id)
        .map_err(ApiError::internal)?;
    if let Some(token) = state.sample_cancellations.read().await.get(&id) {
        token.cancel();
    }
    Ok(Json(json!(owned(&state, &project_id, &id)?)))
}

pub(super) async fn start_operation(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(input): Json<StartSampleRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let request_id = uuid::Uuid::parse_str(&input.request_id)
        .map_err(|_| ApiError::bad_request("A UUID sample request_id is required"))?
        .to_string();
    let execution = DryRunWorkflowRequest {
        image_indices: input.image_indices,
        expected_revision: Some(input.expected_revision),
        authorization_fingerprint: Some(input.authorization_fingerprint.clone()),
    };
    let now = chrono::Utc::now().to_rfc3339();
    let request = if let Some(consent) = &input.conversation {
        json!({"execution":execution,"conversation":consent})
    } else {
        serde_json::to_value(&execution).map_err(ApiError::internal)?
    };
    let operation = SampleOperation {
        id: request_id,
        project_id: project_id.clone(),
        draft_id: input.draft_id,
        authorization_fingerprint: input.authorization_fingerprint,
        request,
        status: "queued".into(),
        error: None,
        created_at: now.clone(),
        updated_at: now,
    };
    // Retrying an identical request reads its receipt, even when it has completed or stopped.
    if state
        .application
        .store()
        .sample_operation(&operation.id)
        .map_err(ApiError::internal)?
        .is_some()
    {
        state
            .application
            .store()
            .reserve_sample_operation(&operation)
            .map_err(ApiError::bad_request)?;
        return Ok((
            StatusCode::OK,
            Json(json!(owned(&state, &project_id, &operation.id)?)),
        ));
    }
    let (draft, models) = state
        .application
        .resolved_workflow_draft_model_profiles(&operation.draft_id)
        .map_err(ApiError::bad_request)?;
    if draft.project_id != project_id {
        return Err(ApiError::not_found(
            "This plan does not belong to this Project",
        ));
    }
    validate_scope(&state, &draft, &models, &execution)?;
    let mut settings = state.settings.read().await.clone();
    settings.budget.max_requests = Some(12);
    let (provider, credential) =
        resolve_runtime_model_profiles(&state, &models, workflow_uses_model(&draft)).await?;
    let (latest, models) = state
        .application
        .resolved_workflow_draft_model_profiles(&operation.draft_id)
        .map_err(ApiError::bad_request)?;
    validate_scope(&state, &latest, &models, &execution)?;
    let scope_seal = guided_sample_seal(&state, &latest, &models)?;
    let conversation_calls = if let Some(consent) = &input.conversation {
        if !consent.allow_unknown_cost
            || consent.expires_at <= chrono::Utc::now()
            || consent.expires_at > chrono::Utc::now() + chrono::Duration::minutes(31)
        {
            return Err(ApiError::bad_request(
                "Review and confirm the current sample authorization",
            ));
        }
        let id = uuid::Uuid::parse_str(&operation.id).map_err(ApiError::bad_request)?;
        let preview = conversation_scope(
            &state,
            &project_id,
            consent.conversation_id,
            consent.task_id,
            &latest,
            &operation.authorization_fingerprint,
            id,
            Some(consent.previous_grant_id),
        )?;
        if preview["scope_hash"] != consent.scope_hash {
            return Err(ApiError::bad_request("Sample task authorization changed"));
        }
        state
            .application
            .advance_conversation_builder_authorization(
                &project_id,
                consent.conversation_id,
                consent.previous_grant_id,
                &annotagent_storage::ConversationCallGrant {
                    id,
                    task_id: consent.task_id,
                    scope_hash: consent.scope_hash.clone(),
                    maximum_calls: preview["maximum_calls"].as_u64().unwrap_or(0) as u32,
                    expires_at: consent.expires_at,
                },
            )
            .map_err(ApiError::bad_request)?;
        settings.provider.max_retries = 0;
        Some(
            state
                .application
                .conversation_vision_calls(
                    &project_id,
                    consent.conversation_id,
                    consent.task_id,
                    &consent.scope_hash,
                )
                .map_err(ApiError::bad_request)?,
        )
    } else {
        None
    };
    if !state
        .application
        .store()
        .reserve_sample_operation_sealed(&operation, Some(&scope_seal))
        .map_err(ApiError::bad_request)?
    {
        return Ok((
            StatusCode::OK,
            Json(json!(owned(&state, &project_id, &operation.id)?)),
        ));
    }
    let cancellation = CancellationToken::new();
    state
        .sample_cancellations
        .write()
        .await
        .insert(operation.id.clone(), cancellation.clone());
    let response = operation.clone();
    tokio::spawn(async move {
        let id = operation.id.clone();
        let execution_state = state.clone();
        let execution_id = id.clone();
        let result = tokio::spawn(async move {
            let state = execution_state;
            let id = execution_id;
            if !state.application.store().start_sample_operation(&id)? {
                return Err(anyhow!("Sample task was stopped before execution"));
            }
            let scope_state = state.clone();
            let scope_draft_id = operation.draft_id.clone();
            let scope_input = execution.clone();
            // Preparation changes status/revision; later checks protect the captured scope.
            let baseline = latest;
            let initial_check = std::sync::atomic::AtomicBool::new(true);
            let control = SampleExecutionControl {
                id: id.clone(),
                cancellation,
                conversation_calls,
                check_scope: Some(Arc::new(move || {
                    let (draft, models) = scope_state
                        .application
                        .resolved_workflow_draft_model_profiles(&scope_draft_id)?;
                    let checked = if initial_check.swap(false, std::sync::atomic::Ordering::SeqCst)
                    {
                        &draft
                    } else {
                        &baseline
                    };
                    validate_scope(&scope_state, checked, &models, &scope_input).map_err(|error| {
                        anyhow!(
                            error.body["error"]
                                .as_str()
                                .unwrap_or("Sample scope validation failed")
                                .to_owned()
                        )
                    })
                })),
            };
            state
                .application
                .dry_run_workflow_samples_controlled(
                    &operation.draft_id,
                    &settings,
                    &execution.image_indices,
                    &provider,
                    credential.as_deref(),
                    control,
                )
                .await?;
            Ok::<_, anyhow::Error>(())
        })
        .await;
        let error = match result {
            Ok(result) => result.err().map(|error| error.to_string()),
            Err(_) => Some(
                "Sample worker stopped unexpectedly. No automatic retry was started.".to_owned(),
            ),
        };
        if let Err(error) = state
            .application
            .store()
            .finish_sample_operation(&id, error.as_deref())
        {
            eprintln!("could not finish sample operation {id}: {error}");
        }
        state.sample_cancellations.write().await.remove(&id);
        if let Err(error) = state.application.recover_conversation_sample_assistance() {
            eprintln!("could not deliver sample assistance for {id}: {error}");
        }
    });
    Ok((StatusCode::ACCEPTED, Json(json!(response))))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn sample_consent_scope_binds_schema_task_fingerprint_and_cumulative_grant() {
        let temp = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        let project = "TEST-sample-consent";
        app.create_project(project, "version: 1\nproject:\n  name: TEST samples\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: scene\n    kind: classification\n    labels: [day, night]\n    required: true\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n").unwrap();
        let conversation = app.create_project_conversation(project).unwrap();
        let task = uuid::Uuid::new_v4();
        let source = annotagent_storage::ConversationMessageInput {
            id: uuid::Uuid::new_v4(),
            text: "TEST classify day/night".into(),
            image: None,
        };
        app.append_project_conversation_message(project, conversation, &source)
            .unwrap();
        app.begin_conversation_task(
            project,
            conversation,
            &annotagent_storage::BeginConversationTask {
                id: task,
                source_message_id: source.id,
                schema_revision: app.project_goal(project).unwrap()["revision"]
                    .as_str()
                    .unwrap()
                    .into(),
            },
        )
        .unwrap();
        let owner =
            stable_project_id(app.project_path(project).unwrap().parent().unwrap()).to_string();
        let first = annotagent_storage::ConversationCallGrant {
            id: uuid::Uuid::new_v4(),
            task_id: task,
            scope_hash: "a".repeat(64),
            maximum_calls: 1,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(30),
        };
        let call = uuid::Uuid::new_v4();
        app.store()
            .authorize_conversation_calls(&owner, &first)
            .unwrap();
        app.store()
            .reserve_conversation_call(&owner, task, call, &first.scope_hash, &"b".repeat(64))
            .unwrap();
        app.store()
            .finish_conversation_call(
                &owner,
                task,
                call,
                annotagent_storage::ConversationCallStatus::Completed,
                json!({"TEST":"schema fixture"}),
            )
            .unwrap();
        let schema = app.store().create_conversation_schema_draft(&owner,task,call,&annotagent_storage::ConversationSchemaDefinition { goal: source.text, task: serde_json::from_value(json!({"id":"scene","kind":"classification","labels":["day","night"],"required":true})).unwrap(), boundary_rules: vec![] }).unwrap();
        let settings = annotagent_application::load_settings(None).unwrap();
        let draft = app
            .suggest_label_pipeline(
                project,
                &settings,
                "scene",
                "day",
                &WorkflowConstraints::default(),
            )
            .unwrap()
            .draft;
        let draft = app
            .bind_conversation_schema_to_workflow(project, &draft.id, draft.revision, schema.id, 1)
            .unwrap();
        let reference = CredentialReference {
            provider_id: ProviderId(uuid::Uuid::new_v4()),
            source: CredentialSource::WorkspaceFile,
            locator: "TEST-unused".into(),
        };
        let state = ServerState::with_secret_store(
            app,
            Arc::new(annotagent_provider::InMemorySecretStore::default()),
            reference.clone(),
            reference,
        )
        .await
        .unwrap();
        let request = uuid::Uuid::new_v4();
        let scope = conversation_scope(
            &state,
            project,
            conversation,
            task,
            &draft,
            "TEST-fingerprint",
            request,
            None,
        )
        .unwrap();
        assert_eq!(scope["used_calls"], 1);
        assert_eq!(scope["maximum_calls"], 13);
        assert!(scope["estimated_cost"].is_null());
        assert_eq!(
            scope["scope_hash"],
            conversation_scope(
                &state,
                project,
                conversation,
                task,
                &draft,
                "TEST-fingerprint",
                request,
                None
            )
            .unwrap()["scope_hash"]
        );
        let next = annotagent_storage::ConversationCallGrant {
            id: request,
            task_id: task,
            scope_hash: scope["scope_hash"].as_str().unwrap().into(),
            maximum_calls: 13,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(30),
        };
        state
            .application
            .advance_conversation_builder_authorization(project, conversation, first.id, &next)
            .unwrap();
        // Recover between authorization persistence and Sample Operation reservation.
        let retry = conversation_scope(
            &state,
            project,
            conversation,
            task,
            &draft,
            "TEST-fingerprint",
            request,
            Some(first.id),
        )
        .unwrap();
        assert_eq!(retry["scope_hash"], scope["scope_hash"]);
        assert_eq!(retry["maximum_calls"], 13);
        assert_ne!(
            scope["scope_hash"],
            conversation_scope(
                &state,
                project,
                conversation,
                task,
                &draft,
                "changed",
                request,
                None
            )
            .unwrap()["scope_hash"]
        );
        assert!(
            conversation_scope(
                &state,
                project,
                conversation,
                uuid::Uuid::new_v4(),
                &draft,
                "TEST-fingerprint",
                request,
                None
            )
            .is_err()
        );
        assert!(
            conversation_scope(
                &state,
                project,
                conversation,
                task,
                &draft,
                "TEST-fingerprint",
                request,
                Some(uuid::Uuid::new_v4())
            )
            .is_err()
        );
        assert_eq!(
            state
                .application
                .store()
                .conversation_call_budget(&owner, task)
                .unwrap()
                .unwrap()
                .used_calls,
            1,
            "Preview never calls a model or advances the grant"
        );
    }
}
