//! Passive, task-owned Registry and authorization readiness snapshot.
//! No secret bytes are resolved and no Provider, Plugin, installer or model is called.
use super::*;
use uuid::Uuid;

fn profile_roles(model: &ModelProfile) -> Vec<&'static str> {
    let mut roles = Vec::new();
    if model.input_modalities.contains(&InputModality::Text)
        && model
            .task_capabilities
            .contains(&ModelCapability::TextGeneration)
        && model.protocol_features.tool_calls
        && model.protocol_features.structured_output
    {
        roles.push("agent");
    }
    if model.input_modalities.contains(&InputModality::Image) {
        if model
            .task_capabilities
            .contains(&ModelCapability::VisionLanguage)
        {
            roles.push("vision_language");
        }
        if model.task_capabilities.iter().any(|capability| {
            matches!(
                capability,
                ModelCapability::ObjectDetection
                    | ModelCapability::OpenVocabularyDetection
                    | ModelCapability::PhraseGrounding
            )
        }) {
            roles.push("detection");
        }
        if model.task_capabilities.iter().any(|capability| {
            matches!(
                capability,
                ModelCapability::SemanticSegmentation
                    | ModelCapability::PromptedSegmentation
                    | ModelCapability::InstanceSegmentation
            )
        }) {
            roles.push("segmentation");
        }
        if model
            .task_capabilities
            .contains(&ModelCapability::ImageClassification)
        {
            roles.push("classification");
        }
    }
    roles
}

fn visual_roles(capabilities: &BTreeSet<ModelCapability>) -> Vec<&'static str> {
    let mut roles = vec!["visual"];
    if capabilities.contains(&ModelCapability::VisionLanguage) {
        roles.push("vision_language");
    }
    if capabilities.iter().any(|capability| {
        matches!(
            capability,
            ModelCapability::ObjectDetection
                | ModelCapability::OpenVocabularyDetection
                | ModelCapability::PhraseGrounding
        )
    }) {
        roles.push("detection");
    }
    if capabilities.iter().any(|capability| {
        matches!(
            capability,
            ModelCapability::SemanticSegmentation
                | ModelCapability::PromptedSegmentation
                | ModelCapability::InstanceSegmentation
        )
    }) {
        roles.push("segmentation");
    }
    if capabilities.contains(&ModelCapability::ImageClassification) {
        roles.push("classification");
    }
    roles
}

fn profile_state(model: &ModelProfile, provider: &ProviderProfile) -> (&'static str, Value) {
    if !model.enabled
        || !provider.enabled
        || model.status == ModelProfileStatus::Disabled
        || provider.health.status == ProviderHealthStatus::Disabled
    {
        return (
            "disabled",
            json!({"code":"disabled","message":"Enable both the Model Profile and Provider before use."}),
        );
    }
    if matches!(model.status, ModelProfileStatus::Unavailable)
        || matches!(
            provider.health.status,
            ProviderHealthStatus::Unreachable
                | ProviderHealthStatus::InvalidCredential
                | ProviderHealthStatus::RateLimited
                | ProviderHealthStatus::IncompatibleProtocol
        )
    {
        return (
            "unavailable",
            json!({"code":"unavailable","message":provider.health.safe_message.as_deref().unwrap_or("The saved availability check is not usable.")}),
        );
    }
    if model.status == ModelProfileStatus::Available
        && provider.health.status == ProviderHealthStatus::Available
        && (provider.adapter == ProviderAdapterKind::Mock || provider.credential_ref.is_some())
    {
        return ("ready", Value::Null);
    }
    (
        "unknown",
        json!({"code":"not_verified","message":"Configuration exists, but readiness is not proven by a current saved check."}),
    )
}

fn plugin_state(availability: ModelAvailability) -> (&'static str, Value) {
    match availability {
        ModelAvailability::Available => ("ready", Value::Null),
        ModelAvailability::Disabled => (
            "disabled",
            json!({"code":"disabled","message":"The installed Plugin model is disabled."}),
        ),
        ModelAvailability::Unknown | ModelAvailability::Unconfigured => (
            "unknown",
            json!({"code":"not_verified","message":"The installed Plugin model has no current readiness evidence."}),
        ),
        other => (
            "unavailable",
            json!({"code":serde_json::to_value(other).unwrap_or(json!("unavailable")),"message":"The installed Plugin model is not currently selectable."}),
        ),
    }
}

fn current_draft(
    state: &ServerState,
    project: &str,
    journeys: &[Value],
    builders: &Value,
) -> ApiResult<Value> {
    let from_sample = journeys.iter().find_map(|journey| {
        journey
            .get("sample")
            .and_then(|sample| sample.get("draft_id"))
            .and_then(Value::as_str)
    });
    let from_builder = builders["items"].as_array().and_then(|items| {
        items.iter().find_map(|item| {
            item["operation"]["evidence"]["draft_id"]
                .as_str()
                .or_else(|| item["session"]["working_draft"]["draft_id"].as_str())
        })
    });
    let Some(id) = from_sample.or(from_builder) else {
        return Ok(Value::Null);
    };
    let draft = state
        .application
        .store()
        .get_workflow_draft(id)
        .map_err(ApiError::internal)?;
    if draft.project_id != project {
        return Err(ApiError::not_found("Task Draft belongs to another Project"));
    }
    Ok(
        json!({"id":draft.id,"revision":draft.revision,"content_hash":draft.content_hash,"status":draft.status}),
    )
}

fn active_authorization(journeys: &[Value]) -> Value {
    let now = Utc::now();
    journeys
        .iter()
        .find_map(|journey| {
            let record = journey.get("record")?;
            if record.get("revoked")?.as_bool()? {
                return None;
            }
            let consent = record
                .get("resolved_consent")
                .filter(|value| !value.is_null())
                .unwrap_or_else(|| record.get("consent").unwrap_or(&Value::Null));
            let expires_at = consent
                .get("expires_at")?
                .as_str()?
                .parse::<chrono::DateTime<Utc>>()
                .ok()?;
            if expires_at <= now {
                return None;
            }
            let allowed = consent.get("allowed_models")?.clone();
            let digest = annotagent_image_tools::sha256(
                &serde_json::to_vec(&json!({
                    "consent_id":consent.get("id"),"expires_at":expires_at,
                    "allowed_models":allowed,"maximum_builder_calls":consent.get("maximum_builder_calls"),
                    "maximum_sample_calls":consent.get("maximum_sample_calls")
                }))
                .ok()?,
            );
            Some(json!({
                "source":"journey_consent","consent_id":consent.get("id"),
                "expires_at":expires_at,"permission_digest":digest,
                "allowed_models":allowed,"active":true,
                "can_resume_without_authorization":false
            }))
        })
        .unwrap_or_else(|| {
            json!({
                "source":null,"consent_id":null,"expires_at":null,
                "permission_digest":null,"allowed_models":[],"active":false,
                "can_resume_without_authorization":false
            })
        })
}

pub(super) fn snapshot(
    state: &ServerState,
    project: &str,
    conversation: Uuid,
    task: Uuid,
) -> ApiResult<Value> {
    // Establish ownership before reading any Registry state.
    let task_record = state
        .application
        .conversation_tasks(project, conversation)
        .map_err(ApiError::conversation)?
        .into_iter()
        .find(|record| record.input.id == task)
        .ok_or_else(|| ApiError::not_found("Task was not found"))?;
    let owner = registry_project_id(state, project)?;
    let store = state.application.store();
    let providers = store.list_provider_profiles().map_err(ApiError::internal)?;
    let profiles = store
        .list_model_profiles(None, false)
        .map_err(ApiError::internal)?;
    let bindings = store
        .list_project_model_bindings(owner)
        .map_err(ApiError::internal)?;
    let journeys = state
        .application
        .conversation_journey_history(project, conversation, task)
        .map_err(ApiError::conversation)?;
    let builders = state
        .application
        .conversation_builder_history(project, conversation, task)
        .map_err(ApiError::conversation)?;
    let authorization = active_authorization(&journeys);
    let agent_model = state
        .application
        .project_conversation_agent_model(project, conversation)
        .map_err(ApiError::conversation)?;
    let allowed = authorization["allowed_models"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut candidates = Vec::new();
    for model in &profiles {
        let Some(provider) = providers
            .iter()
            .find(|provider| provider.id == model.provider_id)
        else {
            continue;
        };
        let roles = profile_roles(model);
        if roles.is_empty() {
            continue;
        }
        let selection_id = format!("model-profile:{}", model.id);
        let (readiness, blocker) = profile_state(model, provider);
        let exact = state
            .application
            .conversation_journey_model_description(&selection_id)
            .ok();
        let digest = exact.as_ref().map_or_else(
            || {
                annotagent_image_tools::sha256(
                    &serde_json::to_vec(&json!({"model":model,"provider":provider}))
                        .expect("Registry profile is serializable"),
                )
            },
            |description| description.scope.binding_digest.clone(),
        );
        let allowed_by_current_scope = allowed
            .iter()
            .any(|scope| scope["model_id"] == selection_id && scope["binding_digest"] == digest);
        let project_bindings = bindings
            .iter()
            .filter(|binding| binding.model_profile_id == model.id)
            .collect::<Vec<_>>();
        candidates.push(json!({
            "id":selection_id,"candidate_type":"model_profile","model_profile_id":model.id,
            "revision":model.revision,"digest":digest,"roles":roles,
            "capabilities":model.task_capabilities,"quality_contracts":effective_model_quality_contracts(model),
            "readiness":readiness,"production_eligible":readiness=="ready"&&provider.adapter!=ProviderAdapterKind::Mock,
            "test_fixture":provider.adapter==ProviderAdapterKind::Mock,"blocker":blocker,
            "project_bindings":project_bindings,"allowed_by_current_scope":allowed_by_current_scope,
            "selected_for_next_agent_request":agent_model.model_profile_id==Some(model.id),
            "setup":{"kind":"model_profile","api_url":format!("/api/model-profiles/{}",model.id)}
        }));
    }

    let plugin_profiles = state
        .application
        .plugin_registry()
        .lock()
        .map_err(|_| ApiError::internal("Plugin Registry lock is poisoned"))?
        .ready_models();
    for profile in plugin_profiles {
        let id = plugin_model_selection_id(&profile.reference);
        let (readiness, blocker) = plugin_state(profile.availability);
        let exact = state
            .application
            .conversation_journey_model_description(&id)
            .ok();
        let digest = exact.as_ref().map_or_else(
            || {
                annotagent_image_tools::sha256(
                    &serde_json::to_vec(&profile).expect("Plugin profile is serializable"),
                )
            },
            |description| description.scope.binding_digest.clone(),
        );
        candidates.push(json!({
            "id":id,"candidate_type":"plugin_model","revision":profile.reference.model_profile_revision,
            "digest":digest,"roles":visual_roles(&profile.capabilities),"capabilities":profile.capabilities,
            "readiness":readiness,"production_eligible":readiness=="ready",
            "test_fixture":false,"blocker":blocker,"project_bindings":[],
            "allowed_by_current_scope":allowed.iter().any(|scope|scope["model_id"]==id&&scope["binding_digest"]==digest),
            "setup":{"kind":"plugin","api_url":"/api/plugins"}
        }));
    }

    let instance_profiles = state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .model_profiles();
    for profile in instance_profiles {
        let id = profile.selection_id.clone();
        let (readiness, blocker) = plugin_state(profile.availability);
        let exact = state
            .application
            .conversation_journey_model_description(&id)
            .ok();
        let digest = exact.as_ref().map_or_else(
            || {
                annotagent_image_tools::sha256(
                    &serde_json::to_vec(&profile).expect("Model instance profile is serializable"),
                )
            },
            |description| description.scope.binding_digest.clone(),
        );
        candidates.push(json!({
            "id":id,"candidate_type":"model_instance","model_profile_id":profile.model_profile_id,
            "model_instance_id":profile.model_instance_id,"revision":profile.model_profile_revision,
            "digest":digest,"roles":visual_roles(&profile.capabilities),"capabilities":profile.capabilities,
            "readiness":readiness,"production_eligible":profile.selectable&&readiness=="ready",
            "test_fixture":false,"blocker":blocker,"project_bindings":[],
            "allowed_by_current_scope":allowed.iter().any(|scope|scope["model_id"]==id&&scope["binding_digest"]==digest),
            "setup":{"kind":"model_instance","api_url":format!("/api/model-instances/{}",profile.model_instance_id)}
        }));
    }
    candidates.sort_by(|left, right| {
        left["id"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["id"].as_str().unwrap_or_default())
    });
    let registry_digest = annotagent_image_tools::sha256(
        &serde_json::to_vec(&json!({"candidates":candidates,"project_bindings":bindings}))
            .map_err(ApiError::internal)?,
    );
    let delivery = state
        .application
        .task_delivery_intent(project, conversation, task)
        .map_err(ApiError::conversation)?;
    let mut required_capabilities = vec!["text_generation"];
    if delivery
        .saved
        .as_ref()
        .and_then(|saved| saved.intent.training_target.as_ref())
        .is_some_and(annotagent_core::dataset_delivery::TrainingTarget::is_detection_preset)
    {
        required_capabilities.push("object_detection");
    }
    let compatible_model_ids = candidates
        .iter()
        .filter(|candidate| {
            candidate["capabilities"]
                .as_array()
                .is_some_and(|capabilities| {
                    capabilities.iter().any(|capability| {
                        required_capabilities
                            .iter()
                            .any(|required| capability == required)
                    })
                })
                && !candidate["test_fixture"].as_bool().unwrap_or(false)
                && (candidate["production_eligible"].as_bool().unwrap_or(false)
                    || candidate["readiness"] == "unknown")
        })
        .filter_map(|candidate| candidate["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let setup_ready = required_capabilities.iter().all(|required| {
        candidates.iter().any(|candidate| {
            candidate["readiness"] == "ready"
                && candidate["production_eligible"].as_bool().unwrap_or(false)
                && candidate["capabilities"]
                    .as_array()
                    .is_some_and(|capabilities| capabilities.iter().any(|value| value == required))
        })
    });
    let setup_id = annotagent_image_tools::sha256(
        &serde_json::to_vec(&json!({
            "project_id":project,"conversation_id":conversation,"task_id":task,
            "task_revision":task_record.input.schema_revision,
            "registry_revision":registry_digest,
            "role":"task_planning_and_vision",
            "required_capabilities":required_capabilities,
            "compatible_model_ids":compatible_model_ids
        }))
        .map_err(ApiError::internal)?,
    );
    let draft = current_draft(state, project, &journeys, &builders)?;
    let mut return_url =
        url::Url::parse("http://annotagent.local").expect("fixed internal return URL is valid");
    return_url.set_path(&format!("/projects/{project}/work"));
    {
        let mut query = return_url.query_pairs_mut();
        query.append_pair("task", &task.to_string());
        if let Some(draft_id) = draft["id"].as_str() {
            query.append_pair("draft", draft_id);
        }
    }
    let return_path = return_url[url::Position::BeforePath..].to_owned();
    let budget = state
        .application
        .conversation_task_budget(project, conversation, task)
        .map_err(ApiError::conversation)?;
    let calls = store
        .conversation_call_history(&owner.to_string(), task)
        .map_err(ApiError::internal)?;
    let no_calls = calls.is_empty();
    Ok(json!({
        "contract_version":"mainline-capability-v1","project_id":project,"project_owner_id":owner,
        "conversation_id":conversation,"task_id":task,"task_schema_revision":task_record.input.schema_revision,
        "draft":draft,"registry_revision":registry_digest,"registry_revision_kind":"snapshot_sha256",
        "candidates":candidates,"agent_model_preference":agent_model,
        "setup_requests":[{
            "id":setup_id,"project_id":project,"task_id":task,
            "task_revision":task_record.input.schema_revision,
            "registry_revision":registry_digest,
            "role":"task_planning_and_vision",
            "required_capabilities":required_capabilities,
            "compatible_model_ids":compatible_model_ids,
            "status":if setup_ready{"ready"}else{"required"},
            "return_path":return_path
        }],
        "authorization":authorization,"budget":budget,
        "task_cost":{"scope":"conversation_task_model_calls","receipt_count":calls.len(),
            "known":no_calls,"amount":if no_calls {json!("0")} else {Value::Null},"currency":null,
            "reason":if no_calls {Value::Null} else {json!("Task receipts do not all contain comparable priced usage; unknown cost is preserved.")}},
        "passive":true,"setup_recheck_only":true,"auto_expands_allowed_models":false,
        "consistency":"server_composed_versioned_snapshot"
    }))
}

pub(super) async fn get(
    State(state): State<ServerState>,
    AxumPath((project, conversation, task)): AxumPath<(String, Uuid, Uuid)>,
) -> ApiResult<Json<Value>> {
    snapshot(&state, &project, conversation, task).map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{request, response_json, test_state};
    use annotagent_provider::InMemorySecretStore;

    #[tokio::test]
    async fn task_capability_snapshot_is_passive_owned_and_changes_with_registry_revision() {
        let temp = tempfile::tempdir().unwrap();
        let app = Arc::new(LocalApplication::new(temp.path()).unwrap());
        for project in ["TEST-capability", "TEST-foreign"] {
            app.create_project(
                project,
                "version: 1\nproject:\n  name: TEST capability\ndataset:\n  root: images\nruntime: {}\ntasks:\n  - id: objects\n    kind: bounding_box\n    labels: [target]\n    required: true\nreview:\n  auto_accept_confidence: 0.9\n  force_review_below: 0.5\nexport:\n  formats: [native]\n",
            )
            .unwrap();
        }
        let state = test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await;
        let provider = app
            .store()
            .list_provider_profiles()
            .unwrap()
            .into_iter()
            .find(|provider| provider.adapter == ProviderAdapterKind::Mock)
            .unwrap();
        let now = Utc::now();
        let mut model = ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id: provider.id,
            display_name: "TEST task agent".into(),
            remote_model_id: "TEST-task-agent".into(),
            input_modalities: BTreeSet::from([InputModality::Text]),
            protocol_features: ProtocolFeatures {
                tool_calls: true,
                structured_output: true,
                ..ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([ModelCapability::TextGeneration]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing::default(),
            quality_contracts: Vec::new(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        app.store().save_model_profile(&model).unwrap();
        let conversation = app.create_project_conversation("TEST-capability").unwrap();
        let sent = app
            .send_project_conversation_message(
                "TEST-capability",
                conversation,
                &serde_json::from_value(json!({
                    "message":{"id":Uuid::new_v4(),"text":"TEST goal","image":null},
                    "task_id":null,"schema_revision":app.project_goal("TEST-capability").unwrap()["revision"],
                    "mode":"plan"
                }))
                .unwrap(),
            )
            .unwrap();
        let owner = registry_project_id(&state, "TEST-capability").unwrap();
        let service = router(state, None);
        let url = format!(
            "/api/projects/TEST-capability/conversations/{conversation}/tasks/{}/capability-readiness",
            sent.task_id
        );
        let first =
            response_json(request(&service, axum::http::Method::GET, &url, None).await).await;
        assert_eq!(first["passive"], true);
        assert_eq!(first["authorization"]["active"], false);
        assert_eq!(
            first["authorization"]["can_resume_without_authorization"],
            false
        );
        assert_eq!(first["auto_expands_allowed_models"], false);
        let candidate = first["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|candidate| candidate["model_profile_id"] == model.id.to_string())
            .unwrap();
        assert_eq!(candidate["roles"], json!(["agent"]));
        assert_eq!(candidate["readiness"], "ready");
        assert_eq!(candidate["production_eligible"], false);
        assert_eq!(candidate["test_fixture"], true);
        assert_eq!(first["task_cost"]["known"], true);
        assert_eq!(first["task_cost"]["amount"], "0");
        let setup = &first["setup_requests"][0];
        assert_eq!(setup["project_id"], "TEST-capability");
        assert_eq!(setup["task_id"], sent.task_id.to_string());
        assert_eq!(setup["task_revision"], first["task_schema_revision"]);
        assert_eq!(setup["registry_revision"], first["registry_revision"]);
        assert_eq!(setup["role"], "task_planning_and_vision");
        assert_eq!(setup["required_capabilities"], json!(["text_generation"]));
        assert_eq!(setup["status"], "required");
        assert_eq!(
            setup["return_path"],
            format!("/projects/TEST-capability/work?task={}", sent.task_id)
        );
        assert!(
            app.store()
                .conversation_call_history(&owner.to_string(), sent.task_id)
                .unwrap()
                .is_empty()
        );

        let first_revision = first["registry_revision"].clone();
        model.revision = 2;
        model.display_name = "TEST task agent revision 2".into();
        model.updated_at = Utc::now();
        app.store().save_model_profile_cas(&model, 1).unwrap();
        let second =
            response_json(request(&service, axum::http::Method::GET, &url, None).await).await;
        assert_ne!(second["registry_revision"], first_revision);
        assert_eq!(
            second["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .find(|candidate| candidate["model_profile_id"] == model.id.to_string())
                .unwrap()["revision"],
            2
        );

        let workspace = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!(
                    "/api/projects/TEST-capability/conversations/{conversation}/tasks/{}/workspace",
                    sent.task_id
                ),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            workspace["mainline"]["capability_readiness"]["registry_revision"],
            second["registry_revision"]
        );
        let foreign = request(
            &service,
            axum::http::Method::GET,
            &format!(
                "/api/projects/TEST-foreign/conversations/{conversation}/tasks/{}/capability-readiness",
                sent.task_id
            ),
            None,
        )
        .await;
        assert!(!foreign.status().is_success());
        assert!(
            app.store()
                .conversation_call_history(&owner.to_string(), sent.task_id)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn authorization_projection_uses_only_current_unrevoked_consent() {
        let active = Uuid::new_v4();
        let model = Uuid::new_v4();
        let future = Utc::now() + chrono::Duration::minutes(5);
        let expired = Utc::now() - chrono::Duration::minutes(5);
        let journeys = vec![
            json!({"record":{"revoked":true,"consent":{"id":Uuid::new_v4(),"expires_at":future,"allowed_models":[]}}}),
            json!({"record":{"revoked":false,"consent":{"id":Uuid::new_v4(),"expires_at":expired,"allowed_models":[]}}}),
            json!({"record":{"revoked":false,"consent":{"id":active,"expires_at":future,"allowed_models":[{"model_id":format!("model-profile:{model}"),"binding_digest":"a".repeat(64)}],"maximum_builder_calls":4,"maximum_sample_calls":3}}}),
        ];
        let projected = active_authorization(&journeys);
        assert_eq!(projected["active"], true);
        assert_eq!(projected["consent_id"], active.to_string());
        assert!(
            projected["permission_digest"]
                .as_str()
                .is_some_and(|value| value.len() == 64)
        );
        assert_eq!(projected["can_resume_without_authorization"], false);
    }
}
