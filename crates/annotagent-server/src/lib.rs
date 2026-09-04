//! Thin HTTP/SSE adapter over the shared application service.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    fs::OpenOptions,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use annotagent_application::{
    ActiveRunExists, AnnotAgentApplication, ApplyPipelineImprovementRequest,
    CreatePipelineImprovementRequest, DatasetCoordinator, DetectionWorkerSettings,
    GeometryCalibrationRequest, LocalApplication, ModelBinding, ProjectSummary, Settings,
    WorkflowVersion, stable_project_id, validate_settings,
};
use annotagent_core::{
    Annotation, AnnotationId, AnnotationValue, ArtifactId, ArtifactKind, ArtifactRef,
    ArtifactValidationState, AttributeDefinition, AutoAcceptEligibility, BatchId,
    BindingMutationActor, BoxPrompt, BoxPromptSetArtifact, CandidateAgreement,
    CapabilityDeclarationSource, ContractEvidenceSource, CorrectionFeatures, CorrectionRecord,
    CredentialReference, CredentialSource, DetectionEvidence, EnabledSkillConfig,
    ExpertModelManifest, GenerationDefaults, GeometryAutoAcceptPolicy, GeometryCalibrationId,
    GeometryCalibrationThresholds, GeometryCorrectionInput, GeometryCorrectionReason,
    GeometryQualitySummary, GeometrySemantics, GeometrySnapshot, GlobalModelDefaults,
    ImageArtifact, ImageId, InputModality, LabelId, ModelAvailability, ModelBindingId,
    ModelBindingMatch, ModelBindingRole, ModelCapability, ModelCapabilityQualityContract,
    ModelLimits, ModelPricing, ModelProfile, ModelProfileId, ModelProfileSnapshot,
    ModelProfileStatus, ModelRequirements, NodeId, NormalizedRect, PipelineArtifact,
    PipelineBuilderConstraints, PipelineImprovementId, PipelineImprovementPolicy,
    PipelineInferenceRequest, PipelineModelBackend, ProjectGeometryPolicy, ProjectModelBinding,
    ProjectSchema, ProtocolFeatures, ProviderAdapterKind, ProviderConnectionPolicy,
    ProviderErrorDetails, ProviderHealthSnapshot, ProviderHealthStatus, ProviderId,
    ProviderProfile, PublishedWorkflowVersion, RequiredGeometryQuality, ReviewStatus, RunEvent,
    RunEventKind, RunEventPayload, RunId, RunStatus, ScoreSemantics, SecretScope, SecretStore,
    SecretStoreError, SecretValue, SmallObjectLocalizationSupport, TaskId, TaskKind, UsageTotals,
    VisionCapability, VisionInferenceRequest, VisionModelBackend, VisionModelHealthStatus,
    WorkflowConstraints, WorkflowDraft, WorkflowNodeKind, build_geometry_correction_evidence,
    check_model_compatibility, effective_model_quality_contracts,
};
use annotagent_image_tools::{load_image, to_model_image};
use annotagent_model_bundle::{
    CommercialUseStatus, ExpectedOutputSummary, ModelBundleFile, ModelBundleId,
    ModelBundleManifest, ModelBundleSmokeRequest, ModelContractDocument, ModelContractReference,
    ModelExportMetadata, ModelFileRole, ModelFormat, ModelInstanceId, ModelLicenseMetadata,
    ModelRuntimeMetadata, ModelSourceMetadata, ModelTestSuiteReference, OutputTolerances,
    PluginCompatibilityRequirement, RedistributionStatus, Sha256Digest as ModelBundleSha256Digest,
    SmokeTestCheck, SmokeTestResult, SmokeTestStatus, pack_model_bundle, verify_model_bundle,
};
use annotagent_model_catalog::{
    LicenseAcceptanceActor, ModelBundleCompatibilityResolver, ModelBundleInstallSource,
    ModelCatalogClient, ModelCatalogEntry, ModelLicenseAcceptance, evaluate_bundle_smoke_response,
    parse_model_instance_selection_id, prepare_bundle_smoke_test,
};
use annotagent_plugin_api::{PluginId, PluginVersion, Sha256Digest};
use annotagent_plugin_host::verify_package;
use annotagent_plugin_registry::{
    InstallApproval, PluginInstallation, PluginRegistryError, plugin_model_selection_id,
    run_model_instance_smoke,
};
use annotagent_provider::{
    EnvironmentSecretStore, HttpJsonPipelineBackend, HttpJsonPipelineBackendConfig,
    HttpJsonVisionBackend, HttpJsonVisionBackendConfig, KeyringSecretStore,
    LegacyWorkspaceFileSecretStore, OpenAiCompatibleProvider, SecretStoreRouter,
    SessionSecretStore, WorkspaceFileSecretStore, active_provider_probe, discover_provider_models,
    passive_provider_check,
};
use annotagent_runtime::RuntimeStore;
use annotagent_storage::{HistoryRun, ProviderProbeUsage, RegistryReference};
use anyhow::{Context, anyhow};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{delete, get, patch, post},
};
use chrono::Utc;
use futures::{Stream, StreamExt as _, stream};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt as _, sync::RwLock};
use tokio_util::sync::CancellationToken;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct ServerState {
    application: Arc<LocalApplication>,
    settings: Arc<RwLock<Settings>>,
    api_key: Arc<RwLock<Option<String>>>,
    settings_path: Arc<PathBuf>,
    settings_persisted: Arc<RwLock<bool>>,
    api_key_persisted: Arc<RwLock<bool>>,
    credential_store_error: Arc<RwLock<Option<String>>>,
    secret_store: Arc<dyn SecretStore>,
    credential_reference: Arc<RwLock<CredentialReference>>,
    default_write_reference: Arc<CredentialReference>,
    model_install_operations: Arc<RwLock<BTreeMap<uuid::Uuid, ModelInstallOperation>>>,
}

impl ServerState {
    pub async fn new(application: Arc<LocalApplication>) -> anyhow::Result<Self> {
        let provider_id = ProviderId(stable_project_id(application.workspace()).0);
        let default_keyring_reference = CredentialReference {
            provider_id,
            source: CredentialSource::SystemKeyring,
            locator: format!("workspace-{provider_id}"),
        };
        let default_session_reference = CredentialReference {
            provider_id,
            source: CredentialSource::SessionOnly,
            locator: format!("workspace-session-{provider_id}"),
        };
        let legacy_reference = CredentialReference {
            provider_id,
            source: CredentialSource::LegacyWorkspaceFile,
            locator: "legacy-workspace-provider-api-key".to_owned(),
        };
        let secret_store: Arc<dyn SecretStore> = Arc::new(SecretStoreRouter {
            keyring: Arc::new(KeyringSecretStore::new(SECRET_SERVICE)),
            environment: Arc::new(EnvironmentSecretStore),
            workspace: Arc::new(WorkspaceFileSecretStore::new(
                application.workspace().join(".annotagent/credentials"),
            )),
            session: Arc::new(SessionSecretStore::default()),
            legacy: Arc::new(LegacyWorkspaceFileSecretStore::single(
                legacy_reference.locator.clone(),
                application
                    .workspace()
                    .join(".annotagent/credentials/provider-api-key"),
            )),
        });
        let credential_reference = if secret_store
            .exists(&default_keyring_reference)
            .await
            .is_ok_and(|exists| exists)
        {
            default_keyring_reference.clone()
        } else if secret_store
            .exists(&legacy_reference)
            .await
            .is_ok_and(|exists| exists)
        {
            legacy_reference
        } else {
            default_session_reference.clone()
        };
        Self::with_secret_store(
            application,
            secret_store,
            credential_reference,
            default_session_reference,
        )
        .await
    }

    async fn with_secret_store(
        application: Arc<LocalApplication>,
        secret_store: Arc<dyn SecretStore>,
        credential_reference: CredentialReference,
        default_write_reference: CredentialReference,
    ) -> anyhow::Result<Self> {
        let settings_path = application.workspace().join(".annotagent/settings.toml");
        #[allow(unused_mut)]
        let mut settings_persisted = settings_path.is_file();
        #[allow(unused_mut)]
        let mut settings = if settings_persisted {
            annotagent_application::load_settings(Some(&settings_path))?
        } else {
            annotagent_application::load_settings(None)?
        };
        #[cfg(not(test))]
        if settings.default_provider == "mock" {
            "openai_compatible".clone_into(&mut settings.default_provider);
            persist_settings(&settings_path, &settings)?;
            settings_persisted = true;
        }
        validate_settings(&settings)?;
        #[cfg(not(test))]
        purge_builtin_mock_registry(application.as_ref())?;
        #[cfg(test)]
        ensure_test_mock_registry(application.as_ref())?;
        credential_reference.validate()?;
        default_write_reference.validate()?;
        let (api_key, api_key_persisted, credential_store_error) =
            match secret_store.resolve(&credential_reference).await {
                Ok(value) => (Some(value.expose_secret().to_owned()), true, None),
                Err(SecretStoreError::NotFound) => (None, false, None),
                Err(error) => (None, false, Some(error.to_string())),
            };
        Ok(Self {
            application,
            settings: Arc::new(RwLock::new(settings)),
            api_key: Arc::new(RwLock::new(api_key)),
            settings_path: Arc::new(settings_path),
            settings_persisted: Arc::new(RwLock::new(settings_persisted)),
            api_key_persisted: Arc::new(RwLock::new(api_key_persisted)),
            credential_store_error: Arc::new(RwLock::new(credential_store_error)),
            secret_store,
            credential_reference: Arc::new(RwLock::new(credential_reference)),
            default_write_reference: Arc::new(default_write_reference),
            model_install_operations: Arc::new(RwLock::new(BTreeMap::new())),
        })
    }

    #[must_use]
    pub fn application(&self) -> &Arc<LocalApplication> {
        &self.application
    }
}

#[cfg(not(test))]
fn purge_builtin_mock_registry(application: &LocalApplication) -> anyhow::Result<()> {
    application
        .store()
        .purge_provider_adapter(ProviderAdapterKind::Mock)?;
    application.store().purge_mock_agent_sessions()?;
    Ok(())
}

#[cfg(test)]
fn ensure_test_mock_registry(application: &LocalApplication) -> anyhow::Result<()> {
    let store = application.store();
    let now = Utc::now();
    let provider = store
        .list_provider_profiles()?
        .into_iter()
        .find(|profile| profile.adapter == ProviderAdapterKind::Mock)
        .unwrap_or_else(|| ProviderProfile {
            id: ProviderId::new(),
            display_name: "Mock (offline)".to_owned(),
            preset_id: Some("mock".to_owned()),
            adapter: ProviderAdapterKind::Mock,
            base_url: "http://127.0.0.1".parse().expect("static test URL"),
            organization: None,
            workspace: None,
            credential_ref: None,
            safe_headers: BTreeMap::new(),
            connection_policy: ProviderConnectionPolicy::default(),
            enabled: true,
            health: ProviderHealthSnapshot {
                status: ProviderHealthStatus::Available,
                safe_message: Some("test fixture".to_owned()),
                checked_at: Some(now),
            },
            created_at: now,
            updated_at: now,
        });
    if store
        .list_provider_profiles()?
        .iter()
        .all(|candidate| candidate.id != provider.id)
    {
        store.save_provider_profile(&provider)?;
    }
    let existing = store.list_model_profiles(Some(provider.id), false)?;
    let ensure = |remote_model_id: &str,
                  display_name: &str,
                  capabilities: BTreeSet<ModelCapability>,
                  tool_calls: bool|
     -> anyhow::Result<ModelProfile> {
        if let Some(model) = existing
            .iter()
            .find(|model| model.remote_model_id == remote_model_id)
        {
            return Ok(model.clone());
        }
        let model = ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id: provider.id,
            display_name: display_name.to_owned(),
            remote_model_id: remote_model_id.to_owned(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures {
                tool_calls,
                structured_output: true,
                json_schema: true,
                usage_reporting: true,
                ..ProtocolFeatures::default()
            },
            task_capabilities: capabilities,
            capability_source: CapabilityDeclarationSource::Preset,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing::default(),
            quality_contracts: Vec::new(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: true,
            created_at: now,
            updated_at: now,
        };
        store.save_model_profile(&model)?;
        Ok(model)
    };
    let vision = ensure(
        "mock-vision",
        "Mock Vision Language (offline)",
        BTreeSet::from([ModelCapability::VisionLanguage]),
        true,
    )?;
    ensure(
        "mock-classifier",
        "Mock Classifier (offline)",
        BTreeSet::from([ModelCapability::ImageClassification]),
        false,
    )?;
    ensure(
        "mock-detector",
        "Mock Detector (offline)",
        BTreeSet::from([ModelCapability::ObjectDetection]),
        false,
    )?;
    ensure(
        "mock-grounding",
        "Mock Open Vocabulary (offline)",
        BTreeSet::from([
            ModelCapability::OpenVocabularyDetection,
            ModelCapability::PhraseGrounding,
        ]),
        false,
    )?;
    ensure(
        "mock-segmenter",
        "Mock Segmenter (offline)",
        BTreeSet::from([
            ModelCapability::SemanticSegmentation,
            ModelCapability::PromptedSegmentation,
            ModelCapability::InstanceSegmentation,
        ]),
        false,
    )?;
    let mut defaults = store.get_global_model_defaults()?;
    if defaults.vision_language.is_none() {
        defaults.vision_language = Some(vision.id);
        store.save_global_model_defaults(&defaults)?;
    }
    Ok(())
}

const SECRET_SERVICE: &str = "com.annotagent.provider-api-key";

fn persist_settings(path: &Path, settings: &Settings) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("settings path has no parent"))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("cannot create settings directory {}", parent.display()))?;
    let serialized = toml::to_string_pretty(settings).context("cannot serialize settings")?;
    let temporary_path = path.with_extension(format!("toml.{}.tmp", uuid::Uuid::new_v4()));
    let write_result = (|| -> anyhow::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .with_context(|| {
                format!(
                    "cannot create temporary settings file {}",
                    temporary_path.display()
                )
            })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(serialized.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary_path, path)
            .with_context(|| format!("cannot replace settings file {}", path.display()))?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ignored = std::fs::remove_file(&temporary_path);
    }
    write_result
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    body: Value,
}

impl ApiError {
    fn bad_request(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: json!({"error": error.to_string(), "status": StatusCode::BAD_REQUEST.as_u16()}),
        }
    }

    fn not_found(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            body: json!({"error": error.to_string(), "status": StatusCode::NOT_FOUND.as_u16()}),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: json!({"error": error.to_string(), "status": StatusCode::INTERNAL_SERVER_ERROR.as_u16()}),
        }
    }

    fn conflict(
        code: &str,
        error: impl std::fmt::Display,
        references: &[RegistryReference],
    ) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            body: json!({
                "code": code,
                "error": error.to_string(),
                "status": StatusCode::CONFLICT.as_u16(),
                "references": references,
                "suggested_action": "Disable this item or rebind every reference before deleting it."
            }),
        }
    }

    fn provider(error: &ProviderErrorDetails) -> Self {
        let status = match error.code {
            annotagent_core::ProviderErrorCode::MissingCredential
            | annotagent_core::ProviderErrorCode::InvalidEndpoint
            | annotagent_core::ProviderErrorCode::ModelNotFound
            | annotagent_core::ProviderErrorCode::UnsupportedCapability => StatusCode::BAD_REQUEST,
            annotagent_core::ProviderErrorCode::InvalidCredential => StatusCode::UNAUTHORIZED,
            annotagent_core::ProviderErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            annotagent_core::ProviderErrorCode::Timeout
            | annotagent_core::ProviderErrorCode::Unreachable
            | annotagent_core::ProviderErrorCode::IncompatibleProtocol
            | annotagent_core::ProviderErrorCode::ResponseTooLarge
            | annotagent_core::ProviderErrorCode::InvalidResponse => StatusCode::BAD_GATEWAY,
            annotagent_core::ProviderErrorCode::Cancelled => StatusCode::REQUEST_TIMEOUT,
        };
        Self {
            status,
            body: json!({
                "error": error.safe_message,
                "status": status.as_u16(),
                "details": error,
                "suggested_action": "Review the Provider connection and credential, then retry."
            }),
        }
    }

    fn active_run(conflict: &ActiveRunExists) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            body: json!({
                "code": "active_run_exists",
                "active_run_id": conflict.active_run_id,
                "status": conflict.status,
            }),
        }
    }

    fn active_batch(batch: &annotagent_core::BatchRecord) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            body: json!({
                "code": "active_batch_exists",
                "active_batch_id": batch.id,
                "status": batch.status,
            }),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

pub fn router(state: ServerState, web_dist: Option<&Path>) -> Router {
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/provider-presets", get(list_provider_presets))
        .route(
            "/api/registry-migrations/legacy",
            get(preview_legacy_registry_import).post(apply_legacy_registry_import),
        )
        .route(
            "/api/providers",
            get(list_provider_profiles).post(create_provider_profile),
        )
        .route(
            "/api/providers/{provider_id}",
            get(get_provider_profile)
                .patch(update_provider_profile)
                .delete(delete_provider_profile),
        )
        .route(
            "/api/providers/{provider_id}/credential",
            post(save_provider_credential).delete(delete_provider_credential),
        )
        .route(
            "/api/providers/{provider_id}/migrate-credential",
            post(migrate_provider_credential),
        )
        .route(
            "/api/providers/{provider_id}/check",
            post(check_provider_profile),
        )
        .route(
            "/api/providers/{provider_id}/active-probe",
            post(probe_provider_profile),
        )
        .route(
            "/api/providers/{provider_id}/discover-models",
            post(discover_models_for_provider),
        )
        .route(
            "/api/model-profiles",
            get(list_model_profiles).post(create_model_profile),
        )
        .route(
            "/api/model-profiles/compatible",
            get(list_compatible_model_profiles),
        )
        .route(
            "/api/model-profiles/{model_id}",
            get(get_model_profile)
                .patch(update_model_profile)
                .delete(delete_model_profile),
        )
        .route(
            "/api/model-profiles/{model_id}/usage",
            get(get_model_profile_usage),
        )
        .route(
            "/api/model-profiles/{model_id}/quality-contracts",
            get(get_model_profile_quality_contracts),
        )
        .route("/api/plugins", get(list_expert_model_plugins))
        .route("/api/model-catalogs", get(list_model_catalogs))
        .route("/api/model-catalogs/refresh", post(refresh_model_catalog))
        .route("/api/model-catalogs/{catalog_id}", get(get_model_catalog))
        .route("/api/model-bundles", get(list_model_bundles))
        .route(
            "/api/model-bundles/available",
            get(list_available_model_bundles),
        )
        .route(
            "/api/model-bundles/packages/inspect",
            post(inspect_model_bundle_package),
        )
        .route("/api/model-bundles/install", post(install_model_bundle))
        .route(
            "/api/model-installations",
            get(list_model_install_operations).post(start_model_install_operation),
        )
        .route(
            "/api/model-installations/{operation_id}",
            get(get_model_install_operation),
        )
        .route("/api/model-bundles/import", post(import_model_bundle))
        .route(
            "/api/model-bundles/{bundle_id}/{version}",
            get(get_model_bundle).delete(remove_model_bundle),
        )
        .route(
            "/api/model-bundles/{bundle_id}/{version}/verify",
            post(verify_installed_model_bundle),
        )
        .route(
            "/api/model-bundles/{bundle_id}/{version}/test",
            post(test_model_bundle),
        )
        .route(
            "/api/model-bundles/{bundle_id}/{version}/enable",
            post(enable_model_bundle),
        )
        .route(
            "/api/model-bundles/{bundle_id}/{version}/disable",
            post(disable_model_bundle),
        )
        .route(
            "/api/model-bundles/{bundle_id}/{version}/references",
            get(list_model_bundle_references),
        )
        .route(
            "/api/model-bundles/{bundle_id}/{version}/compatibility",
            get(get_model_bundle_compatibility),
        )
        .route("/api/model-bundles/gc", post(garbage_collect_model_bundles))
        .route(
            "/api/model-bundles/{bundle_id}/{version}/license-acceptance",
            post(accept_model_bundle_license),
        )
        .route("/api/model-instances", get(list_model_instances))
        .route(
            "/api/model-instances/{instance_id}",
            get(get_model_instance),
        )
        .route(
            "/api/model-instances/{instance_id}/test",
            post(test_model_instance),
        )
        .route(
            "/api/plugins/{plugin_id}/{version}/compatible-model-bundles",
            get(list_plugin_compatible_model_bundles),
        )
        .route(
            "/api/plugins/packages/inspect",
            post(inspect_expert_model_plugin_package),
        )
        .route(
            "/api/plugins/packages/install",
            post(install_expert_model_plugin_package),
        )
        .route(
            "/api/plugins/{plugin_id}/{version}",
            delete(uninstall_expert_model_plugin),
        )
        .route(
            "/api/plugins/{plugin_id}/{version}/weights",
            post(provision_expert_model_plugin_weights),
        )
        .route(
            "/api/plugins/{plugin_id}/{version}/legacy-model-bundle",
            post(create_legacy_local_model_bundle),
        )
        .route(
            "/api/plugins/{plugin_id}/{version}/test",
            post(test_expert_model_plugin),
        )
        .route(
            "/api/plugins/{plugin_id}/{version}/enable",
            post(enable_expert_model_plugin),
        )
        .route(
            "/api/plugins/{plugin_id}/{version}/disable",
            post(disable_expert_model_plugin),
        )
        .route(
            "/api/projects/{project_id}/model-bindings",
            get(get_project_model_bindings).put(put_project_model_bindings),
        )
        .route(
            "/api/agent-model-bindings",
            get(get_agent_model_bindings).put(put_agent_model_bindings),
        )
        .route("/api/skills", get(list_skills))
        .route("/api/skills/{skill_id}", get(get_skill))
        .route("/api/projects", get(list_projects).post(create_project))
        .route("/api/workflows", get(list_workflows))
        .route(
            "/api/workflow-drafts",
            get(list_workflow_drafts).post(create_workflow_draft),
        )
        .route("/api/workflow-drafts/suggest", post(suggest_workflow))
        .route("/api/workflow-drafts/diff", post(diff_workflow_drafts))
        .route(
            "/api/workflow-drafts/{draft_id}",
            patch(save_workflow_draft),
        )
        .route(
            "/api/workflow-drafts/{draft_id}/apply-diff",
            post(apply_workflow_draft_diff),
        )
        .route(
            "/api/workflow-drafts/{draft_id}/dry-run",
            post(dry_run_workflow),
        )
        .route(
            "/api/workflow-drafts/{draft_id}/sample-test",
            get(get_workflow_sample_test),
        )
        .route(
            "/api/workflow-drafts/{draft_id}/publish",
            post(publish_workflow),
        )
        .route(
            "/api/workflow-drafts/{draft_id}/archive",
            post(archive_workflow_draft),
        )
        .route(
            "/api/workflows/{workflow_id}/versions/{version}/clone",
            post(clone_workflow_version),
        )
        .route(
            "/api/workflows/{workflow_id}/versions/{version}/create-geometry-safe-draft",
            post(create_geometry_safe_draft),
        )
        .route("/api/workflows/compare", post(compare_workflow_versions))
        .route(
            "/api/projects/{project_id}/pipeline-improvements",
            get(list_pipeline_improvements).post(create_pipeline_improvement),
        )
        .route(
            "/api/pipeline-improvements/{improvement_id}",
            get(get_pipeline_improvement),
        )
        .route(
            "/api/pipeline-improvements/{improvement_id}/compare",
            post(compare_pipeline_improvement),
        )
        .route(
            "/api/pipeline-improvements/{improvement_id}/apply-to-draft",
            post(apply_pipeline_improvement),
        )
        .route("/api/models", get(list_models))
        .route("/api/models/{model_id}/test", post(test_detection_worker))
        .route(
            "/api/models/{model_id}/sample-test",
            post(sample_test_detection_worker),
        )
        .route("/api/runs", get(list_run_summaries))
        .route("/api/projects/{project_id}", get(get_project))
        .route(
            "/api/projects/{project_id}/guidance",
            get(get_project_guidance),
        )
        .route(
            "/api/projects/{project_id}/readiness",
            get(get_project_readiness),
        )
        .route(
            "/api/projects/{project_id}/summary",
            get(get_project_summary),
        )
        .route(
            "/api/projects/{project_id}/schema/labels",
            post(add_project_label),
        )
        .route(
            "/api/projects/{project_id}/schema/tasks",
            post(add_project_task),
        )
        .route(
            "/api/projects/{project_id}/skills",
            post(set_project_skills),
        )
        .route(
            "/api/projects/{project_id}/workflow-catalog",
            get(get_workflow_catalog),
        )
        .route("/api/projects/{project_id}/import", post(import_images))
        .route(
            "/api/projects/{project_id}/annotation-import",
            post(import_annotations),
        )
        .route("/api/projects/{project_id}/images", get(list_images))
        .route(
            "/api/projects/{project_id}/images/{index}",
            delete(remove_image),
        )
        .route(
            "/api/projects/{project_id}/agent-sessions",
            get(list_project_agent_sessions),
        )
        .route(
            "/api/projects/{project_id}/correction-memory",
            get(list_project_correction_memory),
        )
        .route(
            "/api/projects/{project_id}/geometry-corrections",
            get(list_project_geometry_corrections),
        )
        .route(
            "/api/projects/{project_id}/geometry-policy",
            get(get_project_geometry_policy).put(put_project_geometry_policy),
        )
        .route(
            "/api/projects/{project_id}/geometry-calibrations",
            get(list_project_geometry_calibrations).post(create_project_geometry_calibration),
        )
        .route(
            "/api/projects/{project_id}/images/{index}/content",
            get(image_content),
        )
        .route("/api/projects/{project_id}/runs", post(start_run))
        .route("/api/projects/{project_id}/batches", post(start_batch))
        .route("/api/batches", get(list_batches))
        .route("/api/batches/{batch_id}", get(get_batch))
        .route("/api/batches/{batch_id}/pause", post(pause_batch))
        .route("/api/batches/{batch_id}/resume", post(resume_batch))
        .route("/api/batches/{batch_id}/cancel", post(cancel_batch))
        .route(
            "/api/projects/{project_id}/export-readiness",
            get(get_export_readiness),
        )
        .route("/api/projects/{project_id}/export", post(export_dataset))
        .route("/api/runs/{run_id}", get(get_run))
        .route(
            "/api/runs/{run_id}/result-summary",
            get(get_run_result_summary),
        )
        .route(
            "/api/runs/{run_id}/debug-summary",
            get(get_run_debug_summary),
        )
        .route(
            "/api/runs/{run_id}/geometry-quality",
            get(get_run_geometry_quality),
        )
        .route(
            "/api/runs/{run_id}/pipeline-artifacts",
            get(inspect_run_pipeline_artifacts),
        )
        .route(
            "/api/runs/{run_id}/replay/{node_id}",
            post(replay_run_from_node),
        )
        .route("/api/runs/{run_id}/pause", post(pause_run))
        .route("/api/runs/{run_id}/resume", post(resume_run))
        .route("/api/runs/{run_id}/cancel", post(cancel_run))
        .route("/api/runs/{run_id}/events", get(run_events))
        .route(
            "/api/runs/{run_id}/annotations",
            get(list_run_annotations).post(create_annotation),
        )
        .route("/api/reviews", get(list_reviews))
        .route("/api/reviews/{review_id}", get(get_review))
        .route("/api/reviews/{review_id}/next", get(get_next_review))
        .route("/api/reviews/{review_id}/decision", post(review_decision))
        .route(
            "/api/geometry-calibrations/{calibration_id}",
            get(get_geometry_calibration),
        )
        .route(
            "/api/reviews/{review_id}/accept-and-next",
            post(accept_review_and_next),
        )
        .route(
            "/api/reviews/{review_id}/reject-and-next",
            post(reject_review_and_next),
        )
        .route(
            "/api/agent-sessions/{session_id}/cancel",
            post(cancel_agent_session),
        )
        .route("/api/annotations/{annotation_id}", patch(patch_annotation))
        .route(
            "/api/annotations/{annotation_id}/revisions",
            get(annotation_revisions),
        )
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/events", get(events))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());
    if let Some(web_dist) = web_dist.filter(|path| path.join("index.html").is_file()) {
        api.fallback_service(
            ServeDir::new(web_dist).fallback(ServeFile::new(web_dist.join("index.html"))),
        )
    } else {
        api.fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                "AnnotAgent Web build not found; run npm --prefix web run build",
            )
        })
    }
}

pub async fn serve(
    state: ServerState,
    address: SocketAddr,
    web_dist: Option<&Path>,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, router(state, web_dist))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ignored = tokio::signal::ctrl_c().await;
}

async fn health(State(state): State<ServerState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "AnnotAgent",
        "workspace": state.application.workspace(),
        "database": state.application.database_path(),
    }))
}

#[derive(Debug, Clone, Serialize)]
struct ProviderPresetDto {
    id: &'static str,
    display_name: &'static str,
    adapter: ProviderAdapterKind,
    base_url: &'static str,
    description: &'static str,
    suggested_models: &'static [&'static str],
}

async fn list_provider_presets() -> Json<Value> {
    let presets = [
        ProviderPresetDto {
            id: "dashscope",
            display_name: "Alibaba DashScope",
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
            description: "OpenAI-compatible DashScope endpoint.",
            suggested_models: &["qwen3.7-flash-2026-07-15", "qwen-max"],
        },
        ProviderPresetDto {
            id: "openai",
            display_name: "OpenAI",
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: "https://api.openai.com/v1",
            description: "OpenAI Chat Completions-compatible endpoint.",
            suggested_models: &[],
        },
        ProviderPresetDto {
            id: "openrouter",
            display_name: "OpenRouter",
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: "https://openrouter.ai/api/v1",
            description: "OpenAI-compatible multi-model routing endpoint.",
            suggested_models: &[],
        },
        ProviderPresetDto {
            id: "gemini-compatible",
            display_name: "Gemini OpenAI compatibility",
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
            description: "Gemini OpenAI compatibility endpoint.",
            suggested_models: &[],
        },
        ProviderPresetDto {
            id: "custom",
            display_name: "Custom OpenAI-compatible",
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: "https://provider.example/v1",
            description: "User-managed compatible endpoint.",
            suggested_models: &[],
        },
    ];
    Json(json!({"presets": presets}))
}

async fn preview_legacy_registry_import(
    State(state): State<ServerState>,
) -> ApiResult<Json<Value>> {
    let settings = state.settings.read().await.clone();
    let preview = state
        .application
        .preview_legacy_registry_import(&settings)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({ "migration": preview })))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyLegacyRegistryImportRequest {
    confirmed: bool,
}

async fn apply_legacy_registry_import(
    State(state): State<ServerState>,
    Json(input): Json<ApplyLegacyRegistryImportRequest>,
) -> ApiResult<Json<Value>> {
    if !input.confirmed {
        return Err(ApiError::bad_request(
            "explicit confirmation is required before importing legacy Registry configuration",
        ));
    }
    let settings = state.settings.read().await.clone();
    let report = state
        .application
        .apply_legacy_registry_import(&settings)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({
        "migration": report,
        "secret_moved": false,
        "historical_runs_modified": false,
    })))
}

#[derive(Debug, Clone, Serialize)]
struct ProviderProfileDto {
    id: ProviderId,
    display_name: String,
    preset_id: Option<String>,
    adapter: ProviderAdapterKind,
    base_url: String,
    endpoint_summary: String,
    organization: Option<String>,
    workspace: Option<String>,
    safe_headers: BTreeMap<String, String>,
    connection_policy: ProviderConnectionPolicy,
    enabled: bool,
    health: ProviderHealthSnapshot,
    credential_configured: bool,
    credential_source: Option<CredentialSource>,
    model_count: usize,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

fn missing_credential_message(source: Option<CredentialSource>) -> &'static str {
    match source {
        Some(CredentialSource::SessionOnly) => {
            "This session-only credential was cleared when the server stopped. Add it again and choose Local workspace file to keep it across restarts."
        }
        Some(CredentialSource::EnvironmentVariable) => {
            "The configured environment variable is not available in this server process. Set it before starting AnnotAgent or save the key as a Local workspace file."
        }
        Some(CredentialSource::WorkspaceFile) => {
            "The local workspace credential file is missing or unavailable. Add the credential again to recreate it."
        }
        Some(CredentialSource::SystemKeyring) => {
            "The system credential store no longer contains this Provider credential. Add it again or choose Local workspace file."
        }
        Some(CredentialSource::LegacyWorkspaceFile) => {
            "The legacy workspace credential file is missing. Add a current Provider credential."
        }
        None => "Configure a Provider credential before making a connection request.",
    }
}

async fn provider_profile_dto(
    state: &ServerState,
    profile: ProviderProfile,
) -> ApiResult<ProviderProfileDto> {
    let credential_configured = match &profile.credential_ref {
        Some(reference) => state.secret_store.exists(reference).await.unwrap_or(false),
        None => profile.adapter == ProviderAdapterKind::Mock,
    };
    let credential_source = profile
        .credential_ref
        .as_ref()
        .map(|reference| reference.source);
    let mut health = profile.health.clone();
    if profile.adapter != ProviderAdapterKind::Mock && !credential_configured {
        health.status = ProviderHealthStatus::Unknown;
        health.safe_message = Some(missing_credential_message(credential_source).to_owned());
    }
    let model_count = state
        .application
        .store()
        .list_model_profiles(Some(profile.id), false)
        .map_err(ApiError::internal)?
        .len();
    let endpoint_summary = profile.endpoint_summary();
    Ok(ProviderProfileDto {
        id: profile.id,
        display_name: profile.display_name,
        preset_id: profile.preset_id,
        adapter: profile.adapter,
        base_url: profile.base_url.to_string(),
        endpoint_summary,
        organization: profile.organization,
        workspace: profile.workspace,
        safe_headers: profile.safe_headers,
        connection_policy: profile.connection_policy,
        enabled: profile.enabled,
        health,
        credential_configured,
        credential_source,
        model_count,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateProviderProfileRequest {
    display_name: String,
    preset_id: Option<String>,
    adapter: ProviderAdapterKind,
    base_url: String,
    #[serde(default)]
    organization: Option<String>,
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    safe_headers: BTreeMap<String, String>,
    #[serde(default)]
    connection_policy: ProviderConnectionPolicy,
    #[serde(default = "default_true_value")]
    enabled: bool,
}

const fn default_true_value() -> bool {
    true
}

async fn list_provider_profiles(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let profiles = state
        .application
        .store()
        .list_provider_profiles()
        .map_err(ApiError::internal)?;
    let mut result = Vec::with_capacity(profiles.len());
    for profile in profiles {
        result.push(provider_profile_dto(&state, profile).await?);
    }
    Ok(Json(json!({"providers": result})))
}

async fn create_provider_profile(
    State(state): State<ServerState>,
    Json(input): Json<CreateProviderProfileRequest>,
) -> ApiResult<Json<ProviderProfileDto>> {
    #[cfg(not(test))]
    if input.adapter == ProviderAdapterKind::Mock {
        return Err(ApiError::bad_request(
            "Mock Providers are test-only and cannot be added to a product workspace",
        ));
    }
    let now = Utc::now();
    let profile = ProviderProfile {
        id: ProviderId::new(),
        display_name: input.display_name,
        preset_id: input.preset_id,
        adapter: input.adapter,
        base_url: input
            .base_url
            .parse()
            .map_err(|_| ApiError::bad_request("base_url must be a valid URL"))?,
        organization: input.organization,
        workspace: input.workspace,
        credential_ref: None,
        safe_headers: input.safe_headers,
        connection_policy: input.connection_policy,
        enabled: input.enabled,
        health: if input.enabled {
            ProviderHealthSnapshot::default()
        } else {
            ProviderHealthSnapshot {
                status: ProviderHealthStatus::Disabled,
                safe_message: Some("Provider is disabled.".to_owned()),
                checked_at: None,
            }
        },
        created_at: now,
        updated_at: now,
    };
    profile.validate().map_err(ApiError::bad_request)?;
    state
        .application
        .store()
        .save_provider_profile(&profile)
        .map_err(ApiError::internal)?;
    Ok(Json(provider_profile_dto(&state, profile).await?))
}

async fn get_provider_profile(
    State(state): State<ServerState>,
    AxumPath(provider_id): AxumPath<String>,
) -> ApiResult<Json<ProviderProfileDto>> {
    let provider_id = parse_provider_id(&provider_id)?;
    let profile = state
        .application
        .store()
        .get_provider_profile(provider_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(provider_profile_dto(&state, profile).await?))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct UpdateProviderProfileRequest {
    display_name: Option<String>,
    preset_id: Option<String>,
    adapter: Option<ProviderAdapterKind>,
    base_url: Option<String>,
    organization: Option<String>,
    workspace: Option<String>,
    safe_headers: Option<BTreeMap<String, String>>,
    connection_policy: Option<ProviderConnectionPolicy>,
    enabled: Option<bool>,
}

async fn update_provider_profile(
    State(state): State<ServerState>,
    AxumPath(provider_id): AxumPath<String>,
    Json(input): Json<UpdateProviderProfileRequest>,
) -> ApiResult<Json<ProviderProfileDto>> {
    #[cfg(not(test))]
    if input.adapter == Some(ProviderAdapterKind::Mock) {
        return Err(ApiError::bad_request(
            "Mock Providers are test-only and cannot be added to a product workspace",
        ));
    }
    let provider_id = parse_provider_id(&provider_id)?;
    let store = state.application.store();
    let mut profile = store
        .get_provider_profile(provider_id)
        .map_err(ApiError::not_found)?;
    let changes_connection_semantics = input.adapter.is_some_and(|value| value != profile.adapter)
        || input.base_url.as_ref().is_some_and(|value| {
            value
                .parse::<url::Url>()
                .map_or(true, |value| value != profile.base_url)
        });
    if changes_connection_semantics {
        let models = store
            .list_model_profiles(Some(provider_id), false)
            .map_err(ApiError::internal)?;
        if !models.is_empty() {
            let references = models
                .into_iter()
                .map(|model| RegistryReference {
                    kind: "model_profile".to_owned(),
                    location: model.id.to_string(),
                })
                .collect::<Vec<_>>();
            return Err(ApiError::conflict(
                "provider_semantic_change_requires_rebind",
                "Endpoint or adapter changes are blocked while Model Profiles use this Provider.",
                &references,
            ));
        }
    }
    if let Some(value) = input.display_name {
        profile.display_name = value;
    }
    if let Some(value) = input.preset_id {
        profile.preset_id = Some(value);
    }
    if let Some(value) = input.adapter {
        profile.adapter = value;
    }
    if let Some(value) = input.base_url {
        profile.base_url = value
            .parse()
            .map_err(|_| ApiError::bad_request("base_url must be a valid URL"))?;
    }
    if let Some(value) = input.organization {
        profile.organization = Some(value);
    }
    if let Some(value) = input.workspace {
        profile.workspace = Some(value);
    }
    if let Some(value) = input.safe_headers {
        profile.safe_headers = value;
    }
    if let Some(value) = input.connection_policy {
        profile.connection_policy = value;
    }
    if let Some(value) = input.enabled {
        profile.enabled = value;
        profile.health = if value {
            ProviderHealthSnapshot::default()
        } else {
            ProviderHealthSnapshot {
                status: ProviderHealthStatus::Disabled,
                safe_message: Some("Provider is disabled.".to_owned()),
                checked_at: Some(Utc::now()),
            }
        };
    }
    profile.updated_at = Utc::now();
    profile.validate().map_err(ApiError::bad_request)?;
    store
        .save_provider_profile(&profile)
        .map_err(ApiError::internal)?;
    Ok(Json(provider_profile_dto(&state, profile).await?))
}

async fn delete_provider_profile(
    State(state): State<ServerState>,
    AxumPath(provider_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let provider_id = parse_provider_id(&provider_id)?;
    let store = state.application.store();
    let profile = store
        .get_provider_profile(provider_id)
        .map_err(ApiError::not_found)?;
    let references = store
        .provider_references(provider_id)
        .map_err(ApiError::internal)?;
    if !references.is_empty() {
        return Err(ApiError::conflict(
            "provider_in_use",
            "Provider cannot be deleted because durable references still use it.",
            &references,
        ));
    }
    if let Some(reference) = &profile.credential_ref
        && matches!(
            reference.source,
            CredentialSource::SystemKeyring
                | CredentialSource::WorkspaceFile
                | CredentialSource::SessionOnly
        )
    {
        state
            .secret_store
            .delete(reference)
            .await
            .map_err(ApiError::bad_request)?;
    }
    store
        .delete_provider_profile(provider_id)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"deleted": provider_id})))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveProviderCredentialRequest {
    source: CredentialSource,
    #[serde(default)]
    secret: Option<String>,
    #[serde(default)]
    environment_variable: Option<String>,
}

async fn save_provider_credential(
    State(state): State<ServerState>,
    AxumPath(provider_id): AxumPath<String>,
    Json(input): Json<SaveProviderCredentialRequest>,
) -> ApiResult<Json<Value>> {
    let provider_id = parse_provider_id(&provider_id)?;
    let store = state.application.store();
    let mut profile = store
        .get_provider_profile(provider_id)
        .map_err(ApiError::not_found)?;
    let locator = match input.source {
        CredentialSource::SystemKeyring => format!("provider-{provider_id}"),
        CredentialSource::WorkspaceFile => format!("registry-provider-{provider_id}"),
        CredentialSource::SessionOnly => format!("session-provider-{provider_id}"),
        CredentialSource::EnvironmentVariable => input
            .environment_variable
            .ok_or_else(|| ApiError::bad_request("environment_variable is required"))?,
        CredentialSource::LegacyWorkspaceFile => {
            return Err(ApiError::bad_request(
                "legacy credentials can only be attached through explicit migration",
            ));
        }
    };
    let reference = if input.source == CredentialSource::EnvironmentVariable {
        let reference = CredentialReference {
            provider_id,
            source: input.source,
            locator: locator.trim().to_owned(),
        };
        reference.validate().map_err(|_| {
            ApiError::bad_request(
                "Enter an environment variable name such as DASHSCOPE_API_KEY, not the API key itself. To paste a key directly, choose the workspace-file credential source.",
            )
        })?;
        if !state
            .secret_store
            .exists(&reference)
            .await
            .map_err(ApiError::bad_request)?
        {
            return Err(ApiError::bad_request(
                "The selected environment variable is not configured in the server process. Set it before starting AnnotAgent, or choose the workspace-file credential source to paste a key directly.",
            ));
        }
        reference
    } else {
        let secret = input.secret.ok_or_else(|| {
            ApiError::bad_request("secret is required for this credential source")
        })?;
        state
            .secret_store
            .put(
                SecretScope {
                    provider_id,
                    source: input.source,
                    locator,
                },
                SecretValue::new(secret).map_err(ApiError::bad_request)?,
            )
            .await
            .map_err(ApiError::bad_request)?
    };
    if let Some(previous) = profile.credential_ref.replace(reference.clone())
        && previous != reference
        && matches!(
            previous.source,
            CredentialSource::SystemKeyring
                | CredentialSource::WorkspaceFile
                | CredentialSource::SessionOnly
        )
    {
        state
            .secret_store
            .delete(&previous)
            .await
            .map_err(ApiError::bad_request)?;
    }
    profile.health = ProviderHealthSnapshot {
        status: ProviderHealthStatus::Configured,
        safe_message: Some(
            "Credential is configured; run a passive check to verify the connection.".to_owned(),
        ),
        checked_at: None,
    };
    profile.updated_at = Utc::now();
    store
        .save_provider_profile(&profile)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "provider_id": provider_id,
        "credential_configured": true,
        "credential_source": reference.source
    })))
}

async fn delete_provider_credential(
    State(state): State<ServerState>,
    AxumPath(provider_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let provider_id = parse_provider_id(&provider_id)?;
    let store = state.application.store();
    let mut profile = store
        .get_provider_profile(provider_id)
        .map_err(ApiError::not_found)?;
    if let Some(reference) = profile.credential_ref.take()
        && matches!(
            reference.source,
            CredentialSource::SystemKeyring
                | CredentialSource::WorkspaceFile
                | CredentialSource::SessionOnly
        )
    {
        state
            .secret_store
            .delete(&reference)
            .await
            .map_err(ApiError::bad_request)?;
    }
    profile.health = ProviderHealthSnapshot::default();
    profile.updated_at = Utc::now();
    store
        .save_provider_profile(&profile)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "provider_id": provider_id,
        "credential_configured": false
    })))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct MigrateProviderCredentialRequest {
    delete_source_after_success: bool,
}

async fn migrate_provider_credential(
    State(state): State<ServerState>,
    AxumPath(provider_id): AxumPath<String>,
    Json(input): Json<MigrateProviderCredentialRequest>,
) -> ApiResult<Json<Value>> {
    let provider_id = parse_provider_id(&provider_id)?;
    let store = state.application.store();
    let mut profile = store
        .get_provider_profile(provider_id)
        .map_err(ApiError::not_found)?;
    let legacy = profile
        .credential_ref
        .clone()
        .filter(|reference| reference.source == CredentialSource::LegacyWorkspaceFile)
        .ok_or_else(|| ApiError::bad_request("Provider does not reference a legacy credential"))?;
    let secret = state
        .secret_store
        .resolve(&legacy)
        .await
        .map_err(ApiError::bad_request)?;
    let migrated = state
        .secret_store
        .put(
            SecretScope {
                provider_id,
                source: CredentialSource::SystemKeyring,
                locator: format!("provider-{provider_id}"),
            },
            secret,
        )
        .await
        .map_err(ApiError::bad_request)?;
    profile.credential_ref = Some(migrated);
    profile.health = ProviderHealthSnapshot {
        status: ProviderHealthStatus::Configured,
        safe_message: Some(
            "Legacy credential migrated to the native system credential store.".to_owned(),
        ),
        checked_at: None,
    };
    profile.updated_at = Utc::now();
    store
        .save_provider_profile(&profile)
        .map_err(ApiError::internal)?;
    if input.delete_source_after_success {
        state
            .secret_store
            .delete(&legacy)
            .await
            .map_err(ApiError::bad_request)?;
    }
    Ok(Json(json!({
        "provider_id": provider_id,
        "credential_configured": true,
        "credential_source": CredentialSource::SystemKeyring,
        "source_deleted": input.delete_source_after_success
    })))
}

async fn resolve_provider_credential(
    state: &ServerState,
    profile: &ProviderProfile,
) -> ApiResult<Option<SecretValue>> {
    match &profile.credential_ref {
        Some(reference) => state
            .secret_store
            .resolve(reference)
            .await
            .map(Some)
            .map_err(|error| match error {
                SecretStoreError::NotFound => {
                    ApiError::bad_request(missing_credential_message(Some(reference.source)))
                }
                other => ApiError::bad_request(other),
            }),
        None if profile.adapter == ProviderAdapterKind::Mock => Ok(None),
        None => Err(ApiError::bad_request(missing_credential_message(None))),
    }
}

async fn discover_models_for_provider(
    State(state): State<ServerState>,
    AxumPath(provider_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let provider_id = parse_provider_id(&provider_id)?;
    let profile = state
        .application
        .store()
        .get_provider_profile(provider_id)
        .map_err(ApiError::not_found)?;
    let credential = resolve_provider_credential(&state, &profile).await?;
    let (models, latency_ms) = discover_provider_models(&profile, credential.as_ref())
        .await
        .map_err(|error| ApiError::provider(&error))?;
    Ok(Json(json!({
        "provider_id": provider_id,
        "models": models,
        "latency_ms": latency_ms,
        "capability_status": "unknown",
        "warning": "Discovery returns model IDs only. Capabilities must be declared or verified separately."
    })))
}

async fn check_provider_profile(
    State(state): State<ServerState>,
    AxumPath(provider_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let provider_id = parse_provider_id(&provider_id)?;
    let store = state.application.store();
    let mut profile = store
        .get_provider_profile(provider_id)
        .map_err(ApiError::not_found)?;
    let credential = resolve_provider_credential(&state, &profile).await?;
    match passive_provider_check(&profile, credential.as_ref()).await {
        Ok(check) => {
            profile.health = ProviderHealthSnapshot {
                status: ProviderHealthStatus::Available,
                safe_message: Some(check.safe_message.clone()),
                checked_at: Some(Utc::now()),
            };
            profile.updated_at = Utc::now();
            store
                .save_provider_profile(&profile)
                .map_err(ApiError::internal)?;
            Ok(Json(json!({
                "provider": provider_profile_dto(&state, profile).await?,
                "check": check,
                "billable": false
            })))
        }
        Err(error) => {
            profile.health = ProviderHealthSnapshot {
                status: health_status_for_provider_error(&error),
                safe_message: Some(error.safe_message.clone()),
                checked_at: Some(Utc::now()),
            };
            profile.updated_at = Utc::now();
            store
                .save_provider_profile(&profile)
                .map_err(ApiError::internal)?;
            Err(ApiError::provider(&error))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveProbeRequest {
    model_profile_id: ModelProfileId,
    confirmed_billable: bool,
}

async fn probe_provider_profile(
    State(state): State<ServerState>,
    AxumPath(provider_id): AxumPath<String>,
    Json(input): Json<ActiveProbeRequest>,
) -> ApiResult<Json<Value>> {
    if !input.confirmed_billable {
        return Err(ApiError::bad_request(
            "active probe may incur Provider charges; set confirmed_billable=true to continue",
        ));
    }
    let provider_id = parse_provider_id(&provider_id)?;
    let store = state.application.store();
    let mut profile = store
        .get_provider_profile(provider_id)
        .map_err(ApiError::not_found)?;
    let mut model = store
        .get_model_profile(input.model_profile_id, None)
        .map_err(ApiError::not_found)?;
    if model.provider_id != provider_id {
        return Err(ApiError::bad_request(
            "the selected Model Profile belongs to a different Provider",
        ));
    }
    let credential = resolve_provider_credential(&state, &profile).await?;
    let probe = active_provider_probe(&profile, credential.as_ref(), &model.remote_model_id)
        .await
        .map_err(|error| ApiError::provider(&error))?;
    let cost = calculate_probe_cost(&model.pricing, &probe);
    let usage = ProviderProbeUsage {
        id: uuid::Uuid::new_v4(),
        provider_id,
        model_profile_id: model.id,
        model_profile_revision: model.revision,
        request_id: probe.request_id.clone(),
        input_tokens: probe.input_tokens,
        output_tokens: probe.output_tokens,
        total_tokens: probe.total_tokens,
        cost: cost.to_string(),
        currency: model.pricing.currency.clone(),
        duration_ms: probe.latency_ms,
        succeeded: true,
        safe_message: probe.safe_message.clone(),
        created_at: Utc::now(),
    };
    store
        .record_provider_probe_usage(&usage)
        .map_err(ApiError::internal)?;
    profile.health = ProviderHealthSnapshot {
        status: ProviderHealthStatus::Available,
        safe_message: Some("Active model probe succeeded.".to_owned()),
        checked_at: Some(Utc::now()),
    };
    profile.updated_at = Utc::now();
    store
        .save_provider_profile(&profile)
        .map_err(ApiError::internal)?;
    model.status = ModelProfileStatus::Available;
    model.updated_at = Utc::now();
    store
        .save_model_profile(&model)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "provider_id": provider_id,
        "model_profile_id": model.id,
        "billable": true,
        "probe": probe,
        "usage": usage
    })))
}

fn calculate_probe_cost(
    pricing: &ModelPricing,
    probe: &annotagent_provider::ProviderActiveProbe,
) -> Decimal {
    let million = Decimal::from(1_000_000_u64);
    let input = Decimal::from(probe.input_tokens.unwrap_or(0));
    let output = Decimal::from(probe.output_tokens.unwrap_or(0));
    pricing.per_request.unwrap_or(Decimal::ZERO)
        + pricing
            .input_per_million_tokens
            .map_or(Decimal::ZERO, |rate| input * rate / million)
        + pricing
            .output_per_million_tokens
            .map_or(Decimal::ZERO, |rate| output * rate / million)
}

fn health_status_for_provider_error(error: &ProviderErrorDetails) -> ProviderHealthStatus {
    match error.code {
        annotagent_core::ProviderErrorCode::InvalidCredential
        | annotagent_core::ProviderErrorCode::MissingCredential => {
            ProviderHealthStatus::InvalidCredential
        }
        annotagent_core::ProviderErrorCode::RateLimited => ProviderHealthStatus::RateLimited,
        annotagent_core::ProviderErrorCode::IncompatibleProtocol
        | annotagent_core::ProviderErrorCode::InvalidResponse
        | annotagent_core::ProviderErrorCode::ResponseTooLarge => {
            ProviderHealthStatus::IncompatibleProtocol
        }
        _ => ProviderHealthStatus::Unreachable,
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateModelProfileRequest {
    provider_id: ProviderId,
    display_name: String,
    remote_model_id: String,
    input_modalities: BTreeSet<InputModality>,
    #[serde(default)]
    protocol_features: ProtocolFeatures,
    task_capabilities: BTreeSet<ModelCapability>,
    #[serde(default = "user_declared_capabilities")]
    capability_source: CapabilityDeclarationSource,
    #[serde(default)]
    limits: ModelLimits,
    #[serde(default)]
    generation_defaults: GenerationDefaults,
    #[serde(default)]
    pricing: ModelPricing,
    #[serde(default)]
    quality_contracts: Vec<ModelCapabilityQualityContractInput>,
    #[serde(default = "default_true_value")]
    enabled: bool,
    #[serde(default)]
    locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelCapabilityQualityContractInput {
    capability: ModelCapability,
    operation: String,
    output_geometry: GeometrySemantics,
    score_semantics: ScoreSemantics,
    auto_accept_eligibility: AutoAcceptEligibility,
    #[serde(default)]
    small_object_localization: SmallObjectLocalizationSupport,
    #[serde(default)]
    requires_geometry_verification: bool,
}

impl ModelCapabilityQualityContractInput {
    fn bind(
        self,
        model_profile_id: ModelProfileId,
        model_profile_revision: u64,
    ) -> ModelCapabilityQualityContract {
        ModelCapabilityQualityContract {
            model_profile_id,
            model_profile_revision,
            capability: self.capability,
            operation: self.operation,
            output_geometry: self.output_geometry,
            score_semantics: self.score_semantics,
            auto_accept_eligibility: self.auto_accept_eligibility,
            evidence_source: ContractEvidenceSource::UserDeclared,
            small_object_localization: self.small_object_localization,
            requires_geometry_verification: self.requires_geometry_verification,
        }
    }

    fn matches(&self, contract: &ModelCapabilityQualityContract) -> bool {
        self.capability == contract.capability
            && self.operation == contract.operation
            && self.output_geometry == contract.output_geometry
            && self.score_semantics == contract.score_semantics
            && self.auto_accept_eligibility == contract.auto_accept_eligibility
            && contract.evidence_source == ContractEvidenceSource::UserDeclared
            && self.small_object_localization == contract.small_object_localization
            && self.requires_geometry_verification == contract.requires_geometry_verification
    }
}

const fn user_declared_capabilities() -> CapabilityDeclarationSource {
    CapabilityDeclarationSource::UserDeclared
}

async fn list_model_profiles(
    State(state): State<ServerState>,
    Query(query): Query<ModelProfileListQuery>,
) -> ApiResult<Json<Value>> {
    let provider_id = query
        .provider_id
        .as_deref()
        .map(parse_provider_id)
        .transpose()?;
    let models = state
        .application
        .store()
        .list_model_profiles(provider_id, query.all_revisions)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"models": models})))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ModelProfileListQuery {
    provider_id: Option<String>,
    all_revisions: bool,
}

async fn create_model_profile(
    State(state): State<ServerState>,
    Json(input): Json<CreateModelProfileRequest>,
) -> ApiResult<Json<ModelProfile>> {
    state
        .application
        .store()
        .get_provider_profile(input.provider_id)
        .map_err(ApiError::bad_request)?;
    let now = Utc::now();
    let model_profile_id = ModelProfileId::new();
    let quality_contracts = input
        .quality_contracts
        .into_iter()
        .map(|contract| contract.bind(model_profile_id, 1))
        .collect();
    let profile = ModelProfile {
        id: model_profile_id,
        revision: 1,
        provider_id: input.provider_id,
        display_name: input.display_name,
        remote_model_id: input.remote_model_id,
        input_modalities: input.input_modalities,
        protocol_features: input.protocol_features,
        task_capabilities: input.task_capabilities,
        capability_source: input.capability_source,
        limits: input.limits,
        generation_defaults: input.generation_defaults,
        pricing: input.pricing,
        quality_contracts,
        status: if input.enabled {
            ModelProfileStatus::Unverified
        } else {
            ModelProfileStatus::Disabled
        },
        enabled: input.enabled,
        locked: input.locked,
        created_at: now,
        updated_at: now,
    };
    state
        .application
        .store()
        .save_model_profile(&profile)
        .map_err(ApiError::bad_request)?;
    Ok(Json(profile))
}

async fn get_model_profile(
    State(state): State<ServerState>,
    AxumPath(model_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let model_id = parse_model_profile_id(&model_id)?;
    let model = state
        .application
        .store()
        .get_model_profile(model_id, None)
        .map_err(ApiError::not_found)?;
    let revisions = state
        .application
        .store()
        .list_model_profiles(Some(model.provider_id), true)
        .map_err(ApiError::internal)?
        .into_iter()
        .filter(|revision| revision.id == model_id)
        .collect::<Vec<_>>();
    Ok(Json(json!({"model": model, "revisions": revisions})))
}

async fn get_model_profile_quality_contracts(
    State(state): State<ServerState>,
    AxumPath(model_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let model_id = parse_model_profile_id(&model_id)?;
    let model = state
        .application
        .store()
        .get_model_profile(model_id, None)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!({
        "model_profile_id": model.id,
        "model_profile_revision": model.revision,
        "contracts": effective_model_quality_contracts(&model),
    })))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct UpdateModelProfileRequest {
    provider_id: Option<ProviderId>,
    display_name: Option<String>,
    remote_model_id: Option<String>,
    input_modalities: Option<BTreeSet<InputModality>>,
    protocol_features: Option<ProtocolFeatures>,
    task_capabilities: Option<BTreeSet<ModelCapability>>,
    capability_source: Option<CapabilityDeclarationSource>,
    limits: Option<ModelLimits>,
    generation_defaults: Option<GenerationDefaults>,
    pricing: Option<ModelPricing>,
    quality_contracts: Option<Vec<ModelCapabilityQualityContractInput>>,
    enabled: Option<bool>,
    locked: Option<bool>,
}

async fn update_model_profile(
    State(state): State<ServerState>,
    AxumPath(model_id): AxumPath<String>,
    Json(input): Json<UpdateModelProfileRequest>,
) -> ApiResult<Json<ModelProfile>> {
    let model_id = parse_model_profile_id(&model_id)?;
    let store = state.application.store();
    let previous = store
        .get_model_profile(model_id, None)
        .map_err(ApiError::not_found)?;
    let mut profile = previous.clone();
    if let Some(value) = input.provider_id {
        store
            .get_provider_profile(value)
            .map_err(ApiError::bad_request)?;
        profile.provider_id = value;
    }
    if let Some(value) = input.display_name {
        profile.display_name = value;
    }
    if let Some(value) = input.remote_model_id {
        profile.remote_model_id = value;
    }
    if let Some(value) = input.input_modalities {
        profile.input_modalities = value;
    }
    if let Some(value) = input.protocol_features {
        profile.protocol_features = value;
    }
    if let Some(value) = input.task_capabilities {
        profile.task_capabilities = value;
    }
    if let Some(value) = input.capability_source {
        profile.capability_source = value;
    }
    if let Some(value) = input.limits {
        profile.limits = value;
    }
    if let Some(value) = input.generation_defaults {
        profile.generation_defaults = value;
    }
    if let Some(value) = input.pricing {
        profile.pricing = value;
    }
    if let Some(value) = input.enabled {
        profile.enabled = value;
        profile.status = if value {
            ModelProfileStatus::Unverified
        } else {
            ModelProfileStatus::Disabled
        };
    }
    if let Some(value) = input.locked {
        profile.locked = value;
    }
    let quality_contracts_changed = input.quality_contracts.as_ref().is_some_and(|requested| {
        requested.len() != previous.quality_contracts.len()
            || requested
                .iter()
                .zip(&previous.quality_contracts)
                .any(|(requested, previous)| !requested.matches(previous))
    });
    let semantic_change = !profile.has_same_semantics(&previous) || quality_contracts_changed;
    if semantic_change {
        profile.revision = previous.revision.saturating_add(1);
        if profile.enabled {
            profile.status = ModelProfileStatus::Unverified;
        }
    }
    if let Some(contracts) = input.quality_contracts {
        if quality_contracts_changed {
            profile.quality_contracts = contracts
                .into_iter()
                .map(|contract| contract.bind(profile.id, profile.revision))
                .collect();
        }
    } else if semantic_change {
        for contract in &mut profile.quality_contracts {
            contract.model_profile_revision = profile.revision;
        }
    }
    profile.updated_at = Utc::now();
    store
        .save_model_profile(&profile)
        .map_err(ApiError::bad_request)?;
    Ok(Json(profile))
}

async fn delete_model_profile(
    State(state): State<ServerState>,
    AxumPath(model_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let model_id = parse_model_profile_id(&model_id)?;
    let store = state.application.store();
    store
        .get_model_profile(model_id, None)
        .map_err(ApiError::not_found)?;
    let references = store
        .model_profile_references(model_id)
        .map_err(ApiError::internal)?;
    if !references.is_empty() {
        return Err(ApiError::conflict(
            "model_profile_in_use",
            "Model Profile cannot be deleted because durable references still use it.",
            &references,
        ));
    }
    store
        .delete_model_profile(model_id)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"deleted": model_id})))
}

async fn get_model_profile_usage(
    State(state): State<ServerState>,
    AxumPath(model_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let model_id = parse_model_profile_id(&model_id)?;
    state
        .application
        .store()
        .get_model_profile(model_id, None)
        .map_err(ApiError::not_found)?;
    let usage = state
        .application
        .store()
        .list_provider_probe_usage(Some(model_id))
        .map_err(ApiError::internal)?;
    Ok(Json(
        json!({"model_profile_id": model_id, "active_probes": usage}),
    ))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CompatibleModelQuery {
    input_modalities: Option<String>,
    capabilities: Option<String>,
    tool_calls: bool,
    structured_output: bool,
    json_schema: bool,
    allow_unverified: bool,
}

async fn list_compatible_model_profiles(
    State(state): State<ServerState>,
    Query(query): Query<CompatibleModelQuery>,
) -> ApiResult<Json<Value>> {
    let store = state.application.store();
    let models = store
        .list_model_profiles(None, false)
        .map_err(ApiError::internal)?;
    let providers = store
        .list_provider_profiles()
        .map_err(ApiError::internal)?
        .into_iter()
        .map(|provider| (provider.id, provider))
        .collect::<BTreeMap<_, _>>();
    let requirements = ModelRequirements {
        input_modalities: parse_enum_set(query.input_modalities.as_deref())?,
        protocol_features: ProtocolFeatures {
            tool_calls: query.tool_calls,
            structured_output: query.structured_output,
            json_schema: query.json_schema,
            ..ProtocolFeatures::default()
        },
        task_capabilities: parse_enum_set(query.capabilities.as_deref())?,
        allow_unverified: query.allow_unverified,
    };
    let mut compatible = Vec::new();
    for model in models {
        let credential_configured = match providers
            .get(&model.provider_id)
            .and_then(|provider| provider.credential_ref.as_ref())
        {
            Some(reference) => state.secret_store.exists(reference).await.unwrap_or(false),
            None => providers
                .get(&model.provider_id)
                .is_some_and(|provider| provider.adapter == ProviderAdapterKind::Mock),
        };
        let result = check_model_compatibility(
            &model,
            providers.get(&model.provider_id),
            credential_configured,
            &requirements,
        );
        if result.compatible {
            compatible.push(model);
        }
    }
    Ok(Json(
        json!({"models": compatible, "requirements": requirements}),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PutProjectModelBindingsRequest {
    bindings: Vec<ProjectModelBindingInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectModelBindingInput {
    capability: ModelCapability,
    role: ModelBindingRole,
    match_kind: ModelBindingMatch,
    model_profile_id: ModelProfileId,
    #[serde(default)]
    locked: bool,
}

async fn get_project_model_bindings(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let stable_id = registry_project_id(&state, &project_id)?;
    let bindings = state
        .application
        .store()
        .list_project_model_bindings(stable_id)
        .map_err(ApiError::internal)?;
    Ok(Json(
        json!({"project_id": project_id, "bindings": bindings}),
    ))
}

async fn put_project_model_bindings(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(input): Json<PutProjectModelBindingsRequest>,
) -> ApiResult<Json<Value>> {
    let stable_id = registry_project_id(&state, &project_id)?;
    let store = state.application.store();
    let now = Utc::now();
    let bindings = input
        .bindings
        .into_iter()
        .map(|binding| ProjectModelBinding {
            id: ModelBindingId::new(),
            project_id: stable_id,
            capability: binding.capability,
            role: binding.role,
            match_kind: binding.match_kind,
            model_profile_id: binding.model_profile_id,
            locked: binding.locked,
            created_at: now,
        })
        .collect::<Vec<_>>();
    for binding in &bindings {
        let model = store
            .get_model_profile(binding.model_profile_id, None)
            .map_err(ApiError::bad_request)?;
        binding
            .validate_for_model(&model)
            .map_err(ApiError::bad_request)?;
    }
    for existing in store
        .list_project_model_bindings(stable_id)
        .map_err(ApiError::internal)?
    {
        store
            .delete_project_model_binding(existing.id, BindingMutationActor::User)
            .map_err(ApiError::internal)?;
    }
    for binding in &bindings {
        store
            .save_project_model_binding(binding, BindingMutationActor::User)
            .map_err(ApiError::bad_request)?;
    }
    Ok(Json(
        json!({"project_id": project_id, "bindings": bindings}),
    ))
}

async fn get_agent_model_bindings(
    State(state): State<ServerState>,
) -> ApiResult<Json<GlobalModelDefaults>> {
    let defaults = state
        .application
        .store()
        .get_global_model_defaults()
        .map_err(ApiError::internal)?;
    Ok(Json(defaults))
}

async fn put_agent_model_bindings(
    State(state): State<ServerState>,
    Json(defaults): Json<GlobalModelDefaults>,
) -> ApiResult<Json<GlobalModelDefaults>> {
    state
        .application
        .store()
        .save_global_model_defaults(&defaults)
        .map_err(ApiError::bad_request)?;
    Ok(Json(defaults))
}

fn parse_provider_id(value: &str) -> ApiResult<ProviderId> {
    value
        .parse()
        .map_err(|_| ApiError::bad_request("provider_id must be a UUID"))
}

fn parse_model_profile_id(value: &str) -> ApiResult<ModelProfileId> {
    value
        .parse()
        .map_err(|_| ApiError::bad_request("model_profile_id must be a UUID"))
}

fn parse_enum_set<T>(value: Option<&str>) -> ApiResult<BTreeSet<T>>
where
    T: serde::de::DeserializeOwned + Ord,
{
    value
        .unwrap_or_default()
        .split(',')
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            serde_json::from_value(Value::String(value.trim().to_owned()))
                .map_err(ApiError::bad_request)
        })
        .collect()
}

fn registry_project_id(
    state: &ServerState,
    project_id: &str,
) -> ApiResult<annotagent_core::ProjectId> {
    let project_path = state
        .application
        .project_path(project_id)
        .map_err(ApiError::not_found)?;
    let project_root = project_path
        .parent()
        .ok_or_else(|| ApiError::internal("Project path has no parent"))?;
    Ok(stable_project_id(project_root))
}

#[derive(Debug, Serialize)]
struct SkillDetail {
    id: String,
    display_name: String,
    version: String,
    kind: annotagent_core::SkillKind,
    description: String,
    product_visibility: annotagent_core::SkillProductVisibility,
    deprecated_alias_for: Option<String>,
    nodes: Vec<String>,
    tools: Vec<String>,
    validators: Vec<String>,
    refiners: Vec<String>,
    policies: Vec<String>,
    capabilities: Vec<String>,
    capability_requirements: Vec<String>,
    correction_taxonomy: Vec<String>,
    resources: Vec<String>,
    workflow_templates: Vec<Value>,
    projects: Vec<String>,
    project_template: Option<String>,
}

fn skill_detail(
    skill: &dyn annotagent_core::Skill,
    projects: Vec<String>,
    project_template: Option<String>,
) -> SkillDetail {
    let manifest = skill.manifest();
    SkillDetail {
        id: skill.id().to_owned(),
        display_name: manifest.display_name.clone(),
        version: manifest.skill_version.clone(),
        kind: manifest.kind,
        description: manifest.description.clone(),
        product_visibility: manifest.product_visibility,
        deprecated_alias_for: manifest.deprecated_alias_for.clone(),
        nodes: manifest.nodes.clone(),
        tools: skill
            .tool_factories()
            .into_iter()
            .map(|tool| tool.definition().name)
            .collect(),
        validators: skill
            .validators()
            .into_iter()
            .map(|validator| validator.id().to_owned())
            .collect(),
        refiners: skill
            .refiners()
            .into_iter()
            .map(|refiner| refiner.id().to_owned())
            .collect(),
        policies: manifest.policies.clone(),
        capabilities: manifest.capabilities.clone(),
        capability_requirements: manifest
            .dependencies
            .iter()
            .map(|dependency| format!("{}@{}", dependency.id, dependency.version))
            .collect(),
        correction_taxonomy: skill
            .correction_taxonomy()
            .into_iter()
            .map(|kind| kind.code)
            .collect(),
        resources: manifest
            .summary_resources
            .iter()
            .chain(manifest.task_resources.values().flatten())
            .cloned()
            .collect(),
        workflow_templates: skill
            .workflow_templates()
            .into_iter()
            .map(|template| {
                json!({
                    "id": template.id,
                    "name": template.name,
                    "description": template.description,
                    "node_count": template.nodes.len(),
                })
            })
            .collect(),
        projects,
        project_template,
    }
}

async fn list_skills(State(state): State<ServerState>) -> ApiResult<Json<Vec<SkillDetail>>> {
    let projects = product_projects(&state)?;
    Ok(Json(
        state
            .application
            .layered_skills()
            .list()
            .iter()
            .filter(|skill| {
                skill.manifest().product_visibility
                    == annotagent_core::SkillProductVisibility::Primary
            })
            .map(|skill| {
                let used_by = projects
                    .iter()
                    .filter(|project| {
                        project
                            .enabled_skills
                            .iter()
                            .any(|enabled| enabled.id == skill.id())
                    })
                    .map(|project| project.id.clone())
                    .collect();
                let project_template = state
                    .application
                    .skills()
                    .get(skill.id())
                    .ok()
                    .and_then(|legacy| legacy.project_template().map(str::to_owned));
                skill_detail(skill.as_ref(), used_by, project_template)
            })
            .collect(),
    ))
}

async fn get_skill(
    State(state): State<ServerState>,
    AxumPath(skill_id): AxumPath<String>,
) -> ApiResult<Json<SkillDetail>> {
    let skill = state
        .application
        .layered_skills()
        .get(&skill_id)
        .map_err(ApiError::not_found)?;
    let projects = product_projects(&state)?
        .into_iter()
        .filter(|project| {
            project
                .enabled_skills
                .iter()
                .any(|enabled| enabled.id == skill_id)
        })
        .map(|project| project.id)
        .collect();
    let project_template = state
        .application
        .skills()
        .get(&skill_id)
        .ok()
        .and_then(|legacy| legacy.project_template().map(str::to_owned));
    Ok(Json(skill_detail(
        skill.as_ref(),
        projects,
        project_template,
    )))
}

async fn list_project_agent_sessions(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let sessions = state
        .application
        .list_agent_sessions(&project_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!({"sessions": sessions})))
}

async fn cancel_agent_session(
    State(state): State<ServerState>,
    AxumPath(session_id): AxumPath<uuid::Uuid>,
) -> ApiResult<Json<Value>> {
    let session = state
        .application
        .cancel_agent_session(session_id)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"session": session})))
}

async fn list_project_correction_memory(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let records = state
        .application
        .list_project_correction_memory(&project_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!({"records": records})))
}

async fn list_project_geometry_corrections(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let records = state
        .application
        .list_project_geometry_corrections(&project_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(geometry_correction_response(records)))
}

async fn get_run_geometry_quality(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    let records = state
        .application
        .list_run_geometry_corrections(run_id)
        .map_err(ApiError::internal)?;
    Ok(Json(geometry_correction_response(records)))
}

fn geometry_correction_response(
    records: Vec<(
        annotagent_core::GeometryQualityReport,
        annotagent_core::GeometryCorrectionEvidence,
    )>,
) -> Value {
    let mut summary = GeometryQualitySummary::default();
    for (report, evidence) in &records {
        summary.add_correction(report, evidence);
    }
    let (reports, evidence): (Vec<_>, Vec<_>) = records.into_iter().unzip();
    json!({
        "summary": summary,
        "reports": reports,
        "evidence": evidence,
    })
}

#[derive(Debug, Clone, Deserialize)]
struct UpdateGeometryPolicyRequest {
    task_kind: TaskKind,
    required_quality: RequiredGeometryQuality,
    auto_accept_policy: GeometryAutoAcceptPolicy,
    calibration_thresholds: GeometryCalibrationThresholds,
}

async fn get_project_geometry_policy(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let policies = state
        .application
        .project_geometry_policies(&project_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!({"policies": policies})))
}

async fn put_project_geometry_policy(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<UpdateGeometryPolicyRequest>,
) -> ApiResult<Json<Value>> {
    let policy = state
        .application
        .save_project_geometry_policy(
            &project_id,
            ProjectGeometryPolicy {
                project_id: annotagent_core::ProjectId::new(),
                task_kind: request.task_kind,
                required_quality: request.required_quality,
                auto_accept_policy: request.auto_accept_policy,
                calibration_thresholds: request.calibration_thresholds,
            },
        )
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"policy": policy})))
}

async fn list_project_geometry_calibrations(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let calibrations = state
        .application
        .project_geometry_calibrations(&project_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!({"calibrations": calibrations})))
}

async fn create_project_geometry_calibration(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<GeometryCalibrationRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let report = state
        .application
        .create_geometry_calibration(&project_id, &request)
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::CREATED, Json(json!({"calibration": report}))))
}

async fn get_geometry_calibration(
    State(state): State<ServerState>,
    AxumPath(calibration_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = calibration_id
        .parse::<GeometryCalibrationId>()
        .map_err(|_| ApiError::bad_request("calibration_id must be a UUID"))?;
    let report = state
        .application
        .geometry_calibration(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("geometry calibration was not found"))?;
    Ok(Json(json!({"calibration": report})))
}

fn workspace_model_binding(settings: &Settings) -> ModelBinding {
    let offline = settings.default_provider == "mock";
    ModelBinding {
        id: "default-vision".to_owned(),
        provider: settings.default_provider.clone(),
        model: if offline {
            "deterministic-mock".to_owned()
        } else {
            settings.provider.model.clone()
        },
        role: "vision".to_owned(),
        scope: "workspace_default".to_owned(),
        health_status: if offline { "healthy" } else { "unknown" }.to_owned(),
        health_detail: Some(if offline {
            "offline backend is available".to_owned()
        } else {
            "external provider is checked on request".to_owned()
        }),
        availability_group: if offline {
            annotagent_application::ModelAvailabilityGroup::Ready
        } else {
            annotagent_application::ModelAvailabilityGroup::ConfiguredUnavailable
        },
        capabilities: vec!["vision_language".to_owned(), "classification".to_owned()],
        score_semantics: None,
        model_version: None,
        endpoint: None,
        enabled: Some(true),
        license_summary: None,
        architecture: None,
        checkpoint_sha256: None,
        label_space: Vec::new(),
        cost_per_request: Some(settings.pricing.per_request),
    }
}

fn registry_model_binding(model: &ModelProfile, provider: &ProviderProfile) -> ModelBinding {
    let ready = model.enabled
        && model.status == ModelProfileStatus::Available
        && provider.enabled
        && matches!(
            provider.health.status,
            ProviderHealthStatus::Available | ProviderHealthStatus::Configured
        );
    let capabilities = model
        .task_capabilities
        .iter()
        .filter_map(|capability| {
            serde_json::to_value(capability)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
        })
        .collect::<Vec<_>>();
    let role = if model
        .task_capabilities
        .contains(&ModelCapability::ObjectDetection)
        || model
            .task_capabilities
            .contains(&ModelCapability::OpenVocabularyDetection)
    {
        "detection"
    } else if model
        .task_capabilities
        .contains(&ModelCapability::ImageClassification)
    {
        "classification"
    } else if model.task_capabilities.iter().any(|capability| {
        matches!(
            capability,
            ModelCapability::SemanticSegmentation
                | ModelCapability::PromptedSegmentation
                | ModelCapability::InstanceSegmentation
        )
    }) {
        "segmentation"
    } else {
        "vision"
    };
    ModelBinding {
        id: model.id.to_string(),
        provider: provider.display_name.clone(),
        model: model.display_name.clone(),
        role: role.to_owned(),
        scope: format!("registry_profile@{}", model.revision),
        health_status: if ready { "healthy" } else { "unavailable" }.to_owned(),
        health_detail: Some(format!(
            "{} · {}",
            provider.endpoint_summary(),
            model.remote_model_id
        )),
        availability_group: if ready {
            annotagent_application::ModelAvailabilityGroup::Ready
        } else {
            annotagent_application::ModelAvailabilityGroup::ConfiguredUnavailable
        },
        capabilities,
        score_semantics: None,
        model_version: Some(format!("revision {}", model.revision)),
        endpoint: Some(provider.endpoint_summary()),
        enabled: Some(model.enabled && provider.enabled),
        license_summary: None,
        architecture: None,
        checkpoint_sha256: None,
        label_space: Vec::new(),
        cost_per_request: model.pricing.per_request,
    }
}

fn registry_model_bindings(state: &ServerState) -> ApiResult<Vec<ModelBinding>> {
    let providers = state
        .application
        .store()
        .list_provider_profiles()
        .map_err(ApiError::internal)?;
    Ok(state
        .application
        .store()
        .list_model_profiles(None, false)
        .map_err(ApiError::internal)?
        .into_iter()
        .filter_map(|model| {
            providers
                .iter()
                .find(|provider| provider.id == model.provider_id)
                .map(|provider| registry_model_binding(&model, provider))
        })
        .collect())
}

fn worker_model_binding(worker: &DetectionWorkerSettings) -> ModelBinding {
    let manifest = worker.expert_manifest().ok();
    let capabilities = manifest
        .as_ref()
        .map(|manifest| manifest.capabilities.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default()
        .iter()
        .filter_map(|capability| {
            serde_json::to_value(capability)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
        })
        .collect();
    let score_semantics = serde_json::to_value(worker.score_semantics)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned));
    let availability = manifest.as_ref().map_or(
        annotagent_core::ModelAvailability::Unconfigured,
        |manifest| manifest.availability,
    );
    let (health_status, availability_group) = match availability {
        annotagent_core::ModelAvailability::Available => (
            "available",
            annotagent_application::ModelAvailabilityGroup::Ready,
        ),
        annotagent_core::ModelAvailability::MissingWeights => (
            "missing_weights",
            annotagent_application::ModelAvailabilityGroup::Labs,
        ),
        annotagent_core::ModelAvailability::Unconfigured => (
            "unconfigured",
            annotagent_application::ModelAvailabilityGroup::Labs,
        ),
        annotagent_core::ModelAvailability::Disabled => (
            "disabled",
            annotagent_application::ModelAvailabilityGroup::Disabled,
        ),
        annotagent_core::ModelAvailability::Unknown => (
            "unknown",
            annotagent_application::ModelAvailabilityGroup::ConfiguredUnavailable,
        ),
        annotagent_core::ModelAvailability::Unreachable => (
            "unreachable",
            annotagent_application::ModelAvailabilityGroup::ConfiguredUnavailable,
        ),
        annotagent_core::ModelAvailability::IncompatibleProtocol => (
            "incompatible_protocol",
            annotagent_application::ModelAvailabilityGroup::ConfiguredUnavailable,
        ),
        annotagent_core::ModelAvailability::InvalidContract => (
            "invalid_contract",
            annotagent_application::ModelAvailabilityGroup::ConfiguredUnavailable,
        ),
        annotagent_core::ModelAvailability::FailedSmokeTest => (
            "failed_smoke_test",
            annotagent_application::ModelAvailabilityGroup::ConfiguredUnavailable,
        ),
    };
    ModelBinding {
        id: worker.model_id.clone(),
        provider: "http_vision".to_owned(),
        model: worker.display_name.clone(),
        role: if worker.expected_capabilities.iter().any(|capability| {
            matches!(
                capability,
                annotagent_core::VisionCapability::SemanticSegmentation
                    | annotagent_core::VisionCapability::PromptedSegmentation
            )
        }) {
            "segmentation"
        } else {
            "detection"
        }
        .to_owned(),
        scope: "workspace_worker".to_owned(),
        health_status: health_status.to_owned(),
        health_detail: manifest.and_then(|manifest| manifest.availability_evidence.detail),
        availability_group,
        capabilities,
        score_semantics,
        model_version: Some(worker.version.model_version.clone()),
        endpoint: Some(worker.base_url.clone()),
        enabled: Some(worker.enabled),
        license_summary: worker
            .license
            .weight_license
            .clone()
            .or_else(|| worker.license.code_license.clone())
            .or_else(|| Some("License metadata not configured".to_owned())),
        architecture: worker.version.architecture.clone(),
        checkpoint_sha256: worker.version.checkpoint_sha256.clone(),
        label_space: worker.label_space.clone(),
        cost_per_request: Some(worker.cost_per_request),
    }
}

fn product_projects(state: &ServerState) -> ApiResult<Vec<ProjectSummary>> {
    let mut projects = state
        .application
        .list_projects()
        .map_err(ApiError::internal)?;
    let registry_bindings = registry_model_bindings(state)?;
    for project in &mut projects {
        project.model_bindings.clone_from(&registry_bindings);
    }
    Ok(projects)
}

#[derive(Debug, Serialize)]
struct RunSummary {
    id: RunId,
    project_name: String,
    workflow_name: String,
    workflow_version: String,
    skill_versions: Vec<String>,
    model_bindings: Vec<ModelBinding>,
    provider: String,
    model: String,
    status: RunStatus,
    controllable: bool,
    input_tokens: u64,
    output_tokens: u64,
    cost: String,
    current_node: Option<String>,
    current_node_status: Option<String>,
    artifact_count: usize,
    validation_issue_codes: Vec<String>,
    retry_count: u32,
    fallback_nodes: Vec<String>,
    model_identity: String,
    timed_out: bool,
    checkpoint_present: bool,
    review_suspended: bool,
    terminal_reason: Option<String>,
    created_at: String,
    updated_at: String,
}

fn validation_issue_codes(events: &[RunEvent]) -> Vec<String> {
    let mut codes = events
        .iter()
        .filter_map(|event| match &event.payload {
            annotagent_core::RunEventPayload::Validation { issue_codes, .. } => {
                Some(issue_codes.as_slice())
            }
            _ => None,
        })
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    codes.sort();
    codes.dedup();
    codes
}

fn run_summary(state: &ServerState, run: HistoryRun) -> ApiResult<RunSummary> {
    let project = serde_json::from_str::<ProjectSchema>(&run.project_schema_json).ok();
    let workflow_snapshot = run
        .workflow_snapshot_json
        .as_deref()
        .and_then(|snapshot| serde_json::from_str::<Value>(snapshot).ok());
    let explicitly_selected = workflow_snapshot
        .as_ref()
        .is_some_and(|snapshot| !snapshot["selected_workflow"].is_null());
    let workflow_name = if explicitly_selected {
        workflow_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.pointer("/selected_workflow/snapshot/draft/name"))
            .and_then(Value::as_str)
            .unwrap_or("Published workflow")
            .to_owned()
    } else {
        "Configured task graph".to_owned()
    };
    let workflow_version = if explicitly_selected {
        workflow_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot["selected_workflow"]["version"].as_u64())
            .map_or_else(|| "unknown".to_owned(), |version| version.to_string())
    } else {
        project
            .as_ref()
            .map_or_else(|| "legacy".to_owned(), |schema| schema.version.to_string())
    };
    let skill_versions = workflow_snapshot
        .as_ref()
        .and_then(|snapshot| {
            snapshot
                .pointer("/selected_workflow/snapshot/enabled_skills")
                .and_then(Value::as_object)
        })
        .map(|skills| {
            skills
                .iter()
                .map(|(id, version)| format!("{id}@{}", version.as_str().unwrap_or("unknown")))
                .collect::<Vec<_>>()
        })
        .filter(|skills| !skills.is_empty())
        .unwrap_or_else(|| {
            project.as_ref().map_or_else(
                || vec!["unknown".to_owned()],
                |schema| {
                    if run.skill_id == "none" || run.skill_id.is_empty() {
                        Vec::new()
                    } else {
                        vec![format!("{}@{}", run.skill_id, schema.project.skill_version)]
                    }
                },
            )
        });
    let history = state
        .application
        .store()
        .history(run.id)
        .map_err(ApiError::internal)?;
    let mut totals = UsageTotals::default();
    let mut retry_count = 0_u32;
    for record in &history.usage {
        totals.add(record);
        retry_count = retry_count.saturating_add(record.retry_count);
    }
    let current_task = history.task_runs.last();
    let current_node = current_task.map(|task| task.task_id.to_string());
    let current_node_status =
        current_task.map(|task| format!("{:?}", task.status).to_ascii_lowercase());
    let validation_issue_codes = validation_issue_codes(&history.events);
    let fallback_nodes = workflow_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.pointer("/checkpoint/activated_fallbacks"))
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let checkpoint_present = workflow_snapshot
        .as_ref()
        .is_some_and(|snapshot| !snapshot["checkpoint"].is_null());
    let pipeline_artifact_count = workflow_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.pointer("/checkpoint/node_outputs"))
        .and_then(Value::as_object)
        .map(|outputs| {
            outputs
                .values()
                .filter_map(|output| output.get("pipeline_artifacts"))
                .filter_map(Value::as_array)
                .map(Vec::len)
                .sum::<usize>()
        })
        .unwrap_or_default();
    let timed_out = run
        .terminal_reason
        .as_deref()
        .is_some_and(|reason| reason.to_ascii_lowercase().contains("timeout"))
        || history.events.iter().any(|event| match &event.payload {
            annotagent_core::RunEventPayload::ProviderFailure { error_code, .. }
            | annotagent_core::RunEventPayload::TaskFailure { error_code, .. } => {
                error_code.to_ascii_lowercase().contains("timeout")
            }
            _ => false,
        });
    let review_suspended = run.status == RunStatus::AwaitingReview
        || history
            .task_runs
            .iter()
            .any(|task| task.status == annotagent_core::TaskRunStatus::NeedsReview);
    let terminal_reason = if run
        .terminal_reason
        .as_deref()
        .is_some_and(|reason| reason == "run reached a terminal condition")
    {
        history
            .events
            .iter()
            .rev()
            .find_map(|event| match &event.payload {
                annotagent_core::RunEventPayload::ProviderFailure { summary, .. }
                | annotagent_core::RunEventPayload::TaskFailure { summary, .. } => {
                    Some(summary.clone())
                }
                _ => None,
            })
            .or_else(|| {
                (!validation_issue_codes.is_empty()).then(|| {
                    format!(
                        "Run ended with validation issues: {}",
                        validation_issue_codes.join(", ")
                    )
                })
            })
            .or_else(|| {
                matches!(run.status, RunStatus::Failed | RunStatus::Interrupted).then(|| {
                    format!(
                        "Legacy {:?} history has no structured terminal failure; inspect its persisted events",
                        run.status
                    )
                })
            })
    } else {
        run.terminal_reason.clone()
    };
    let controllable = state.application.is_run_controllable(run.id);
    let model_identity = format!("{}/{}", run.provider, run.model);
    Ok(RunSummary {
        id: run.id,
        project_name: run.project_name,
        workflow_name,
        workflow_version,
        skill_versions,
        model_bindings: vec![ModelBinding {
            id: "default-vision".to_owned(),
            provider: run.provider.clone(),
            model: run.model.clone(),
            role: "vision".to_owned(),
            scope: "run_snapshot".to_owned(),
            health_status: if run.status == RunStatus::Failed {
                "degraded".to_owned()
            } else {
                "unknown".to_owned()
            },
            health_detail: terminal_reason.clone(),
            availability_group:
                annotagent_application::ModelAvailabilityGroup::ConfiguredUnavailable,
            capabilities: Vec::new(),
            score_semantics: None,
            model_version: None,
            endpoint: None,
            enabled: None,
            license_summary: None,
            architecture: None,
            checkpoint_sha256: None,
            label_space: Vec::new(),
            cost_per_request: None,
        }],
        provider: run.provider,
        model: run.model,
        status: run.status,
        controllable,
        input_tokens: totals.input_tokens,
        output_tokens: totals.output_tokens,
        cost: totals.cost.to_string(),
        current_node,
        current_node_status,
        artifact_count: history.artifacts.len().max(pipeline_artifact_count),
        validation_issue_codes,
        retry_count,
        fallback_nodes,
        model_identity,
        timed_out,
        checkpoint_present,
        review_suspended,
        terminal_reason,
        created_at: run.created_at,
        updated_at: run.updated_at,
    })
}

fn product_runs(state: &ServerState) -> ApiResult<Vec<RunSummary>> {
    state
        .application
        .list_runs()
        .map_err(ApiError::internal)?
        .into_iter()
        .map(|run| run_summary(state, run))
        .collect()
}

#[derive(Debug, Serialize)]
struct ProjectWorkflow {
    project_id: String,
    project_name: String,
    workflow: WorkflowVersion,
}

async fn list_projects(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let projects = product_projects(&state)?;
    let runs = product_runs(&state)?;
    let models = {
        let settings = state.settings.read().await;
        vec![workspace_model_binding(&settings)]
    };
    let installed_skills = state
        .application
        .layered_skills()
        .catalog()
        .iter()
        .map(|skill| {
            json!({
                "id": skill.id,
                "display_name": skill.display_name,
                "version": skill.version,
            })
        })
        .collect::<Vec<_>>();
    let review_queue = state
        .application
        .store()
        .pending_review_count()
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "projects": projects,
        "runs": runs,
        "models": models,
        "installed_skills": installed_skills,
        "review_queue": review_queue,
    })))
}

async fn list_workflows(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let workflows = product_projects(&state)?
        .into_iter()
        .flat_map(|project| {
            let project_id = project.id;
            let project_name = project.name;
            project
                .available_workflow_versions
                .into_iter()
                .map(move |workflow| ProjectWorkflow {
                    project_id: project_id.clone(),
                    project_name: project_name.clone(),
                    workflow,
                })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({"workflows": workflows})))
}

#[derive(Debug, Deserialize)]
struct WorkflowDraftQuery {
    project_id: Option<String>,
}

async fn list_workflow_drafts(
    State(state): State<ServerState>,
    Query(query): Query<WorkflowDraftQuery>,
) -> ApiResult<Json<Value>> {
    let drafts = state
        .application
        .list_workflow_drafts(query.project_id.as_deref())
        .map_err(ApiError::bad_request)?;
    let mut latest_current_sample_test = None;
    for draft in &drafts {
        if draft.status == annotagent_core::WorkflowDraftStatus::Archived {
            continue;
        }
        let sample_test = state
            .application
            .store()
            .get_workflow_sample_test(&draft.id)
            .map_err(ApiError::internal)?;
        let Some(sample_test) = sample_test else {
            continue;
        };
        let current = sample_test.project_id == draft.project_id
            && (draft.status == annotagent_core::WorkflowDraftStatus::Published
                || sample_test.completed_at >= draft.updated_at);
        if current
            && latest_current_sample_test
                .as_ref()
                .is_none_or(|(_, completed_at)| sample_test.completed_at > *completed_at)
        {
            latest_current_sample_test = Some((draft.id.clone(), sample_test.completed_at));
        }
    }
    Ok(Json(json!({
        "drafts": drafts,
        "latest_current_sample_test_draft_id": latest_current_sample_test
            .map(|(draft_id, _)| draft_id),
    })))
}

#[derive(Debug, Deserialize)]
struct CreateWorkflowDraftRequest {
    project_id: String,
    #[serde(default)]
    from_template: bool,
    template_id: Option<String>,
}

async fn create_workflow_draft(
    State(state): State<ServerState>,
    Json(request): Json<CreateWorkflowDraftRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let settings = state.settings.read().await.clone();
    let draft = state
        .application
        .create_workflow_draft_with_template(
            &request.project_id,
            &settings,
            request.from_template,
            request.template_id.as_deref(),
        )
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::CREATED, Json(json!(draft))))
}

#[derive(Debug, Deserialize)]
struct SuggestWorkflowRequest {
    project_id: String,
    target_task_id: Option<String>,
    target_label: Option<String>,
    agent_model_profile_id: Option<ModelProfileId>,
    /// Optional persisted session/Draft used for a progress-safe retry. Fresh budgets are created;
    /// the editable Draft and its unresolved requirements are retained.
    retry_session_id: Option<uuid::Uuid>,
    base_draft_id: Option<String>,
    #[serde(default = "default_workflow_advisor")]
    advisor: String,
    #[serde(default)]
    constraints: WorkflowConstraints,
    #[serde(default)]
    builder_constraints: PipelineBuilderConstraints,
}

fn default_workflow_advisor() -> String {
    if cfg!(test) {
        "mock".to_owned()
    } else {
        "llm".to_owned()
    }
}

async fn suggest_workflow(
    State(state): State<ServerState>,
    Json(request): Json<SuggestWorkflowRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let settings = state.settings.read().await.clone();
    let mut workflow_constraints = request.constraints.clone();
    if workflow_constraints.preferred_model_id.is_none()
        && settings.default_provider != "mock"
        && request.builder_constraints.allow_external_models
    {
        workflow_constraints.preferred_model_id = Some("default-vision".to_owned());
    }
    if request.target_task_id.is_some() != request.target_label.is_some() {
        return Err(ApiError::bad_request(
            "target_task_id and target_label must be supplied together",
        ));
    }
    let target = request
        .target_task_id
        .as_deref()
        .zip(request.target_label.as_deref());
    let retry_draft_id = if let Some(session_id) = request.retry_session_id {
        let session = state
            .application
            .list_agent_sessions(&request.project_id)
            .map_err(ApiError::bad_request)?
            .into_iter()
            .find(|session| session.id == session_id)
            .ok_or_else(|| {
                ApiError::bad_request("retry session does not belong to this Project")
            })?;
        if session.status == annotagent_core::AgentSessionStatus::Running {
            return Err(ApiError::bad_request(
                "cancel or wait for the active Pipeline Builder before retrying",
            ));
        }
        session.draft_id.or(request.base_draft_id.clone())
    } else {
        request.base_draft_id.clone()
    };
    let (suggestion, agent_report) = match request.advisor.as_str() {
        #[cfg(test)]
        "mock" | "agent" => {
            let report = state
                .application
                .run_workflow_advisor_agent(
                    &request.project_id,
                    &settings,
                    &workflow_constraints,
                    target,
                    request.builder_constraints.clone(),
                    CancellationToken::default(),
                )
                .await
                .map_err(ApiError::bad_request)?;
            let suggestion =
                report.suggestion.clone().ok_or_else(|| {
                    ApiError::bad_request(
                        report.session.stop_reason.clone().unwrap_or_else(|| {
                            "Workflow Advisor stopped without a Draft".to_owned()
                        }),
                    )
                })?;
            (suggestion, Some(report))
        }
        "llm" => {
            let selected_model = state
                .application
                .resolve_pipeline_builder_model(&request.project_id, request.agent_model_profile_id)
                .map_err(ApiError::bad_request)?;
            if selected_model.provider.adapter == ProviderAdapterKind::Mock {
                return Err(ApiError::bad_request(
                    "Scripted Mock is available through advisor=mock; choose an OpenAI-compatible Model Profile for advisor=llm",
                ));
            }
            let credential = resolve_provider_credential(&state, &selected_model.provider)
                    .await?
                    .ok_or_else(|| {
                        ApiError::bad_request(
                            "Provider setup required: configure a credential before starting Pipeline Builder",
                        )
                    })?;
            let provider = OpenAiCompatibleProvider::new_with_api_key(
                selected_model
                    .openai_compatible_config()
                    .map_err(ApiError::bad_request)?,
                Some(credential.expose_secret().to_owned()),
            )
            .map_err(ApiError::bad_request)?;
            let report = state
                .application
                .run_workflow_advisor_with_selected_model_from_draft(
                    &request.project_id,
                    &settings,
                    &selected_model,
                    &provider,
                    &workflow_constraints,
                    target,
                    request.builder_constraints.clone(),
                    retry_draft_id.as_deref(),
                    CancellationToken::default(),
                )
                .await
                .map_err(ApiError::bad_request)?;
            let suggestion =
                report.suggestion.clone().ok_or_else(|| {
                    ApiError::bad_request(
                        report.session.stop_reason.clone().unwrap_or_else(|| {
                            "Pipeline Builder stopped without a Draft".to_owned()
                        }),
                    )
                })?;
            (suggestion, Some(report))
        }
        other => {
            return Err(ApiError::bad_request(format!(
                "unknown Workflow Advisor {other:?}; choose llm"
            )));
        }
    };
    #[cfg(not(test))]
    if workflow_draft_uses_mock(&suggestion.draft) {
        return Err(ApiError::bad_request(
            "Pipeline Builder returned a test-only Mock binding; configure and bind a real Registry Model or Vision Worker",
        ));
    }
    let mut value = serde_json::to_value(suggestion).map_err(ApiError::internal)?;
    if let (Some(report), Some(object)) = (agent_report, value.as_object_mut()) {
        object.insert("agent_session".to_owned(), json!(report.session));
        object.insert("agent_validation".to_owned(), json!(report.validation));
        object.insert("agent_dry_run".to_owned(), json!(report.dry_run));
        object.insert(
            "approval_required".to_owned(),
            json!(report.approval_required),
        );
    }
    Ok((StatusCode::CREATED, Json(value)))
}

#[cfg(not(test))]
fn workflow_draft_uses_mock(draft: &WorkflowDraft) -> bool {
    let is_mock = |value: &str| value.to_ascii_lowercase().starts_with("mock");
    draft
        .nodes
        .iter()
        .filter_map(|node| node.model_binding.as_deref())
        .any(is_mock)
        || draft.label_pipeline.as_ref().is_some_and(|composition| {
            composition
                .shared_stages
                .iter()
                .flat_map(|stage| &stage.steps)
                .chain(
                    composition
                        .label_pipelines
                        .iter()
                        .flat_map(|pipeline| &pipeline.steps),
                )
                .filter_map(|step| step.model_binding.as_ref())
                .any(|binding| is_mock(&binding.model_id))
        })
}

async fn save_workflow_draft(
    State(state): State<ServerState>,
    AxumPath(draft_id): AxumPath<String>,
    Json(mut draft): Json<WorkflowDraft>,
) -> ApiResult<Json<Value>> {
    if draft.id != draft_id {
        return Err(ApiError::bad_request(
            "draft id in the path must match the request body",
        ));
    }
    #[cfg(not(test))]
    if workflow_draft_uses_mock(&draft) {
        return Err(ApiError::bad_request(
            "Mock bindings are test-only; select a real Registry Model or Vision Worker",
        ));
    }
    draft = state
        .application
        .save_workflow_draft(draft)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(draft)))
}

#[derive(Debug, Deserialize)]
struct DiffWorkflowDraftRequest {
    base_draft_id: String,
    proposed_draft_id: String,
}

async fn diff_workflow_drafts(
    State(state): State<ServerState>,
    Json(request): Json<DiffWorkflowDraftRequest>,
) -> ApiResult<Json<Value>> {
    let diff = state
        .application
        .diff_workflow_drafts(&request.base_draft_id, &request.proposed_draft_id)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(diff)))
}

#[derive(Debug, Deserialize)]
struct ApplyWorkflowDraftDiffRequest {
    proposed_draft_id: String,
    selected_change_ids: Vec<String>,
}

async fn apply_workflow_draft_diff(
    State(state): State<ServerState>,
    AxumPath(draft_id): AxumPath<String>,
    Json(request): Json<ApplyWorkflowDraftDiffRequest>,
) -> ApiResult<Json<Value>> {
    let report = state
        .application
        .apply_workflow_draft_diff(
            &draft_id,
            &request.proposed_draft_id,
            &request.selected_change_ids,
        )
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(report)))
}

async fn dry_run_workflow(
    State(state): State<ServerState>,
    AxumPath(draft_id): AxumPath<String>,
    payload: Option<Json<DryRunWorkflowRequest>>,
) -> ApiResult<Json<Value>> {
    let settings = state.settings.read().await.clone();
    let (draft, model_profiles) = state
        .application
        .resolved_workflow_draft_model_profiles(&draft_id)
        .map_err(ApiError::bad_request)?;
    reject_unresolved_registry_model_nodes(&draft)?;
    let (provider_kind, temporary_api_key) =
        resolve_runtime_model_profiles(&state, &model_profiles, workflow_uses_model(&draft))
            .await?;
    let image_indices = payload.map_or_else(Vec::new, |Json(value)| value.image_indices);
    let report = state
        .application
        .dry_run_workflow_samples_with_provider(
            &draft_id,
            &settings,
            &image_indices,
            &provider_kind,
            temporary_api_key.as_deref(),
        )
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(report)))
}

async fn get_workflow_sample_test(
    State(state): State<ServerState>,
    AxumPath(draft_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let draft = state
        .application
        .store()
        .get_workflow_draft(&draft_id)
        .map_err(ApiError::not_found)?;
    let sample_test = state
        .application
        .store()
        .get_workflow_sample_test(&draft_id)
        .map_err(ApiError::internal)?;
    let current = sample_test.as_ref().is_some_and(|record| {
        record.project_id == draft.project_id
            && (draft.status == annotagent_core::WorkflowDraftStatus::Published
                || record.completed_at >= draft.updated_at)
    });
    Ok(Json(json!({
        "sample_test": sample_test,
        "current": current,
    })))
}

#[derive(Debug, Deserialize, Default)]
struct DryRunWorkflowRequest {
    #[serde(default)]
    image_indices: Vec<usize>,
}

async fn publish_workflow(
    State(state): State<ServerState>,
    AxumPath(draft_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let settings = state.settings.read().await.clone();
    let (draft, _) = state
        .application
        .resolved_workflow_draft_model_profiles(&draft_id)
        .map_err(ApiError::bad_request)?;
    #[cfg(not(test))]
    if workflow_draft_uses_mock(&draft) {
        return Err(ApiError::bad_request(
            "Mock bindings are test-only; clone the Draft and bind a real Registry Model or Vision Worker",
        ));
    }
    reject_unresolved_registry_model_nodes(&draft)?;
    let version = state
        .application
        .publish_workflow(&draft_id, &settings)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(version)))
}

async fn archive_workflow_draft(
    State(state): State<ServerState>,
    AxumPath(draft_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let draft = state
        .application
        .archive_workflow_draft(&draft_id)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(draft)))
}

async fn clone_workflow_version(
    State(state): State<ServerState>,
    AxumPath((workflow_id, version)): AxumPath<(String, u32)>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let draft = state
        .application
        .clone_workflow_version(&workflow_id, version)
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::CREATED, Json(json!(draft))))
}

async fn create_geometry_safe_draft(
    State(state): State<ServerState>,
    AxumPath((workflow_id, version)): AxumPath<(String, u32)>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let draft = state
        .application
        .create_geometry_safe_draft(&workflow_id, version)
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::CREATED, Json(json!(draft))))
}

#[derive(Debug, Deserialize)]
struct CompareWorkflowVersionsRequest {
    left_workflow_id: String,
    left_version: u32,
    right_workflow_id: String,
    right_version: u32,
}

async fn compare_workflow_versions(
    State(state): State<ServerState>,
    Json(request): Json<CompareWorkflowVersionsRequest>,
) -> ApiResult<Json<Value>> {
    let comparison = state
        .application
        .compare_workflow_versions(
            &request.left_workflow_id,
            request.left_version,
            &request.right_workflow_id,
            request.right_version,
        )
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(comparison)))
}

async fn list_pipeline_improvements(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let sessions = state
        .application
        .project_pipeline_improvements(&project_id)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"pipeline_improvements": sessions})))
}

async fn create_pipeline_improvement(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<CreatePipelineImprovementRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let settings = state.settings.read().await.clone();
    let session = state
        .application
        .create_pipeline_improvement(&project_id, &settings, &request)
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::CREATED, Json(json!(session))))
}

async fn get_pipeline_improvement(
    State(state): State<ServerState>,
    AxumPath(improvement_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = improvement_id
        .parse::<PipelineImprovementId>()
        .map_err(ApiError::bad_request)?;
    let session = state
        .application
        .pipeline_improvement(id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!(session)))
}

#[derive(Debug, Deserialize, Default)]
struct ComparePipelineImprovementRequest {
    #[serde(default)]
    policy: PipelineImprovementPolicy,
}

async fn compare_pipeline_improvement(
    State(state): State<ServerState>,
    AxumPath(improvement_id): AxumPath<String>,
    payload: Option<Json<ComparePipelineImprovementRequest>>,
) -> ApiResult<Json<Value>> {
    let id = improvement_id
        .parse::<PipelineImprovementId>()
        .map_err(ApiError::bad_request)?;
    let session = state
        .application
        .pipeline_improvement(id)
        .map_err(ApiError::not_found)?;
    let settings = state.settings.read().await.clone();
    let (candidate, profiles) = state
        .application
        .resolved_workflow_draft_model_profiles(&session.candidate_draft_id)
        .map_err(ApiError::bad_request)?;
    reject_unresolved_registry_model_nodes(&candidate)?;
    let (provider_kind, credential) =
        resolve_runtime_model_profiles(&state, &profiles, workflow_uses_model(&candidate)).await?;
    let policy = payload.map_or_else(PipelineImprovementPolicy::default, |Json(value)| {
        value.policy
    });
    let session = state
        .application
        .compare_pipeline_improvement(id, &settings, policy, &provider_kind, credential.as_deref())
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(session)))
}

async fn apply_pipeline_improvement(
    State(state): State<ServerState>,
    AxumPath(improvement_id): AxumPath<String>,
    Json(request): Json<ApplyPipelineImprovementRequest>,
) -> ApiResult<Json<Value>> {
    let id = improvement_id
        .parse::<PipelineImprovementId>()
        .map_err(ApiError::bad_request)?;
    let session = state
        .application
        .apply_pipeline_improvement(id, &request)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(session)))
}

async fn list_models(State(state): State<ServerState>) -> Json<Value> {
    let models = {
        let settings = state.settings.read().await;
        let mut models = vec![workspace_model_binding(&settings)];
        models.extend(settings.detection_workers.iter().map(worker_model_binding));
        models
    };
    Json(json!({"models": models}))
}

async fn test_detection_worker(
    State(state): State<ServerState>,
    AxumPath(model_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let mut worker = configured_detection_worker(&state, &model_id).await?;
    let backend = worker_discovery_backend(&worker)?;
    let checked_at = Utc::now();

    let health = match backend.health().await {
        Ok(health) => health,
        Err(error) => {
            worker.availability = ModelAvailability::Unreachable;
            worker.availability_evidence.health_passed = false;
            worker.availability_evidence.checked_at = Some(checked_at);
            worker.availability_evidence.detail = Some(error.to_string());
            persist_detection_worker(&state, worker).await?;
            return Ok(Json(json!({
                "model_id": model_id,
                "passed": false,
                "failed_stage": "health",
                "availability": "unreachable",
                "error": error.to_string(),
            })));
        }
    };
    let capabilities = match backend.discover_capabilities().await {
        Ok(capabilities) => capabilities,
        Err(error) => {
            worker.availability = ModelAvailability::IncompatibleProtocol;
            worker.availability_evidence.health_passed =
                health.status == VisionModelHealthStatus::Healthy;
            worker.availability_evidence.protocol_compatible = false;
            worker.availability_evidence.checked_at = Some(checked_at);
            worker.availability_evidence.detail = Some(error.to_string());
            persist_detection_worker(&state, worker).await?;
            return Ok(Json(json!({
                "model_id": model_id,
                "passed": false,
                "failed_stage": "capabilities",
                "availability": "incompatible_protocol",
                "health": health,
                "error": error.to_string(),
            })));
        }
    };
    let models = match backend.discover_models().await {
        Ok(models) => models,
        Err(error) => {
            worker.availability = ModelAvailability::InvalidContract;
            worker.availability_evidence.health_passed =
                health.status == VisionModelHealthStatus::Healthy;
            worker.availability_evidence.protocol_compatible = true;
            worker.availability_evidence.contracts_validated = false;
            worker.availability_evidence.checked_at = Some(checked_at);
            worker.availability_evidence.detail = Some(error.to_string());
            persist_detection_worker(&state, worker).await?;
            return Ok(Json(json!({
                "model_id": model_id,
                "passed": false,
                "failed_stage": "models",
                "availability": "invalid_contract",
                "health": health,
                "capabilities": capabilities,
                "error": error.to_string(),
            })));
        }
    };
    let contracts = match backend.discover_contracts().await {
        Ok(contracts) => contracts,
        Err(error) => {
            worker.availability = ModelAvailability::InvalidContract;
            worker.availability_evidence.health_passed =
                health.status == VisionModelHealthStatus::Healthy;
            worker.availability_evidence.protocol_compatible = true;
            worker.availability_evidence.contracts_validated = false;
            worker.availability_evidence.checked_at = Some(checked_at);
            worker.availability_evidence.detail = Some(error.to_string());
            persist_detection_worker(&state, worker).await?;
            return Ok(Json(json!({
                "model_id": model_id,
                "passed": false,
                "failed_stage": "contracts",
                "availability": "invalid_contract",
                "health": health,
                "capabilities": capabilities,
                "models": models,
                "error": error.to_string(),
            })));
        }
    };
    let discovered_manifest = contracts
        .models
        .iter()
        .find(|manifest| manifest.model_id == worker.model_id)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "Worker contracts do not include configured model {:?}",
                worker.model_id
            ))
        })?;
    let weights_ready = match reconcile_discovered_worker_identity(&mut worker, discovered_manifest)
    {
        Ok(weights_ready) => weights_ready,
        Err(error) => {
            worker.availability = ModelAvailability::InvalidContract;
            worker.availability_evidence.health_passed =
                health.status == VisionModelHealthStatus::Healthy;
            worker.availability_evidence.protocol_compatible = true;
            worker.availability_evidence.contracts_validated = false;
            worker.availability_evidence.weights_ready = false;
            worker.availability_evidence.checked_at = Some(checked_at);
            worker.availability_evidence.detail = Some(error.clone());
            persist_detection_worker(&state, worker).await?;
            return Ok(Json(json!({
                "model_id": model_id,
                "passed": false,
                "failed_stage": "model_identity",
                "availability": "invalid_contract",
                "health": health,
                "capabilities": capabilities,
                "models": models,
                "contracts": contracts,
                "error": error,
            })));
        }
    };
    let declared_capabilities_match = worker.expected_capabilities.iter().all(|capability| {
        capabilities.capabilities.contains(capability)
            && models.models.iter().any(|model| {
                model.model_id == worker.model_id && model.capabilities.contains(capability)
            })
    });
    worker.availability_evidence.health_passed = health.status == VisionModelHealthStatus::Healthy;
    worker.availability_evidence.protocol_compatible = declared_capabilities_match;
    worker.availability_evidence.contracts_validated = declared_capabilities_match;
    worker.availability_evidence.weights_ready = weights_ready;
    worker.availability_evidence.checked_at = Some(checked_at);
    worker.availability_evidence.detail = Some(if !weights_ready {
        "Discovery passed, but model weights or immutable identity are incomplete".to_owned()
    } else if !declared_capabilities_match {
        "Worker discovery does not satisfy the configured capability contract".to_owned()
    } else if health.status != VisionModelHealthStatus::Healthy {
        "Worker health is not healthy".to_owned()
    } else if !worker.availability_evidence.sample_conversion_passed {
        "Discovery passed; run a selected-image sample conversion before registration".to_owned()
    } else {
        "Health, protocol, contracts, identity and sample conversion passed".to_owned()
    });
    worker.availability = if !weights_ready {
        ModelAvailability::MissingWeights
    } else if !declared_capabilities_match {
        ModelAvailability::InvalidContract
    } else if health.status != VisionModelHealthStatus::Healthy {
        ModelAvailability::Unreachable
    } else if worker.availability_evidence.available() {
        ModelAvailability::Available
    } else {
        ModelAvailability::Unknown
    };
    let availability = worker.availability;
    let evidence = worker.availability_evidence.clone();
    persist_detection_worker(&state, worker).await?;
    Ok(Json(json!({
        "model_id": model_id,
        "passed": evidence.health_passed && evidence.protocol_compatible && evidence.contracts_validated,
        "availability": availability,
        "health": health,
        "capabilities": capabilities,
        "models": models,
        "contracts": contracts,
        "evidence": evidence,
    })))
}

#[derive(Debug, Deserialize)]
struct VisionWorkerSampleTestRequest {
    project_id: String,
    #[serde(default)]
    image_index: usize,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    box_prompt: Option<[f32; 4]>,
}

async fn sample_test_detection_worker(
    State(state): State<ServerState>,
    AxumPath(model_id): AxumPath<String>,
    Json(request): Json<VisionWorkerSampleTestRequest>,
) -> ApiResult<Json<Value>> {
    let mut worker = configured_detection_worker(&state, &model_id).await?;
    let images = state
        .application
        .list_project_images(&request.project_id)
        .map_err(ApiError::bad_request)?;
    let image_path = images.get(request.image_index).ok_or_else(|| {
        ApiError::bad_request(format!(
            "image_index {} is outside Project {:?}",
            request.image_index, request.project_id
        ))
    })?;
    let frame = load_image(image_path, 40_000_000).map_err(ApiError::bad_request)?;
    let model_image =
        to_model_image("expert-worker-sample", &frame, 1280).map_err(ApiError::bad_request)?;
    let image_id = ImageId::new();
    let run_id = RunId::new();
    let task_id = TaskId::new("expert-worker-sample");
    let node_id = "expert-worker-sample".to_owned();
    let operation = worker
        .expected_capabilities
        .first()
        .copied()
        .ok_or_else(|| ApiError::bad_request("Vision Worker has no configured capability"))?;
    let started = std::time::Instant::now();

    let result = if operation == VisionCapability::PromptedSegmentation {
        let prompt_rect = request.box_prompt.unwrap_or([0.25, 0.25, 0.5, 0.5]);
        let prompt_rect = NormalizedRect::new(
            prompt_rect[0],
            prompt_rect[1],
            prompt_rect[2],
            prompt_rect[3],
        )
        .map_err(ApiError::bad_request)?;
        let source_detections = ArtifactRef {
            artifact_id: format!("sample-detections:{run_id}"),
            source_node: "sample-prompt".to_owned(),
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        };
        let prompts = BoxPromptSetArtifact {
            reference: ArtifactRef {
                artifact_id: format!("sample-box-prompts:{run_id}"),
                source_node: "sample-prompt".to_owned(),
                port: "prompts".to_owned(),
                artifact_type: ArtifactKind::BoxPromptSet,
                item_id: None,
            },
            image_id,
            source_detections: source_detections.clone(),
            prompts: vec![BoxPrompt {
                id: "sample-box-prompt".to_owned(),
                subject: source_detections.item("sample-candidate"),
                bbox: prompt_rect,
                attributes: BTreeMap::new(),
            }],
        };
        let input_image = sample_image_artifact(&node_id, image_id, &frame, image_path);
        let backend = HttpJsonPipelineBackend::new(HttpJsonPipelineBackendConfig {
            id: worker.id.clone(),
            endpoint: format!("{}/v1/infer", worker.base_url.trim_end_matches('/')),
            capability: operation,
            request_timeout: Duration::from_secs(worker.timeout_seconds),
            authorization: worker
                .authorization_header()
                .map_err(ApiError::bad_request)?,
            expected_model_identity: Some(worker.model_id.clone()),
            max_retries: worker.max_retries,
            max_response_bytes: worker.max_response_bytes,
            allow_remote: worker.allow_remote,
        })
        .map_err(ApiError::bad_request)?;
        backend
            .infer_pipeline(
                PipelineInferenceRequest {
                    protocol_version: annotagent_core::PIPELINE_VISION_PROTOCOL_VERSION,
                    request_id: uuid::Uuid::new_v4().to_string(),
                    run_id,
                    image_id,
                    node_id: node_id.clone(),
                    model_id: worker.model_id.clone(),
                    operation,
                    image: Some(model_image.clone()),
                    input_artifacts: vec![
                        PipelineArtifact::Image(input_image),
                        PipelineArtifact::BoxPromptSet(prompts),
                    ],
                    parameters: BTreeMap::new(),
                    timeout_ms: Some(worker.timeout_seconds.saturating_mul(1_000)),
                },
                CancellationToken::new(),
            )
            .await
            .map(|response| {
                let artifacts =
                    serde_json::to_value(&response.artifacts).unwrap_or_else(|_| json!([]));
                let coordinates = response
                    .artifacts
                    .iter()
                    .map(pipeline_artifact_coordinates)
                    .collect::<Vec<_>>();
                (
                    response.artifacts.len(),
                    artifacts,
                    coordinates,
                    response.model_identity,
                    response.request_id,
                    response.timings.total_ms,
                    response.warnings,
                    response.metadata,
                )
            })
    } else {
        let backend = worker_discovery_backend(&worker)?;
        backend
            .infer(
                VisionInferenceRequest {
                    protocol_version: annotagent_core::VISION_WORKER_PROTOCOL_VERSION,
                    request_id: uuid::Uuid::new_v4().to_string(),
                    operation,
                    run_id,
                    image_id,
                    task_id,
                    node_id: node_id.clone(),
                    model_id: worker.model_id.clone(),
                    image: Some(model_image.clone()),
                    input_artifacts: Vec::new(),
                    prompt: request.query.clone(),
                    parameters: BTreeMap::new(),
                    timeout_ms: Some(worker.timeout_seconds.saturating_mul(1_000)),
                    cancellation_requested: false,
                },
                CancellationToken::new(),
            )
            .await
            .map(|response| {
                let coordinates = response
                    .artifacts
                    .iter()
                    .filter_map(|artifact| match &artifact.value {
                        annotagent_core::VisionArtifactValue::BoundingBox { rect } => {
                            Some(json!(rect))
                        }
                        annotagent_core::VisionArtifactValue::InstanceMask { mask }
                        | annotagent_core::VisionArtifactValue::SemanticMask { mask } => {
                            Some(json!({
                                "mask": mask,
                                "tight_bbox": annotagent_core::mask_tight_bbox(mask).ok(),
                            }))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let artifacts =
                    serde_json::to_value(&response.artifacts).unwrap_or_else(|_| json!([]));
                (
                    response.artifacts.len(),
                    artifacts,
                    coordinates,
                    response.model_identity,
                    response.request_id,
                    response.timings.total_ms,
                    response.warnings,
                    response.metadata,
                )
            })
    };

    let elapsed_ms = started.elapsed().as_millis() as u64;
    let checked_at = Utc::now();
    match result {
        Ok((
            artifact_count,
            artifacts,
            coordinates,
            model_identity,
            request_id,
            worker_ms,
            warnings,
            metadata,
        )) if artifact_count > 0 => {
            worker.availability_evidence.sample_conversion_passed = true;
            worker.availability_evidence.checked_at = Some(checked_at);
            let weights_ready = worker
                .expert_manifest()
                .map_err(ApiError::bad_request)?
                .availability_evidence
                .weights_ready;
            worker.availability_evidence.weights_ready = weights_ready;
            worker.availability = if worker.availability_evidence.available() {
                ModelAvailability::Available
            } else if !weights_ready {
                ModelAvailability::MissingWeights
            } else {
                ModelAvailability::Unknown
            };
            worker.availability_evidence.detail = Some(
                if worker.availability_evidence.available() {
                    "Health, protocol, contracts, identity and sample conversion passed".to_owned()
                } else {
                    "Sample conversion passed; complete discovery and model identity before registration"
                    .to_owned()
                },
            );
            let availability = worker.availability;
            let evidence = worker.availability_evidence.clone();
            let manifest = worker.expert_manifest().map_err(ApiError::bad_request)?;
            persist_detection_worker(&state, worker).await?;
            Ok(Json(json!({
                "model_id": model_id,
                "passed": true,
                "availability": availability,
                "evidence": evidence,
                "input": {
                    "project_id": request.project_id,
                    "image_index": request.image_index,
                    "image_url": format!("/api/projects/{}/images/{}/content", request.project_id, request.image_index),
                    "width": frame.metadata.width,
                    "height": frame.metadata.height,
                    "query": request.query,
                    "box_prompt": request.box_prompt.unwrap_or([0.25, 0.25, 0.5, 0.5]),
                },
                "raw_output_summary": {
                    "request_id": request_id,
                    "model_identity": model_identity,
                    "artifact_count": artifact_count,
                    "metadata": metadata,
                },
                "converted_artifacts": artifacts,
                "coordinates": coordinates,
                "score_semantics": manifest.score_semantics,
                "geometry_semantics": manifest.geometry_semantics,
                "duration_ms": worker_ms.unwrap_or(elapsed_ms),
                "warnings": warnings,
            })))
        }
        Ok((_, _, _, _, _, _, warnings, _)) => {
            worker.availability = ModelAvailability::FailedSmokeTest;
            worker.availability_evidence.sample_conversion_passed = false;
            worker.availability_evidence.checked_at = Some(checked_at);
            worker.availability_evidence.detail =
                Some("Sample request returned no convertible Artifact".to_owned());
            let evidence = worker.availability_evidence.clone();
            persist_detection_worker(&state, worker).await?;
            Ok(Json(json!({
                "model_id": model_id,
                "passed": false,
                "availability": "failed_smoke_test",
                "evidence": evidence,
                "duration_ms": elapsed_ms,
                "warnings": warnings,
                "error": "Sample request returned no convertible Artifact",
            })))
        }
        Err(error) => {
            worker.availability = ModelAvailability::FailedSmokeTest;
            worker.availability_evidence.sample_conversion_passed = false;
            worker.availability_evidence.checked_at = Some(checked_at);
            worker.availability_evidence.detail = Some(error.to_string());
            let evidence = worker.availability_evidence.clone();
            persist_detection_worker(&state, worker).await?;
            Ok(Json(json!({
                "model_id": model_id,
                "passed": false,
                "availability": "failed_smoke_test",
                "evidence": evidence,
                "duration_ms": elapsed_ms,
                "error": error.to_string(),
            })))
        }
    }
}

async fn configured_detection_worker(
    state: &ServerState,
    model_id: &str,
) -> ApiResult<DetectionWorkerSettings> {
    state
        .settings
        .read()
        .await
        .detection_workers
        .iter()
        .find(|worker| worker.model_id == model_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("unknown Vision Worker model {model_id:?}")))
}

fn worker_discovery_backend(worker: &DetectionWorkerSettings) -> ApiResult<HttpJsonVisionBackend> {
    HttpJsonVisionBackend::new(HttpJsonVisionBackendConfig {
        id: worker.id.clone(),
        endpoint: format!("{}/v1/infer", worker.base_url.trim_end_matches('/')),
        capabilities: worker.expected_capabilities.clone(),
        request_timeout: Duration::from_secs(worker.timeout_seconds),
        authorization: worker
            .authorization_header()
            .map_err(ApiError::bad_request)?,
        expected_model_identity: Some(worker.model_id.clone()),
        max_retries: worker.max_retries,
        max_response_bytes: worker.max_response_bytes,
        allow_remote: worker.allow_remote,
    })
    .map_err(ApiError::bad_request)
}

fn reconcile_discovered_worker_identity(
    worker: &mut DetectionWorkerSettings,
    manifest: &ExpertModelManifest,
) -> Result<bool, String> {
    let configured_version = worker.version.model_version.trim();
    if configured_version.is_empty() || matches!(configured_version, "unconfigured" | "unversioned")
    {
        worker
            .version
            .model_version
            .clone_from(&manifest.model_version);
    } else if configured_version != manifest.model_version {
        return Err(format!(
            "configured model version {:?} does not match live Worker version {:?}",
            configured_version, manifest.model_version
        ));
    }
    if worker.version.architecture.is_none() {
        worker
            .version
            .architecture
            .clone_from(&manifest.architecture);
    }

    match (
        worker.version.checkpoint_sha256.as_deref(),
        manifest.checkpoint.as_ref(),
    ) {
        (Some(configured), Some(discovered))
            if !configured.eq_ignore_ascii_case(&discovered.sha256) =>
        {
            return Err(
                "configured checkpoint SHA-256 does not match the live Worker checkpoint"
                    .to_owned(),
            );
        }
        (Some(_), None) => {
            return Err(
                "configured checkpoint SHA-256 is not reported by the live Worker".to_owned(),
            );
        }
        (None, Some(discovered)) => {
            worker.version.checkpoint_sha256 = Some(discovered.sha256.clone());
        }
        _ => {}
    }
    if worker.version.training_dataset_version.is_none() {
        worker.version.training_dataset_version = manifest
            .checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.training_dataset_version.clone());
    }

    match (
        worker.license.weight_license.as_deref(),
        manifest.license.weight_license.as_deref(),
    ) {
        (Some(configured), Some(discovered)) if configured != discovered => {
            return Err(
                "configured checkpoint license does not match the live Worker license".to_owned(),
            );
        }
        (Some(_), None) => {
            return Err(
                "configured checkpoint license is not reported by the live Worker".to_owned(),
            );
        }
        (None, Some(discovered)) => {
            worker.license.weight_license = Some(discovered.to_owned());
        }
        _ => {}
    }
    if worker.label_space.is_empty() {
        worker.label_space = manifest.label_space.clone().unwrap_or_default();
    }

    let immutable_identity_ready = !worker.requires_checkpoint_metadata
        || (!matches!(
            worker.version.model_version.trim(),
            "" | "unconfigured" | "unversioned"
        ) && worker
            .version
            .checkpoint_sha256
            .as_deref()
            .is_some_and(|value| value.len() == 64)
            && worker
                .license
                .weight_license
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()));
    Ok(manifest.availability_evidence.weights_ready && immutable_identity_ready)
}

async fn persist_detection_worker(
    state: &ServerState,
    worker: DetectionWorkerSettings,
) -> ApiResult<()> {
    let saved_settings = {
        let mut settings = state.settings.write().await;
        let target = settings
            .detection_workers
            .iter_mut()
            .find(|candidate| candidate.model_id == worker.model_id)
            .ok_or_else(|| ApiError::not_found("Vision Worker disappeared while testing"))?;
        *target = worker;
        validate_settings(&settings).map_err(ApiError::bad_request)?;
        settings.clone()
    };
    let settings_path = state.settings_path.clone();
    tokio::task::spawn_blocking(move || persist_settings(&settings_path, &saved_settings))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)?;
    *state.settings_persisted.write().await = true;
    Ok(())
}

fn sample_image_artifact(
    source_node: &str,
    image_id: ImageId,
    frame: &annotagent_core::ImageFrame,
    image_path: &Path,
) -> ImageArtifact {
    ImageArtifact {
        reference: ArtifactRef {
            artifact_id: format!("sample-image:{image_id}"),
            source_node: source_node.to_owned(),
            port: "image".to_owned(),
            artifact_type: ArtifactKind::Image,
            item_id: None,
        },
        image_id,
        width: frame.metadata.width,
        height: frame.metadata.height,
        mime_type: frame.metadata.mime_type.clone(),
        blob_ref: image_path.display().to_string(),
        parent: None,
        root_region: None,
    }
}

fn pipeline_artifact_coordinates(artifact: &PipelineArtifact) -> Value {
    match artifact {
        PipelineArtifact::DetectionSet(detections) => json!(
            detections
                .detections
                .iter()
                .map(|detection| detection.bbox)
                .collect::<Vec<_>>()
        ),
        PipelineArtifact::MaskSet(masks) => json!(
            masks
                .masks
                .iter()
                .map(|mask| json!({
                    "mask": mask.mask,
                    "tight_bbox": annotagent_core::mask_tight_bbox(&mask.mask).ok(),
                }))
                .collect::<Vec<_>>()
        ),
        PipelineArtifact::CropSet(crops) => {
            json!(crops.crops.iter().map(|crop| crop.rect).collect::<Vec<_>>())
        }
        _ => json!([]),
    }
}

async fn list_run_summaries(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({"runs": product_runs(&state)?})))
}

#[derive(Debug, Deserialize)]
struct CreateProjectRequest {
    id: String,
    yaml: String,
}

async fn create_project(
    State(state): State<ServerState>,
    Json(request): Json<CreateProjectRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let project = state
        .application
        .create_project(&request.id, &request.yaml)
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::CREATED, Json(json!(project))))
}

async fn get_project(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let mut project = state
        .application
        .get_project(&project_id)
        .map_err(ApiError::not_found)?;
    project.model_bindings = registry_model_bindings(&state)?;
    Ok(Json(json!(project)))
}

async fn guidance_context(state: &ServerState, project_id: &str) -> ApiResult<(Settings, bool)> {
    let settings = state.settings.read().await.clone();
    let workspace_model_connected = registry_model_bindings(state)?
        .iter()
        .any(|binding| binding.health_status == "healthy");
    state
        .application
        .get_project(project_id)
        .map_err(ApiError::not_found)?;
    Ok((settings, workspace_model_connected))
}

async fn get_project_guidance(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let (settings, workspace_model_connected) = guidance_context(&state, &project_id).await?;
    let guidance = state
        .application
        .project_guidance(&project_id, &settings, workspace_model_connected)
        .map_err(ApiError::internal)?;
    Ok(Json(json!(guidance)))
}

async fn get_project_readiness(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let (settings, workspace_model_connected) = guidance_context(&state, &project_id).await?;
    let readiness = state
        .application
        .project_guidance(&project_id, &settings, workspace_model_connected)
        .map_err(ApiError::internal)?
        .readiness_summary();
    Ok(Json(json!(readiness)))
}

async fn get_project_summary(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let (settings, workspace_model_connected) = guidance_context(&state, &project_id).await?;
    let mut summary = state
        .application
        .project_workspace_summary(&project_id, &settings, workspace_model_connected)
        .map_err(ApiError::internal)?;
    let binding = workspace_model_binding(&settings);
    summary.project.model_bindings = vec![binding.clone()];
    summary.project.readiness = summary.readiness.readiness;
    for node in &mut summary.project.active_workflow.nodes {
        if node.model_binding.is_some() {
            node.model_binding = Some(binding.id.clone());
        }
    }
    for workflow in &mut summary.project.available_workflow_versions {
        for node in &mut workflow.nodes {
            if node.model_binding.is_some() {
                node.model_binding = Some(binding.id.clone());
            }
        }
    }
    Ok(Json(json!(summary)))
}

#[derive(Debug, Deserialize)]
struct AddProjectLabelRequest {
    task_id: String,
    label: String,
}

#[derive(Debug, Deserialize)]
struct SetProjectSkillsRequest {
    enabled_skills: Vec<EnabledSkillConfig>,
}

#[derive(Debug, Deserialize)]
struct AddProjectTaskRequest {
    display_name: String,
    kind: TaskKind,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    attributes: BTreeMap<String, AttributeDefinition>,
}

async fn add_project_task(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<AddProjectTaskRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let project = state
        .application
        .add_project_task(
            &project_id,
            &request.display_name,
            request.kind,
            request.labels,
            request.attributes,
        )
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::CREATED, Json(json!(project))))
}

async fn add_project_label(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<AddProjectLabelRequest>,
) -> ApiResult<Json<Value>> {
    let project = state
        .application
        .add_project_label(&project_id, &request.task_id, &request.label)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(project)))
}

async fn set_project_skills(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<SetProjectSkillsRequest>,
) -> ApiResult<Json<Value>> {
    let project = state
        .application
        .set_project_enabled_skills(&project_id, request.enabled_skills)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(project)))
}

async fn get_workflow_catalog(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Query(query): Query<WorkflowCatalogQuery>,
) -> ApiResult<Json<Value>> {
    if query.target_task_id.is_some() != query.target_label.is_some() {
        return Err(ApiError::bad_request(
            "target_task_id and target_label must be supplied together",
        ));
    }
    let settings = state.settings.read().await.clone();
    let input = state
        .application
        .workflow_advisor_input_for_label(
            &project_id,
            &settings,
            WorkflowConstraints::default(),
            query.target_task_id.as_deref(),
            query.target_label.as_deref(),
        )
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(input)))
}

#[derive(Debug, Deserialize, Default)]
struct WorkflowCatalogQuery {
    target_task_id: Option<String>,
    target_label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImportRequest {
    source: PathBuf,
}

#[derive(Debug, Deserialize)]
struct AnnotationImportRequest {
    format: String,
    source: PathBuf,
    #[serde(default)]
    label_mapping: BTreeMap<String, String>,
    #[serde(default)]
    dry_run: bool,
}

async fn import_annotations(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<AnnotationImportRequest>,
) -> ApiResult<Json<Value>> {
    let report = state
        .application
        .import_project_annotations(
            &project_id,
            &request.format,
            &request.source,
            request.label_mapping,
            request.dry_run,
        )
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(report)))
}

async fn import_images(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<ImportRequest>,
) -> ApiResult<Json<Value>> {
    let report = state
        .application
        .import_images_with_report(&project_id, &request.source)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(report)))
}

async fn list_images(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let images = state
        .application
        .list_project_images(&project_id)
        .map_err(ApiError::not_found)?;
    let project = state
        .application
        .get_project(&project_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!({
        "images": images.iter().enumerate().map(|(index, path)| json!({
            "index": index,
            "name": path.file_name().unwrap_or_default().to_string_lossy(),
            "path": format!("{}/{}", project.dataset.root.trim_end_matches('/'), path.file_name().unwrap_or_default().to_string_lossy()),
            "size_bytes": path.metadata().map(|metadata| metadata.len()).unwrap_or_default(),
            "url": format!("/api/projects/{project_id}/images/{index}/content"),
        })).collect::<Vec<_>>()
    })))
}

async fn remove_image(
    State(state): State<ServerState>,
    AxumPath((project_id, index)): AxumPath<(String, usize)>,
) -> ApiResult<Json<Value>> {
    let removed = state
        .application
        .remove_project_image(&project_id, index)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({ "removed": removed })))
}

async fn image_content(
    State(state): State<ServerState>,
    AxumPath((project_id, index)): AxumPath<(String, usize)>,
) -> ApiResult<Response> {
    let path = state
        .application
        .list_project_images(&project_id)
        .map_err(ApiError::not_found)?
        .get(index)
        .cloned()
        .ok_or_else(|| ApiError::not_found("image index was not found"))?;
    let content_type = match path.extension().and_then(|value| value.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("png") => "image/png",
        _ => "image/jpeg",
    };
    let bytes = std::fs::read(path).map_err(ApiError::internal)?;
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    Ok(response)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartRunRequest {
    idempotency_key: Option<String>,
    workflow_id: String,
    version: u32,
}

async fn resolve_published_runtime_provider(
    state: &ServerState,
    workflow_id: &str,
    version: u32,
) -> ApiResult<(String, Option<String>)> {
    let workflow = state
        .application
        .store()
        .get_published_workflow_version(workflow_id, version)
        .map_err(ApiError::not_found)?;
    reject_unresolved_registry_model_nodes(&workflow.draft)?;
    resolve_runtime_model_profiles(
        state,
        &workflow.snapshot.model_profiles,
        workflow_uses_model(&workflow.draft),
    )
    .await
}

fn workflow_uses_model(draft: &WorkflowDraft) -> bool {
    draft.nodes.iter().any(|node| {
        node.model_profile_binding.is_some()
            || (matches!(
                node.kind,
                WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
            ) && !node
                .model_binding
                .as_deref()
                .is_some_and(plugin_model_selection))
    })
}

fn plugin_model_selection(value: &str) -> bool {
    value.starts_with("plugin:") || parse_model_instance_selection_id(value).is_some()
}

fn reject_unresolved_registry_model_nodes(draft: &WorkflowDraft) -> ApiResult<()> {
    let unresolved = draft
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind,
                WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
            ) && node.model_profile_binding.is_none()
                && !node
                    .model_binding
                    .as_deref()
                    .is_some_and(plugin_model_selection)
        })
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();
    if unresolved.is_empty() {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "Registry Model Profile required for model nodes: {}; bind each node before Dry Run or publish",
            unresolved.join(", ")
        )))
    }
}

async fn resolve_runtime_model_profiles(
    state: &ServerState,
    model_profiles: &[ModelProfileSnapshot],
    model_required: bool,
) -> ApiResult<(String, Option<String>)> {
    let Some(first) = model_profiles.first() else {
        if model_required {
            return Err(ApiError::bad_request(
                "Workflow has model nodes but no frozen Model Profile; bind a Registry Model Profile, Dry Run, and publish a new Workflow Version",
            ));
        }
        return Ok(("core".to_owned(), None));
    };
    if model_profiles.iter().any(|profile| {
        profile.provider_id != first.provider_id
            || profile.provider_adapter != first.provider_adapter
    }) {
        return Err(ApiError::bad_request(
            "this Runtime currently requires one frozen Provider connection per Published Workflow",
        ));
    }
    let provider = state
        .application
        .store()
        .get_provider_profile(first.provider_id)
        .map_err(ApiError::bad_request)?;
    let provider_kind = match first.provider_adapter {
        ProviderAdapterKind::Mock => {
            #[cfg(test)]
            {
                "mock"
            }
            #[cfg(not(test))]
            {
            return Err(ApiError::bad_request(
                "Published Workflow references a test-only Mock Model Profile; clone it and bind a live Provider Model or Vision Worker",
            ));
            }
        }
        ProviderAdapterKind::OpenAiCompatible => "openai_compatible",
    }
    .to_owned();
    let credential = resolve_provider_credential(state, &provider)
        .await?
        .map(|secret| secret.expose_secret().to_owned());
    Ok((provider_kind, credential))
}

async fn start_run(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    headers: HeaderMap,
    payload: Option<Json<StartRunRequest>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let project_path = state
        .application
        .project_path(&project_id)
        .map_err(ApiError::not_found)?;
    let project = state
        .application
        .get_project(&project_id)
        .map_err(ApiError::internal)?;
    if let Some(batch) = project.active_batch {
        return Err(ApiError::active_batch(&batch));
    }
    if let Some(run) = project.active_run {
        return Err(ApiError::active_run(&ActiveRunExists {
            active_run_id: run.id,
            status: run.status,
        }));
    }
    let settings = state.settings.read().await.clone();
    let request = payload
        .map(|Json(value)| value)
        .ok_or_else(|| ApiError::bad_request(
            "workflow_id and version are required; publish a Registry-backed Workflow Version before starting a Run",
        ))?;
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .or(request.idempotency_key);
    if idempotency_key
        .as_ref()
        .is_some_and(|key| key.is_empty() || key.len() > 200)
    {
        return Err(ApiError::bad_request(
            "idempotency key must contain between 1 and 200 bytes",
        ));
    }
    let selected_workflow = Some((request.workflow_id.as_str(), request.version));
    let (provider, api_key) =
        resolve_published_runtime_provider(&state, &request.workflow_id, request.version).await?;
    let started = state
        .application
        .start_run_path_with_settings_idempotent_workflow(
            &project_path,
            &provider,
            settings,
            api_key,
            idempotency_key.as_deref(),
            selected_workflow,
        )
        .map_err(|error| {
            if let Some(conflict) = error.downcast_ref::<ActiveRunExists>() {
                ApiError::active_run(conflict)
            } else {
                ApiError::bad_request(error)
            }
        })?;
    Ok((StatusCode::ACCEPTED, Json(json!(started))))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartBatchRequest {
    limit: Option<usize>,
    workflow_id: String,
    version: u32,
}

async fn start_batch(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    payload: Option<Json<StartBatchRequest>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let project_path = state
        .application
        .project_path(&project_id)
        .map_err(ApiError::not_found)?;
    let project = state
        .application
        .get_project(&project_id)
        .map_err(ApiError::internal)?;
    if let Some(batch) = project.active_batch {
        return Err(ApiError::active_batch(&batch));
    }
    if let Some(run) = project.active_run {
        return Err(ApiError::active_run(&ActiveRunExists {
            active_run_id: run.id,
            status: run.status,
        }));
    }
    let request = payload
        .map(|Json(value)| value)
        .ok_or_else(|| ApiError::bad_request(
            "workflow_id and version are required; publish a Registry-backed Workflow Version before starting a Batch",
        ))?;
    if request.limit == Some(0) {
        return Err(ApiError::bad_request(
            "batch limit must be greater than zero",
        ));
    }
    let selected_workflow = Some((request.workflow_id.as_str(), request.version));
    let (provider, api_key) =
        resolve_published_runtime_provider(&state, &request.workflow_id, request.version).await?;
    let config_path = state
        .settings_path
        .is_file()
        .then_some(state.settings_path.as_path());
    let batch = DatasetCoordinator::new(state.application.as_ref())
        .create_with_workflow(
            &project_path,
            &provider,
            config_path,
            request.limit,
            selected_workflow,
        )
        .map_err(ApiError::bad_request)?;
    let application = state.application.clone();
    let batch_id = batch.id;
    tokio::spawn(async move {
        let _ignored = DatasetCoordinator::new(application.as_ref())
            .execute(batch_id, api_key)
            .await;
    });
    Ok((StatusCode::ACCEPTED, Json(json!({"batch": batch}))))
}

fn parse_batch_id(value: &str) -> ApiResult<BatchId> {
    value.parse().map_err(ApiError::bad_request)
}

async fn list_batches(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let batches = state
        .application
        .store()
        .list_batches(false)
        .map_err(ApiError::internal)?;
    let batches = batches
        .into_iter()
        .map(|batch| {
            let progress = state
                .application
                .store()
                .batch_progress(batch.id)
                .map_err(ApiError::internal)?;
            let child_run_ids = state
                .application
                .store()
                .list_batch_images(batch.id)
                .map_err(ApiError::internal)?
                .into_iter()
                .filter_map(|image| image.child_run_id)
                .collect::<Vec<_>>();
            let mut summary = serde_json::to_value(batch).map_err(ApiError::internal)?;
            if let Value::Object(fields) = &mut summary {
                fields.insert("progress".to_owned(), json!(progress));
                fields.insert("child_run_ids".to_owned(), json!(child_run_ids));
            }
            Ok(summary)
        })
        .collect::<ApiResult<Vec<_>>>()?;
    Ok(Json(json!({"batches": batches})))
}

async fn get_batch(
    State(state): State<ServerState>,
    AxumPath(batch_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let batch_id = parse_batch_id(&batch_id)?;
    let checkpoint = state
        .application
        .store()
        .batch_checkpoint(batch_id)
        .map_err(ApiError::not_found)?;
    let events = state
        .application
        .store()
        .list_batch_events(batch_id)
        .map_err(ApiError::internal)?;
    let progress = state
        .application
        .store()
        .batch_progress(batch_id)
        .map_err(ApiError::internal)?;
    Ok(Json(
        json!({"checkpoint": checkpoint, "progress": progress, "events": events}),
    ))
}

async fn pause_batch(
    State(state): State<ServerState>,
    AxumPath(batch_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let batch_id = parse_batch_id(&batch_id)?;
    let batch = DatasetCoordinator::new(state.application.as_ref())
        .pause(batch_id)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"batch": batch})))
}

async fn resume_batch(
    State(state): State<ServerState>,
    AxumPath(batch_id): AxumPath<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let batch_id = parse_batch_id(&batch_id)?;
    let batch = state
        .application
        .store()
        .get_batch(batch_id)
        .map_err(ApiError::not_found)?;
    let application = state.application.clone();
    let api_key = state.api_key.read().await.clone();
    tokio::spawn(async move {
        let _ignored = DatasetCoordinator::new(application.as_ref())
            .resume(batch_id, api_key)
            .await;
    });
    Ok((StatusCode::ACCEPTED, Json(json!({"batch": batch}))))
}

async fn cancel_batch(
    State(state): State<ServerState>,
    AxumPath(batch_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let batch_id = parse_batch_id(&batch_id)?;
    let batch = DatasetCoordinator::new(state.application.as_ref())
        .cancel(batch_id)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"batch": batch})))
}

fn parse_run_id(value: &str) -> ApiResult<RunId> {
    value.parse().map_err(ApiError::bad_request)
}

async fn inspect_run_pipeline_artifacts(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    let inspection = state
        .application
        .inspect_run_pipeline_artifacts(run_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!(inspection)))
}

async fn get_run_result_summary(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    let summary = state
        .application
        .run_result_summary(run_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!(summary)))
}

async fn get_run_debug_summary(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    let summary = state
        .application
        .run_debug_summary(run_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!(summary)))
}

async fn replay_run_from_node(
    State(state): State<ServerState>,
    AxumPath((run_id, node_id)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    let settings = state.settings.read().await.clone();
    let replay = state
        .application
        .replay_run_from_node(run_id, &node_id, &settings)
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(replay)))
}

async fn get_run(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    let run = state
        .application
        .list_runs()
        .map_err(ApiError::internal)?
        .into_iter()
        .find(|run| run.id == run_id)
        .ok_or_else(|| ApiError::not_found("run was not found"))?;
    let events = state
        .application
        .list_events(run_id)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"run": run, "event_count": events.len()})))
}

async fn pause_run(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    state
        .application
        .pause_run(run_id)
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"run_id": run_id, "status": "paused"})))
}

async fn resume_run(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    state
        .application
        .resume_run(run_id)
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"run_id": run_id, "status": "running"})))
}

async fn cancel_run(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    state
        .application
        .cancel_run(run_id)
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"run_id": run_id, "status": "cancelled"})))
}

async fn run_events(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let run_id = parse_run_id(&run_id)?;
    let events = state
        .application
        .list_events(run_id)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"events": events})))
}

fn rect_iou(left: NormalizedRect, right: NormalizedRect) -> f32 {
    let intersection = left.intersection_area(right);
    let union = left.area() + right.area() - intersection;
    if union > 0.0 {
        intersection / union
    } else {
        0.0
    }
}

fn review_detection_evidence(
    inspection: Option<&annotagent_application::RunNodeArtifactInspection>,
    annotation: &Annotation,
) -> (
    Vec<DetectionEvidence>,
    Option<CandidateAgreement>,
    Option<Value>,
) {
    let AnnotationValue::BoundingBox { rect } = &annotation.value else {
        return (Vec::new(), None, None);
    };
    let Some(inspection) = inspection else {
        return (Vec::new(), None, None);
    };
    let decision = inspection.nodes.iter().rev().find_map(|node| {
        node.metadata
            .get("evidence_gate")
            .or_else(|| node.metadata.get("recovery_agent"))
            .cloned()
    });
    let target_label = annotation.label.as_ref();
    let mut best_cluster: Option<(f32, Vec<DetectionEvidence>, CandidateAgreement)> = None;
    for set in inspection
        .nodes
        .iter()
        .flat_map(|node| &node.outputs)
        .filter_map(|artifact| match artifact {
            PipelineArtifact::CandidateClusterSet(set) => Some(set),
            _ => None,
        })
    {
        for candidate in &set.candidates {
            if target_label.is_some_and(|label| label != &candidate.target_label) {
                continue;
            }
            let iou = rect_iou(*rect, candidate.representative_bbox);
            if best_cluster
                .as_ref()
                .is_none_or(|(best_iou, _, _)| iou > *best_iou)
            {
                best_cluster = Some((iou, candidate.members.clone(), candidate.agreement.clone()));
            }
        }
    }
    if let Some((_, evidence, agreement)) = best_cluster {
        return (evidence, Some(agreement), decision);
    }
    let mut best_detection: Option<(f32, Vec<DetectionEvidence>)> = None;
    for set in inspection
        .nodes
        .iter()
        .flat_map(|node| &node.outputs)
        .filter_map(|artifact| match artifact {
            PipelineArtifact::DetectionSet(set) => Some(set),
            _ => None,
        })
    {
        for detection in &set.detections {
            if target_label.is_some_and(|label| detection.project_label.as_ref() != Some(label)) {
                continue;
            }
            let iou = rect_iou(*rect, detection.bbox);
            if best_detection
                .as_ref()
                .is_none_or(|(best_iou, _)| iou > *best_iou)
            {
                best_detection = Some((iou, detection.evidence.clone()));
            }
        }
    }
    best_detection.map_or((Vec::new(), None, decision.clone()), |(_, evidence)| {
        (evidence, Some(CandidateAgreement::SingleSource), decision)
    })
}

fn review_explanation(
    annotation: &Annotation,
    issue_codes: &[String],
    issue_details: &[String],
    evidence: &[DetectionEvidence],
    agreement: Option<&CandidateAgreement>,
    evidence_decision: Option<&Value>,
) -> Value {
    let source_models = evidence
        .iter()
        .map(|item| item.source_model_id.as_str())
        .collect::<BTreeSet<_>>();
    let no_score = evidence
        .iter()
        .any(|item| item.score.semantics == ScoreSemantics::NotProvided);
    let decision_text = evidence_decision
        .and_then(|value| value.get("decision"))
        .and_then(Value::as_str);
    let reason_codes = evidence_decision
        .and_then(|value| value.get("reasons"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|reason| reason.get("code").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let domain_validation = reason_codes.contains("domain_issue");

    if domain_validation && !issue_codes.is_empty() {
        return json!({
            "code": "domain_validation_issue",
            "title": "Needs review",
            "summary": "A domain validator found evidence that needs a human decision.",
            "details": issue_details
        });
    }
    if matches!(agreement, Some(CandidateAgreement::GeometryConflict)) {
        let minimum_iou = evidence
            .iter()
            .enumerate()
            .flat_map(|(index, left)| {
                evidence
                    .iter()
                    .skip(index + 1)
                    .map(move |right| rect_iou(left.bbox, right.bbox))
            })
            .reduce(f32::min);
        return json!({
            "code": "geometry_conflict",
            "title": "Needs review",
            "summary": format!("{} disagree on the object's location.", source_models.into_iter().collect::<Vec<_>>().join(" and ")),
            "details": minimum_iou.map_or_else(
                || vec!["Choose one source box or merge the result manually.".to_owned()],
                |iou| vec![format!("Bounding-box IoU: {iou:.2}"), "Choose one source box or merge the result manually.".to_owned()],
            )
        });
    }
    if matches!(agreement, Some(CandidateAgreement::LabelConflict)) {
        return json!({
            "code": "label_conflict",
            "title": "Needs review",
            "summary": "The detectors disagree on the candidate label.",
            "details": ["Inspect each model's original label before accepting."]
        });
    }
    if decision_text == Some("fallback")
        || reason_codes
            .iter()
            .any(|code| code.contains("empty") || code.contains("fallback"))
        || (source_models.len() == 1 && no_score)
    {
        let source = source_models
            .iter()
            .next()
            .copied()
            .unwrap_or("The fallback detector");
        let mut details = vec![
            "The primary detector did not produce evidence that could be accepted.".to_owned(),
            format!("{source} found this candidate as fallback evidence."),
        ];
        if no_score {
            details.push("This model does not provide a confidence score.".to_owned());
        }
        return json!({
            "code": "fallback_evidence",
            "title": "Needs review",
            "summary": "A fallback detector found a candidate after the primary path was uncertain.",
            "details": details
        });
    }
    if no_score {
        return json!({
            "code": "score_not_provided",
            "title": "Needs review",
            "summary": "The detector found a candidate without a comparable confidence score.",
            "details": ["Review the source evidence and geometry before accepting."]
        });
    }
    if annotation.confidence.is_some_and(|value| value < 0.8) {
        return json!({
            "code": "low_confidence",
            "title": "Needs review",
            "summary": "The model confidence is below this Automation's acceptance threshold.",
            "details": [format!("Recorded confidence: {:.0}%", annotation.confidence.unwrap_or_default() * 100.0)]
        });
    }
    if !issue_codes.is_empty() {
        return json!({
            "code": "validation_issue",
            "title": "Needs review",
            "summary": "Validation needs a human decision.",
            "details": issue_details
        });
    }
    json!({
        "code": "review_policy",
        "title": "Needs review",
        "summary": "This Automation routes the result through a Human Review gate.",
        "details": []
    })
}

fn reviews(state: &ServerState, target: Option<AnnotationId>) -> ApiResult<Vec<Value>> {
    let mut reviews = Vec::new();
    let target_annotation = target
        .map(|id| {
            state
                .application
                .store()
                .find_annotation(id)
                .map_err(ApiError::internal)
        })
        .transpose()?
        .flatten();
    if target.is_some() && target_annotation.is_none() {
        return Ok(reviews);
    }
    let projects = state
        .application
        .list_projects()
        .map_err(ApiError::internal)?;
    let project_ids = projects
        .iter()
        .filter_map(|project| {
            let path = state.application.project_path(&project.id).ok()?;
            Some((stable_project_id(path.parent()?), project.id.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let image_indices = projects
        .iter()
        .map(|project| {
            state
                .application
                .project_image_indices_by_sha256(&project.id)
                .map(|indices| (project.id.clone(), indices))
                .map_err(ApiError::internal)
        })
        .collect::<ApiResult<BTreeMap<_, _>>>()?;
    for run in state.application.list_runs().map_err(ApiError::internal)? {
        let annotations = if let Some((target_run_id, annotation)) = target_annotation.as_ref() {
            if *target_run_id != run.id {
                continue;
            }
            if annotation.review_status == ReviewStatus::NeedsReview {
                vec![annotation.clone()]
            } else {
                Vec::new()
            }
        } else {
            state
                .application
                .store()
                .list_annotations(run.id)
                .map_err(ApiError::internal)?
                .into_iter()
                .filter(|annotation| annotation.review_status == ReviewStatus::NeedsReview)
                .collect::<Vec<_>>()
        };
        if annotations.is_empty() {
            continue;
        }
        let project_id = run
            .project_id
            .as_ref()
            .and_then(|id| project_ids.get(id))
            .map(String::as_str);
        let image_sha256 = run
            .workflow_snapshot_json
            .as_deref()
            .and_then(|snapshot| serde_json::from_str::<Value>(snapshot).ok())
            .and_then(|snapshot| {
                snapshot
                    .pointer("/image/sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        let indexed_image = project_id
            .and_then(|project_id| image_indices.get(project_id))
            .and_then(|indices| {
                image_sha256
                    .as_deref()
                    .and_then(|sha256| indices.get(sha256))
            })
            .copied();
        let artifacts = state
            .application
            .store()
            .list_artifacts(run.id)
            .map_err(ApiError::internal)?;
        let inspection = state
            .application
            .inspect_run_pipeline_artifacts_from_history(&run, indexed_image)
            .ok();
        let image_index = indexed_image.or_else(|| {
            inspection
                .as_ref()
                .and_then(|value| value.image_index)
                .or_else(|| {
                    state
                        .application
                        .inspect_run_annotations(run.id)
                        .ok()
                        .and_then(|value| value.image_index)
                })
        });
        let events = state
            .application
            .store()
            .list_events(run.id)
            .map_err(ApiError::internal)?;
        let legacy_validation_issue_codes = validation_issue_codes(&events);
        let persisted_validation_issues = state
            .application
            .store()
            .list_validation_issues(run.id)
            .map_err(ApiError::internal)?;
        let has_persisted_validation_issues = !persisted_validation_issues.is_empty();
        let current_node = state
            .application
            .store()
            .list_task_runs(run.id)
            .map_err(ApiError::internal)?
            .last()
            .map(|task| task.task_id.to_string());
        let fallback_workflow_version =
            serde_json::from_str::<ProjectSchema>(&run.project_schema_json)
                .map_or(0, |schema| schema.version);
        for annotation in annotations {
            let annotation_validation_issues = persisted_validation_issues
                .iter()
                .filter(|issue| {
                    issue.annotation_ids.is_empty() || issue.annotation_ids.contains(&annotation.id)
                })
                .collect::<Vec<_>>();
            let mut validation_issue_codes = if has_persisted_validation_issues {
                annotation_validation_issues
                    .iter()
                    .map(|issue| issue.code.clone())
                    .collect::<Vec<_>>()
            } else {
                legacy_validation_issue_codes.clone()
            };
            validation_issue_codes.sort();
            validation_issue_codes.dedup();
            let mut validation_issue_details = if has_persisted_validation_issues {
                annotation_validation_issues
                    .iter()
                    .map(|issue| issue.message.clone())
                    .collect::<Vec<_>>()
            } else {
                validation_issue_codes.clone()
            };
            validation_issue_details.sort();
            validation_issue_details.dedup();
            let source_artifact_id = annotation.provenance.artifact_ids.first().copied();
            let mut lineage_ids = annotation
                .provenance
                .artifact_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let mut refinement_chain = BTreeSet::new();
            let mut changed = true;
            while changed {
                changed = false;
                for artifact in &artifacts {
                    if !lineage_ids.contains(&artifact.id) {
                        continue;
                    }
                    if let Some(tool) = artifact.provenance.tool.as_deref() {
                        if tool.contains("refiner") || tool.contains("sam") {
                            refinement_chain.insert(tool.to_owned());
                        }
                    }
                    for parent in &artifact.provenance.input_artifact_ids {
                        changed |= lineage_ids.insert(*parent);
                    }
                }
            }
            let pipeline_artifact_ref = annotation
                .attributes
                .get("pipeline_artifact_ref")
                .and_then(|value| match value {
                    annotagent_core::AttributeValue::String(value) => Some(value.as_str()),
                    _ => None,
                });
            if let (Some(inspection), Some(reference)) =
                (inspection.as_ref(), pipeline_artifact_ref)
            {
                for node in &inspection.nodes {
                    if node
                        .outputs
                        .iter()
                        .any(|artifact| artifact.reference().artifact_id == reference)
                    {
                        refinement_chain.extend(node.configuration.refiners.iter().cloned());
                    }
                }
            }
            let source_node = inspection.as_ref().and_then(|inspection| {
                inspection.nodes.iter().find_map(|node| {
                    node.outputs
                        .iter()
                        .any(|artifact| {
                            source_artifact_id.is_some_and(|id| {
                                artifact.reference().artifact_id == id.to_string()
                            }) || pipeline_artifact_ref.is_some_and(|reference| {
                                artifact.reference().artifact_id == reference
                            })
                        })
                        .then_some(node.node_id.as_str())
                })
            });
            let source_skill_id = source_node.and_then(|source_node| {
                inspection.as_ref().and_then(|inspection| {
                    inspection
                        .nodes
                        .iter()
                        .find(|node| node.node_id == source_node)
                        .and_then(|node| node.configuration.required_skills.first())
                        .map(String::as_str)
                })
            });
            let (detection_evidence, candidate_agreement, evidence_decision) =
                review_detection_evidence(inspection.as_ref(), &annotation);
            let explanation = review_explanation(
                &annotation,
                &validation_issue_codes,
                &validation_issue_details,
                &detection_evidence,
                candidate_agreement.as_ref(),
                evidence_decision.as_ref(),
            );
            reviews.push(json!({
                    "id": annotation.id,
                    "run_id": run.id,
                    "project_id": project_id,
                    "project_name": run.project_name,
                    "annotation": annotation,
                    "workflow_id": inspection.as_ref().map(|value| value.workflow_id.as_str()),
                    "workflow_version": inspection.as_ref().map_or_else(
                        || fallback_workflow_version,
                        |value| value.workflow_version,
                    ),
                    "image_index": image_index,
                    "source_node": source_node.or(current_node.as_deref()),
                    "source_skill_id": source_skill_id,
                    "source_artifact_id": source_artifact_id,
                    "refinement_chain": refinement_chain,
                    "review_reason": if annotation.confidence.is_some_and(|value| value < 0.8) { "low_confidence" } else if !validation_issue_codes.is_empty() { "validation_issue" } else { "review_policy" },
                    "confidence": annotation.confidence,
                    "validation_issues": validation_issue_codes.clone(),
                    "detection_evidence": detection_evidence,
                    "candidate_agreement": candidate_agreement,
                    "evidence_decision": evidence_decision,
                    "review_explanation": explanation,
                }));
        }
    }
    reviews.sort_by(|left, right| {
        left.pointer("/annotation/created_at")
            .and_then(Value::as_str)
            .cmp(
                &right
                    .pointer("/annotation/created_at")
                    .and_then(Value::as_str),
            )
            .then_with(|| left["id"].as_str().cmp(&right["id"].as_str()))
    });
    Ok(reviews)
}

#[derive(Debug, Default, Deserialize)]
struct ReviewQueueQuery {
    project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ReviewQueueProgress {
    reviewed_count: usize,
    total_count: usize,
    remaining_count: usize,
    current_position: Option<usize>,
}

fn reviews_in_scope(items: Vec<Value>, project_id: Option<&str>) -> Vec<Value> {
    items
        .into_iter()
        .filter(|item| project_id.is_none_or(|project_id| item["project_id"] == json!(project_id)))
        .collect()
}

fn review_queue_progress(
    state: &ServerState,
    project_id: Option<&str>,
    pending: &[Value],
    current: Option<AnnotationId>,
) -> ApiResult<ReviewQueueProgress> {
    let projects = state
        .application
        .list_projects()
        .map_err(ApiError::internal)?;
    let project_ids = projects
        .iter()
        .filter_map(|project| {
            let path = state.application.project_path(&project.id).ok()?;
            Some((stable_project_id(path.parent()?), project.id.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut reviewed_count = 0;
    for run in state.application.list_runs().map_err(ApiError::internal)? {
        let run_project_id = run
            .project_id
            .as_ref()
            .and_then(|id| project_ids.get(id))
            .map(String::as_str);
        if project_id.is_some_and(|project_id| run_project_id != Some(project_id)) {
            continue;
        }
        reviewed_count += state
            .application
            .store()
            .list_annotations(run.id)
            .map_err(ApiError::internal)?
            .into_iter()
            .filter(|annotation| {
                matches!(
                    annotation.review_status,
                    ReviewStatus::HumanAccepted | ReviewStatus::Rejected
                )
            })
            .count();
    }
    let current_position = current.and_then(|current| {
        pending
            .iter()
            .position(|item| item["id"] == json!(current))
            .map(|position| position + 1)
    });
    Ok(ReviewQueueProgress {
        reviewed_count,
        total_count: reviewed_count + pending.len(),
        remaining_count: pending.len(),
        current_position,
    })
}

fn review_navigation(
    state: &ServerState,
    review_id: AnnotationId,
    project_id: Option<&str>,
) -> ApiResult<Value> {
    let pending = reviews_in_scope(reviews(state, None)?, project_id);
    let current = pending
        .iter()
        .position(|item| item["id"] == json!(review_id))
        .ok_or_else(|| ApiError::not_found("review was not found in this queue"))?;
    Ok(json!({
        "previous_review": current.checked_sub(1).and_then(|index| pending.get(index)),
        "next_review": pending.get(current + 1),
        "progress": review_queue_progress(state, project_id, &pending, Some(review_id))?,
    }))
}

async fn list_reviews(
    State(state): State<ServerState>,
    Query(query): Query<ReviewQueueQuery>,
) -> ApiResult<Json<Value>> {
    let pending = reviews_in_scope(reviews(&state, None)?, query.project_id.as_deref());
    let progress = review_queue_progress(&state, query.project_id.as_deref(), &pending, None)?;
    Ok(Json(json!({"reviews": pending, "progress": progress})))
}

fn parse_annotation_id(value: &str) -> ApiResult<AnnotationId> {
    value.parse().map_err(ApiError::bad_request)
}

async fn get_review(
    State(state): State<ServerState>,
    AxumPath(review_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&review_id)?;
    let item = reviews(&state, Some(id))?
        .into_iter()
        .find(|item| item["id"] == json!(id))
        .ok_or_else(|| ApiError::not_found("review was not found"))?;
    Ok(Json(item))
}

async fn get_next_review(
    State(state): State<ServerState>,
    AxumPath(review_id): AxumPath<String>,
    Query(query): Query<ReviewQueueQuery>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&review_id)?;
    Ok(Json(review_navigation(
        &state,
        id,
        query.project_id.as_deref(),
    )?))
}

#[derive(Debug, Deserialize)]
struct AnnotationPatch {
    annotation: Annotation,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnnotationCreate {
    annotation: Annotation,
}

async fn list_run_annotations(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<RunId>,
) -> ApiResult<Json<Value>> {
    let inspection = state
        .application
        .inspect_run_annotations(run_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!(inspection)))
}

async fn create_annotation(
    State(state): State<ServerState>,
    AxumPath(run_id): AxumPath<RunId>,
    Json(request): Json<AnnotationCreate>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let annotation = state
        .application
        .create_human_annotation(run_id, request.annotation)
        .await
        .map_err(ApiError::bad_request)?;
    Ok((StatusCode::CREATED, Json(json!({"annotation": annotation}))))
}

async fn patch_annotation(
    State(state): State<ServerState>,
    AxumPath(annotation_id): AxumPath<String>,
    Json(request): Json<AnnotationPatch>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&annotation_id)?;
    if request.annotation.id != id {
        return Err(ApiError::bad_request(
            "path and annotation ids do not match",
        ));
    }
    let revision = state
        .application
        .store()
        .update_annotation(&request.annotation, request.reason.as_deref())
        .map_err(ApiError::bad_request)?;
    let geometry_metrics = revision
        .before
        .as_ref()
        .zip(revision.after.as_ref())
        .map_or_else(BTreeMap::new, |(before, after)| {
            annotagent_core::manual_geometry_feature_map(before, after)
        });
    Ok(Json(json!({
        "annotation": request.annotation,
        "revision": revision,
        "geometry_metrics": geometry_metrics,
    })))
}

#[derive(Debug, Clone, Deserialize)]
struct ReviewDecisionRequest {
    decision: String,
    project_id: String,
    queue_project_id: Option<String>,
    skill_id: Option<String>,
    reason_code: String,
    note: Option<String>,
    corrected_label: Option<LabelId>,
}

fn correction_artifact_id(run_id: RunId, annotation_id: AnnotationId, role: &str) -> ArtifactId {
    ArtifactId(uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        format!("annotagent://runs/{run_id}/annotations/{annotation_id}/{role}").as_bytes(),
    ))
}

fn geometry_correction_lineage(
    history: &HistoryRun,
    annotation: &Annotation,
) -> (NodeId, Option<ModelProfileId>, Option<u64>) {
    let snapshot = history
        .workflow_snapshot_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok());
    let workflow = snapshot
        .as_ref()
        .and_then(|value| value.get("selected_workflow").cloned())
        .and_then(|value| serde_json::from_value::<PublishedWorkflowVersion>(value).ok());
    let source_node = annotation
        .provenance
        .tool_names
        .first()
        .cloned()
        .unwrap_or_else(|| "legacy.unresolved".to_owned());
    let Some(workflow) = workflow else {
        return (NodeId::from(source_node), None, None);
    };
    let mut queue = vec![source_node.clone()];
    let mut visited = BTreeSet::new();
    let mut bound_model = None;
    while let Some(node_id) = queue.pop() {
        if !visited.insert(node_id.clone()) {
            continue;
        }
        if let Some(node) = workflow.draft.nodes.iter().find(|node| node.id == node_id)
            && let Some(binding) = node.model_profile_binding.as_ref()
        {
            bound_model = Some((node.id.clone(), binding.model_profile_id));
            break;
        }
        queue.extend(
            workflow
                .draft
                .edges
                .iter()
                .filter(|edge| edge.to_node == node_id)
                .map(|edge| edge.from_node.clone()),
        );
    }
    let bound_model = bound_model.or_else(|| {
        let mut bound_nodes = workflow.draft.nodes.iter().filter_map(|node| {
            node.model_profile_binding
                .as_ref()
                .map(|binding| (node.id.clone(), binding.model_profile_id))
        });
        let only = bound_nodes.next()?;
        bound_nodes.next().is_none().then_some(only)
    });
    let profile = bound_model
        .as_ref()
        .and_then(|(_, model_id)| {
            workflow
                .snapshot
                .model_profiles
                .iter()
                .find(|profile| profile.model_profile_id == *model_id)
        })
        .or_else(|| {
            (workflow.snapshot.model_profiles.len() == 1)
                .then(|| workflow.snapshot.model_profiles.first())
                .flatten()
        });
    (
        NodeId::from(bound_model.map_or(source_node, |(node_id, _)| node_id)),
        profile.map(|profile| profile.model_profile_id),
        profile.map(|profile| profile.revision),
    )
}

fn save_structured_geometry_correction(
    state: &ServerState,
    run_id: RunId,
    project_id: annotagent_core::ProjectId,
    annotation: &Annotation,
    original: &annotagent_core::AnnotationSnapshot,
    corrected: &annotagent_core::AnnotationSnapshot,
    reason_code: &str,
) -> ApiResult<Option<annotagent_core::GeometryQualityReport>> {
    let (
        AnnotationValue::BoundingBox {
            rect: original_rect,
        },
        AnnotationValue::BoundingBox {
            rect: corrected_rect,
        },
    ) = (&original.value, &corrected.value)
    else {
        return Ok(None);
    };
    if original_rect == corrected_rect {
        return Ok(None);
    }
    let history = state
        .application
        .store()
        .list_runs()
        .map_err(ApiError::internal)?
        .into_iter()
        .find(|run| run.id == run_id)
        .ok_or_else(|| ApiError::not_found("source Run was not found"))?;
    let snapshot = history
        .workflow_snapshot_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok());
    let width = snapshot
        .as_ref()
        .and_then(|value| value.pointer("/image/width"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or_default();
    let height = snapshot
        .as_ref()
        .and_then(|value| value.pointer("/image/height"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or_default();
    let (source_node_id, source_model_profile_id, source_model_revision) =
        geometry_correction_lineage(&history, annotation);
    let candidate_artifact_id = annotation
        .provenance
        .artifact_ids
        .first()
        .copied()
        .or_else(|| {
            annotation
                .attributes
                .get("pipeline_artifact_ref")
                .and_then(|value| match value {
                    annotagent_core::AttributeValue::String(value) => value.parse().ok(),
                    _ => None,
                })
        })
        .unwrap_or_else(|| correction_artifact_id(run_id, annotation.id, "candidate"));
    let created_at = Utc::now();
    let (report, evidence) = build_geometry_correction_evidence(GeometryCorrectionInput {
        project_id,
        run_id,
        image_id: annotation.image_id,
        annotation_id: annotation.id,
        source_node_id,
        source_model_profile_id,
        source_model_revision,
        candidate_artifact_id,
        reference_artifact_id: correction_artifact_id(run_id, annotation.id, "human-reference"),
        original_geometry: GeometrySnapshot {
            rect: *original_rect,
            image_width: width,
            image_height: height,
        },
        corrected_geometry: GeometrySnapshot {
            rect: *corrected_rect,
            image_width: width,
            image_height: height,
        },
        reason: GeometryCorrectionReason::from_code(reason_code),
        created_at,
    });
    state
        .application
        .store()
        .save_geometry_correction(&report, &evidence)
        .map_err(ApiError::internal)?;
    Ok(Some(report))
}

async fn review_decision(
    State(state): State<ServerState>,
    AxumPath(review_id): AxumPath<String>,
    Json(request): Json<ReviewDecisionRequest>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&review_id)?;
    apply_review_decision(&state, id, request).await
}

async fn apply_review_decision(
    state: &ServerState,
    id: AnnotationId,
    request: ReviewDecisionRequest,
) -> ApiResult<Json<Value>> {
    let (run_id, mut annotation) = state
        .application
        .store()
        .find_annotation(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("review was not found"))?;
    let original = annotation.snapshot();
    let geometry_revision = state
        .application
        .store()
        .list_revisions(id)
        .map_err(ApiError::internal)?
        .into_iter()
        .rev()
        .find(|revision| {
            revision
                .before
                .as_ref()
                .zip(revision.after.as_ref())
                .and_then(|(before, after)| annotagent_core::manual_geometry_metrics(before, after))
                .is_some()
        });
    let project_path = state
        .application
        .project_path(&request.project_id)
        .map_err(ApiError::bad_request)?;
    let project = ProjectSchema::from_yaml(
        &std::fs::read_to_string(&project_path).map_err(ApiError::bad_request)?,
    )
    .map_err(ApiError::bad_request)?;
    let configured_skills = project.project.enabled_skill_versions();
    let requested_skill_id = request
        .skill_id
        .clone()
        .or_else(|| configured_skills.keys().next().cloned());
    if request
        .skill_id
        .as_ref()
        .is_some_and(|id| !configured_skills.contains_key(id))
    {
        return Err(ApiError::bad_request(
            "Review correction referenced a Skill not enabled by the Project",
        ));
    }
    let common_reason = matches!(
        request.reason_code.as_str(),
        "accepted_as_is"
            | "too_loose"
            | "too_tight"
            | "shifted"
            | "wrong_object"
            | "missed_object"
            | "duplicate"
            | "wrong_label"
            | "other"
            | "manual_edit"
            | "not_target"
            | "wrong_box"
    );
    let skill_registry = state.application.skills();
    let layered_registry = state.application.layered_skills();
    let skill_reason = configured_skills.keys().any(|skill_id| {
        skill_registry
            .get(skill_id)
            .map(|skill| skill.correction_taxonomy())
            .or_else(|_| {
                layered_registry
                    .get(skill_id)
                    .map(|skill| skill.correction_taxonomy())
            })
            .is_ok_and(|taxonomy| {
                taxonomy
                    .into_iter()
                    .any(|kind| kind.code == request.reason_code)
            })
    });
    if !common_reason && !skill_reason {
        return Err(ApiError::bad_request(format!(
            "unknown structured Review reason {:?}",
            request.reason_code
        )));
    }
    let requested_status = match request.decision.as_str() {
        "accept" => ReviewStatus::HumanAccepted,
        "reject" | "delete" => ReviewStatus::Rejected,
        other => return Err(ApiError::bad_request(format!("unknown decision {other:?}"))),
    };
    let already_applied = annotation.review_status == requested_status
        && request
            .corrected_label
            .as_ref()
            .is_none_or(|label| annotation.label.as_ref() == Some(label));
    annotation.review_status = requested_status;
    if let Some(label) = request.corrected_label.clone() {
        annotation.label = Some(label);
    }
    let revision = if already_applied {
        None
    } else {
        Some(
            state
                .application
                .store()
                .update_annotation(&annotation, Some(&request.reason_code))
                .map_err(ApiError::bad_request)?,
        )
    };
    let stable_project = stable_project_id(
        project_path
            .parent()
            .unwrap_or(state.application.workspace()),
    );
    let correction_original = geometry_revision
        .as_ref()
        .and_then(|revision| revision.before.clone())
        .unwrap_or_else(|| original.clone());
    let corrected = annotation.snapshot();
    let correction_id = if already_applied {
        None
    } else if let Some(skill_id) = requested_skill_id {
        let record = CorrectionRecord {
            id: uuid::Uuid::new_v4(),
            project_id: stable_project,
            skill_id,
            task_id: annotation.task_id.clone(),
            predicted_label: original.label.clone(),
            corrected_label: annotation.label.clone(),
            reason_code: request.reason_code.clone(),
            original_annotation: Some(correction_original.clone()),
            corrected_annotation: Some(corrected.clone()),
            note: request.note.clone(),
            image_features: CorrectionFeatures {
                geometry: annotagent_core::manual_geometry_feature_map(
                    &correction_original,
                    &corrected,
                ),
                colors: BTreeMap::new(),
            },
            created_at: Utc::now(),
        };
        state
            .application
            .store()
            .save_correction(&record)
            .map_err(ApiError::internal)?;
        Some(record.id)
    } else {
        None
    };
    let geometry_quality = if already_applied {
        None
    } else {
        save_structured_geometry_correction(
            state,
            run_id,
            stable_project,
            &annotation,
            &correction_original,
            &corrected,
            &request.reason_code,
        )?
    };
    if annotation.review_status == ReviewStatus::HumanAccepted {
        let settings = state.settings.read().await.clone();
        let resumed = state
            .application
            .resume_published_review(run_id, &annotation, &settings)
            .await
            .map_err(ApiError::internal)?;
        if already_applied && !resumed {
            return Ok(Json(json!({
                "annotation": annotation,
                "revision": revision,
                "correction_id": correction_id,
                "geometry_quality": geometry_quality,
            })));
        }
        let artifact_ids = annotation.provenance.artifact_ids.clone();
        for artifact_id in &artifact_ids {
            state
                .application
                .store()
                .set_artifact_validation_state(run_id, *artifact_id, ArtifactValidationState::Valid)
                .await
                .map_err(ApiError::internal)?;
        }
        if !artifact_ids.is_empty() {
            state
                .application
                .store()
                .record_event(
                    &RunEvent::new(
                        run_id,
                        RunEventKind::ArtifactCommitted,
                        RunEventPayload::Artifact {
                            artifact_ids,
                            summary: "human-approved Artifact committed".to_owned(),
                        },
                    )
                    .scoped(Some(annotation.image_id), Some(annotation.task_id.clone())),
                )
                .await
                .map_err(ApiError::internal)?;
        }
        state
            .application
            .store()
            .record_event(
                &RunEvent::new(
                    run_id,
                    RunEventKind::AnnotationCommitted,
                    RunEventPayload::Annotation {
                        annotation_ids: vec![annotation.id],
                        summary: "human accepted the edited annotation".to_owned(),
                    },
                )
                .scoped(Some(annotation.image_id), Some(annotation.task_id.clone())),
            )
            .await
            .map_err(ApiError::internal)?;
        let remaining = state
            .application
            .store()
            .list_annotations(run_id)
            .map_err(ApiError::internal)?
            .into_iter()
            .any(|item| item.review_status == ReviewStatus::NeedsReview);
        if !remaining {
            let previous = state
                .application
                .list_runs()
                .map_err(ApiError::internal)?
                .into_iter()
                .find(|run| run.id == run_id)
                .map_or(RunStatus::CompletedWithReview, |run| run.status);
            state
                .application
                .store()
                .set_run_status(run_id, RunStatus::Completed, Some("human review committed"))
                .await
                .map_err(ApiError::internal)?;
            state
                .application
                .store()
                .record_event(
                    &RunEvent::new(
                        run_id,
                        RunEventKind::RunCompleted,
                        RunEventPayload::State {
                            from: Some(previous),
                            to: RunStatus::Completed,
                            reason: Some("all reviewed annotations committed".to_owned()),
                        },
                    )
                    .scoped(Some(annotation.image_id), None),
                )
                .await
                .map_err(ApiError::internal)?;
        }
    }
    Ok(Json(
        json!({"annotation": annotation, "revision": revision, "correction_id": correction_id, "geometry_quality": geometry_quality}),
    ))
}

async fn review_and_next(
    state: &ServerState,
    review_id: AnnotationId,
    mut request: ReviewDecisionRequest,
    decision: &str,
) -> ApiResult<Json<Value>> {
    let queue_project_id = request.queue_project_id.clone();
    let pending = reviews_in_scope(reviews(state, None)?, queue_project_id.as_deref());
    let current = pending
        .iter()
        .position(|item| item["id"] == json!(review_id))
        .ok_or_else(|| ApiError::not_found("review was not found in this Project queue"))?;
    let candidate_ids = pending
        .iter()
        .cycle()
        .skip(current + 1)
        .take(pending.len().saturating_sub(1))
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    request.decision = decision.to_owned();
    let Json(mut response) = apply_review_decision(state, review_id, request.clone()).await?;
    let remaining = reviews_in_scope(reviews(state, None)?, queue_project_id.as_deref());
    let next_review = candidate_ids.iter().find_map(|candidate_id| {
        remaining
            .iter()
            .find(|item| item["id"] == json!(candidate_id))
    });
    let progress = review_queue_progress(state, queue_project_id.as_deref(), &remaining, None)?;
    if let Some(object) = response.as_object_mut() {
        object.insert("next_review".to_owned(), json!(next_review));
        object.insert("progress".to_owned(), json!(progress));
    }
    Ok(Json(response))
}

async fn accept_review_and_next(
    State(state): State<ServerState>,
    AxumPath(review_id): AxumPath<String>,
    Json(request): Json<ReviewDecisionRequest>,
) -> ApiResult<Json<Value>> {
    review_and_next(&state, parse_annotation_id(&review_id)?, request, "accept").await
}

async fn reject_review_and_next(
    State(state): State<ServerState>,
    AxumPath(review_id): AxumPath<String>,
    Json(request): Json<ReviewDecisionRequest>,
) -> ApiResult<Json<Value>> {
    review_and_next(&state, parse_annotation_id(&review_id)?, request, "reject").await
}

async fn annotation_revisions(
    State(state): State<ServerState>,
    AxumPath(annotation_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&annotation_id)?;
    let revisions = state
        .application
        .store()
        .list_revisions(id)
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"revisions": revisions})))
}

#[derive(Debug, Deserialize)]
struct ExportBody {
    #[serde(default = "default_export_format")]
    format: String,
}

fn default_export_format() -> String {
    "native".to_owned()
}

async fn get_export_readiness(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let readiness = state
        .application
        .export_readiness(&project_id)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(readiness)))
}

async fn export_dataset(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<ExportBody>,
) -> ApiResult<Json<Value>> {
    let result = state
        .application
        .export_project_dataset(&project_id, &request.format)
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(result)))
}

async fn get_settings(State(state): State<ServerState>) -> Json<Value> {
    let settings = state.settings.read().await.clone();
    let mut settings = serde_json::to_value(settings).expect("Settings always serialize");
    if let Some(object) = settings.as_object_mut() {
        let api_key_persisted = *state.api_key_persisted.read().await;
        let api_key_configured = state.api_key.read().await.is_some();
        object.insert(
            "api_key_configured".to_owned(),
            Value::Bool(api_key_configured),
        );
        object.insert(
            "api_key_persisted".to_owned(),
            Value::Bool(api_key_persisted),
        );
        object.insert(
            "settings_persisted".to_owned(),
            Value::Bool(*state.settings_persisted.read().await),
        );
        object.insert(
            "settings_path".to_owned(),
            Value::String(state.settings_path.display().to_string()),
        );
        let credential_source = state.credential_reference.read().await.source;
        object.insert(
            "credential_store".to_owned(),
            Value::String(
                match credential_source {
                    CredentialSource::SystemKeyring => "system_keyring",
                    CredentialSource::EnvironmentVariable => "environment_variable",
                    CredentialSource::WorkspaceFile => "workspace_file",
                    CredentialSource::SessionOnly => "session_only",
                    CredentialSource::LegacyWorkspaceFile => "legacy_workspace_file",
                }
                .to_owned(),
            ),
        );
        if let Some(error) = state.credential_store_error.read().await.clone() {
            object.insert("credential_store_error".to_owned(), Value::String(error));
        }
    }
    Json(settings)
}

async fn put_settings(
    State(state): State<ServerState>,
    Json(mut settings): Json<Value>,
) -> ApiResult<Json<Value>> {
    let object = settings
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("settings must be a JSON object"))?;
    let api_key = object
        .remove("api_key")
        .or_else(|| object.remove("temporary_api_key"))
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|value| !value.trim().is_empty());
    let clear_saved_api_key = object
        .remove("clear_saved_api_key")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    for field in [
        "api_key_configured",
        "api_key_persisted",
        "temporary_api_key_configured",
        "settings_persisted",
        "settings_path",
        "credential_store",
        "credential_store_error",
    ] {
        object.remove(field);
    }
    if clear_saved_api_key && api_key.is_some() {
        return Err(ApiError::bad_request(
            "cannot save and clear the API key in one request",
        ));
    }
    let validated = serde_json::from_value::<Settings>(settings).map_err(ApiError::bad_request)?;
    validate_settings(&validated).map_err(ApiError::bad_request)?;

    let settings_path = state.settings_path.clone();
    let saved_settings = validated.clone();
    tokio::task::spawn_blocking(move || persist_settings(&settings_path, &saved_settings))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)?;
    *state.settings.write().await = validated;
    *state.settings_persisted.write().await = true;

    if clear_saved_api_key || api_key.is_some() {
        let credential_result = if let Some(secret) = api_key.as_ref() {
            let reference = state.default_write_reference.as_ref().clone();
            let scope = SecretScope {
                provider_id: reference.provider_id,
                source: reference.source,
                locator: reference.locator.clone(),
            };
            let value = SecretValue::new(secret.trim()).map_err(ApiError::bad_request)?;
            state.secret_store.put(scope, value).await.map(Some)
        } else {
            let reference = state.credential_reference.read().await.clone();
            state.secret_store.delete(&reference).await.map(|()| None)
        };
        let saved_reference = match credential_result {
            Ok(reference) => reference,
            Err(error) => {
                *state.credential_store_error.write().await = Some(error.to_string());
                return Err(ApiError::internal(error));
            }
        };
        if let Some(reference) = saved_reference {
            let previous = state.credential_reference.read().await.clone();
            if previous != reference
                && matches!(
                    previous.source,
                    CredentialSource::SystemKeyring | CredentialSource::SessionOnly
                )
                && let Err(error) = state.secret_store.delete(&previous).await
            {
                let _ = state.secret_store.delete(&reference).await;
                return Err(ApiError::internal(error));
            }
            *state.credential_reference.write().await = reference;
        } else {
            *state.credential_reference.write().await =
                state.default_write_reference.as_ref().clone();
        }
        *state.api_key.write().await = api_key.clone();
        *state.api_key_persisted.write().await = api_key.is_some();
        *state.credential_store_error.write().await = None;
    }

    Ok(get_settings(State(state)).await)
}

#[derive(Debug, Deserialize)]
struct EventQuery {
    run_id: Option<RunId>,
}

async fn events(
    State(state): State<ServerState>,
    Query(query): Query<EventQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.application.subscribe();
    let stream = stream::unfold(
        (receiver, query.run_id),
        |(mut receiver, run_id)| async move {
            loop {
                match receiver.recv().await {
                    Ok(value) if run_id.is_none_or(|filter| filter == value.run_id) => {
                        let event = Event::default()
                            .event(serde_json::to_value(value.kind).ok()?.as_str()?)
                            .json_data(&value)
                            .ok()?;
                        return Some((Ok(event), (receiver, run_id)));
                    }
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    );
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    )
}

const MAX_PLUGIN_PACKAGE_UPLOAD_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_PLUGIN_WEIGHT_UPLOAD_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_MODEL_BUNDLE_UPLOAD_BYTES: u64 = annotagent_model_bundle::MAX_MODEL_BUNDLE_BYTES;

#[derive(Debug, Deserialize)]
struct ModelCatalogRefreshRequest {
    url: url::Url,
}

#[derive(Debug, Deserialize)]
struct ModelBundleUploadQuery {
    filename: String,
    #[serde(default)]
    license_accepted: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelBundleInstallRequest {
    catalog_id: String,
    bundle_id: String,
    bundle_version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelInstallOperationRequest {
    catalog_id: String,
    bundle_id: String,
    bundle_version: String,
    plugin_id: String,
    plugin_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ModelInstallOperationStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ModelInstallStage {
    ResolvingModel,
    DownloadingBundle,
    VerifyingBundleDigest,
    VerifyingModelFiles,
    CheckingOnnxContract,
    StartingRustPlugin,
    LoadingModel,
    RunningSampleInference,
    RegisteringModelProfile,
    Ready,
}

#[derive(Debug, Clone, Serialize)]
struct ModelInstallOperation {
    id: uuid::Uuid,
    catalog_id: String,
    bundle_id: String,
    bundle_version: String,
    plugin_id: String,
    plugin_version: String,
    status: ModelInstallOperationStatus,
    stage: ModelInstallStage,
    bytes_completed: u64,
    bytes_total: Option<u64>,
    detail: String,
    error: Option<String>,
    suggested_action: Option<String>,
    model_instance_ids: Vec<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ModelBundleLicenseRequest {
    license_digest: String,
}

fn model_bundle_registry_root(state: &ServerState) -> ApiResult<PathBuf> {
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    Ok(registry.data_root().to_owned())
}

async fn receive_model_bundle_upload(
    state: &ServerState,
    filename: &str,
    body: Body,
) -> ApiResult<PathBuf> {
    let filename = safe_upload_filename(filename)?;
    let upload_root = model_bundle_registry_root(state)?.join("model-uploads");
    tokio::fs::create_dir_all(&upload_root)
        .await
        .map_err(ApiError::internal)?;
    let path = upload_root.join(format!("{}-{filename}", uuid::Uuid::new_v4()));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
        .map_err(ApiError::internal)?;
    let mut stream = body.into_data_stream();
    let mut written = 0_u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(ApiError::bad_request)?;
        written = written.saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        if written > MAX_MODEL_BUNDLE_UPLOAD_BYTES {
            drop(file);
            let _ = tokio::fs::remove_file(&path).await;
            return Err(ApiError::bad_request(
                "Model Bundle upload exceeds the safety limit",
            ));
        }
        file.write_all(&chunk).await.map_err(ApiError::internal)?;
    }
    file.flush().await.map_err(ApiError::internal)?;
    if written == 0 {
        drop(file);
        let _ = tokio::fs::remove_file(&path).await;
        return Err(ApiError::bad_request("Model Bundle upload cannot be empty"));
    }
    Ok(path)
}

fn installed_model_bundle_view(
    installed: &annotagent_model_catalog::InstalledModelBundle,
) -> Value {
    json!({
        "manifest": installed.manifest,
        "bundle_sha256": installed.bundle_digest,
        "status": installed.status,
        "source": installed.source,
        "installed_at": installed.installed_at,
        "updated_at": installed.updated_at,
        "verification": installed.verification,
        "enabled": installed.enabled,
    })
}

fn installed_model_instance_view(
    instance: &annotagent_model_catalog::InstalledModelInstance,
) -> Value {
    json!({
        "id": instance.id,
        "plugin_id": instance.plugin_id,
        "plugin_version": instance.plugin_version,
        "plugin_package_sha256": instance.plugin_package_digest,
        "model_id": instance.model_id,
        "model_bundle_id": instance.model_bundle_id,
        "model_bundle_version": instance.model_bundle_version,
        "model_bundle_sha256": instance.model_bundle_digest,
        "model_variant": instance.model_variant,
        "model_file_digests": instance.model_file_digests,
        "execution_provider": instance.execution_provider,
        "capability_contract_sha256": instance.capability_contract_hash,
        "status": instance.status,
        "contract_inspection": instance.contract_inspection,
        "smoke_test_id": instance.smoke_test_id,
        "smoke_test_result": instance.smoke_test_result,
        "model_profile_id": instance.model_profile_id,
        "model_profile_revision": instance.model_profile_revision,
        "created_at": instance.created_at,
        "updated_at": instance.updated_at,
    })
}

fn bind_compatible_installed_plugins(
    state: &ServerState,
    installed: &annotagent_model_catalog::InstalledModelBundle,
) -> ApiResult<Vec<annotagent_model_catalog::InstalledModelInstance>> {
    let plugins = state.application.plugin_registry();
    let plugins = plugins
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?
        .list();
    let mut instances = Vec::new();
    let registry = state.application.model_bundle_registry();
    let mut registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    for requirement in &installed.manifest.compatible_plugins {
        for plugin in plugins.iter().filter(|plugin| {
            requirement.accepts(
                &plugin.manifest.id,
                &plugin.manifest.version,
                &requirement.model_id,
            )
        }) {
            let execution_provider = installed
                .manifest
                .runtime
                .execution_providers
                .iter()
                .find(|provider| {
                    plugin
                        .manifest
                        .models
                        .iter()
                        .find(|model| model.id == requirement.model_id)
                        .is_some_and(|model| model.runtime_requirements.devices.contains(provider))
                })
                .cloned()
                .unwrap_or_else(|| "cpu".to_owned());
            if let Ok(instance) =
                registry.bind_model_instance(annotagent_model_catalog::BindModelInstanceRequest {
                    plugin: &plugin.manifest,
                    plugin_package_digest: plugin.package_digest.clone(),
                    runtime_status: plugin.runtime_status(),
                    bundle_id: &installed.manifest.id,
                    bundle_version: &installed.manifest.version,
                    model_id: &requirement.model_id,
                    target: &annotagent_plugin_host::current_target(),
                    execution_provider: &execution_provider,
                })
            {
                instances.push(instance);
            }
        }
    }
    Ok(instances)
}

async fn list_model_instances(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    Ok(Json(json!({
        "instances": registry.model_instances().iter().map(installed_model_instance_view).collect::<Vec<_>>(),
        "model_profiles": registry.model_profiles(),
    })))
}

async fn get_model_instance(
    State(state): State<ServerState>,
    AxumPath(instance_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    let instance = registry
        .model_instances()
        .into_iter()
        .find(|instance| instance.id.to_string() == instance_id)
        .ok_or_else(|| ApiError::not_found("Model Instance was not found"))?;
    Ok(Json(installed_model_instance_view(&instance)))
}

enum CatalogPluginSetupMatch {
    Compatible,
    Irrelevant,
    Blocked { code: &'static str, message: String },
}

fn catalog_plugin_setup_match(
    plugin: &PluginInstallation,
    entry: &ModelCatalogEntry,
) -> CatalogPluginSetupMatch {
    let relevant = entry
        .compatible_plugins
        .iter()
        .filter(|requirement| requirement.plugin_id == plugin.manifest.id)
        .collect::<Vec<_>>();
    if relevant.is_empty() {
        return CatalogPluginSetupMatch::Irrelevant;
    }
    let version_matches = relevant
        .iter()
        .copied()
        .filter(|requirement| {
            requirement.accepts(
                &plugin.manifest.id,
                &plugin.manifest.version,
                &requirement.model_id,
            )
        })
        .collect::<Vec<_>>();
    if version_matches.is_empty() {
        let required = relevant
            .iter()
            .map(|requirement| requirement.plugin_version.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return CatalogPluginSetupMatch::Blocked {
            code: "plugin_version_incompatible",
            message: format!(
                "Installed Plugin {} does not satisfy the model requirement {required}. Install a compatible immutable Plugin runtime version before installing this Bundle.",
                plugin.manifest.version
            ),
        };
    }
    if matches!(
        plugin.runtime_status(),
        annotagent_plugin_api::PluginRuntimeStatus::NotInstalled
            | annotagent_plugin_api::PluginRuntimeStatus::Disabled
            | annotagent_plugin_api::PluginRuntimeStatus::Crashed
            | annotagent_plugin_api::PluginRuntimeStatus::Incompatible
    ) {
        return CatalogPluginSetupMatch::Blocked {
            code: "plugin_runtime_unavailable",
            message: "The matching Plugin runtime is not enabled and available.".to_owned(),
        };
    }
    let target = annotagent_plugin_host::current_target();
    let mut first_blocker = None;
    for requirement in version_matches {
        let Some(model) = plugin
            .manifest
            .models
            .iter()
            .find(|model| model.id == requirement.model_id)
        else {
            first_blocker.get_or_insert((
                "plugin_model_missing",
                format!(
                    "Plugin {} does not declare the required model {}.",
                    plugin.manifest.version, requirement.model_id
                ),
            ));
            continue;
        };
        let plugin_roles = model
            .required_file_roles
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let bundle_roles = requirement
            .required_file_roles
            .iter()
            .map(ModelFileRole::as_str)
            .collect::<BTreeSet<_>>();
        if plugin_roles != bundle_roles {
            first_blocker.get_or_insert((
                "plugin_file_roles_incompatible",
                "The installed Plugin package uses an older model-file role contract. Install a newer immutable Plugin runtime version.".to_owned(),
            ));
            continue;
        }
        let contract_hash = Sha256Digest::of_bytes(
            &serde_json::to_vec(model).expect("validated Plugin model always serializes"),
        );
        if contract_hash.as_str() != requirement.contract_hash.as_str() {
            first_blocker.get_or_insert((
                "plugin_contract_incompatible",
                "The installed Plugin package has a different model Contract. Install the exact compatible Plugin runtime version.".to_owned(),
            ));
            continue;
        }
        if !entry
            .capabilities
            .iter()
            .all(|capability| model.capabilities.contains(capability))
        {
            first_blocker.get_or_insert((
                "plugin_capability_incompatible",
                "The installed Plugin model does not declare every capability required by this Bundle.".to_owned(),
            ));
            continue;
        }
        let Some(platform) = entry
            .platform_requirements
            .iter()
            .find(|platform| platform.target == target)
        else {
            first_blocker.get_or_insert((
                "platform_incompatible",
                format!("This Bundle does not support the current platform {target}."),
            ));
            continue;
        };
        if !platform.execution_providers.iter().any(|provider| {
            model
                .runtime_requirements
                .devices
                .iter()
                .any(|device| device == provider)
        }) {
            first_blocker.get_or_insert((
                "execution_provider_incompatible",
                "The Plugin and Bundle have no common execution provider.".to_owned(),
            ));
            continue;
        }
        return CatalogPluginSetupMatch::Compatible;
    }
    let (code, message) = first_blocker.unwrap_or((
        "plugin_contract_incompatible",
        "The installed Plugin package cannot bind this Model Bundle.".to_owned(),
    ));
    CatalogPluginSetupMatch::Blocked { code, message }
}

async fn list_plugin_compatible_model_bundles(
    State(state): State<ServerState>,
    AxumPath((plugin_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let plugin_id = PluginId::parse(plugin_id).map_err(ApiError::bad_request)?;
    let version = PluginVersion::parse(&version).map_err(ApiError::bad_request)?;
    let plugins = state.application.plugin_registry();
    let plugin = plugins
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?
        .get(&plugin_id, &version)
        .map_err(ApiError::not_found)?
        .clone();
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    let mut available = Vec::new();
    let mut setup_blockers = Vec::new();
    for catalog in registry.catalogs() {
        for entry in catalog.entries {
            match catalog_plugin_setup_match(&plugin, &entry) {
                CatalogPluginSetupMatch::Compatible => {
                    let mut value =
                        serde_json::to_value(entry).expect("Catalog entries always serialize");
                    value
                        .as_object_mut()
                        .expect("Catalog entries serialize as objects")
                        .insert("catalog_id".to_owned(), json!(catalog.catalog_id));
                    available.push(value);
                }
                CatalogPluginSetupMatch::Blocked { code, message } => {
                    setup_blockers.push(json!({
                        "bundle_id": entry.bundle_id,
                        "bundle_version": entry.bundle_version,
                        "code": code,
                        "message": message,
                    }));
                }
                CatalogPluginSetupMatch::Irrelevant => {}
            }
        }
    }
    let installed = registry
        .list()
        .into_iter()
        .filter(|bundle| {
            bundle
                .manifest
                .compatible_plugins
                .iter()
                .any(|requirement| {
                    matches!(
                        ModelBundleCompatibilityResolver::resolve(
                            Some(&plugin.manifest),
                            plugin.runtime_status(),
                            &bundle.manifest,
                            &requirement.model_id,
                            &annotagent_plugin_host::current_target(),
                            bundle
                                .manifest
                                .runtime
                                .execution_providers
                                .first()
                                .map_or("cpu", String::as_str),
                            true,
                        ),
                        annotagent_model_catalog::ModelBundleCompatibility::Compatible { .. }
                    )
                })
        })
        .map(|bundle| installed_model_bundle_view(&bundle))
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "plugin_runtime_status": plugin.runtime_status(),
        "available": available,
        "installed": installed,
        "setup_blockers": setup_blockers,
    })))
}

async fn list_model_catalogs(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    Ok(Json(json!({ "catalogs": registry.catalogs() })))
}

async fn get_model_catalog(
    State(state): State<ServerState>,
    AxumPath(catalog_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    let catalog = registry
        .catalogs()
        .into_iter()
        .find(|catalog| catalog.catalog_id == catalog_id)
        .ok_or_else(|| ApiError::not_found("Model Catalog was not found"))?;
    Ok(Json(json!(catalog)))
}

async fn refresh_model_catalog(
    State(state): State<ServerState>,
    Json(request): Json<ModelCatalogRefreshRequest>,
) -> ApiResult<Json<Value>> {
    let client = ModelCatalogClient::new().map_err(ApiError::bad_request)?;
    let catalog = client
        .fetch_catalog(&request.url)
        .await
        .map_err(ApiError::bad_request)?;
    state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .save_catalog(catalog.clone())
        .map_err(ApiError::internal)?;
    Ok(Json(json!({ "catalog": catalog })))
}

async fn list_model_bundles(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    Ok(Json(json!({
        "bundles": registry.list().iter().map(installed_model_bundle_view).collect::<Vec<_>>(),
    })))
}

async fn list_available_model_bundles(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    let bundles = registry
        .catalogs()
        .into_iter()
        .flat_map(|catalog| {
            let catalog_id = catalog.catalog_id;
            catalog.entries.into_iter().map(move |entry| {
                let mut value =
                    serde_json::to_value(entry).expect("Catalog entries always serialize");
                value
                    .as_object_mut()
                    .expect("Catalog entries serialize as objects")
                    .insert("catalog_id".to_owned(), json!(catalog_id));
                value
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({ "bundles": bundles })))
}

async fn get_model_bundle(
    State(state): State<ServerState>,
    AxumPath((bundle_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let bundle_id = ModelBundleId::parse(bundle_id).map_err(ApiError::bad_request)?;
    let version = semver::Version::parse(&version).map_err(ApiError::bad_request)?;
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    let installed = registry
        .get(&bundle_id, &version)
        .ok_or_else(|| ApiError::not_found("Model Bundle was not found"))?;
    Ok(Json(installed_model_bundle_view(installed)))
}

async fn inspect_model_bundle_package(
    State(state): State<ServerState>,
    Query(query): Query<ModelBundleUploadQuery>,
    body: Body,
) -> ApiResult<Json<Value>> {
    if !query.filename.ends_with(".annotmodel") {
        return Err(ApiError::bad_request(
            "Model Bundles must use the .annotmodel extension",
        ));
    }
    let upload = receive_model_bundle_upload(&state, &query.filename, body).await?;
    let path = upload.clone();
    let verified = tokio::task::spawn_blocking(move || verify_model_bundle(&path))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::bad_request);
    let _ = tokio::fs::remove_file(upload).await;
    let verified = verified?;
    Ok(Json(json!({
        "manifest": verified.manifest,
        "bundle_sha256": verified.bundle_digest,
        "signature": verified.signature,
        "file_count": verified.files.len(),
        "verified": true,
        "installed": false,
    })))
}

async fn import_model_bundle(
    State(state): State<ServerState>,
    Query(query): Query<ModelBundleUploadQuery>,
    body: Body,
) -> ApiResult<(StatusCode, Json<Value>)> {
    if !query.filename.ends_with(".annotmodel") {
        return Err(ApiError::bad_request(
            "Model Bundles must use the .annotmodel extension",
        ));
    }
    let upload = receive_model_bundle_upload(&state, &query.filename, body).await?;
    let path = upload.clone();
    let verified = tokio::task::spawn_blocking(move || verify_model_bundle(&path))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::bad_request);
    let verified = match verified {
        Ok(verified) => verified,
        Err(error) => {
            let _ = tokio::fs::remove_file(upload).await;
            return Err(error);
        }
    };
    let installed = {
        let registry = state.application.model_bundle_registry();
        let mut registry = registry
            .lock()
            .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
        if query.license_accepted {
            registry
                .accept_license(ModelLicenseAcceptance {
                    bundle_id: verified.manifest.id.clone(),
                    bundle_version: verified.manifest.version.clone(),
                    license_digest: verified.manifest.license.license_digest.clone(),
                    accepted_at: Utc::now(),
                    accepted_by: LicenseAcceptanceActor::LocalUser,
                })
                .map_err(ApiError::internal)?;
        }
        registry
            .install_verified(verified, ModelBundleInstallSource::LocalImport)
            .map_err(ApiError::bad_request)?
    };
    let _ = tokio::fs::remove_file(upload).await;
    let instances = bind_compatible_installed_plugins(&state, &installed)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "bundle": installed_model_bundle_view(&installed),
            "model_instances": instances.iter().map(installed_model_instance_view).collect::<Vec<_>>(),
        })),
    ))
}

async fn accept_model_bundle_license(
    State(state): State<ServerState>,
    AxumPath((bundle_id, version)): AxumPath<(String, String)>,
    Json(request): Json<ModelBundleLicenseRequest>,
) -> ApiResult<StatusCode> {
    let bundle_id = ModelBundleId::parse(bundle_id).map_err(ApiError::bad_request)?;
    let bundle_version = semver::Version::parse(&version).map_err(ApiError::bad_request)?;
    let license_digest =
        ModelBundleSha256Digest::parse(request.license_digest).map_err(ApiError::bad_request)?;
    state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .accept_license(ModelLicenseAcceptance {
            bundle_id,
            bundle_version,
            license_digest,
            accepted_at: Utc::now(),
            accepted_by: LicenseAcceptanceActor::LocalUser,
        })
        .map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_model_install_operations(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let operations = state.model_install_operations.read().await;
    let mut operations = operations.values().cloned().collect::<Vec<_>>();
    operations.sort_by_key(|operation| std::cmp::Reverse(operation.updated_at));
    Ok(Json(json!({ "operations": operations })))
}

async fn get_model_install_operation(
    State(state): State<ServerState>,
    AxumPath(operation_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let operation_id = operation_id
        .parse::<uuid::Uuid>()
        .map_err(|_| ApiError::bad_request("invalid model installation operation id"))?;
    let operations = state.model_install_operations.read().await;
    let operation = operations
        .get(&operation_id)
        .ok_or_else(|| ApiError::not_found("Model installation operation was not found"))?;
    Ok(Json(json!(operation)))
}

async fn start_model_install_operation(
    State(state): State<ServerState>,
    Json(request): Json<ModelInstallOperationRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let bundle_id = ModelBundleId::parse(&request.bundle_id).map_err(ApiError::bad_request)?;
    let bundle_version =
        semver::Version::parse(&request.bundle_version).map_err(ApiError::bad_request)?;
    let plugin_id = PluginId::parse(&request.plugin_id).map_err(ApiError::bad_request)?;
    let plugin_version =
        PluginVersion::parse(&request.plugin_version).map_err(ApiError::bad_request)?;

    let plugin = state
        .application
        .plugin_registry()
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?
        .get(&plugin_id, &plugin_version)
        .map_err(ApiError::bad_request)?
        .clone();
    let entry = state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .catalogs()
        .into_iter()
        .find(|catalog| catalog.catalog_id == request.catalog_id)
        .and_then(|catalog| {
            catalog.entries.into_iter().find(|entry| {
                entry.bundle_id == bundle_id && entry.bundle_version == bundle_version
            })
        })
        .ok_or_else(|| ApiError::not_found("Catalog Model Bundle was not found"))?;
    if !matches!(
        catalog_plugin_setup_match(&plugin, &entry),
        CatalogPluginSetupMatch::Compatible
    ) {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({
                "code": "model_plugin_incompatible",
                "error": "The selected Model Bundle is not compatible with this immutable Plugin version.",
                "suggested_action": "Install the compatible Plugin package version shown by the Catalog, then retry."
            }),
        });
    }

    let now = Utc::now();
    let operation = ModelInstallOperation {
        id: uuid::Uuid::new_v4(),
        catalog_id: request.catalog_id.clone(),
        bundle_id: request.bundle_id.clone(),
        bundle_version: request.bundle_version.clone(),
        plugin_id: request.plugin_id.clone(),
        plugin_version: request.plugin_version.clone(),
        status: ModelInstallOperationStatus::Running,
        stage: ModelInstallStage::ResolvingModel,
        bytes_completed: 0,
        bytes_total: Some(entry.bundle_size_bytes),
        detail: "Resolving the exact Catalog entry and immutable Plugin requirement".to_owned(),
        error: None,
        suggested_action: None,
        model_instance_ids: Vec::new(),
        created_at: now,
        updated_at: now,
    };
    {
        let mut operations = state.model_install_operations.write().await;
        if operations.values().any(|existing| {
            existing.status == ModelInstallOperationStatus::Running
                && existing.bundle_id == request.bundle_id
                && existing.bundle_version == request.bundle_version
                && existing.plugin_id == request.plugin_id
                && existing.plugin_version == request.plugin_version
        }) {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                body: json!({
                    "code": "model_installation_active",
                    "error": "This exact model installation is already running.",
                    "suggested_action": "Open the existing installation progress instead of starting a duplicate."
                }),
            });
        }
        operations.insert(operation.id, operation.clone());
        while operations.len() > 32 {
            let removable = operations
                .iter()
                .filter(|(_, value)| value.status != ModelInstallOperationStatus::Running)
                .min_by_key(|(_, value)| value.updated_at)
                .map(|(id, _)| *id);
            if let Some(id) = removable {
                operations.remove(&id);
            } else {
                break;
            }
        }
    }
    let operation_id = operation.id;
    tokio::spawn(async move {
        if let Err((error, suggested_action)) =
            run_model_install_operation(&state, operation_id, request).await
        {
            finish_model_install_operation_failure(&state, operation_id, error, suggested_action)
                .await;
        }
    });
    Ok((StatusCode::ACCEPTED, Json(json!(operation))))
}

async fn update_model_install_operation(
    state: &ServerState,
    operation_id: uuid::Uuid,
    installation_stage: ModelInstallStage,
    detail: impl Into<String>,
    bytes_completed: Option<u64>,
    bytes_total: Option<u64>,
) {
    if let Some(operation) = state
        .model_install_operations
        .write()
        .await
        .get_mut(&operation_id)
    {
        operation.stage = installation_stage;
        operation.detail = detail.into();
        if let Some(bytes) = bytes_completed {
            operation.bytes_completed = bytes;
        }
        if bytes_total.is_some() {
            operation.bytes_total = bytes_total;
        }
        operation.updated_at = Utc::now();
    }
}

async fn finish_model_install_operation_failure(
    state: &ServerState,
    operation_id: uuid::Uuid,
    error: String,
    suggested_action: String,
) {
    if let Some(operation) = state
        .model_install_operations
        .write()
        .await
        .get_mut(&operation_id)
    {
        operation.status = ModelInstallOperationStatus::Failed;
        operation.error = Some(error);
        operation.suggested_action = Some(suggested_action);
        operation.updated_at = Utc::now();
    }
}

fn api_error_message(error: &ApiError) -> String {
    error
        .body
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("model setup failed")
        .to_owned()
}

async fn run_model_install_operation(
    state: &ServerState,
    operation_id: uuid::Uuid,
    request: ModelInstallOperationRequest,
) -> Result<(), (String, String)> {
    let bundle_id = ModelBundleId::parse(&request.bundle_id).map_err(|error| {
        (
            error.to_string(),
            "Choose the Catalog entry again and retry.".to_owned(),
        )
    })?;
    let bundle_version = semver::Version::parse(&request.bundle_version).map_err(|error| {
        (
            error.to_string(),
            "Choose the Catalog entry again and retry.".to_owned(),
        )
    })?;
    update_model_install_operation(
        state,
        operation_id,
        ModelInstallStage::ResolvingModel,
        "Resolved the pinned Catalog identity and download policy",
        Some(0),
        None,
    )
    .await;
    let (entry, local_bundle) = {
        let registry = state.application.model_bundle_registry();
        let registry = registry.lock().map_err(|_| {
            (
                "Model Bundle Registry lock is poisoned".to_owned(),
                "Restart AnnotAgent and retry the installation.".to_owned(),
            )
        })?;
        let entry = registry
            .catalogs()
            .into_iter()
            .find(|catalog| catalog.catalog_id == request.catalog_id)
            .and_then(|catalog| {
                catalog.entries.into_iter().find(|entry| {
                    entry.bundle_id == bundle_id && entry.bundle_version == bundle_version
                })
            })
            .ok_or_else(|| {
                (
                    "Catalog Model Bundle was not found".to_owned(),
                    "Refresh the configured Catalog, then choose the model again.".to_owned(),
                )
            })?;
        let local_bundle = registry.local_catalog_bundle_path(
            &request.catalog_id,
            &entry.bundle_id,
            &entry.bundle_version,
        );
        (entry, local_bundle)
    };
    update_model_install_operation(
        state,
        operation_id,
        ModelInstallStage::DownloadingBundle,
        if local_bundle.is_some() {
            "Copying the pinned Bundle from the trusted local Catalog"
        } else {
            "Downloading the pinned Bundle from its audited HTTPS source"
        },
        Some(0),
        Some(entry.bundle_size_bytes),
    )
    .await;
    let upload_root = model_bundle_registry_root(state)
        .map_err(|error| {
            (
                api_error_message(&error),
                "Restart AnnotAgent and retry the installation.".to_owned(),
            )
        })?
        .join("model-downloads");
    tokio::fs::create_dir_all(&upload_root)
        .await
        .map_err(|error| {
            (
                error.to_string(),
                "Check that the workspace is writable and has enough free disk space.".to_owned(),
            )
        })?;
    let download = upload_root.join(format!("{}.annotmodel", uuid::Uuid::new_v4()));
    let download_result = if let Some(source) = local_bundle {
        tokio::fs::copy(source, &download)
            .await
            .map(|_| ())
            .map_err(annotagent_model_catalog::ModelCatalogError::Io)
    } else {
        let client = ModelCatalogClient::new().map_err(|error| {
            (
                error.to_string(),
                "Check network access to the audited Catalog source and retry.".to_owned(),
            )
        })?;
        let (sender, mut receiver) =
            tokio::sync::mpsc::unbounded_channel::<annotagent_model_catalog::ProvisionProgress>();
        let progress_state = state.clone();
        let progress_task = tokio::spawn(async move {
            while let Some(progress) = receiver.recv().await {
                let installation_stage = match progress.stage {
                    annotagent_model_catalog::ProvisionStage::Resolving => {
                        ModelInstallStage::ResolvingModel
                    }
                    annotagent_model_catalog::ProvisionStage::Downloading => {
                        ModelInstallStage::DownloadingBundle
                    }
                    annotagent_model_catalog::ProvisionStage::Verifying
                    | annotagent_model_catalog::ProvisionStage::Installing
                    | annotagent_model_catalog::ProvisionStage::Complete => {
                        ModelInstallStage::VerifyingBundleDigest
                    }
                };
                update_model_install_operation(
                    &progress_state,
                    operation_id,
                    installation_stage,
                    progress.detail,
                    Some(progress.bytes_completed),
                    progress.bytes_total,
                )
                .await;
            }
        });
        let result = client
            .download_bundle(&entry, &download, &CancellationToken::new(), Some(&sender))
            .await;
        drop(sender);
        let _ = progress_task.await;
        result
    };
    if let Err(error) = download_result {
        let _ = tokio::fs::remove_file(&download).await;
        return Err((
            error.to_string(),
            "Check network access, free disk space, and the exact Catalog digest, then retry. A partial download was removed.".to_owned(),
        ));
    }
    update_model_install_operation(
        state,
        operation_id,
        ModelInstallStage::VerifyingBundleDigest,
        "Checking the complete Bundle against the Catalog SHA-256 identity",
        Some(entry.bundle_size_bytes),
        Some(entry.bundle_size_bytes),
    )
    .await;
    update_model_install_operation(
        state,
        operation_id,
        ModelInstallStage::VerifyingModelFiles,
        "Verifying the Manifest, every model file, Contract, license, and test vector",
        None,
        None,
    )
    .await;
    let path = download.clone();
    let verified = tokio::task::spawn_blocking(move || verify_model_bundle(&path))
        .await
        .map_err(|error| {
            (
                error.to_string(),
                "Retry the installation. If it repeats, refresh the Catalog or choose a newer Bundle version.".to_owned(),
            )
        })?
        .map_err(|error| {
            (
                error.to_string(),
                "Refresh the Catalog and download the exact verified Bundle again.".to_owned(),
            )
        });
    let verified = match verified {
        Ok(verified) => verified,
        Err(error) => {
            let _ = tokio::fs::remove_file(&download).await;
            return Err(error);
        }
    };
    if verified.manifest.id != entry.bundle_id
        || verified.manifest.version != entry.bundle_version
        || verified.bundle_digest != entry.bundle_sha256
        || verified.manifest.license.license_digest != entry.license_summary.license_digest
    {
        let _ = tokio::fs::remove_file(&download).await;
        return Err((
            "Downloaded Bundle identity or license does not match the curated Catalog".to_owned(),
            "Refresh the Catalog and retry. Do not import or rename the mismatched file."
                .to_owned(),
        ));
    }
    let installed = state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| {
            (
                "Model Bundle Registry lock is poisoned".to_owned(),
                "Restart AnnotAgent and retry the installation.".to_owned(),
            )
        })?
        .install_verified(
            verified,
            ModelBundleInstallSource::CuratedCatalog {
                catalog_id: request.catalog_id,
            },
        )
        .map_err(|error| {
            (
                error.to_string(),
                "Accept the exact license if requested, verify workspace permissions, then retry."
                    .to_owned(),
            )
        })?;
    let _ = tokio::fs::remove_file(download).await;
    update_model_install_operation(
        state,
        operation_id,
        ModelInstallStage::CheckingOnnxContract,
        "Inspecting real ONNX graph inputs, outputs, dtypes, shapes, and graph connections",
        None,
        None,
    )
    .await;
    let instances = bind_compatible_installed_plugins(state, &installed).map_err(|error| {
        (
            api_error_message(&error),
            "Install the immutable compatible Plugin runtime and retry from the installed Bundle."
                .to_owned(),
        )
    })?;
    let selected = instances
        .into_iter()
        .filter(|instance| {
            instance.plugin_id.as_str() == request.plugin_id
                && instance.plugin_version.to_string() == request.plugin_version
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err((
            "The Bundle is valid, but no Model Instance matched the selected Plugin version and Contract".to_owned(),
            "Install the compatible immutable Plugin package shown by the Catalog, then retry the Smoke Test.".to_owned(),
        ));
    }
    {
        let mut operations = state.model_install_operations.write().await;
        if let Some(operation) = operations.get_mut(&operation_id) {
            operation.model_instance_ids = selected
                .iter()
                .map(|instance| instance.id.to_string())
                .collect();
            operation.updated_at = Utc::now();
        }
    }
    update_model_install_operation(
        state,
        operation_id,
        ModelInstallStage::StartingRustPlugin,
        "Starting the exact installed Rust Plugin package in its constrained process",
        None,
        None,
    )
    .await;
    update_model_install_operation(
        state,
        operation_id,
        ModelInstallStage::LoadingModel,
        "Loading the verified encoder and decoder into the Rust ONNX Runtime",
        None,
        None,
    )
    .await;
    update_model_install_operation(
        state,
        operation_id,
        ModelInstallStage::RunningSampleInference,
        "Running the Bundle's real image and bbox prompt through encoder, decoder, and mask validation",
        None,
        None,
    )
    .await;
    let mut tested = Vec::new();
    for instance in selected {
        tested.push(
            execute_model_instance_test(state, instance.id)
                .await
                .map_err(|error| {
                    (
                        api_error_message(&error),
                        "Open Model Setup to inspect the failed Smoke Test check, then retry. The verified Bundle remains installed.".to_owned(),
                    )
                })?,
        );
    }
    if tested
        .iter()
        .any(|instance| instance.status != annotagent_model_bundle::ModelInstanceStatus::Ready)
    {
        let failure = tested
            .iter()
            .flat_map(|instance| {
                instance
                    .smoke_test_result
                    .iter()
                    .flat_map(|result| result.checks.iter())
            })
            .filter(|check| !check.passed)
            .map(|check| check.detail.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        return Err((
            if failure.is_empty() {
                "The real sample inference did not produce a Ready Model Instance".to_owned()
            } else {
                failure
            },
            "Inspect the failed Smoke Test evidence and retry after correcting the Plugin, Bundle, or platform issue.".to_owned(),
        ));
    }
    update_model_install_operation(
        state,
        operation_id,
        ModelInstallStage::RegisteringModelProfile,
        "Persisting the immutable Model Instance and selectable Model Profile revision",
        None,
        None,
    )
    .await;
    if let Some(operation) = state
        .model_install_operations
        .write()
        .await
        .get_mut(&operation_id)
    {
        operation.status = ModelInstallOperationStatus::Succeeded;
        operation.stage = ModelInstallStage::Ready;
        "Real sample inference passed; the Model Profile is Ready for Workflow Drafts"
            .clone_into(&mut operation.detail);
        operation.error = None;
        operation.suggested_action = None;
        operation.updated_at = Utc::now();
    }
    Ok(())
}

async fn install_model_bundle(
    State(state): State<ServerState>,
    Json(request): Json<ModelBundleInstallRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let bundle_id = ModelBundleId::parse(&request.bundle_id).map_err(ApiError::bad_request)?;
    let bundle_version =
        semver::Version::parse(&request.bundle_version).map_err(ApiError::bad_request)?;
    let (entry, local_bundle) = {
        let registry = state.application.model_bundle_registry();
        let registry = registry
            .lock()
            .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
        let entry = registry
            .catalogs()
            .into_iter()
            .find(|catalog| catalog.catalog_id == request.catalog_id)
            .and_then(|catalog| {
                catalog.entries.into_iter().find(|entry| {
                    entry.bundle_id == bundle_id && entry.bundle_version == bundle_version
                })
            })
            .ok_or_else(|| ApiError::not_found("Catalog Model Bundle was not found"))?;
        let local_bundle = registry.local_catalog_bundle_path(
            &request.catalog_id,
            &entry.bundle_id,
            &entry.bundle_version,
        );
        (entry, local_bundle)
    };
    let upload_root = model_bundle_registry_root(&state)?.join("model-downloads");
    tokio::fs::create_dir_all(&upload_root)
        .await
        .map_err(ApiError::internal)?;
    let download = upload_root.join(format!("{}.annotmodel", uuid::Uuid::new_v4()));
    if let Some(source) = local_bundle {
        tokio::fs::copy(source, &download)
            .await
            .map_err(ApiError::internal)?;
    } else {
        let client = ModelCatalogClient::new().map_err(ApiError::bad_request)?;
        client
            .download_bundle(&entry, &download, &CancellationToken::new(), None)
            .await
            .map_err(ApiError::bad_request)?;
    }
    let path = download.clone();
    let verified = tokio::task::spawn_blocking(move || verify_model_bundle(&path))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::bad_request);
    let verified = match verified {
        Ok(verified) => verified,
        Err(error) => {
            let _ = tokio::fs::remove_file(download).await;
            return Err(error);
        }
    };
    if verified.manifest.id != entry.bundle_id
        || verified.manifest.version != entry.bundle_version
        || verified.bundle_digest != entry.bundle_sha256
        || verified.manifest.license.license_digest != entry.license_summary.license_digest
    {
        let _ = tokio::fs::remove_file(download).await;
        return Err(ApiError::bad_request(
            "downloaded Bundle identity or license does not match the curated Catalog",
        ));
    }
    let installed = state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .install_verified(
            verified,
            ModelBundleInstallSource::CuratedCatalog {
                catalog_id: request.catalog_id,
            },
        )
        .map_err(ApiError::bad_request)?;
    let _ = tokio::fs::remove_file(download).await;
    let instances = bind_compatible_installed_plugins(&state, &installed)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "bundle": installed_model_bundle_view(&installed),
            "model_instances": instances.iter().map(installed_model_instance_view).collect::<Vec<_>>(),
        })),
    ))
}

async fn verify_installed_model_bundle(
    State(state): State<ServerState>,
    AxumPath((bundle_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let bundle_id = ModelBundleId::parse(bundle_id).map_err(ApiError::bad_request)?;
    let version = semver::Version::parse(&version).map_err(ApiError::bad_request)?;
    let registry = state.application.model_bundle_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
    let installed = registry
        .get(&bundle_id, &version)
        .ok_or_else(|| ApiError::not_found("Model Bundle was not found"))?;
    Ok(Json(json!({ "verification": installed.verification })))
}

async fn execute_model_instance_test(
    state: &ServerState,
    instance_id: ModelInstanceId,
) -> ApiResult<annotagent_model_catalog::InstalledModelInstance> {
    let (instance, bundle, prepared) = {
        let registry = state.application.model_bundle_registry();
        let registry = registry
            .lock()
            .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
        let instance = registry
            .model_instance(instance_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Model Instance was not found"))?;
        let bundle = registry
            .get(&instance.model_bundle_id, &instance.model_bundle_version)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Model Bundle was not found"))?;
        if !bundle.enabled {
            return Err(ApiError::bad_request("Model Bundle is disabled"));
        }
        let prepared = prepare_bundle_smoke_test(&bundle, &instance.model_id)
            .map_err(ApiError::bad_request)?;
        (instance, bundle, prepared)
    };
    let (manifest, config) = {
        let shared = state.application.plugin_registry();
        let registry = shared
            .lock()
            .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?;
        let installation = registry
            .get(&instance.plugin_id, &instance.plugin_version)
            .map_err(ApiError::bad_request)?
            .clone();
        if installation.package_digest != instance.plugin_package_digest {
            return Err(ApiError::bad_request(
                "installed Plugin package no longer matches the Model Instance",
            ));
        }
        let model_files = bundle
            .manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.role.as_str().to_owned(),
                    bundle.content_root.join(&file.path),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let config = registry
            .process_config_for_model_files(&installation, &bundle.content_root, model_files)
            .map_err(ApiError::bad_request)?;
        (installation.manifest, config)
    };
    let started_at = Utc::now();
    let started = std::time::Instant::now();
    let result = match run_model_instance_smoke(manifest, config, &prepared.request).await {
        Ok(plugin_report) => {
            let mut result = evaluate_bundle_smoke_response(
                &prepared.definition,
                &prepared.request,
                &plugin_report.response,
                started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
                started_at,
            );
            result.checks.push(SmokeTestCheck {
                name: "plugin conformance".to_owned(),
                passed: plugin_report.conformance.passed,
                detail: "Rust Plugin health, capability, model and Contract discovery passed"
                    .to_owned(),
            });
            if !plugin_report.conformance.passed {
                result.status = SmokeTestStatus::Failed;
            }
            result
        }
        Err(error) => SmokeTestResult {
            test_id: prepared.definition.test_id,
            status: SmokeTestStatus::Crashed,
            checks: vec![SmokeTestCheck {
                name: "plugin process".to_owned(),
                passed: false,
                detail: format!(
                    "Rust Plugin smoke process failed ({})",
                    match error {
                        PluginRegistryError::Host(_) => "plugin_host",
                        PluginRegistryError::InvalidWeight(_) => "model_files",
                        _ => "plugin_registry",
                    }
                ),
            }],
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            started_at,
            finished_at: Utc::now(),
        },
    };
    state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .record_model_instance_smoke(instance_id, result)
        .map_err(ApiError::bad_request)
}

async fn test_model_instance(
    State(state): State<ServerState>,
    AxumPath(instance_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let instance_id = instance_id
        .parse::<ModelInstanceId>()
        .map_err(ApiError::bad_request)?;
    let instance = execute_model_instance_test(&state, instance_id).await?;
    Ok(Json(installed_model_instance_view(&instance)))
}

async fn test_model_bundle(
    State(state): State<ServerState>,
    AxumPath((bundle_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let bundle_id = ModelBundleId::parse(bundle_id).map_err(ApiError::bad_request)?;
    let version = semver::Version::parse(&version).map_err(ApiError::bad_request)?;
    let instance_ids = state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .model_instances()
        .into_iter()
        .filter(|instance| {
            instance.model_bundle_id == bundle_id && instance.model_bundle_version == version
        })
        .map(|instance| instance.id)
        .collect::<Vec<_>>();
    if instance_ids.is_empty() {
        return Err(ApiError::bad_request(
            "Model Bundle has no compatible installed Plugin instance",
        ));
    }
    let mut tested = Vec::new();
    for instance_id in instance_ids {
        tested.push(execute_model_instance_test(&state, instance_id).await?);
    }
    Ok(Json(json!({
        "model_instances": tested.iter().map(installed_model_instance_view).collect::<Vec<_>>()
    })))
}

async fn enable_model_bundle(
    State(state): State<ServerState>,
    AxumPath((bundle_id, version)): AxumPath<(String, String)>,
) -> ApiResult<StatusCode> {
    let bundle_id = ModelBundleId::parse(bundle_id).map_err(ApiError::bad_request)?;
    let version = semver::Version::parse(&version).map_err(ApiError::bad_request)?;
    state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .enable(&bundle_id, &version)
        .map_err(ApiError::bad_request)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn disable_model_bundle(
    State(state): State<ServerState>,
    AxumPath((bundle_id, version)): AxumPath<(String, String)>,
) -> ApiResult<StatusCode> {
    let bundle_id = ModelBundleId::parse(bundle_id).map_err(ApiError::bad_request)?;
    let version = semver::Version::parse(&version).map_err(ApiError::bad_request)?;
    state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .disable(&bundle_id, &version)
        .map_err(ApiError::bad_request)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_model_bundle(
    State(state): State<ServerState>,
    AxumPath((bundle_id, version)): AxumPath<(String, String)>,
) -> ApiResult<StatusCode> {
    let bundle_id = ModelBundleId::parse(bundle_id).map_err(ApiError::bad_request)?;
    let version = semver::Version::parse(&version).map_err(ApiError::bad_request)?;
    state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .remove(&bundle_id, &version)
        .map_err(ApiError::bad_request)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_model_bundle_references(
    State(state): State<ServerState>,
    AxumPath((bundle_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let bundle_id = ModelBundleId::parse(bundle_id).map_err(ApiError::bad_request)?;
    let version = semver::Version::parse(&version).map_err(ApiError::bad_request)?;
    let references = state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .references(&bundle_id, &version);
    Ok(Json(json!({ "references": references })))
}

async fn get_model_bundle_compatibility(
    State(state): State<ServerState>,
    AxumPath((bundle_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let bundle_id = ModelBundleId::parse(bundle_id).map_err(ApiError::bad_request)?;
    let version = semver::Version::parse(&version).map_err(ApiError::bad_request)?;
    let (bundle, license_accepted) = {
        let shared = state.application.model_bundle_registry();
        let registry = shared
            .lock()
            .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
        let bundle = registry
            .get(&bundle_id, &version)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Model Bundle was not found"))?;
        let accepted = !bundle.manifest.license.requires_acceptance
            || registry.license_acceptances().iter().any(|acceptance| {
                acceptance.bundle_id == bundle_id
                    && acceptance.bundle_version == version
                    && acceptance.license_digest == bundle.manifest.license.license_digest
            });
        (bundle, accepted)
    };
    let plugins = state
        .application
        .plugin_registry()
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?
        .list();
    let mut outcomes = Vec::new();
    for requirement in &bundle.manifest.compatible_plugins {
        let matching = plugins
            .iter()
            .filter(|plugin| plugin.manifest.id == requirement.plugin_id)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            outcomes.push(json!({
                "plugin_id": requirement.plugin_id,
                "model_id": requirement.model_id,
                "compatibility": ModelBundleCompatibilityResolver::resolve(
                    None,
                    annotagent_plugin_api::PluginRuntimeStatus::NotInstalled,
                    &bundle.manifest,
                    &requirement.model_id,
                    &annotagent_plugin_host::current_target(),
                    "cpu",
                    license_accepted,
                )
            }));
            continue;
        }
        for plugin in matching {
            let execution_provider = bundle
                .manifest
                .runtime
                .execution_providers
                .iter()
                .next()
                .map_or("cpu", String::as_str);
            outcomes.push(json!({
                "plugin_id": plugin.manifest.id,
                "plugin_version": plugin.manifest.version,
                "model_id": requirement.model_id,
                "compatibility": ModelBundleCompatibilityResolver::resolve(
                    Some(&plugin.manifest),
                    plugin.runtime_status(),
                    &bundle.manifest,
                    &requirement.model_id,
                    &annotagent_plugin_host::current_target(),
                    execution_provider,
                    license_accepted,
                )
            }));
        }
    }
    Ok(Json(json!({ "compatibility": outcomes })))
}

async fn garbage_collect_model_bundles(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    let report = state
        .application
        .model_bundle_registry()
        .lock()
        .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?
        .garbage_collect()
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(report)))
}

#[derive(Debug, Deserialize)]
struct PluginUploadQuery {
    filename: String,
}

#[derive(Debug, Deserialize)]
struct PluginInstallQuery {
    filename: String,
    #[serde(default)]
    permissions_reviewed: bool,
    #[serde(default)]
    code_license_accepted: bool,
    #[serde(default)]
    weight_license_accepted: bool,
}

#[derive(Debug, Deserialize)]
struct PluginWeightUploadQuery {
    filename: String,
    model_id: String,
    component_id: Option<String>,
    sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyLocalModelBundleRequest {
    model_id: String,
    bundle_version: semver::Version,
    display_name: String,
    upstream_project: String,
    upstream_model_id: String,
    upstream_version: Option<String>,
    source_url: Option<url::Url>,
    exporter_name: String,
    exporter_version: String,
    opset: u32,
    license_name: String,
    license_url: Option<url::Url>,
    redistribution: RedistributionStatus,
    commercial_use: CommercialUseStatus,
    license_text: String,
    contract_document: String,
    license_accepted: bool,
}

fn safe_upload_filename(value: &str) -> ApiResult<String> {
    let path = Path::new(value);
    let filename = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| ApiError::bad_request("upload filename must be valid UTF-8"))?;
    if filename != value || filename.is_empty() || matches!(filename, "." | "..") {
        return Err(ApiError::bad_request(
            "upload filename must be one safe file name without directories",
        ));
    }
    Ok(filename.to_owned())
}

fn plugin_registry_root(state: &ServerState) -> ApiResult<PathBuf> {
    let registry = state.application.plugin_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?;
    Ok(registry.data_root().to_path_buf())
}

async fn receive_plugin_upload(
    state: &ServerState,
    filename: &str,
    body: Body,
    maximum_bytes: u64,
) -> ApiResult<PathBuf> {
    let filename = safe_upload_filename(filename)?;
    let upload_root = plugin_registry_root(state)?.join("uploads");
    tokio::fs::create_dir_all(&upload_root)
        .await
        .map_err(ApiError::internal)?;
    let extension = Path::new(&filename)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("upload");
    let path = upload_root.join(format!("{}.{}", uuid::Uuid::new_v4(), extension));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
        .map_err(ApiError::internal)?;
    let mut stream = body.into_data_stream();
    let mut written = 0_u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(ApiError::bad_request)?;
        written = written.saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        if written > maximum_bytes {
            drop(file);
            let _ = tokio::fs::remove_file(&path).await;
            return Err(ApiError::bad_request(format!(
                "upload exceeds the {maximum_bytes} byte safety limit"
            )));
        }
        file.write_all(&chunk).await.map_err(ApiError::internal)?;
    }
    file.flush().await.map_err(ApiError::internal)?;
    if written == 0 {
        drop(file);
        let _ = tokio::fs::remove_file(&path).await;
        return Err(ApiError::bad_request("upload cannot be empty"));
    }
    Ok(path)
}

fn plugin_registry_view(state: &ServerState) -> ApiResult<Value> {
    let registry = state.application.plugin_registry();
    let registry = registry
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?;
    let installations = registry
        .list()
        .into_iter()
        .map(|installation| {
            let weights = registry
                .weight_sets(&installation.manifest.id, &installation.manifest.version)
                .into_iter()
                .map(|weights| {
                    json!({
                        "model_id": weights.model_id,
                        "component_id": weights.component_id,
                        "checkpoint_sha256": weights.checkpoint_sha256,
                        "original_filename": weights.original_filename,
                        "size_bytes": weights.size_bytes,
                        "provisioned_at": weights.provisioned_at,
                    })
                })
                .collect::<Vec<_>>();
            let references =
                registry.references(&installation.manifest.id, &installation.manifest.version);
            let legacy_model_status = (!weights.is_empty()).then_some("legacy_unbundled_model");
            json!({
                "manifest": installation.manifest,
                "package_sha256": installation.package_digest,
                "signature": installation.signature,
                "status": installation.status,
                "enabled": installation.enabled,
                "installed_at": installation.installed_at,
                "updated_at": installation.updated_at,
                "last_test": installation.last_test,
                "weights": weights,
                "legacy_model_status": legacy_model_status,
                "references": references,
            })
        })
        .collect::<Vec<_>>();
    let models = registry
        .ready_models()
        .into_iter()
        .map(|profile| {
            json!({
                "selection_id": plugin_model_selection_id(&profile.reference),
                "reference": profile.reference,
                "display_name": profile.display_name,
                "capabilities": profile.capabilities,
                "availability": profile.availability,
                "plugin_status": profile.plugin_status,
                "enabled": profile.enabled,
                "selectable": profile.enabled && profile.availability == ModelAvailability::Available,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "installations": installations,
        "models": models,
        "agent_permissions": {
            "discover": true,
            "install": false,
            "accept_licenses": false,
            "provision_weights": false,
        },
    }))
}

async fn list_expert_model_plugins(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    Ok(Json(plugin_registry_view(&state)?))
}

async fn inspect_expert_model_plugin_package(
    State(state): State<ServerState>,
    Query(query): Query<PluginUploadQuery>,
    body: Body,
) -> ApiResult<Json<Value>> {
    if !query.filename.ends_with(".annotplugin") {
        return Err(ApiError::bad_request(
            "Expert Model plugin packages must use the .annotplugin extension",
        ));
    }
    let upload = receive_plugin_upload(
        &state,
        &query.filename,
        body,
        MAX_PLUGIN_PACKAGE_UPLOAD_BYTES,
    )
    .await?;
    let verify_path = upload.clone();
    let verified = tokio::task::spawn_blocking(move || verify_package(&verify_path))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::bad_request);
    let _ = tokio::fs::remove_file(upload).await;
    let verified = verified?;
    Ok(Json(json!({
        "manifest": verified.manifest,
        "package_sha256": verified.package_digest,
        "signature": format!("{:?}", verified.signature).to_ascii_lowercase(),
        "verified": true,
        "installed": false,
    })))
}

async fn install_expert_model_plugin_package(
    State(state): State<ServerState>,
    Query(query): Query<PluginInstallQuery>,
    body: Body,
) -> ApiResult<(StatusCode, Json<Value>)> {
    if !query.filename.ends_with(".annotplugin") {
        return Err(ApiError::bad_request(
            "Expert Model plugin packages must use the .annotplugin extension",
        ));
    }
    let upload = receive_plugin_upload(
        &state,
        &query.filename,
        body,
        MAX_PLUGIN_PACKAGE_UPLOAD_BYTES,
    )
    .await?;
    let approval = InstallApproval {
        permissions_reviewed: query.permissions_reviewed,
        code_license_accepted: query.code_license_accepted,
        weight_license_accepted: query.weight_license_accepted,
    };
    let shared = state.application.plugin_registry();
    let install_path = upload.clone();
    let installed = tokio::task::spawn_blocking(move || {
        shared
            .lock()
            .map_err(|_| "Rust plugin Registry lock is poisoned".to_owned())?
            .install(&install_path, &approval)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::bad_request);
    let _ = tokio::fs::remove_file(upload).await;
    let installed = installed?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "plugin_id": installed.manifest.id,
            "version": installed.manifest.version,
            "status": installed.status,
            "enabled": installed.enabled,
        })),
    ))
}

async fn provision_expert_model_plugin_weights(
    State(state): State<ServerState>,
    AxumPath((plugin_id, version)): AxumPath<(String, String)>,
    Query(query): Query<PluginWeightUploadQuery>,
    body: Body,
) -> ApiResult<Json<Value>> {
    let plugin_id = PluginId::parse(plugin_id).map_err(ApiError::bad_request)?;
    let version = PluginVersion::parse(&version).map_err(ApiError::bad_request)?;
    let expected = query
        .sha256
        .map(Sha256Digest::parse)
        .transpose()
        .map_err(ApiError::bad_request)?;
    let upload = receive_plugin_upload(
        &state,
        &query.filename,
        body,
        MAX_PLUGIN_WEIGHT_UPLOAD_BYTES,
    )
    .await?;
    let shared = state.application.plugin_registry();
    let upload_path = upload.clone();
    let model_id = query.model_id;
    let component_id = query.component_id;
    let provisioned = tokio::task::spawn_blocking(move || {
        let mut registry = shared
            .lock()
            .map_err(|_| "Rust plugin Registry lock is poisoned".to_owned())?;
        let result = if let Some(component_id) = component_id {
            registry.provision_local_weight_component(
                &plugin_id,
                &version,
                &model_id,
                &component_id,
                &upload_path,
                expected.as_ref(),
            )
        } else {
            registry.provision_local_weights(
                &plugin_id,
                &version,
                &model_id,
                &upload_path,
                expected.as_ref(),
            )
        };
        result.map_err(|error| error.to_string())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::bad_request);
    let _ = tokio::fs::remove_file(upload).await;
    let provisioned = provisioned?;
    Ok(Json(json!({
        "model_id": provisioned.model_id,
        "component_id": provisioned.component_id,
        "checkpoint_sha256": provisioned.checkpoint_sha256,
        "size_bytes": provisioned.size_bytes,
        "status": "installed",
    })))
}

fn legacy_bundle_smoke_shape(
    capability: ModelCapability,
) -> ApiResult<(VisionCapability, ArtifactKind, bool)> {
    match capability {
        ModelCapability::ImageClassification => Ok((
            VisionCapability::Classification,
            ArtifactKind::ClassificationSet,
            false,
        )),
        ModelCapability::ObjectDetection => Ok((
            VisionCapability::ObjectDetection,
            ArtifactKind::DetectionSet,
            false,
        )),
        ModelCapability::OpenVocabularyDetection => Ok((
            VisionCapability::OpenVocabularyDetection,
            ArtifactKind::DetectionSet,
            false,
        )),
        ModelCapability::PhraseGrounding => Ok((
            VisionCapability::PhraseGrounding,
            ArtifactKind::DetectionSet,
            false,
        )),
        ModelCapability::SemanticSegmentation => Ok((
            VisionCapability::SemanticSegmentation,
            ArtifactKind::SemanticMask,
            false,
        )),
        ModelCapability::PromptedSegmentation => Ok((
            VisionCapability::PromptedSegmentation,
            ArtifactKind::MaskSet,
            true,
        )),
        ModelCapability::InstanceSegmentation => Ok((
            VisionCapability::InstanceSegmentation,
            ArtifactKind::MaskSet,
            false,
        )),
        ModelCapability::TextGeneration
        | ModelCapability::VisionLanguage
        | ModelCapability::KeypointDetection => Err(ApiError::bad_request(
            "Legacy local Bundle migration does not have a typed smoke-test template for this capability",
        )),
    }
}

fn legacy_bundle_segment(value: &str) -> String {
    let mut value = value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' {
                char::from(byte)
            } else if byte.is_ascii_uppercase() {
                char::from(byte.to_ascii_lowercase())
            } else {
                '-'
            }
        })
        .collect::<String>();
    while value.contains("--") {
        value = value.replace("--", "-");
    }
    value = value.trim_matches('-').chars().take(63).collect();
    value.trim_end_matches('-').to_owned()
}

async fn create_legacy_local_model_bundle(
    State(state): State<ServerState>,
    AxumPath((plugin_id, version)): AxumPath<(String, String)>,
    Json(request): Json<LegacyLocalModelBundleRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let plugin_id = PluginId::parse(plugin_id).map_err(ApiError::bad_request)?;
    let plugin_version = PluginVersion::parse(&version).map_err(ApiError::bad_request)?;
    if !request.license_accepted {
        return Err(ApiError::bad_request(
            "creating a local Model Bundle requires explicit acceptance of the supplied license",
        ));
    }
    if request.license_text.trim().is_empty() || request.license_text.len() > 1024 * 1024 {
        return Err(ApiError::bad_request(
            "the supplied model license must be non-empty and at most 1 MiB",
        ));
    }
    if request.contract_document.len() > 1024 * 1024 {
        return Err(ApiError::bad_request(
            "the supplied Model Contract must be at most 1 MiB",
        ));
    }
    let contract = ModelContractDocument::from_json(request.contract_document.as_bytes())
        .map_err(ApiError::bad_request)?;
    let (plugin, model, legacy_weights) = {
        let shared = state.application.plugin_registry();
        let registry = shared
            .lock()
            .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?;
        let plugin = registry
            .get(&plugin_id, &plugin_version)
            .map_err(ApiError::not_found)?
            .clone();
        let model = plugin
            .manifest
            .models
            .iter()
            .find(|model| model.id == request.model_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Plugin model was not found"))?;
        let weights = registry
            .weight_sets(&plugin_id, &plugin_version)
            .into_iter()
            .filter(|weight| weight.model_id == request.model_id)
            .collect::<Vec<_>>();
        (plugin, model, weights)
    };
    if legacy_weights.is_empty() {
        return Err(ApiError::bad_request(
            "no legacy model files exist for this Plugin model",
        ));
    }
    let required_roles = model
        .required_file_roles
        .iter()
        .map(|role| ModelFileRole::parse(role.clone()).map_err(ApiError::bad_request))
        .collect::<ApiResult<BTreeSet<_>>>()?;
    let provided_roles = legacy_weights
        .iter()
        .map(|weight| {
            ModelFileRole::parse(weight.component_id.clone()).map_err(ApiError::bad_request)
        })
        .collect::<ApiResult<BTreeSet<_>>>()?;
    if required_roles != provided_roles
        || contract.roles.keys().cloned().collect::<BTreeSet<_>>() != required_roles
    {
        return Err(ApiError::bad_request(format!(
            "legacy files, Plugin requirements, and Contract roles must match exactly; required={required_roles:?}, files={provided_roles:?}"
        )));
    }
    let capability = *model
        .capabilities
        .first()
        .ok_or_else(|| ApiError::bad_request("Plugin model declares no capability"))?;
    let (operation, expected_kind, prompted) = legacy_bundle_smoke_shape(capability)?;
    let plugin_segment = legacy_bundle_segment(plugin_id.as_str());
    let model_segment = legacy_bundle_segment(&request.model_id);
    if plugin_segment.is_empty() || model_segment.is_empty() {
        return Err(ApiError::bad_request(
            "Plugin and model ids cannot form a safe local Bundle id",
        ));
    }
    let bundle_id = ModelBundleId::parse(format!("local.{plugin_segment}.{model_segment}"))
        .map_err(ApiError::bad_request)?;
    let contract_hash = ModelBundleSha256Digest::of_bytes(request.contract_document.as_bytes());
    let license_hash = ModelBundleSha256Digest::of_bytes(request.license_text.as_bytes());
    let plugin_contract_hash =
        ModelBundleSha256Digest::of_bytes(&serde_json::to_vec(&model).map_err(ApiError::internal)?);
    let registry_root = model_bundle_registry_root(&state)?;
    let staging = registry_root
        .join("legacy-bundle-staging")
        .join(uuid::Uuid::new_v4().to_string());
    let export_root = registry_root.join("local-bundle-exports");
    let output = export_root.join(format!(
        "{}@{}.annotmodel",
        bundle_id, request.bundle_version
    ));
    if output.exists() {
        return Err(ApiError::bad_request(format!(
            "local Bundle {}@{} already exists; choose a new version after changing its metadata or Contract",
            bundle_id, request.bundle_version
        )));
    }
    let manifest = {
        let source_url = request.source_url.clone();
        let license_url = request.license_url.clone();
        let upstream_checksum = if legacy_weights.len() == 1 {
            Some(
                ModelBundleSha256Digest::parse(
                    legacy_weights[0].checkpoint_sha256.as_str().to_owned(),
                )
                .map_err(ApiError::bad_request)?,
            )
        } else {
            None
        };
        let files = legacy_weights
            .iter()
            .map(|weight| {
                let role = ModelFileRole::parse(weight.component_id.clone())
                    .map_err(ApiError::bad_request)?;
                Ok(ModelBundleFile {
                    path: format!("files/{role}.onnx"),
                    role,
                    sha256: ModelBundleSha256Digest::parse(
                        weight.checkpoint_sha256.as_str().to_owned(),
                    )
                    .map_err(ApiError::bad_request)?,
                    size_bytes: weight.size_bytes,
                    external_data_files: Vec::new(),
                })
            })
            .collect::<ApiResult<Vec<_>>>()?;
        ModelBundleManifest {
            schema_version: "1".to_owned(),
            id: bundle_id.clone(),
            version: request.bundle_version.clone(),
            display_name: request.display_name.clone(),
            description: Some(format!(
                "Local migration of legacy files for {}",
                plugin.manifest.display_name
            )),
            model_family: request.upstream_project.clone(),
            architecture: request.upstream_model_id.clone(),
            format: ModelFormat::Onnx,
            variant: "legacy-local-migration".to_owned(),
            capabilities: model.capabilities.iter().copied().collect(),
            compatible_plugins: vec![PluginCompatibilityRequirement {
                plugin_id: plugin_id.clone(),
                plugin_version: format!("={plugin_version}"),
                model_id: request.model_id.clone(),
                contract_hash: plugin_contract_hash,
                required_file_roles: required_roles.clone(),
            }],
            files,
            contracts: vec![ModelContractReference {
                id: "legacy-user-supplied-contract".to_owned(),
                path: "contracts/model-contract.json".to_owned(),
                sha256: contract_hash,
                file_roles: required_roles,
            }],
            transforms: Vec::new(),
            source: ModelSourceMetadata {
                upstream_project: request.upstream_project.clone(),
                upstream_model_id: request.upstream_model_id.clone(),
                upstream_version: request.upstream_version.clone(),
                upstream_checkpoint_sha256: upstream_checksum,
                source_url,
            },
            export: ModelExportMetadata {
                exporter_name: request.exporter_name.clone(),
                exporter_version: request.exporter_version.clone(),
                exporter_revision: None,
                export_date: Some(Utc::now()),
                opset: Some(request.opset),
                numerical_validation: None,
            },
            runtime: ModelRuntimeMetadata {
                execution_providers: model.runtime_requirements.devices.iter().cloned().collect(),
                platforms: plugin
                    .manifest
                    .compatibility
                    .targets
                    .iter()
                    .cloned()
                    .collect(),
                minimum_memory_mb: plugin.manifest.resources.minimum_memory_mb,
                recommended_memory_mb: plugin.manifest.resources.recommended_memory_mb,
            },
            license: ModelLicenseMetadata {
                name: request.license_name.clone(),
                license_url,
                license_file: "licenses/MODEL-LICENSE".to_owned(),
                source_notice: None,
                license_digest: license_hash,
                redistribution: request.redistribution,
                commercial_use: request.commercial_use,
                requires_acceptance: true,
                usage_notes: vec![
                    "User-supplied metadata for a local legacy-file migration".to_owned(),
                ],
            },
            test_suite: ModelTestSuiteReference {
                test_id: "legacy-local-smoke-v1".to_owned(),
                input_artifacts: vec![
                    "tests/input-image.png".to_owned(),
                    "tests/request.json".to_owned(),
                ],
                expected_summary: "tests/expected-summary.json".to_owned(),
                tolerances: "tests/tolerances.json".to_owned(),
            },
            fixture: false,
            publishable: true,
        }
    };
    manifest.validate().map_err(ApiError::bad_request)?;
    let image_id = ImageId::new();
    let input_artifacts = if prompted {
        let detection = ArtifactRef {
            artifact_id: "legacy-smoke-detections".to_owned(),
            source_node: "legacy_bundle_migration".to_owned(),
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        };
        vec![PipelineArtifact::BoxPromptSet(BoxPromptSetArtifact {
            reference: ArtifactRef {
                artifact_id: "legacy-smoke-prompts".to_owned(),
                source_node: "legacy_bundle_migration".to_owned(),
                port: "box_prompts".to_owned(),
                artifact_type: ArtifactKind::BoxPromptSet,
                item_id: None,
            },
            image_id,
            source_detections: detection.clone(),
            prompts: vec![BoxPrompt {
                id: "legacy-smoke-box".to_owned(),
                subject: detection.item("legacy-smoke-object"),
                bbox: NormalizedRect::new(0.2, 0.2, 0.6, 0.6).map_err(ApiError::bad_request)?,
                attributes: BTreeMap::new(),
            }],
        })]
    } else {
        Vec::new()
    };
    let smoke_request = ModelBundleSmokeRequest {
        image_path: "tests/input-image.png".to_owned(),
        operation,
        input_artifacts,
        parameters: BTreeMap::new(),
        timeout_ms: Some(120_000),
    };
    let expected = ExpectedOutputSummary {
        required_artifact_kinds: BTreeSet::from([expected_kind]),
        minimum_artifact_count: 1,
        minimum_item_count: 1,
        require_non_empty_mask: prompted,
    };
    let tolerances = OutputTolerances {
        maximum_duration_ms: 120_000,
        minimum_mask_coverage: prompted.then_some(0.000_001),
        maximum_mask_coverage: prompted.then_some(1.0),
    };
    let plugin_files = legacy_weights.clone();
    let manifest_for_pack = manifest.clone();
    let contract_document = request.contract_document;
    let license_text = request.license_text;
    let staging_for_pack = staging.clone();
    let output_for_pack = output.clone();
    let packed = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let result = (|| -> Result<_, Box<dyn std::error::Error>> {
            for directory in ["files", "contracts", "licenses", "tests"] {
                std::fs::create_dir_all(staging_for_pack.join(directory))?;
            }
            for (weight, file) in plugin_files.iter().zip(&manifest_for_pack.files) {
                std::fs::copy(&weight.stored_path, staging_for_pack.join(&file.path))?;
            }
            std::fs::write(
                staging_for_pack.join("contracts/model-contract.json"),
                contract_document,
            )?;
            std::fs::write(
                staging_for_pack.join("licenses/MODEL-LICENSE"),
                license_text,
            )?;
            std::fs::write(
                staging_for_pack.join("tests/input-image.png"),
                [
                    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49,
                    0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04,
                    0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c, 0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44,
                    0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00, 0x01, 0x05, 0x01, 0x01,
                    0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
                    0x42, 0x60, 0x82,
                ],
            )?;
            std::fs::write(
                staging_for_pack.join("tests/request.json"),
                serde_json::to_vec_pretty(&smoke_request)?,
            )?;
            std::fs::write(
                staging_for_pack.join("tests/expected-summary.json"),
                serde_json::to_vec_pretty(&expected)?,
            )?;
            std::fs::write(
                staging_for_pack.join("tests/tolerances.json"),
                serde_json::to_vec_pretty(&tolerances)?,
            )?;
            std::fs::write(
                staging_for_pack.join(annotagent_model_bundle::MODEL_BUNDLE_MANIFEST_FILE),
                manifest_for_pack.to_toml()?,
            )?;
            std::fs::create_dir_all(
                output_for_pack
                    .parent()
                    .ok_or("local Bundle output has no parent")?,
            )?;
            pack_model_bundle(&staging_for_pack, &output_for_pack)?;
            Ok(verify_model_bundle(&output_for_pack)?)
        })()
        .map_err(|error| error.to_string());
        let _ = std::fs::remove_dir_all(&staging_for_pack);
        result
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::bad_request)?;
    let installed = {
        let registry = state.application.model_bundle_registry();
        let mut registry = registry
            .lock()
            .map_err(|_| ApiError::internal("Model Bundle Registry lock is poisoned"))?;
        registry
            .accept_license(ModelLicenseAcceptance {
                bundle_id: packed.manifest.id.clone(),
                bundle_version: packed.manifest.version.clone(),
                license_digest: packed.manifest.license.license_digest.clone(),
                accepted_at: Utc::now(),
                accepted_by: LicenseAcceptanceActor::LocalUser,
            })
            .map_err(ApiError::internal)?;
        registry
            .install_verified(packed, ModelBundleInstallSource::LocalImport)
            .map_err(ApiError::bad_request)?
    };
    let created_instances = bind_compatible_installed_plugins(&state, &installed)?;
    let mut tested_instances = Vec::new();
    for instance in created_instances {
        match execute_model_instance_test(&state, instance.id).await {
            Ok(tested) => tested_instances.push(tested),
            Err(error) => {
                return Err(ApiError {
                    status: StatusCode::UNPROCESSABLE_ENTITY,
                    body: json!({
                        "code": "legacy_bundle_smoke_test_failed",
                        "error": error.body.get("error").cloned().unwrap_or_else(|| json!("Local Model Bundle smoke test failed")),
                        "failed_stage": "smoke_test",
                        "local_bundle_path": output,
                        "legacy_files_preserved": true,
                        "model_instance": installed_model_instance_view(&instance),
                        "suggested_action": "Correct the supplied Model Contract or model provenance, choose a new Bundle version, and retry. The original legacy files remain unchanged.",
                    }),
                });
            }
        }
    }
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "bundle": installed_model_bundle_view(&installed),
            "model_instances": tested_instances.iter().map(installed_model_instance_view).collect::<Vec<_>>(),
            "local_bundle_path": output,
            "legacy_files_preserved": true,
        })),
    ))
}

async fn test_expert_model_plugin(
    State(state): State<ServerState>,
    AxumPath((plugin_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let plugin_id = PluginId::parse(plugin_id).map_err(ApiError::bad_request)?;
    let version = PluginVersion::parse(&version).map_err(ApiError::bad_request)?;
    let root = plugin_registry_root(&state)?;
    let test_registry =
        annotagent_plugin_registry::PluginRegistry::open(root).map_err(ApiError::internal)?;
    let installation = test_registry
        .get(&plugin_id, &version)
        .map_err(ApiError::bad_request)?
        .clone();
    let report = test_registry
        .test_installation(&installation)
        .await
        .map_err(ApiError::bad_request)?;
    let status = state
        .application
        .plugin_registry()
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?
        .record_test(report.clone())
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"report": report, "status": status})))
}

async fn enable_expert_model_plugin(
    State(state): State<ServerState>,
    AxumPath((plugin_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let plugin_id = PluginId::parse(plugin_id).map_err(ApiError::bad_request)?;
    let version = PluginVersion::parse(&version).map_err(ApiError::bad_request)?;
    let status = state
        .application
        .plugin_registry()
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?
        .enable(&plugin_id, &version)
        .map_err(ApiError::bad_request)?;
    Ok(Json(
        json!({"status": status, "enabled": status != annotagent_plugin_api::PluginStatus::UnsupportedPlatform}),
    ))
}

async fn disable_expert_model_plugin(
    State(state): State<ServerState>,
    AxumPath((plugin_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let plugin_id = PluginId::parse(plugin_id).map_err(ApiError::bad_request)?;
    let version = PluginVersion::parse(&version).map_err(ApiError::bad_request)?;
    state
        .application
        .plugin_registry()
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?
        .disable(&plugin_id, &version)
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({"status": "disabled", "enabled": false})))
}

async fn uninstall_expert_model_plugin(
    State(state): State<ServerState>,
    AxumPath((plugin_id, version)): AxumPath<(String, String)>,
) -> ApiResult<Json<Value>> {
    let plugin_id = PluginId::parse(plugin_id).map_err(ApiError::bad_request)?;
    let version = PluginVersion::parse(&version).map_err(ApiError::bad_request)?;
    let shared = state.application.plugin_registry();
    let mut registry = shared
        .lock()
        .map_err(|_| ApiError::internal("Rust plugin Registry lock is poisoned"))?;
    let references = registry.references(&plugin_id, &version);
    if !references.is_empty() {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({
                "code": "plugin_version_referenced",
                "error": "This exact plugin version is frozen into a Published Workflow and cannot be uninstalled.",
                "references": references,
                "suggested_action": "Publish a replacement Workflow using another model version before uninstalling this version.",
            }),
        });
    }
    match registry.uninstall(&plugin_id, &version) {
        Ok(()) => Ok(Json(
            json!({"uninstalled": format!("{plugin_id}@{version}")}),
        )),
        Err(PluginRegistryError::Referenced(detail)) => Err(ApiError {
            status: StatusCode::CONFLICT,
            body: json!({"code": "plugin_version_referenced", "error": detail}),
        }),
        Err(error) => Err(ApiError::bad_request(error)),
    }
}

#[cfg(test)]
mod tests {
    use annotagent_core::RunStatus;
    use annotagent_image_tools::{generate_synthetic_inspection, generate_synthetic_robocup};
    use annotagent_provider::InMemorySecretStore;
    use axum::body::to_bytes;
    use futures::StreamExt;
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;

    async fn test_state(
        application: Arc<LocalApplication>,
        secret_store: Arc<InMemorySecretStore>,
    ) -> ServerState {
        let provider_id = ProviderId(stable_project_id(application.workspace()).0);
        let reference = CredentialReference {
            provider_id,
            source: CredentialSource::SystemKeyring,
            locator: format!("test-{provider_id}"),
        };
        ServerState::with_secret_store(application, secret_store, reference.clone(), reference)
            .await
            .expect("state")
    }

    async fn call_json(
        service: &Router,
        method: axum::http::Method,
        uri: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let response = service
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("body");
        let value = serde_json::from_slice(&body).unwrap_or_else(|_| json!({}));
        (status, value)
    }

    async fn call_bytes(
        service: &Router,
        method: axum::http::Method,
        uri: &str,
        body: Vec<u8>,
    ) -> (StatusCode, Value) {
        let response = service
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/octet-stream")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("body");
        let value = serde_json::from_slice(&body).unwrap_or_else(|_| json!({}));
        (status, value)
    }

    fn plugin_package(temp: &tempfile::TempDir) -> Vec<u8> {
        let source = temp.path().join("plugin-source");
        let binary = source
            .join("bin")
            .join(annotagent_plugin_host::current_target())
            .join("annotagent-plugin-dummy-detector");
        std::fs::create_dir_all(binary.parent().expect("binary parent")).expect("dirs");
        std::fs::write(
            source.join(annotagent_plugin_api::PLUGIN_MANIFEST_FILE),
            include_str!("../../../plugins/dummy-detector/annotagent-plugin.toml"),
        )
        .expect("manifest");
        std::fs::write(binary, b"server install fixture").expect("binary");
        let package = temp.path().join("fixture.annotplugin");
        annotagent_plugin_host::pack_directory(&source, &package).expect("package");
        std::fs::read(package).expect("package bytes")
    }

    fn sam_plugin_package(temp: &tempfile::TempDir) -> Vec<u8> {
        let source = temp.path().join("sam-plugin-source");
        let binary = source
            .join("bin")
            .join(annotagent_plugin_host::current_target())
            .join("annotagent-plugin-sam-onnx");
        std::fs::create_dir_all(binary.parent().expect("binary parent")).expect("dirs");
        std::fs::write(
            source.join(annotagent_plugin_api::PLUGIN_MANIFEST_FILE),
            include_str!("../../../plugins/sam-onnx/annotagent-plugin.toml"),
        )
        .expect("manifest");
        std::fs::write(binary, b"server SAM migration fixture").expect("binary");
        let package = temp.path().join("sam-fixture.annotplugin");
        annotagent_plugin_host::pack_directory(&source, &package).expect("package");
        std::fs::read(package).expect("package bytes")
    }

    fn legacy_http_sam_fixture() -> DetectionWorkerSettings {
        serde_json::from_value(json!({
            "id": "legacy-e2e-sam",
            "display_name": "Legacy HTTP SAM fixture",
            "model_id": "sam2.1-hiera-tiny",
            "base_url": "http://127.0.0.1:8796",
            "authentication_reference": null,
            "enabled": false,
            "allow_remote": false,
            "requires_checkpoint_metadata": true,
            "expected_capabilities": ["prompted_segmentation"],
            "score_semantics": "not_provided",
            "version": {
                "architecture": "sam2.1-hiera-tiny",
                "model_version": "unconfigured",
                "checkpoint_sha256": null,
                "training_dataset_version": null,
                "backend_protocol_version": "1"
            },
            "label_space": [],
            "runtime_requirements": {
                "devices": ["cpu"],
                "minimum_gpu_memory_mb": null,
                "dependencies": [],
                "supports_batch": false
            },
            "license": {
                "code_license": null,
                "weight_license": null,
                "source_url": null,
                "commercial_use": "unknown",
                "redistribution": "unknown",
                "usage_notes": [],
                "verified_from_official_source": false
            },
            "timeout_seconds": 10,
            "max_request_bytes": 20_000_000,
            "max_response_bytes": 4_000_000,
            "max_retries": 0,
            "cost_per_request": "0",
            "availability": "missing_weights",
            "availability_evidence": {
                "health_passed": false,
                "protocol_compatible": false,
                "contracts_validated": false,
                "sample_conversion_passed": false,
                "weights_ready": false,
                "checked_at": null,
                "detail": null
            }
        }))
        .expect("legacy HTTP Worker fixture")
    }

    #[tokio::test]
    async fn plugin_install_wizard_uses_real_package_validation_and_explicit_approval() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            test_state(application, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let package = plugin_package(&temp);

        let (status, inspected) = call_bytes(
            &service,
            axum::http::Method::POST,
            "/api/plugins/packages/inspect?filename=fixture.annotplugin",
            package.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{inspected:#?}");
        assert_eq!(inspected["verified"], json!(true));
        assert_eq!(
            inspected["manifest"]["id"],
            json!("org.annotagent.dummy-detector")
        );

        let (status, rejected) = call_bytes(
            &service,
            axum::http::Method::POST,
            "/api/plugins/packages/install?filename=fixture.annotplugin",
            package.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected:#?}");
        assert!(
            rejected["error"]
                .as_str()
                .is_some_and(|error| error.contains("approval"))
        );

        let (status, installed) = call_bytes(
            &service,
            axum::http::Method::POST,
            "/api/plugins/packages/install?filename=fixture.annotplugin&permissions_reviewed=true&code_license_accepted=true&weight_license_accepted=true",
            package,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{installed:#?}");

        let (status, catalog) = call_json(
            &service,
            axum::http::Method::GET,
            "/api/plugins",
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{catalog:#?}");
        assert_eq!(catalog["installations"].as_array().map(Vec::len), Some(1));
        assert_eq!(catalog["models"].as_array().map(Vec::len), Some(1));
        assert_eq!(catalog["models"][0]["selectable"], json!(false));
        assert_eq!(catalog["agent_permissions"]["install"], json!(false));
        assert!(
            catalog["installations"][0]
                .get("installation_root")
                .is_none()
        );
    }

    #[tokio::test]
    async fn model_install_operation_exposes_recoverable_stage_and_actionable_failure() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let catalog = application
            .model_bundle_registry()
            .lock()
            .expect("registry")
            .catalogs()[0]
            .clone();
        let entry = catalog.entries[0].clone();
        let state = test_state(application, Arc::new(InMemorySecretStore::default())).await;
        let service = router(state, None);
        let package = sam_plugin_package(&temp);
        let (status, installed) = call_bytes(
            &service,
            axum::http::Method::POST,
            "/api/plugins/packages/install?filename=sam.annotplugin&permissions_reviewed=true&code_license_accepted=true&weight_license_accepted=true",
            package,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{installed:#?}");

        let request = json!({
            "catalog_id": catalog.catalog_id,
            "bundle_id": entry.bundle_id,
            "bundle_version": entry.bundle_version,
            "plugin_id": "org.annotagent.sam-onnx",
            "plugin_version": "1.1.0"
        });
        let (status, started) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/model-installations",
            request,
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED, "{started:#?}");
        assert_eq!(started["status"], json!("running"));
        assert_eq!(started["stage"], json!("resolving_model"));
        let operation_id = started["id"].as_str().expect("operation id");

        let mut terminal = Value::Null;
        for _ in 0..100 {
            let (status, current) = call_json(
                &service,
                axum::http::Method::GET,
                &format!("/api/model-installations/{operation_id}"),
                Value::Null,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{current:#?}");
            if current["status"] != json!("running") {
                terminal = current;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(terminal["status"], json!("failed"), "{terminal:#?}");
        assert!(
            terminal["error"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert!(
            terminal["suggested_action"]
                .as_str()
                .is_some_and(|value| value.contains("retry")),
            "{terminal:#?}"
        );

        let (status, recovered) = call_json(
            &service,
            axum::http::Method::GET,
            "/api/model-installations",
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{recovered:#?}");
        assert_eq!(recovered["operations"][0]["id"], json!(operation_id));
        assert_eq!(recovered["operations"][0]["status"], json!("failed"));
    }

    #[test]
    fn compatible_bundle_list_blocks_a_persisted_legacy_plugin_before_installation() {
        let temp = tempfile::tempdir().expect("temp");
        let application = LocalApplication::new(temp.path()).expect("application");
        let entry = application
            .model_bundle_registry()
            .lock()
            .expect("registry")
            .catalogs()[0]
            .entries[0]
            .clone();
        let mut manifest = annotagent_plugin_api::PluginManifest::from_toml(include_str!(
            "../../../plugins/sam-onnx/annotagent-plugin.toml"
        ))
        .expect("current manifest");
        manifest.version = PluginVersion::parse("1.0.0").expect("legacy version");
        manifest.models[0].required_file_roles.clear();
        let legacy = PluginInstallation {
            manifest,
            package_digest: Sha256Digest::of_bytes(b"persisted legacy package"),
            signature: "unsigned".to_owned(),
            status: annotagent_plugin_api::PluginStatus::NeedsWeights,
            enabled: true,
            installation_root: temp.path().join("legacy-plugin"),
            installed_at: Utc::now(),
            updated_at: Utc::now(),
            last_test: None,
        };
        match catalog_plugin_setup_match(&legacy, &entry) {
            CatalogPluginSetupMatch::Blocked { code, message } => {
                assert_eq!(code, "plugin_version_incompatible");
                assert!(message.contains("1.1.0"));
            }
            CatalogPluginSetupMatch::Compatible | CatalogPluginSetupMatch::Irrelevant => {
                panic!("persisted legacy Plugin must be blocked before Bundle installation")
            }
        }
    }

    #[tokio::test]
    async fn legacy_model_files_are_untrusted_and_preserved_during_local_bundle_migration() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            test_state(application, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let package = sam_plugin_package(&temp);
        let (status, installed) = call_bytes(
            &service,
            axum::http::Method::POST,
            "/api/plugins/packages/install?filename=sam.annotplugin&permissions_reviewed=true&code_license_accepted=true&weight_license_accepted=true",
            package,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{installed:#?}");
        for (component, filename) in [
            ("image_encoder", "encoder.onnx"),
            ("mask_decoder", "decoder.onnx"),
        ] {
            let (status, provisioned) = call_bytes(
                &service,
                axum::http::Method::POST,
                &format!(
                    "/api/plugins/org.annotagent.sam-onnx/1.1.0/weights?filename={filename}&model_id=sam-vit-b-onnx&component_id={component}"
                ),
                format!("untrusted legacy {component}").into_bytes(),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{provisioned:#?}");
        }
        let (_, registry) = call_json(
            &service,
            axum::http::Method::GET,
            "/api/plugins",
            Value::Null,
        )
        .await;
        assert_eq!(
            registry["installations"][0]["legacy_model_status"],
            json!("legacy_unbundled_model")
        );
        assert_eq!(
            registry["installations"][0]["weights"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );

        let contract = json!({
            "contract_version": "1",
            "roles": {
                "image_encoder": {
                    "inputs": [{"name": "image", "aliases": [], "dtype": "f32", "shape": [1, 3, 1024, 1024]}],
                    "outputs": [{"name": "embedding", "aliases": [], "dtype": "f32", "shape": [1, 256, 64, 64]}]
                },
                "mask_decoder": {
                    "inputs": [{"name": "embedding", "aliases": [], "dtype": "f32", "shape": [1, 256, 64, 64]}],
                    "outputs": [{"name": "masks", "aliases": [], "dtype": "f32", "shape": [1, 1, 256, 256]}]
                }
            },
            "connections": [{
                "source_role": "image_encoder",
                "source_output": "embedding",
                "target_role": "mask_decoder",
                "target_input": "embedding"
            }]
        });
        let request = json!({
            "model_id": "sam-vit-b-onnx",
            "bundle_version": "1.0.0",
            "display_name": "Local legacy SAM",
            "upstream_project": "Owner supplied SAM export",
            "upstream_model_id": "sam-vit-b",
            "upstream_version": "unknown",
            "source_url": "https://example.invalid/owner-record",
            "exporter_name": "Owner supplied exporter",
            "exporter_version": "unknown",
            "opset": 17,
            "license_name": "Owner supplied terms",
            "license_url": null,
            "redistribution": "unknown",
            "commercial_use": "unknown",
            "license_text": "Owner supplied license record for local testing.",
            "contract_document": contract.to_string(),
            "license_accepted": false
        });
        let (status, rejected) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/plugins/org.annotagent.sam-onnx/1.1.0/legacy-model-bundle",
            request.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected:#?}");

        let mut accepted = request;
        accepted["license_accepted"] = json!(true);
        let (status, migrated) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/plugins/org.annotagent.sam-onnx/1.1.0/legacy-model-bundle",
            accepted,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{migrated:#?}");
        assert_eq!(migrated["legacy_files_preserved"], json!(true));
        assert!(
            migrated["error"]
                .as_str()
                .is_some_and(|error| error.contains("Contract-mismatched"))
        );
        let (_, model_registry) = call_json(
            &service,
            axum::http::Method::GET,
            "/api/model-instances",
            Value::Null,
        )
        .await;
        assert_eq!(
            model_registry["instances"][0]["status"],
            json!("contract_mismatch"),
            "invalid fixture ONNX files cannot become a Ready Model Instance"
        );
        let (_, after) = call_json(
            &service,
            axum::http::Method::GET,
            "/api/plugins",
            Value::Null,
        )
        .await;
        assert_eq!(
            after["installations"][0]["weights"]
                .as_array()
                .map(Vec::len),
            Some(2),
            "local migration never removes legacy files"
        );
    }

    #[tokio::test]
    async fn geometry_policy_and_calibration_apis_are_project_scoped() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        application
            .create_project(
                "geometry-api",
                r"
version: 1
project:
  name: Geometry API
  language: en
dataset:
  root: images
runtime: {}
tasks:
  - id: objects
    kind: bounding_box
    labels: [ball]
    required: false
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
",
            )
            .expect("Project");
        let service = router(
            test_state(
                application.clone(),
                Arc::new(InMemorySecretStore::default()),
            )
            .await,
            None,
        );

        let (status, default_policy) = call_json(
            &service,
            axum::http::Method::GET,
            "/api/projects/geometry-api/geometry-policy",
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            default_policy["policies"][0]["auto_accept_policy"],
            json!("refiner_or_review")
        );

        let (status, saved_policy) = call_json(
            &service,
            axum::http::Method::PUT,
            "/api/projects/geometry-api/geometry-policy",
            json!({
                "task_kind": "bounding_box",
                "required_quality": "tight_bounding_box",
                "auto_accept_policy": "calibration_required",
                "calibration_thresholds": {
                    "minimum_iou": 0.8,
                    "maximum_normalized_center_shift": 0.03,
                    "minimum_area_ratio": 0.85,
                    "maximum_area_ratio": 1.2,
                    "minimum_sample_count": 12
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{saved_policy:#?}");
        assert_eq!(
            saved_policy["policy"]["calibration_thresholds"]["minimum_sample_count"],
            json!(12)
        );

        let project_path = application
            .project_path("geometry-api")
            .expect("Project path");
        let project_id = stable_project_id(project_path.parent().expect("Project root"));
        let report = annotagent_core::evaluate_geometry_calibration(
            annotagent_core::GeometryCalibrationKey {
                project_id,
                task_id: annotagent_core::TaskId::from("objects"),
                label_id: Some(annotagent_core::LabelId::from("ball")),
                model_profile_id: annotagent_core::ModelProfileId::new(),
                model_profile_revision: 1,
                node_definition_id: "vlm_detection.detect".to_owned(),
                node_config_hash: "node-v1".to_owned(),
                prompt_version: Some("prompt-v1".to_owned()),
                preprocessing_hash: "preprocess-v1".to_owned(),
                dataset_profile_revision: "dataset-v1".to_owned(),
                label_schema_hash: "labels-v1".to_owned(),
                refinement_hash: "refinement-v1".to_owned(),
            },
            annotagent_core::GeometryCalibrationThresholds::default(),
            &[],
            0,
            chrono::Utc::now(),
        );
        application
            .store()
            .save_geometry_calibration(&report)
            .expect("calibration report");

        let (status, listed) = call_json(
            &service,
            axum::http::Method::GET,
            "/api/projects/geometry-api/geometry-calibrations",
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed["calibrations"][0]["report"]["id"], json!(report.id));
        assert_eq!(
            listed["calibrations"][0]["effective_status"],
            json!("stale"),
            "a report without a matching current Published Workflow fails closed"
        );

        let (status, detail) = call_json(
            &service,
            axum::http::Method::GET,
            &format!("/api/geometry-calibrations/{}", report.id),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detail["calibration"]["id"], json!(report.id));

        let (status, _) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/projects/geometry-api/geometry-calibrations",
            json!({
                "workflow_id": "missing",
                "workflow_version": 1,
                "node_id": "detector",
                "task_id": "objects",
                "label_id": "ball",
                "evidence_run_ids": []
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn discovered_worker_identity_is_authoritative_for_registration() {
        let mut worker = legacy_http_sam_fixture();
        let mut manifest = worker.expert_manifest().expect("SAM Manifest");
        manifest.model_version = "sam2.1-test-v1".to_owned();
        manifest.checkpoint = Some(annotagent_core::CheckpointIdentity {
            sha256: "a".repeat(64),
            source: Some("owner-supplied checkpoint".to_owned()),
            training_dataset_version: Some("sam2.1-upstream".to_owned()),
        });
        manifest.license.weight_license = Some("owner-verified terms".to_owned());
        manifest.availability = ModelAvailability::Unknown;
        manifest.availability_evidence.weights_ready = true;

        assert!(
            reconcile_discovered_worker_identity(&mut worker, &manifest)
                .expect("live identity should reconcile")
        );
        assert_eq!(worker.version.model_version, "sam2.1-test-v1");
        assert_eq!(worker.version.checkpoint_sha256, Some("a".repeat(64)));
        assert_eq!(
            worker.license.weight_license.as_deref(),
            Some("owner-verified terms")
        );

        worker.version.model_version = "forged-local-version".to_owned();
        assert!(reconcile_discovered_worker_identity(&mut worker, &manifest).is_err());
        worker.version.model_version = manifest.model_version.clone();
        worker.version.checkpoint_sha256 = Some("b".repeat(64));
        assert!(reconcile_discovered_worker_identity(&mut worker, &manifest).is_err());

        worker.version.checkpoint_sha256 = Some("a".repeat(64));
        manifest.availability_evidence.weights_ready = false;
        assert!(
            !reconcile_discovered_worker_identity(&mut worker, &manifest)
                .expect("unready live identity remains non-publishable")
        );
    }

    #[tokio::test]
    async fn legacy_registry_import_requires_confirmation_and_is_idempotent() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        app.create_project(
            "legacy-import",
            r"
version: 1
project:
  name: Legacy import
  language: en
dataset:
  root: images
runtime: {}
tasks: []
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
",
        )
        .expect("Project");
        let service = router(
            test_state(app.clone(), Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let (status, preview) = call_json(
            &service,
            axum::http::Method::GET,
            "/api/registry-migrations/legacy",
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(preview["migration"]["already_applied"], json!(false));
        assert_eq!(preview["migration"]["project_binding_count"], json!(1));
        assert_eq!(preview["migration"]["moves_secret"], json!(false));
        assert_eq!(
            preview["migration"]["modifies_historical_runs"],
            json!(false)
        );

        let (status, _) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/registry-migrations/legacy",
            json!({"confirmed": false}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            app.store()
                .list_provider_profiles()
                .expect("Providers")
                .len(),
            1,
            "the only Provider is the built-in Registry Mock"
        );

        let (status, imported) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/registry-migrations/legacy",
            json!({"confirmed": true}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(imported["secret_moved"], json!(false));
        assert_eq!(imported["historical_runs_modified"], json!(false));
        assert_eq!(imported["migration"]["bindings_created"], json!(1));
        assert_eq!(imported["migration"]["historical_runs_modified"], json!(0));
        assert_eq!(
            app.store()
                .list_provider_profiles()
                .expect("Providers")
                .len(),
            2
        );
        assert_eq!(
            app.store()
                .list_model_profiles(None, false)
                .expect("Models")
                .len(),
            6
        );

        let (_, repeated) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/registry-migrations/legacy",
            json!({"confirmed": true}),
        )
        .await;
        assert_eq!(repeated["migration"]["already_applied"], json!(true));
    }

    #[tokio::test]
    async fn draft_runtime_resolves_the_frozen_profile_credential_without_exposing_it() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let secrets = Arc::new(InMemorySecretStore::default());
        let state = test_state(app.clone(), secrets.clone()).await;
        let now = Utc::now();
        let provider_id = ProviderId::new();
        let credential_ref = secrets
            .put(
                SecretScope {
                    provider_id,
                    source: CredentialSource::SessionOnly,
                    locator: "draft-runtime-session".to_owned(),
                },
                SecretValue::new("draft-runtime-secret").expect("secret"),
            )
            .await
            .expect("credential reference");
        let provider = ProviderProfile {
            id: provider_id,
            display_name: "Draft Runtime Provider".to_owned(),
            preset_id: Some("custom".to_owned()),
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: "https://provider.example/v1".parse().expect("URL"),
            organization: None,
            workspace: None,
            credential_ref: Some(credential_ref),
            safe_headers: BTreeMap::new(),
            connection_policy: ProviderConnectionPolicy::default(),
            enabled: true,
            health: ProviderHealthSnapshot {
                status: ProviderHealthStatus::Available,
                safe_message: None,
                checked_at: Some(now),
            },
            created_at: now,
            updated_at: now,
        };
        app.store()
            .save_provider_profile(&provider)
            .expect("Provider");
        let model = ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id,
            display_name: "Draft Runtime Model".to_owned(),
            remote_model_id: "remote-draft-model".to_owned(),
            input_modalities: BTreeSet::from([InputModality::Image]),
            protocol_features: ProtocolFeatures::default(),
            task_capabilities: BTreeSet::from([ModelCapability::ImageClassification]),
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
        let frozen = ModelProfileSnapshot::frozen(&model, &provider).expect("snapshot");

        let (provider_kind, credential) = resolve_runtime_model_profiles(&state, &[frozen], true)
            .await
            .expect("resolved Draft Runtime");
        assert_eq!(provider_kind, "openai_compatible");
        assert_eq!(credential.as_deref(), Some("draft-runtime-secret"));

        let serialized = serde_json::to_string(&provider).expect("Provider JSON");
        assert!(!serialized.contains("draft-runtime-secret"));
    }

    #[tokio::test]
    async fn registry_api_keeps_credentials_write_only_and_records_confirmed_probes() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            test_state(app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let (status, remote_provider) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/providers",
            json!({
                "display_name": "Remote fixture",
                "preset_id": "custom",
                "adapter": "open_ai_compatible",
                "base_url": "https://provider.invalid/v1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let remote_id = remote_provider["id"].as_str().expect("provider id");
        let (status, invalid_environment) = call_json(
            &service,
            axum::http::Method::POST,
            &format!("/api/providers/{remote_id}/credential"),
            json!({
                "source": "environment_variable",
                "environment_variable": "not-a-variable-name"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            invalid_environment["error"],
            json!(
                "Enter an environment variable name such as DASHSCOPE_API_KEY, not the API key itself. To paste a key directly, choose the workspace-file credential source."
            )
        );
        let (status, credential) = call_json(
            &service,
            axum::http::Method::POST,
            &format!("/api/providers/{remote_id}/credential"),
            json!({"source": "system_keyring", "secret": "registry-fixture-secret"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(credential["credential_configured"], json!(true));
        assert!(!credential.to_string().contains("registry-fixture-secret"));
        let (status, fetched) = call_json(
            &service,
            axum::http::Method::GET,
            &format!("/api/providers/{remote_id}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched["credential_configured"], json!(true));
        assert!(fetched.get("credential_ref").is_none());
        assert!(!fetched.to_string().contains("provider-"));

        let (status, mock_provider) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/providers",
            json!({
                "display_name": "Offline fixture",
                "preset_id": "mock",
                "adapter": "mock",
                "base_url": "http://127.0.0.1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let mock_id = mock_provider["id"].as_str().expect("mock id");
        let (status, model) = call_json(
            &service,
            axum::http::Method::POST,
            "/api/model-profiles",
            json!({
                "provider_id": mock_id,
                "display_name": "Mock builder",
                "remote_model_id": "mock-builder",
                "input_modalities": ["text", "image"],
                "task_capabilities": ["text_generation", "vision_language"],
                "protocol_features": {
                    "tool_calls": true,
                    "structured_output": true
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let model_id = model["id"].as_str().expect("model id");
        let (status, quality_contracts) = call_json(
            &service,
            axum::http::Method::GET,
            &format!("/api/model-profiles/{model_id}/quality-contracts"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let vlm_contract = quality_contracts["contracts"]
            .as_array()
            .and_then(|contracts| {
                contracts
                    .iter()
                    .find(|contract| contract["operation"] == "vlm_detection.detect")
            })
            .expect("default VLM quality contract");
        assert_eq!(vlm_contract["output_geometry"], json!("coarse_hypothesis"));
        assert_eq!(
            vlm_contract["score_semantics"],
            json!("semantic_confidence")
        );
        assert_eq!(
            vlm_contract["auto_accept_eligibility"],
            json!("never_from_score_alone")
        );
        assert_eq!(vlm_contract["evidence_source"], json!("system_default"));
        let (status, updated_model) = call_json(
            &service,
            axum::http::Method::PATCH,
            &format!("/api/model-profiles/{model_id}"),
            json!({
                "quality_contracts": [{
                    "capability": "vision_language",
                    "operation": "vlm_detection.detect",
                    "output_geometry": "coarse_hypothesis",
                    "score_semantics": "relative_confidence",
                    "auto_accept_eligibility": "never_from_score_alone",
                    "small_object_localization": "unknown",
                    "requires_geometry_verification": true
                }]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated_model["revision"], json!(2));
        assert_eq!(
            updated_model["quality_contracts"][0]["evidence_source"],
            json!("user_declared")
        );
        let (status, declared_contracts) = call_json(
            &service,
            axum::http::Method::GET,
            &format!("/api/model-profiles/{model_id}/quality-contracts"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(declared_contracts["model_profile_revision"], json!(2));
        assert_eq!(
            declared_contracts["contracts"][0]["evidence_source"],
            json!("user_declared")
        );
        let (status, error) = call_json(
            &service,
            axum::http::Method::POST,
            &format!("/api/providers/{mock_id}/active-probe"),
            json!({"model_profile_id": model_id, "confirmed_billable": false}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            error["error"]
                .as_str()
                .is_some_and(|value| value.contains("may incur"))
        );
        let (status, probe) = call_json(
            &service,
            axum::http::Method::POST,
            &format!("/api/providers/{mock_id}/active-probe"),
            json!({"model_profile_id": model_id, "confirmed_billable": true}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(probe["billable"], json!(true));
        assert_eq!(probe["usage"]["total_tokens"], json!(2));
        let (status, usage) = call_json(
            &service,
            axum::http::Method::GET,
            &format!("/api/model-profiles/{model_id}/usage"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(usage["active_probes"].as_array().map(Vec::len), Some(1));
        let (status, conflict) = call_json(
            &service,
            axum::http::Method::DELETE,
            &format!("/api/model-profiles/{model_id}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(conflict["code"], json!("model_profile_in_use"));
        let (status, conflict) = call_json(
            &service,
            axum::http::Method::DELETE,
            &format!("/api/providers/{mock_id}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(conflict["code"], json!("provider_in_use"));
    }

    #[tokio::test]
    async fn workspace_file_registry_credential_survives_server_restart() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let first_state = ServerState::new(application).await.expect("state");
        let first_service = router(first_state.clone(), None);
        let (status, provider) = call_json(
            &first_service,
            axum::http::Method::POST,
            "/api/providers",
            json!({
                "display_name": "Persistent fixture",
                "preset_id": "custom",
                "adapter": "open_ai_compatible",
                "base_url": "https://provider.invalid/v1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let provider_id = provider["id"].as_str().expect("provider id");
        let (status, saved) = call_json(
            &first_service,
            axum::http::Method::POST,
            &format!("/api/providers/{provider_id}/credential"),
            json!({"source": "workspace_file", "secret": "persistent-fixture-secret"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(saved["credential_source"], json!("workspace_file"));
        assert!(!saved.to_string().contains("persistent-fixture-secret"));
        let (status, session_provider) = call_json(
            &first_service,
            axum::http::Method::POST,
            "/api/providers",
            json!({
                "display_name": "Expired session fixture",
                "preset_id": "custom",
                "adapter": "open_ai_compatible",
                "base_url": "https://session-provider.invalid/v1"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let session_provider_id = session_provider["id"].as_str().expect("provider id");
        let (status, _) = call_json(
            &first_service,
            axum::http::Method::POST,
            &format!("/api/providers/{session_provider_id}/credential"),
            json!({"source": "session_only", "secret": "session-fixture-secret"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        drop(first_service);
        drop(first_state);

        let restarted_application =
            Arc::new(LocalApplication::new(temp.path()).expect("restarted application"));
        let restarted_state = ServerState::new(restarted_application)
            .await
            .expect("restarted state");
        let parsed_provider_id = parse_provider_id(provider_id).expect("provider id");
        let profile = restarted_state
            .application
            .store()
            .get_provider_profile(parsed_provider_id)
            .expect("persisted provider");
        let resolved = resolve_provider_credential(&restarted_state, &profile)
            .await
            .expect("resolve after restart")
            .expect("credential");
        assert_eq!(resolved.expose_secret(), "persistent-fixture-secret");

        let restarted_service = router(restarted_state, None);
        let (status, fetched) = call_json(
            &restarted_service,
            axum::http::Method::GET,
            &format!("/api/providers/{provider_id}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched["credential_configured"], json!(true));
        assert_eq!(fetched["credential_source"], json!("workspace_file"));
        assert!(!fetched.to_string().contains("persistent-fixture-secret"));

        let (status, expired) = call_json(
            &restarted_service,
            axum::http::Method::GET,
            &format!("/api/providers/{session_provider_id}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(expired["credential_configured"], json!(false));
        assert_eq!(expired["credential_source"], json!("session_only"));
        assert_eq!(expired["health"]["status"], json!("unknown"));
        assert!(
            expired["health"]["safe_message"]
                .as_str()
                .is_some_and(|message| message.contains("cleared when the server stopped"))
        );
        let (status, error) = call_json(
            &restarted_service,
            axum::http::Method::POST,
            &format!("/api/providers/{session_provider_id}/check"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            error["error"]
                .as_str()
                .is_some_and(|message| message.contains("choose Local workspace file"))
        );
    }

    #[tokio::test]
    async fn health_works_and_traversal_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            test_state(app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let response = service
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body");
        let health: Value = serde_json::from_slice(&body).expect("health JSON");
        assert_eq!(health["service"], json!("AnnotAgent"));
        let response = service
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/projects/..%2Fsecret")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert!(matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
        ));
    }

    #[tokio::test]
    #[ignore = "known P0 integrity failure: permissive CORS accepts an untrusted origin"]
    async fn integrity_rejects_cross_origin_preflight_for_mutations() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            test_state(app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let response = service
            .oneshot(
                axum::http::Request::builder()
                    .method(axum::http::Method::OPTIONS)
                    .uri("/api/settings")
                    .header(axum::http::header::ORIGIN, "https://untrusted.example")
                    .header(
                        axum::http::header::ACCESS_CONTROL_REQUEST_METHOD,
                        axum::http::Method::PUT.as_str(),
                    )
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(
            response
                .headers()
                .get(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none(),
            "untrusted origins must not receive an allow-origin header"
        );
    }

    #[tokio::test]
    #[ignore = "known P0 integrity failure: health response leaks local absolute paths"]
    async fn integrity_health_response_does_not_expose_local_paths() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            test_state(app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let response = service
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body");
        let health: Value = serde_json::from_slice(&body).expect("health JSON");

        assert!(health.get("workspace").is_none());
        assert!(health.get("database").is_none());
    }

    #[tokio::test]
    async fn pipeline_improvement_http_surface_is_registered_and_project_scoped() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        app.create_project(
            "improvement-api",
            include_str!("../../../examples/robocup/project.yaml"),
        )
        .expect("Project");
        let service = router(
            test_state(app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let response = request(
            &service,
            axum::http::Method::GET,
            "/api/projects/improvement-api/pipeline-improvements",
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["pipeline_improvements"], json!([]));

        let response = request(
            &service,
            axum::http::Method::GET,
            "/api/pipeline-improvements/not-a-uuid",
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn guidance_readiness_and_summary_are_server_owned() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        app.create_project(
            "guided-api",
            include_str!(
                "../../../examples/label-pipelines/whole-image-classification/project.yaml"
            ),
        )
        .expect("Project");
        let service = router(
            test_state(app, Arc::new(InMemorySecretStore::default())).await,
            None,
        );

        let guidance = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/guided-api/guidance",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(guidance["stage"], json!("needs_data"));
        assert_eq!(guidance["primary_action"]["kind"], json!("add_images"));
        assert_eq!(guidance["journey"].as_array().map(Vec::len), Some(8));
        assert_eq!(guidance["journey"][0]["state"], json!("current"));
        assert_eq!(
            guidance["journey"][0]["detail"],
            json!("Add at least one supported image")
        );
        assert_eq!(
            guidance["primary_action"]["destination"],
            json!("/projects/guided-api/build/data")
        );

        let readiness = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/guided-api/readiness",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(readiness["readiness"], json!("incomplete"));
        assert_eq!(readiness["stage"], guidance["stage"]);

        let summary = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/guided-api/summary",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(summary["project"]["id"], json!("guided-api"));
        assert_eq!(summary["guidance"], guidance);
        assert_eq!(summary["readiness"], readiness);

        let incoming = temp.path().join("incoming.png");
        annotagent_image_tools::generate_synthetic_inspection(&incoming).expect("incoming image");
        let imported = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/guided-api/import",
                Some(json!({ "source": incoming })),
            )
            .await,
        )
        .await;
        assert_eq!(imported["discovered"], json!(1));
        assert_eq!(imported["imported"], json!(1));
        assert_eq!(imported["corrupt"], json!([]));
        let images = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/guided-api/images",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(images["images"][0]["path"], json!("images/incoming.png"));
        assert!(
            images["images"][0]["size_bytes"]
                .as_u64()
                .unwrap_or_default()
                > 0
        );
        let removed = response_json(
            request(
                &service,
                axum::http::Method::DELETE,
                "/api/projects/guided-api/images/0",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(removed["removed"], json!("incoming.png"));
    }

    #[tokio::test]
    async fn workflow_designer_http_journey_validates_dry_runs_publishes_and_clones() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            test_state(
                application.clone(),
                Arc::new(InMemorySecretStore::default()),
            )
            .await,
            None,
        );
        let skill = application.skills().get("robocup").expect("skill");
        let project_yaml = skill.project_template().expect("template");
        assert_eq!(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects",
                Some(json!({"id": "workflow-ui", "yaml": project_yaml})),
            )
            .await
            .status(),
            StatusCode::CREATED
        );
        generate_synthetic_robocup(&temp.path().join("workflow-ui/images/sample.png"))
            .expect("sample image");

        let catalog = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/workflow-ui/workflow-catalog",
                None,
            )
            .await,
        )
        .await;
        assert!(
            catalog["node_catalog"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
        let public_node_ids = catalog["node_catalog"]
            .as_array()
            .expect("node catalog")
            .iter()
            .filter_map(|node| node["id"].as_str())
            .collect::<BTreeSet<_>>();
        assert!(public_node_ids.contains("core.resize"));
        assert!(public_node_ids.contains("core.tile"));
        assert!(public_node_ids.contains("core.project_coordinates"));
        assert!(public_node_ids.contains("capability.detect"));
        assert!(!public_node_ids.contains("core.artifact_cache"));
        assert!(!public_node_ids.contains("core.filter"));
        assert!(
            catalog["runtime_policies"]
                .as_array()
                .is_some_and(|policies| policies.iter().any(|policy| policy["id"] == "cache"))
        );
        assert_eq!(catalog["model_registry"][0]["id"], json!("default-vision"));
        assert_eq!(
            catalog["workflow_templates"].as_array().map(Vec::len),
            Some(1)
        );
        let hybrid_draft = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/workflow-drafts",
                Some(json!({
                    "project_id": "workflow-ui",
                    "template_id": "robocup.ball.vlm-bootstrap"
                })),
            )
            .await,
        )
        .await;
        assert_eq!(hybrid_draft["name"], json!("RoboCup Ball · VLM bootstrap"));
        assert_eq!(hybrid_draft["enabled_skills"]["robocup"], json!("1"));

        let suggestion = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/workflow-drafts/suggest",
                Some(json!({"project_id": "workflow-ui", "constraints": {"require_review_gate": true}})),
            )
            .await,
        )
        .await;
        let mut draft = suggestion["draft"].clone();
        let draft_id = draft["id"].as_str().expect("draft id").to_owned();
        let node_index = draft["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .position(|node| {
                node["inputs"]
                    .as_array()
                    .is_some_and(|ports| !ports.is_empty())
            })
            .expect("typed input node");
        let original_type = draft["nodes"][node_index]["inputs"][0]["artifact_type"].clone();
        draft["nodes"][node_index]["inputs"][0]["artifact_type"] = json!("relations");
        assert_eq!(
            request(
                &service,
                axum::http::Method::PATCH,
                &format!("/api/workflow-drafts/{draft_id}"),
                Some(draft.clone()),
            )
            .await
            .status(),
            StatusCode::OK
        );
        let invalid = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/workflow-drafts/{draft_id}/dry-run"),
                Some(json!({"image_indices": [0]})),
            )
            .await,
        )
        .await;
        assert_eq!(invalid["validation"]["valid"], json!(false));
        assert!(
            invalid["validation"]["issues"]
                .as_array()
                .is_some_and(|issues| {
                    issues
                        .iter()
                        .any(|issue| issue["code"] == "artifact_type_mismatch")
                })
        );

        draft["nodes"][node_index]["inputs"][0]["artifact_type"] = original_type;
        let saved = response_json(
            request(
                &service,
                axum::http::Method::PATCH,
                &format!("/api/workflow-drafts/{draft_id}"),
                Some(draft),
            )
            .await,
        )
        .await;
        assert_eq!(saved["status"], json!("editing"));
        let dry_run = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/workflow-drafts/{draft_id}/dry-run"),
                Some(json!({"image_indices": [0]})),
            )
            .await,
        )
        .await;
        assert_eq!(dry_run["validation"]["valid"], json!(true));
        assert_eq!(dry_run["sandbox"], json!(true));
        assert_eq!(dry_run["samples"][0]["image_name"], json!("sample.png"));

        let restored_sample_test = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/workflow-drafts/{draft_id}/sample-test"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(restored_sample_test["current"], json!(true));
        assert_eq!(
            restored_sample_test["sample_test"]["report"]["samples"][0]["image_name"],
            json!("sample.png")
        );
        let drafts_after_sample_test = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/workflow-drafts?project_id=workflow-ui",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            drafts_after_sample_test["latest_current_sample_test_draft_id"],
            json!(draft_id)
        );

        let mut changed_after_test = saved;
        changed_after_test["name"] = json!("Changed after Sample Test");
        assert_eq!(
            request(
                &service,
                axum::http::Method::PATCH,
                &format!("/api/workflow-drafts/{draft_id}"),
                Some(changed_after_test),
            )
            .await
            .status(),
            StatusCode::OK
        );
        let stale_sample_test = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/workflow-drafts/{draft_id}/sample-test"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(stale_sample_test["current"], json!(false));
        assert!(stale_sample_test["sample_test"].is_object());
        let drafts_after_change = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/workflow-drafts?project_id=workflow-ui",
                None,
            )
            .await,
        )
        .await;
        assert!(drafts_after_change["latest_current_sample_test_draft_id"].is_null());
        let refreshed_dry_run = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/workflow-drafts/{draft_id}/dry-run"),
                Some(json!({"image_indices": [0]})),
            )
            .await,
        )
        .await;
        assert_eq!(refreshed_dry_run["validation"]["valid"], json!(true));

        let published = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/workflow-drafts/{draft_id}/publish"),
                None,
            )
            .await,
        )
        .await;
        let workflow_id = published["workflow_id"].as_str().expect("workflow id");
        let version = published["version"].as_u64().expect("version");
        let activated_sample_test = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/workflow-drafts/{draft_id}/sample-test"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(activated_sample_test["current"], json!(true));
        assert_eq!(
            activated_sample_test["sample_test"]["report"]["validation"]["valid"],
            json!(true)
        );
        let project = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/workflow-ui",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            project["active_workflow"]["workflow_id"],
            json!(workflow_id)
        );
        assert_eq!(
            project["active_workflow"]["version"],
            json!(version.to_string())
        );

        let clone = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/workflows/{workflow_id}/versions/{version}/clone"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(clone["status"], json!("editing"));
        assert_ne!(clone["id"], json!(draft_id));

        let started = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/workflow-ui/runs",
                Some(json!({
                    "workflow_id": workflow_id,
                    "version": version
                })),
            )
            .await,
        )
        .await;
        assert!(started["run_id"].as_str().is_some());
        let mut run = Value::Null;
        for _ in 0..100 {
            let runs =
                response_json(request(&service, axum::http::Method::GET, "/api/runs", None).await)
                    .await;
            run = runs["runs"]
                .as_array()
                .and_then(|runs| runs.first())
                .cloned()
                .unwrap_or(Value::Null);
            if run["status"].as_str().is_some_and(|status| {
                !matches!(status, "pending" | "running" | "paused" | "awaiting_review")
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(run["workflow_name"], published["draft"]["name"]);
        assert_eq!(run["workflow_version"], json!(version.to_string()));
        assert!(
            run["artifact_count"]
                .as_u64()
                .is_some_and(|count| count > 0)
        );
        assert_eq!(run["checkpoint_present"], json!(true));
        assert_eq!(run["model_identity"], json!("mock/mock-vision"));
        assert!(run["current_node"].as_str().is_some());
        assert!(run["current_node_status"].as_str().is_some());
        assert!(run["validation_issue_codes"].as_array().is_some());
        assert!(run["fallback_nodes"].as_array().is_some());

        let batch_started = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/workflow-ui/batches",
                Some(json!({
                    "workflow_id": workflow_id,
                    "version": version
                })),
            )
            .await,
        )
        .await;
        assert_eq!(
            batch_started["batch"]["workflow_version"],
            json!(format!("{workflow_id}@{version}"))
        );
        assert_eq!(
            batch_started["batch"]["workflow_snapshot"]["published_workflow"]["content_hash"],
            published["content_hash"]
        );
        let batch_id = batch_started["batch"]["id"].as_str().expect("batch id");
        for _ in 0..100 {
            let detail = response_json(
                request(
                    &service,
                    axum::http::Method::GET,
                    &format!("/api/batches/{batch_id}"),
                    None,
                )
                .await,
            )
            .await;
            if detail["batch"]["status"]
                .as_str()
                .is_some_and(|status| !matches!(status, "pending" | "running"))
            {
                assert_eq!(detail["progress"]["completed_images"], json!(1));
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let runs =
            response_json(request(&service, axum::http::Method::GET, "/api/runs", None).await)
                .await;
        assert_eq!(
            runs["runs"]
                .as_array()
                .expect("Run summaries")
                .iter()
                .filter(|run| run["checkpoint_present"] == json!(true))
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn project_sse_review_revision_and_budget_flow_works_over_http() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let state = test_state(
            application.clone(),
            Arc::new(InMemorySecretStore::default()),
        )
        .await;
        let service = router(state.clone(), None);
        let skill = application
            .skills()
            .get("robocup")
            .expect("registered test skill");
        let project_yaml = skill
            .project_template()
            .expect("project template")
            .replace("max_retries: 3", "max_retries: 0");
        let response = request(
            &service,
            axum::http::Method::POST,
            "/api/projects",
            Some(json!({"id": "review-demo", "yaml": project_yaml})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let project_path = application
            .project_path("review-demo")
            .expect("review Project path");

        let dashboard =
            response_json(request(&service, axum::http::Method::GET, "/api/projects", None).await)
                .await;
        let project = &dashboard["projects"][0];
        assert_eq!(project["name"], json!("RoboCup Ball Demo"));
        assert_eq!(project["enabled_skills"][0]["id"], json!("robocup"));
        assert_eq!(
            project["active_workflow"]["name"],
            json!("Unpublished Project task graph")
        );
        assert_eq!(project["active_workflow"]["status"], json!("draft"));
        assert!(project["default_workflow_version"].is_null());
        assert!(
            project["model_bindings"]
                .as_array()
                .is_some_and(|bindings| {
                    bindings.iter().any(|binding| {
                        binding["model"] == json!("Mock Vision Language (offline)")
                            && binding["scope"] == json!("registry_profile@1")
                    })
                })
        );

        let workflows =
            response_json(request(&service, axum::http::Method::GET, "/api/workflows", None).await)
                .await;
        assert_eq!(
            workflows["workflows"][0]["project_id"],
            json!("review-demo")
        );
        assert!(
            workflows["workflows"][0]["workflow"]["nodes"]
                .as_array()
                .is_some_and(|nodes| !nodes.is_empty())
        );

        let models =
            response_json(request(&service, axum::http::Method::GET, "/api/models", None).await)
                .await;
        assert_eq!(models["models"][0]["provider"], json!("mock"));
        assert_eq!(models["models"][0]["health_status"], json!("healthy"));
        assert_eq!(models["models"][0]["availability_group"], json!("ready"));
        assert_eq!(models["models"].as_array().map(Vec::len), Some(1));

        let sse = request(&service, axum::http::Method::GET, "/api/events", None).await;
        assert_eq!(sse.status(), StatusCode::OK);
        let mut event_stream = sse.into_body().into_data_stream();

        let started = application
            .start_run_path_with_settings(
                &project_path,
                "mock",
                state.settings.read().await.clone(),
                None,
            )
            .expect("low-level compatibility Run used only by this history test");
        let run_id = started.run_id.to_string();
        let first_event = tokio::time::timeout(Duration::from_secs(2), event_stream.next())
            .await
            .expect("SSE timeout")
            .expect("SSE item")
            .expect("SSE body");
        assert!(String::from_utf8_lossy(&first_event).contains("run_"));

        wait_for_status(&application, &run_id, RunStatus::CompletedWithReview).await;
        let runs =
            response_json(request(&service, axum::http::Method::GET, "/api/runs", None).await)
                .await;
        assert_eq!(runs["runs"][0]["workflow_version"], json!("1"));
        assert_eq!(runs["runs"][0]["skill_versions"][0], json!("robocup@1"));
        assert_eq!(
            runs["runs"][0]["model_bindings"][0]["scope"],
            json!("run_snapshot")
        );
        let run_response = request(
            &service,
            axum::http::Method::GET,
            &format!("/api/runs/{run_id}"),
            None,
        )
        .await;
        assert_eq!(run_response.status(), StatusCode::OK);
        let events = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/runs/{run_id}/events"),
                None,
            )
            .await,
        )
        .await;
        assert!(
            events["events"]
                .as_array()
                .is_some_and(|events| !events.is_empty())
        );

        let reviews =
            response_json(request(&service, axum::http::Method::GET, "/api/reviews", None).await)
                .await;
        let review_id = reviews["reviews"][0]["id"].as_str().expect("review id");
        assert_eq!(reviews["reviews"][0]["run_id"], json!(run_id));
        assert!(reviews["reviews"][0]["workflow_version"].is_number());
        assert!(reviews["reviews"][0]["review_reason"].is_string());
        assert!(reviews["reviews"][0]["review_explanation"].is_object());
        assert!(reviews["reviews"][0]["detection_evidence"].is_array());
        assert!(reviews["reviews"][0]["refinement_chain"].is_array());
        assert!(reviews["reviews"][0]["validation_issues"].is_array());
        assert_eq!(reviews["progress"]["reviewed_count"], json!(0));
        assert_eq!(reviews["progress"]["remaining_count"], json!(1));
        assert_eq!(reviews["progress"]["total_count"], json!(1));
        let unknown_reason = request(
            &service,
            axum::http::Method::POST,
            &format!("/api/reviews/{review_id}/decision"),
            Some(json!({
                "project_id": "review-demo",
                "decision": "accept",
                "reason_code": "unregistered_free_form_reason"
            })),
        )
        .await;
        assert_eq!(unknown_reason.status(), StatusCode::BAD_REQUEST);
        let untouched = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/reviews/{review_id}"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            untouched["annotation"]["review_status"],
            json!("needs_review")
        );
        let navigation = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/reviews/{review_id}/next?project_id=review-demo"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(navigation["progress"]["current_position"], json!(1));
        assert!(navigation["previous_review"].is_null());
        assert!(navigation["next_review"].is_null());
        let import_directory = temp.path().join("review-demo/import");
        std::fs::create_dir_all(&import_directory).expect("import directory");
        let import_file = import_directory.join("labels.json");
        std::fs::write(
            &import_file,
            serde_json::to_vec(&json!({
                "imagePath": "synthetic-robocup.png",
                "imageWidth": 640,
                "imageHeight": 400,
                "shapes": [{
                    "label": "ball",
                    "shape_type": "rectangle",
                    "points": [[100, 100], [150, 150]]
                }]
            }))
            .expect("LabelMe JSON"),
        )
        .expect("LabelMe fixture");
        let preview = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/review-demo/annotation-import",
                Some(json!({
                    "format": "labelme",
                    "source": import_file,
                    "dry_run": true
                })),
            )
            .await,
        )
        .await;
        assert_eq!(preview["imported_count"], json!(1));
        assert_eq!(preview["dry_run"], json!(true));
        let imported = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/review-demo/annotation-import",
                Some(json!({
                    "format": "labelme",
                    "source": import_file,
                    "dry_run": false
                })),
            )
            .await,
        )
        .await;
        assert_eq!(imported["imported_count"], json!(1));
        assert_eq!(imported["annotations"][0]["source"], json!("imported"));
        let imported_review_id = imported["annotations"][0]["id"]
            .as_str()
            .expect("imported review id");
        let reviews_after_import =
            response_json(request(&service, axum::http::Method::GET, "/api/reviews", None).await)
                .await;
        assert_eq!(
            reviews_after_import["reviews"].as_array().map(Vec::len),
            reviews["reviews"]
                .as_array()
                .map(Vec::len)
                .map(|count| count + 1)
        );
        let mut human_annotation = reviews["reviews"][0]["annotation"].clone();
        let human_id = uuid::Uuid::new_v4().to_string();
        human_annotation["id"] = json!(human_id);
        human_annotation["confidence"] = json!(0.99);
        let created = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/runs/{run_id}/annotations"),
                Some(json!({"annotation": human_annotation})),
            )
            .await,
        )
        .await;
        assert_eq!(created["annotation"]["source"], json!("human"));
        assert_eq!(
            created["annotation"]["review_status"],
            json!("needs_review")
        );
        assert!(created["annotation"]["confidence"].is_null());
        let run_annotations = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/runs/{run_id}/annotations"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(run_annotations["project_id"], json!("review-demo"));
        assert!(run_annotations["image_index"].is_number());
        assert!(
            run_annotations["annotations"]
                .as_array()
                .is_some_and(|annotations| annotations.len() >= 2)
        );
        let reason_code = skill
            .correction_taxonomy()
            .into_iter()
            .next()
            .expect("correction taxonomy")
            .code;
        let decision = request(
            &service,
            axum::http::Method::POST,
            &format!("/api/reviews/{imported_review_id}/reject-and-next"),
            Some(json!({
                "project_id": "review-demo",
                "queue_project_id": "review-demo",
                "decision": "reject",
                "reason_code": reason_code,
                "note": "deterministic server test"
            })),
        )
        .await;
        assert_eq!(decision.status(), StatusCode::OK);
        let decision = response_json(decision).await;
        assert!(decision["next_review"].is_object());
        assert_eq!(decision["progress"]["reviewed_count"], json!(1));
        assert_eq!(decision["progress"]["remaining_count"], json!(2));
        assert_eq!(decision["progress"]["total_count"], json!(3));
        let memory = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/review-demo/correction-memory",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(memory["records"][0]["reason_code"], json!(reason_code));
        assert!(memory["records"][0]["project_id"].is_string());
        let revisions = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/annotations/{imported_review_id}/revisions"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(revisions["revisions"].as_array().map(Vec::len), Some(1));
        let mut adjusted_annotation = reviews["reviews"][0]["annotation"].clone();
        adjusted_annotation["value"]["rect"] = json!([0.56, 0.74, 0.03, 0.04]);
        let adjusted = response_json(
            request(
                &service,
                axum::http::Method::PATCH,
                &format!("/api/annotations/{review_id}"),
                Some(json!({
                    "annotation": adjusted_annotation,
                    "reason": "tighten_bbox"
                })),
            )
            .await,
        )
        .await;
        assert!(adjusted["geometry_metrics"]["manual_center_shift"].is_number());
        assert!(adjusted["geometry_metrics"]["manual_area_change"].is_number());
        let accepted = request(
            &service,
            axum::http::Method::POST,
            &format!("/api/reviews/{review_id}/accept-and-next"),
            Some(json!({
                "project_id": "review-demo",
                "queue_project_id": "review-demo",
                "decision": "accept",
                "reason_code": "too_loose",
                "note": "deterministic accept-and-next test"
            })),
        )
        .await;
        assert_eq!(accepted.status(), StatusCode::OK);
        let accepted = response_json(accepted).await;
        assert!(accepted["next_review"].is_object());
        assert_eq!(accepted["progress"]["reviewed_count"], json!(2));
        assert_eq!(accepted["progress"]["remaining_count"], json!(1));
        assert_eq!(accepted["progress"]["total_count"], json!(3));
        assert_eq!(
            accepted["geometry_quality"]["source"],
            json!("human_correction")
        );
        assert!(accepted["geometry_quality"]["iou"].is_number());
        assert!(accepted["geometry_quality"]["pixel_center_shift"].is_number());
        assert!(accepted["geometry_quality"]["area_ratio"].is_number());
        assert!(accepted["geometry_quality"]["width_ratio"].is_number());
        assert!(accepted["geometry_quality"]["height_ratio"].is_number());
        assert!(accepted["geometry_quality"]["size_bucket"].is_string());
        let geometry_history = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/review-demo/geometry-corrections",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            geometry_history["reports"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(
            geometry_history["evidence"][0]["reason"],
            json!("too_loose")
        );
        assert_eq!(
            geometry_history["summary"]["human_adjustment_count"],
            json!(1)
        );
        let run_geometry = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/runs/{run_id}/geometry-quality"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(run_geometry["reports"].as_array().map(Vec::len), Some(1));
        let memory_after_adjustment = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/review-demo/correction-memory",
                None,
            )
            .await,
        )
        .await;
        assert!(
            memory_after_adjustment["records"]
                .as_array()
                .is_some_and(|records| records.iter().any(|record| {
                    record["image_features"]["geometry"]["manual_center_shift"].is_number()
                        && record["image_features"]["geometry"]["manual_area_change"].is_number()
                }))
        );

        let mut settings =
            response_json(request(&service, axum::http::Method::GET, "/api/settings", None).await)
                .await;
        settings["budget"]["max_requests"] = json!(0);
        let settings_response = request(
            &service,
            axum::http::Method::PUT,
            "/api/settings",
            Some(settings),
        )
        .await;
        assert_eq!(settings_response.status(), StatusCode::OK);
        let budget_run = application
            .start_run_path_with_settings(
                &project_path,
                "mock",
                state.settings.read().await.clone(),
                None,
            )
            .expect("low-level budget compatibility Run");
        wait_for_status(
            &application,
            &budget_run.run_id.to_string(),
            RunStatus::BudgetExceeded,
        )
        .await;
    }

    #[tokio::test]
    async fn settings_and_keyring_reference_survive_server_restart() {
        let temp = tempfile::tempdir().expect("temp");
        let secrets = Arc::new(InMemorySecretStore::default());
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let provider_id = ProviderId(stable_project_id(application.workspace()).0);
        let reference = CredentialReference {
            provider_id,
            source: CredentialSource::SystemKeyring,
            locator: format!("restart-test-{provider_id}"),
        };
        let service = router(
            ServerState::with_secret_store(
                application,
                secrets.clone(),
                reference.clone(),
                reference.clone(),
            )
            .await
            .expect("state"),
            None,
        );

        let mut settings =
            response_json(request(&service, axum::http::Method::GET, "/api/settings", None).await)
                .await;
        let mut unsafe_settings = settings.clone();
        unsafe_settings["provider"]["custom_headers"] =
            json!({"Authorization": "Bearer must-not-be-persisted"});
        let rejected = request(
            &service,
            axum::http::Method::PUT,
            "/api/settings",
            Some(unsafe_settings),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
        assert!(
            !temp.path().join(".annotagent/settings.toml").exists(),
            "invalid secret-bearing provider metadata must be rejected before writing settings"
        );

        settings["default_provider"] = json!("openai_compatible");
        settings["provider"]["endpoint"] = json!("https://provider.example/v1");
        settings["provider"]["model"] = json!("persisted-vision-model");
        settings["api_key"] = json!("test-secret-that-must-not-reach-disk");
        let saved = response_json(
            request(
                &service,
                axum::http::Method::PUT,
                "/api/settings",
                Some(settings),
            )
            .await,
        )
        .await;
        assert_eq!(saved["settings_persisted"], json!(true));
        assert_eq!(saved["api_key_persisted"], json!(true));
        assert_eq!(saved["api_key_configured"], json!(true));
        assert_eq!(saved["credential_store"], json!("system_keyring"));
        assert!(saved.get("api_key").is_none());

        let settings_path = temp.path().join(".annotagent/settings.toml");
        let persisted = std::fs::read_to_string(&settings_path).expect("persisted settings");
        assert!(persisted.contains("persisted-vision-model"));
        assert!(!persisted.contains("test-secret-that-must-not-reach-disk"));
        assert_eq!(
            secrets
                .resolve(&reference)
                .await
                .expect("saved Keyring-style secret")
                .expose_secret(),
            "test-secret-that-must-not-reach-disk"
        );

        let restarted_application =
            Arc::new(LocalApplication::new(temp.path()).expect("restarted application"));
        let restarted = router(
            ServerState::with_secret_store(
                restarted_application,
                secrets.clone(),
                reference.clone(),
                reference.clone(),
            )
            .await
            .expect("state"),
            None,
        );
        let restored = response_json(
            request(&restarted, axum::http::Method::GET, "/api/settings", None).await,
        )
        .await;
        assert_eq!(restored["default_provider"], json!("openai_compatible"));
        assert_eq!(
            restored["provider"]["endpoint"],
            json!("https://provider.example/v1")
        );
        assert_eq!(restored["api_key_persisted"], json!(true));

        let mut clear_request = restored;
        clear_request["clear_saved_api_key"] = json!(true);
        let cleared = response_json(
            request(
                &restarted,
                axum::http::Method::PUT,
                "/api/settings",
                Some(clear_request),
            )
            .await,
        )
        .await;
        assert_eq!(cleared["api_key_configured"], json!(false));
        assert_eq!(cleared["api_key_persisted"], json!(false));
        assert!(!secrets.exists(&reference).await.expect("secret status"));
    }

    #[tokio::test]
    async fn compatibility_settings_rotate_an_existing_keyring_value_into_session_only() {
        let temp = tempfile::tempdir().expect("temp");
        let secrets = Arc::new(InMemorySecretStore::default());
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let provider_id = ProviderId(stable_project_id(application.workspace()).0);
        let keyring_reference = secrets
            .put(
                SecretScope {
                    provider_id,
                    source: CredentialSource::SystemKeyring,
                    locator: format!("old-keyring-{provider_id}"),
                },
                SecretValue::new("old-keyring-value").expect("old secret"),
            )
            .await
            .expect("old Keyring reference");
        let session_reference = CredentialReference {
            provider_id,
            source: CredentialSource::SessionOnly,
            locator: format!("new-session-{provider_id}"),
        };
        let service = router(
            ServerState::with_secret_store(
                application,
                secrets.clone(),
                keyring_reference.clone(),
                session_reference.clone(),
            )
            .await
            .expect("state"),
            None,
        );

        let mut settings =
            response_json(request(&service, axum::http::Method::GET, "/api/settings", None).await)
                .await;
        settings["api_key"] = json!("new-session-value");
        let saved = response_json(
            request(
                &service,
                axum::http::Method::PUT,
                "/api/settings",
                Some(settings),
            )
            .await,
        )
        .await;
        assert_eq!(saved["credential_store"], json!("session_only"));
        assert!(!saved.to_string().contains("new-session-value"));
        assert!(
            !secrets
                .exists(&keyring_reference)
                .await
                .expect("old key removed")
        );
        assert_eq!(
            secrets
                .resolve(&session_reference)
                .await
                .expect("session secret")
                .expose_secret(),
            "new-session-value"
        );
        let persisted = std::fs::read_to_string(temp.path().join(".annotagent/settings.toml"))
            .expect("settings file");
        assert!(!persisted.contains("new-session-value"));
    }

    #[tokio::test]
    async fn legacy_workspace_key_is_read_without_automatic_migration_or_deletion() {
        let temp = tempfile::tempdir().expect("temp");
        let credential_path = temp.path().join(".annotagent/credentials/provider-api-key");
        std::fs::create_dir_all(credential_path.parent().expect("credential parent"))
            .expect("credential directory");
        std::fs::write(&credential_path, "legacy-secret\n").expect("legacy credential");

        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let provider_id = ProviderId(stable_project_id(application.workspace()).0);
        let keyring_reference = CredentialReference {
            provider_id,
            source: CredentialSource::SystemKeyring,
            locator: format!("legacy-migration-test-{provider_id}"),
        };
        let legacy_reference = CredentialReference {
            provider_id,
            source: CredentialSource::LegacyWorkspaceFile,
            locator: "legacy-test-file".to_owned(),
        };
        let keyring = Arc::new(InMemorySecretStore::default());
        let store: Arc<dyn SecretStore> = Arc::new(SecretStoreRouter {
            keyring: keyring.clone(),
            environment: Arc::new(EnvironmentSecretStore),
            workspace: Arc::new(WorkspaceFileSecretStore::new(
                temp.path().join(".annotagent/credentials"),
            )),
            session: Arc::new(SessionSecretStore::default()),
            legacy: Arc::new(LegacyWorkspaceFileSecretStore::single(
                legacy_reference.locator.clone(),
                credential_path.clone(),
            )),
        });
        let state = ServerState::with_secret_store(
            application,
            store,
            legacy_reference,
            keyring_reference.clone(),
        )
        .await
        .expect("legacy state");
        assert_eq!(state.api_key.read().await.as_deref(), Some("legacy-secret"));
        assert!(credential_path.is_file(), "legacy secret remains in place");
        assert!(
            !keyring
                .exists(&keyring_reference)
                .await
                .expect("keyring status"),
            "legacy credential is never migrated implicitly"
        );
    }

    #[tokio::test]
    async fn duplicate_project_start_returns_structured_409_conflict() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        application
            .create_project(
                "duplicate-demo",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("project");
        let project_path = application
            .project_path("duplicate-demo")
            .expect("project path");
        let project_id = stable_project_id(project_path.parent().expect("project root"));
        let active_run_id = RunId::new();
        application
            .store()
            .reserve_project_run(project_id, active_run_id, None)
            .expect("active reservation");
        let service = router(
            test_state(
                application.clone(),
                Arc::new(InMemorySecretStore::default()),
            )
            .await,
            None,
        );
        let settings = annotagent_application::load_settings(None).expect("Settings");
        let draft = application
            .create_workflow_draft_with_template(
                "duplicate-demo",
                &settings,
                false,
                Some("robocup.ball.vlm-bootstrap"),
            )
            .expect("Workflow Draft");
        let published = application
            .publish_workflow(&draft.id, &settings)
            .expect("Published Workflow");
        let response = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/duplicate-demo/runs",
            Some(json!({
                "workflow_id": published.workflow_id,
                "version": published.version
            })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = response_json(response).await;
        assert_eq!(body["code"], json!("active_run_exists"));
        assert_eq!(body["active_run_id"], json!(active_run_id));
        assert_eq!(body["status"], json!("pending"));
    }

    #[tokio::test]
    async fn formal_execution_rejects_legacy_provider_fallback_requests() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        application
            .create_project(
                "registry-only",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("project");
        let service = router(
            test_state(application, Arc::new(InMemorySecretStore::default())).await,
            None,
        );

        let missing_version = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/registry-only/runs",
            Some(json!({})),
        )
        .await;
        assert_eq!(missing_version.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let legacy_provider = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/registry-only/batches",
            Some(json!({
                "provider": "mock",
                "workflow_id": "not-a-published-version",
                "version": 1
            })),
        )
        .await;
        assert_eq!(legacy_provider.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn batch_api_exposes_durable_progress_and_controls() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        application
            .create_project(
                "batch-api",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("project");
        generate_synthetic_robocup(&temp.path().join("batch-api/images/one.png")).expect("image");
        let batch = DatasetCoordinator::new(application.as_ref())
            .create(
                &temp.path().join("batch-api/project.yaml"),
                "mock",
                None,
                None,
            )
            .expect("batch");
        let service = router(
            test_state(application, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let listed =
            response_json(request(&service, axum::http::Method::GET, "/api/batches", None).await)
                .await;
        assert_eq!(listed["batches"][0]["id"], json!(batch.id));
        assert_eq!(listed["batches"][0]["progress"]["total_images"], json!(1));
        assert_eq!(listed["batches"][0]["child_run_ids"], json!([]));
        let detail = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/batches/{}", batch.id),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(detail["progress"]["total_images"], json!(1));
        let paused = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/batches/{}/pause", batch.id),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(paused["batch"]["status"], json!("paused"));
        let projects =
            response_json(request(&service, axum::http::Method::GET, "/api/projects", None).await)
                .await;
        assert_eq!(
            projects["projects"][0]["active_batch"]["id"],
            json!(batch.id)
        );
        assert_eq!(
            projects["projects"][0]["active_batch_progress"]["pending_images"],
            json!(1)
        );
        let duplicate_run = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/batch-api/runs",
            Some(json!({"workflow_id": "unused", "version": 1})),
        )
        .await;
        assert_eq!(duplicate_run.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(duplicate_run).await["code"],
            json!("active_batch_exists")
        );
        let duplicate_batch = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/batch-api/batches",
            Some(json!({"workflow_id": "unused", "version": 1})),
        )
        .await;
        assert_eq!(duplicate_batch.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(duplicate_batch).await["code"],
            json!("active_batch_exists")
        );
        let cancelled = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/batches/{}/cancel", batch.id),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(cancelled["batch"]["status"], json!("cancelled"));
    }

    #[tokio::test]
    async fn label_pipeline_http_advisor_dry_run_inspector_and_replay_are_real() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let state = test_state(
            application.clone(),
            Arc::new(InMemorySecretStore::default()),
        )
        .await;
        let service = router(state, None);
        let project_yaml = r"
version: 1
project:
  name: HTTP Label Pipeline
  language: en
dataset:
  root: images
runtime:
  max_parallel_images: 2
tasks:
  - id: scene
    kind: classification
    labels: [day, night]
    required: true
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
";
        let created = request(
            &service,
            axum::http::Method::POST,
            "/api/projects",
            Some(json!({"id": "http-label", "yaml": project_yaml})),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        generate_synthetic_inspection(&temp.path().join("http-label/images/sample.png"))
            .expect("sample image");
        let schema = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/http-label/schema/labels",
                Some(json!({"task_id": "scene", "label": "dawn"})),
            )
            .await,
        )
        .await;
        assert!(
            schema["annotation_schema"][0]["labels"]
                .as_array()
                .is_some_and(|labels| labels.contains(&json!("dawn")))
        );
        let added_task = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/http-label/schema/tasks",
                Some(json!({
                    "display_name": "Object Quality",
                    "kind": "classification",
                    "labels": ["usable", "reject"],
                    "attributes": {"occluded": {"type": "boolean", "required": false, "values": []}}
                })),
            )
            .await,
        )
        .await;
        assert!(
            added_task["annotation_schema"]
                .as_array()
                .is_some_and(|tasks| {
                    tasks.iter().any(|task| {
                        task["id"] == json!("object_quality")
                            && task["display_name"] == json!("Object Quality")
                    })
                })
        );

        let suggestion = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/workflow-drafts/suggest",
                Some(json!({
                    "project_id": "http-label",
                    "advisor": "mock",
                    "target_task_id": "scene",
                    "target_label": "day"
                })),
            )
            .await,
        )
        .await;
        assert_eq!(suggestion["draft"]["status"], json!("suggested"));
        assert_eq!(
            suggestion["draft"]["label_pipeline"]["label_pipelines"][0]["target_label"],
            json!("day")
        );
        assert_eq!(
            suggestion["agent_session"]["kind"],
            json!("pipeline_builder")
        );
        assert_eq!(
            suggestion["agent_session"]["status"],
            json!("waiting_for_human")
        );
        assert!(suggestion["agent_session"]["phase"].is_string());
        assert!(suggestion["agent_session"]["outcome"].is_string());
        assert!(suggestion["agent_session"]["total_tool_calls"].is_number());
        assert!(suggestion["agent_session"]["remaining_tool_calls"].is_number());
        assert!(suggestion["agent_session"]["reserved_finalization_calls"].is_number());
        assert!(suggestion["agent_session"]["draft_id"].is_string());
        assert!(suggestion["agent_session"]["next_action"].is_string());
        let sessions = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/http-label/agent-sessions",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(
            sessions["sessions"][0]["id"],
            suggestion["agent_session"]["id"]
        );
        let advisor_session_id = sessions["sessions"][0]["id"]
            .as_str()
            .expect("Advisor Session id");
        let cancelled = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/agent-sessions/{advisor_session_id}/cancel"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(cancelled["session"]["status"], json!("cancelled"));
        let draft_id = suggestion["draft"]["id"].as_str().expect("draft id");
        let dry_run = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/workflow-drafts/{draft_id}/dry-run"),
                Some(json!({"image_indices": [0]})),
            )
            .await,
        )
        .await;
        assert_eq!(dry_run["sandbox"], json!(true));
        assert_eq!(dry_run["validation"]["valid"], json!(true));
        assert_eq!(dry_run["summary"]["image_count"], json!(1));
        assert_eq!(dry_run["summary"]["auto_accepted_count"], json!(1));
        assert_eq!(dry_run["summary"]["empty_count"], json!(0));
        assert_eq!(dry_run["summary"]["provider_failure_count"], json!(0));
        assert_eq!(dry_run["summary"]["no_candidate_count"], json!(0));
        assert_eq!(
            dry_run["summary"]["geometry_quality"]["total_candidates"],
            json!(0)
        );
        assert_eq!(
            dry_run["summary"]["estimated_full_run"]["image_count"],
            json!(1)
        );
        assert_eq!(dry_run["samples"][0]["result_count"], json!(1));
        assert_eq!(dry_run["samples"][0]["outcomes"][0]["label"], json!("day"));
        assert!(
            dry_run["samples"][0]["nodes"]
                .as_array()
                .is_some_and(|nodes| nodes.iter().any(|node| {
                    node["node_id"] == json!("scene.day.classifier")
                        && node["output_types"] == json!(["classification_set"])
                }))
        );
        assert!(
            application
                .list_runs()
                .expect("Dry Run isolation")
                .is_empty()
        );

        let published = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/workflow-drafts/{draft_id}/publish"),
                None,
            )
            .await,
        )
        .await;
        let workflow_id = published["workflow_id"].as_str().expect("workflow id");
        let version = published["version"].as_u64().expect("version");
        let started = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/http-label/runs",
                Some(json!({
                    "workflow_id": workflow_id,
                    "version": version
                })),
            )
            .await,
        )
        .await;
        let run_id = started["run_id"].as_str().expect("run id");
        wait_for_status(&application, run_id, RunStatus::Completed).await;
        let inspection = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/runs/{run_id}/pipeline-artifacts"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(inspection["workflow_id"], json!(workflow_id));
        assert_eq!(inspection["image_index"], json!(0));
        let classifier = inspection["nodes"]
            .as_array()
            .and_then(|nodes| {
                nodes
                    .iter()
                    .find(|node| node["node_id"] == json!("scene.day.classifier"))
            })
            .expect("classifier Inspector");
        assert_eq!(
            classifier["outputs"][0]["kind"],
            json!("classification_set")
        );
        assert_eq!(classifier["attempts"], json!(1));
        assert!(classifier["configuration"]["parameters"]["labels"].is_array());
        assert!(classifier["latency_ms"].is_number());
        assert!(classifier["error"].is_null());

        let result_summary = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/runs/{run_id}/result-summary"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(result_summary["result_count"], json!(1));
        assert_eq!(result_summary["ready_count"], json!(1));
        assert_eq!(result_summary["needs_review_count"], json!(0));
        assert_eq!(result_summary["no_target_count"], json!(0));
        assert_eq!(result_summary["labels"][0]["label"], json!("day"));
        let debug_summary = response_json(
            request(
                &service,
                axum::http::Method::GET,
                &format!("/api/runs/{run_id}/debug-summary"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(debug_summary["node_count"], json!(4));
        assert_eq!(debug_summary["failed_node_count"], json!(0));
        assert_eq!(debug_summary["issues"], json!([]));

        let replay = response_json(
            request(
                &service,
                axum::http::Method::POST,
                &format!("/api/runs/{run_id}/replay/scene.day.classifier"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(replay["sandbox"], json!(true));
        assert!(
            replay["reexecuted_nodes"]
                .as_array()
                .is_some_and(|nodes| nodes.contains(&json!("scene.day.classifier")))
        );
        assert!(
            replay["preserved_upstream_nodes"]
                .as_array()
                .is_some_and(|nodes| nodes.contains(&json!("core.image_input")))
        );
        let ready = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/http-label/export-readiness",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(ready["ready"], json!(true));
        assert_eq!(ready["accepted_annotations"], json!(0));
        assert_eq!(ready["unresolved_reviews"], json!(0));
        assert_eq!(ready["recommended_format"], json!("native"));

        let annotation_id = uuid::Uuid::new_v4();
        let image_id = uuid::Uuid::new_v4();
        let created_annotation = request(
            &service,
            axum::http::Method::POST,
            &format!("/api/runs/{run_id}/annotations"),
            Some(json!({
                "annotation": {
                    "id": annotation_id,
                    "image_id": image_id,
                    "task_id": "scene",
                    "label": "day",
                    "value": {"kind": "classification", "labels": ["day"]},
                    "attributes": {},
                    "confidence": null,
                    "source": "human",
                    "review_status": "needs_review",
                    "provenance": {
                        "run_step_id": null,
                        "provider": null,
                        "model": null,
                        "tool_names": [],
                        "parent_annotation_id": null,
                        "artifact_ids": []
                    },
                    "created_at": Utc::now()
                }
            })),
        )
        .await;
        assert_eq!(created_annotation.status(), StatusCode::CREATED);
        let blocked = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/http-label/export-readiness",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(blocked["ready"], json!(false));
        assert_eq!(blocked["unresolved_reviews"], json!(1));
        assert_eq!(
            blocked["blocking_issues"][0]["code"],
            json!("reviews_unresolved")
        );
        let blocked_export = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/http-label/export",
            Some(json!({"format": "native"})),
        )
        .await;
        assert_eq!(blocked_export.status(), StatusCode::BAD_REQUEST);
        let accepted = request(
            &service,
            axum::http::Method::POST,
            &format!("/api/reviews/{annotation_id}/accept-and-next"),
            Some(json!({
                "project_id": "http-label",
                "queue_project_id": "http-label",
                "decision": "accept",
                "reason_code": "accepted_as_is",
                "note": "release export readiness test"
            })),
        )
        .await;
        assert_eq!(accepted.status(), StatusCode::OK);
        let ready = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/http-label/export-readiness",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(ready["ready"], json!(true));
        assert_eq!(ready["accepted_annotations"], json!(1));
        assert_eq!(ready["unresolved_reviews"], json!(0));
        let exported = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/http-label/export",
            Some(json!({"format": "native"})),
        )
        .await;
        assert_eq!(exported.status(), StatusCode::OK);
        let exported = response_json(exported).await;
        assert_eq!(exported["format"], json!("native"));
        assert_eq!(exported["report"]["exported_count"], json!(1));
        assert!(exported["output_path"].is_string());
        assert!(
            exported["report"]["output_files"]
                .as_array()
                .is_some_and(|files| {
                    files.iter().any(|file| {
                        file.as_str()
                            .is_some_and(|file| file.ends_with("export-report.json"))
                    })
                })
        );
        let persisted = response_json(
            request(
                &service,
                axum::http::Method::GET,
                "/api/projects/http-label/export-readiness",
                None,
            )
            .await,
        )
        .await;
        assert_eq!(persisted["last_export"]["format"], json!("native"));
    }

    #[tokio::test]
    async fn skill_api_groups_layered_registry_contributions() {
        let temp = tempfile::tempdir().expect("temp");
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            test_state(application, Arc::new(InMemorySecretStore::default())).await,
            None,
        );
        let skills =
            response_json(request(&service, axum::http::Method::GET, "/api/skills", None).await)
                .await;
        let entries = skills.as_array().expect("Skill catalog");
        for kind in ["capability", "domain", "pack"] {
            assert!(entries.iter().any(|entry| entry["kind"] == json!(kind)));
        }
        let capability_ids = entries
            .iter()
            .filter(|entry| entry["kind"] == json!("capability"))
            .filter_map(|entry| entry["id"].as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            capability_ids,
            std::collections::BTreeSet::from([
                "annotagent.classification",
                "annotagent.detection",
                "annotagent.segmentation",
            ])
        );
        assert!(entries.iter().all(|entry| {
            entry["product_visibility"] == json!("primary")
                && entry["deprecated_alias_for"].is_null()
        }));
        assert!(entries.iter().all(|entry| {
            entry["nodes"].is_array()
                && entry["policies"].is_array()
                && entry["capabilities"].is_array()
                && entry["projects"].is_array()
        }));
        let project_yaml = r"
version: 1
project:
  name: Layered Project
  language: en
dataset:
  root: images
runtime: {}
tasks:
  - id: targets
    kind: bounding_box
    labels: [target]
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native]
";
        let created = request(
            &service,
            axum::http::Method::POST,
            "/api/projects",
            Some(json!({"id": "layered", "yaml": project_yaml})),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let domain = entries
            .iter()
            .find(|entry| entry["kind"] == json!("domain"))
            .expect("Domain Skill");
        let mut enabled = vec![json!({
            "id": domain["id"],
            "version": domain["version"],
        })];
        for requirement in domain["capability_requirements"]
            .as_array()
            .expect("requirements")
        {
            let (id, version) = requirement
                .as_str()
                .expect("requirement")
                .split_once('@')
                .expect("versioned requirement");
            enabled.push(json!({"id": id, "version": version}));
        }
        let expected_enabled_count = enabled.len();
        let configured = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/layered/skills",
            Some(json!({"enabled_skills": enabled})),
        )
        .await;
        assert_eq!(configured.status(), StatusCode::OK);
        let configured = response_json(configured).await;
        assert_eq!(
            configured["enabled_skills"].as_array().map(Vec::len),
            Some(expected_enabled_count)
        );
    }

    async fn request(
        service: &Router,
        method: axum::http::Method,
        uri: &str,
        body: Option<Value>,
    ) -> Response {
        let request = axum::http::Request::builder().method(method).uri(uri);
        let request = if let Some(value) = body {
            request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(value.to_string()))
        } else {
            request.body(Body::empty())
        }
        .expect("request");
        service.clone().oneshot(request).await.expect("response")
    }

    async fn response_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .expect("response body");
        serde_json::from_slice(&bytes).expect("JSON response")
    }

    async fn wait_for_status(application: &LocalApplication, run_id: &str, expected: RunStatus) {
        let run_id: RunId = run_id.parse().expect("valid run id");
        for _ in 0..100 {
            if application
                .list_runs()
                .expect("runs")
                .into_iter()
                .any(|run| run.id == run_id && run.status == expected)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!(
            "run {run_id} did not reach {expected:?}; runs={:#?}; tasks={:#?}",
            application.list_runs().expect("runs"),
            application
                .store()
                .list_task_runs(run_id)
                .expect("task runs")
        );
    }
}
