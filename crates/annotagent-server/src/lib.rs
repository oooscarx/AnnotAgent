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
    ActiveRunExists, AnnotAgentApplication, DatasetCoordinator, DetectionWorkerSettings,
    LocalApplication, ModelBinding, ProjectSummary, Settings, WorkflowVersion, stable_project_id,
    validate_settings,
};
use annotagent_core::{
    Annotation, AnnotationId, AnnotationValue, ArtifactValidationState, AttributeDefinition,
    BatchId, CandidateAgreement, CorrectionFeatures, CorrectionRecord, DetectionEvidence,
    EnabledSkillConfig, LabelId, NormalizedRect, PipelineArtifact, PipelineBuilderConstraints,
    ProjectSchema, ReviewStatus, RunEvent, RunEventKind, RunEventPayload, RunId, RunStatus,
    ScoreSemantics, TaskKind, UsageTotals, WorkflowConstraints, WorkflowDraft,
};
use annotagent_provider::HttpVisionWorkerClient;
use annotagent_runtime::RuntimeStore;
use annotagent_storage::HistoryRun;
use anyhow::{Context, anyhow, bail};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{delete, get, patch, post},
};
use chrono::Utc;
use futures::{Stream, stream};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::RwLock;
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
    secret_account: Arc<String>,
}

impl ServerState {
    pub fn new(application: Arc<LocalApplication>) -> anyhow::Result<Self> {
        let secret_account = format!("workspace-{}", stable_project_id(application.workspace()));
        let local_store = Arc::new(LocalSecretStore::new(
            application
                .workspace()
                .join(".annotagent/credentials/provider-api-key"),
        ));
        let migration_error = migrate_legacy_keychain(local_store.as_ref(), &secret_account)
            .err()
            .map(|error| error.to_string());
        let mut state = Self::with_secret_store(application, local_store)?;
        if migration_error.is_some() {
            state.credential_store_error = Arc::new(RwLock::new(migration_error));
        }
        Ok(state)
    }

    fn with_secret_store(
        application: Arc<LocalApplication>,
        secret_store: Arc<dyn SecretStore>,
    ) -> anyhow::Result<Self> {
        let settings_path = application.workspace().join(".annotagent/settings.toml");
        let settings_persisted = settings_path.is_file();
        let settings = if settings_persisted {
            annotagent_application::load_settings(Some(&settings_path))?
        } else {
            annotagent_application::load_settings(None)?
        };
        validate_settings(&settings)?;
        let secret_account = format!("workspace-{}", stable_project_id(application.workspace()));
        let (api_key, api_key_persisted, credential_store_error) =
            match secret_store.load(&secret_account) {
                Ok(value) => {
                    let configured = value.is_some();
                    (value, configured, None)
                }
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
            secret_account: Arc::new(secret_account),
        })
    }

    #[must_use]
    pub fn application(&self) -> &Arc<LocalApplication> {
        &self.application
    }
}

const SECRET_SERVICE: &str = "com.annotagent.provider-api-key";

trait SecretStore: Send + Sync {
    fn load(&self, account: &str) -> anyhow::Result<Option<String>>;
    fn save(&self, account: &str, secret: &str) -> anyhow::Result<()>;
    fn delete(&self, account: &str) -> anyhow::Result<()>;
}

struct LocalSecretStore {
    path: PathBuf,
}

struct LegacySystemSecretStore;

impl LocalSecretStore {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SecretStore for LocalSecretStore {
    fn load(&self, _account: &str) -> anyhow::Result<Option<String>> {
        match std::fs::symlink_metadata(&self.path) {
            Ok(metadata) => {
                if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                    bail!(
                        "local credential path must be a regular non-symlink file: {}",
                        self.path.display()
                    );
                }
                let secret = std::fs::read_to_string(&self.path).with_context(|| {
                    format!("cannot read local credential file {}", self.path.display())
                })?;
                let secret = secret.trim().to_owned();
                if secret.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(secret))
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| {
                format!(
                    "cannot inspect local credential file {}",
                    self.path.display()
                )
            }),
        }
    }

    fn save(&self, _account: &str, secret: &str) -> anyhow::Result<()> {
        if secret.trim().is_empty() {
            bail!("API key cannot be empty");
        }
        persist_local_secret(&self.path, secret.trim())
    }

    fn delete(&self, _account: &str) -> anyhow::Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| {
                format!(
                    "cannot remove local credential file {}",
                    self.path.display()
                )
            }),
        }
    }
}

impl LegacySystemSecretStore {
    fn entry(account: &str) -> anyhow::Result<keyring::Entry> {
        keyring::Entry::new(SECRET_SERVICE, account)
            .map_err(|error| anyhow!("cannot access the system credential store: {error}"))
    }
}

impl SecretStore for LegacySystemSecretStore {
    fn load(&self, account: &str) -> anyhow::Result<Option<String>> {
        match Self::entry(account)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(anyhow!("cannot read the saved API key: {error}")),
        }
    }

    fn save(&self, account: &str, secret: &str) -> anyhow::Result<()> {
        Self::entry(account)?.set_password(secret).map_err(|error| {
            anyhow!("cannot save the API key in the system credential store: {error}")
        })
    }

    fn delete(&self, account: &str) -> anyhow::Result<()> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(anyhow!("cannot clear the saved API key: {error}")),
        }
    }
}

fn migrate_legacy_keychain(local: &LocalSecretStore, account: &str) -> anyhow::Result<()> {
    let legacy = LegacySystemSecretStore;
    if local.load(account)?.is_none()
        && let Some(secret) = legacy.load(account)?
    {
        local.save(account, &secret)?;
    }
    legacy.delete(account)
}

fn validate_provider_kind(provider: &str) -> anyhow::Result<()> {
    if matches!(provider, "mock" | "openai_compatible") {
        Ok(())
    } else {
        Err(anyhow!(
            "default_provider must be either \"mock\" or \"openai_compatible\""
        ))
    }
}

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

fn persist_local_secret(path: &Path, secret: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("credential path has no parent"))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("cannot create credential directory {}", parent.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    let temporary_path = parent.join(format!(".provider-api-key.{}.tmp", uuid::Uuid::new_v4()));
    let write_result = (|| -> anyhow::Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary_path).with_context(|| {
            format!(
                "cannot create temporary credential file {}",
                temporary_path.display()
            )
        })?;
        file.write_all(secret.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&temporary_path, path)
            .with_context(|| format!("cannot replace credential file {}", path.display()))?;
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
        .route("/api/workflows/compare", post(compare_workflow_versions))
        .route("/api/models", get(list_models))
        .route("/api/models/{model_id}/test", post(test_detection_worker))
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
    let projects = product_projects(&state).await?;
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
    let projects = product_projects(&state)
        .await?
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

fn worker_model_binding(worker: &DetectionWorkerSettings) -> ModelBinding {
    let capabilities = worker
        .expected_capabilities
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
        health_status: if worker.enabled {
            "unknown"
        } else {
            "unavailable"
        }
        .to_owned(),
        health_detail: Some(if worker.enabled {
            "Run Test Worker to discover live health and capabilities".to_owned()
        } else {
            "Disabled in workspace Settings".to_owned()
        }),
        availability_group: if worker.enabled {
            annotagent_application::ModelAvailabilityGroup::ConfiguredUnavailable
        } else if matches!(
            worker.model_id.as_str(),
            "locate-anything-local" | "rfdetr-specialist-local" | "sam2.1-hiera-tiny"
        ) {
            annotagent_application::ModelAvailabilityGroup::Labs
        } else {
            annotagent_application::ModelAvailabilityGroup::Disabled
        },
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

fn labs_model_bindings() -> Vec<ModelBinding> {
    vec![
        ModelBinding {
            id: "sam2.1-hiera-tiny".to_owned(),
            provider: "http_vision".to_owned(),
            model: "SAM 2.1 Prompted Segmentation".to_owned(),
            role: "segmentation".to_owned(),
            scope: "optional_local_worker".to_owned(),
            health_status: "unavailable".to_owned(),
            health_detail: Some(
                "Labs adapter is installed; configure and start the workspace-private SAM Worker"
                    .to_owned(),
            ),
            availability_group: annotagent_application::ModelAvailabilityGroup::Labs,
            capabilities: vec!["prompted_segmentation".to_owned()],
            score_semantics: Some("not_provided".to_owned()),
            model_version: Some("local-unpinned".to_owned()),
            endpoint: Some("http://127.0.0.1:8790".to_owned()),
            enabled: Some(false),
            license_summary: Some(
                "Configure and verify the concrete SAM checkpoint license".to_owned(),
            ),
            architecture: Some("sam2.1-hiera-tiny".to_owned()),
            checkpoint_sha256: None,
            label_space: Vec::new(),
            cost_per_request: None,
        },
        ModelBinding {
            id: "yolo-http-worker".to_owned(),
            provider: "http_vision".to_owned(),
            model: "YOLO HTTP Worker".to_owned(),
            role: "detection".to_owned(),
            scope: "optional_local_worker".to_owned(),
            health_status: "unavailable".to_owned(),
            health_detail: Some(
                "Labs reference adapter only; register an explicit versioned Worker and weights"
                    .to_owned(),
            ),
            availability_group: annotagent_application::ModelAvailabilityGroup::Labs,
            capabilities: vec!["object_detection".to_owned()],
            score_semantics: Some("relative_confidence".to_owned()),
            model_version: Some("unconfigured".to_owned()),
            endpoint: None,
            enabled: Some(false),
            license_summary: Some(
                "Depends on the configured implementation and weights".to_owned(),
            ),
            architecture: Some("yolo".to_owned()),
            checkpoint_sha256: None,
            label_space: Vec::new(),
            cost_per_request: None,
        },
    ]
}

async fn product_projects(state: &ServerState) -> ApiResult<Vec<ProjectSummary>> {
    let mut projects = state
        .application
        .list_projects()
        .map_err(ApiError::internal)?;
    let binding = {
        let settings = state.settings.read().await;
        workspace_model_binding(&settings)
    };
    for project in &mut projects {
        project.model_bindings = vec![binding.clone()];
        for workflow in &mut project.available_workflow_versions {
            for node in &mut workflow.nodes {
                if node.model_binding.is_some() {
                    node.model_binding = Some(binding.id.clone());
                }
            }
        }
        for node in &mut project.active_workflow.nodes {
            if node.model_binding.is_some() {
                node.model_binding = Some(binding.id.clone());
            }
        }
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
        artifact_count: history.artifacts.len(),
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
    let projects = product_projects(&state).await?;
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
    let workflows = product_projects(&state)
        .await?
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
    Ok(Json(json!({"drafts": drafts})))
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
    #[serde(default = "default_workflow_advisor")]
    advisor: String,
    #[serde(default)]
    constraints: WorkflowConstraints,
    #[serde(default)]
    builder_constraints: PipelineBuilderConstraints,
}

fn default_workflow_advisor() -> String {
    "mock".to_owned()
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
        && state.api_key.read().await.is_some()
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
    let (suggestion, agent_report) =
        match request.advisor.as_str() {
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
                        ApiError::bad_request(report.session.stop_reason.clone().unwrap_or_else(
                            || "Workflow Advisor stopped without a Draft".to_owned(),
                        ))
                    })?;
                (suggestion, Some(report))
            }
            "llm" => {
                let report = state
                    .application
                    .run_workflow_advisor_live_agent(
                        &request.project_id,
                        &settings,
                        state.api_key.read().await.clone(),
                        &workflow_constraints,
                        target,
                        request.builder_constraints.clone(),
                        CancellationToken::default(),
                    )
                    .await
                    .map_err(ApiError::bad_request)?;
                let suggestion =
                    report.suggestion.clone().ok_or_else(|| {
                        ApiError::bad_request(report.session.stop_reason.clone().unwrap_or_else(
                            || "Pipeline Builder stopped without a Draft".to_owned(),
                        ))
                    })?;
                (suggestion, Some(report))
            }
            other => {
                return Err(ApiError::bad_request(format!(
                    "unknown Workflow Advisor {other:?}; choose mock or llm"
                )));
            }
        };
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
    let temporary_api_key = state.api_key.read().await.clone();
    let image_indices = payload.map_or_else(Vec::new, |Json(value)| value.image_indices);
    let report = state
        .application
        .dry_run_workflow_samples_with_api_key(
            &draft_id,
            &settings,
            &image_indices,
            temporary_api_key,
        )
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!(report)))
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

async fn list_models(State(state): State<ServerState>) -> Json<Value> {
    let models = {
        let settings = state.settings.read().await;
        let mut models = vec![workspace_model_binding(&settings)];
        models.extend(settings.detection_workers.iter().map(worker_model_binding));
        models.extend(labs_model_bindings());
        models
    };
    Json(json!({"models": models}))
}

async fn test_detection_worker(
    State(state): State<ServerState>,
    AxumPath(model_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let worker = {
        let settings = state.settings.read().await;
        settings
            .detection_workers
            .iter()
            .find(|worker| worker.model_id == model_id)
            .cloned()
    }
    .ok_or_else(|| ApiError::not_found(format!("unknown Detection Worker model {model_id:?}")))?;
    if !worker.enabled {
        return Err(ApiError::bad_request(format!(
            "Detection Worker model {model_id:?} is disabled"
        )));
    }
    let client =
        HttpVisionWorkerClient::new(worker.http_config()).map_err(ApiError::bad_request)?;
    let health = client.health().await.map_err(ApiError::bad_request)?;
    let capabilities = client
        .discover_capabilities()
        .await
        .map_err(ApiError::bad_request)?;
    Ok(Json(json!({
        "model_id": model_id,
        "health": health,
        "capabilities": capabilities,
    })))
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
    let binding = {
        let settings = state.settings.read().await;
        workspace_model_binding(&settings)
    };
    project.model_bindings = vec![binding.clone()];
    for node in &mut project.active_workflow.nodes {
        if node.model_binding.is_some() {
            node.model_binding = Some(binding.id.clone());
        }
    }
    for workflow in &mut project.available_workflow_versions {
        for node in &mut workflow.nodes {
            if node.model_binding.is_some() {
                node.model_binding = Some(binding.id.clone());
            }
        }
    }
    Ok(Json(json!(project)))
}

async fn guidance_context(state: &ServerState, project_id: &str) -> ApiResult<(Settings, bool)> {
    let settings = state.settings.read().await.clone();
    let workspace_model_connected =
        settings.default_provider == "mock" || *state.api_key_persisted.read().await;
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
struct StartRunRequest {
    provider: Option<String>,
    idempotency_key: Option<String>,
    workflow_id: Option<String>,
    version: Option<u32>,
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
    if let Some(batch) = state
        .application
        .get_project(&project_id)
        .map_err(ApiError::internal)?
        .active_batch
    {
        return Err(ApiError::active_batch(&batch));
    }
    let settings = state.settings.read().await.clone();
    let request = payload.map_or(
        StartRunRequest {
            provider: None,
            idempotency_key: None,
            workflow_id: None,
            version: None,
        },
        |Json(value)| value,
    );
    let provider = request
        .provider
        .clone()
        .unwrap_or_else(|| settings.default_provider.clone());
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
    validate_provider_kind(&provider).map_err(ApiError::bad_request)?;
    let api_key = state.api_key.read().await.clone();
    if request.workflow_id.is_some() != request.version.is_some() {
        return Err(ApiError::bad_request(
            "workflow_id and version must be selected together",
        ));
    }
    let selected_workflow = request.workflow_id.as_deref().zip(request.version);
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
struct StartBatchRequest {
    provider: Option<String>,
    limit: Option<usize>,
    workflow_id: Option<String>,
    version: Option<u32>,
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
    let request = payload.map_or(
        StartBatchRequest {
            provider: None,
            limit: None,
            workflow_id: None,
            version: None,
        },
        |Json(value)| value,
    );
    if request.limit == Some(0) {
        return Err(ApiError::bad_request(
            "batch limit must be greater than zero",
        ));
    }
    if request.workflow_id.is_some() != request.version.is_some() {
        return Err(ApiError::bad_request(
            "workflow_id and version must be selected together",
        ));
    }
    let selected_workflow = request.workflow_id.as_deref().zip(request.version);
    let settings = state.settings.read().await.clone();
    let provider = request
        .provider
        .unwrap_or_else(|| settings.default_provider.clone());
    validate_provider_kind(&provider).map_err(ApiError::bad_request)?;
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
    let api_key = state.api_key.read().await.clone();
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
    Ok(Json(
        json!({"annotation": request.annotation, "revision": revision}),
    ))
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
    let correction_id = if already_applied {
        None
    } else if let Some(skill_id) = requested_skill_id {
        let record = CorrectionRecord {
            id: uuid::Uuid::new_v4(),
            project_id: stable_project_id(
                project_path
                    .parent()
                    .unwrap_or(state.application.workspace()),
            ),
            skill_id,
            task_id: annotation.task_id.clone(),
            predicted_label: original.label.clone(),
            corrected_label: annotation.label.clone(),
            reason_code: request.reason_code.clone(),
            original_annotation: Some(original),
            corrected_annotation: Some(annotation.snapshot()),
            note: request.note.clone(),
            image_features: CorrectionFeatures {
                geometry: BTreeMap::new(),
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
        json!({"annotation": annotation, "revision": revision, "correction_id": correction_id}),
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
        object.insert(
            "credential_store".to_owned(),
            Value::String("workspace_private_file".to_owned()),
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
        let secret_store = state.secret_store.clone();
        let secret_account = state.secret_account.clone();
        let requested_key = api_key.clone();
        let credential_result = tokio::task::spawn_blocking(move || {
            if let Some(secret) = requested_key {
                secret_store.save(&secret_account, &secret)
            } else {
                secret_store.delete(&secret_account)
            }
        })
        .await
        .map_err(ApiError::internal)?;
        if let Err(error) = credential_result {
            *state.credential_store_error.write().await = Some(error.to_string());
            return Err(ApiError::internal(error));
        }
        *state.api_key.write().await = api_key;
        *state.api_key_persisted.write().await = !clear_saved_api_key;
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use annotagent_core::RunStatus;
    use annotagent_image_tools::{generate_synthetic_inspection, generate_synthetic_robocup};
    use axum::body::to_bytes;
    use futures::StreamExt;
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;

    #[derive(Default)]
    struct MemorySecretStore {
        secrets: Mutex<HashMap<String, String>>,
    }

    impl SecretStore for MemorySecretStore {
        fn load(&self, account: &str) -> anyhow::Result<Option<String>> {
            Ok(self
                .secrets
                .lock()
                .expect("secret store lock")
                .get(account)
                .cloned())
        }

        fn save(&self, account: &str, secret: &str) -> anyhow::Result<()> {
            self.secrets
                .lock()
                .expect("secret store lock")
                .insert(account.to_owned(), secret.to_owned());
            Ok(())
        }

        fn delete(&self, account: &str) -> anyhow::Result<()> {
            self.secrets
                .lock()
                .expect("secret store lock")
                .remove(account);
            Ok(())
        }
    }

    fn test_state(
        application: Arc<LocalApplication>,
        secret_store: Arc<MemorySecretStore>,
    ) -> ServerState {
        ServerState::with_secret_store(application, secret_store).expect("state")
    }

    #[tokio::test]
    async fn health_works_and_traversal_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let app = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            test_state(app, Arc::new(MemorySecretStore::default())),
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
            test_state(app, Arc::new(MemorySecretStore::default())),
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
            test_state(application.clone(), Arc::new(MemorySecretStore::default())),
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
                    "provider": "mock",
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
        assert_eq!(run["model_identity"], json!("mock/vision-model"));
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
                    "provider": "mock",
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
        let state = test_state(application.clone(), Arc::new(MemorySecretStore::default()));
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

        let dashboard =
            response_json(request(&service, axum::http::Method::GET, "/api/projects", None).await)
                .await;
        let project = &dashboard["projects"][0];
        assert_eq!(project["name"], json!("RoboCup Ball Demo"));
        assert_eq!(project["enabled_skills"][0]["id"], json!("robocup"));
        assert_eq!(
            project["active_workflow"]["name"],
            json!("Configured task graph")
        );
        assert_eq!(project["active_workflow"]["status"], json!("published"));
        assert_eq!(project["model_bindings"][0]["id"], json!("default-vision"));

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
        assert_eq!(models["models"][1]["id"], json!("locate-anything-local"));
        assert_eq!(models["models"][1]["enabled"], json!(false));
        assert_eq!(models["models"][1]["availability_group"], json!("labs"));
        assert_eq!(
            models["models"][1]["score_semantics"],
            json!("not_provided")
        );
        assert_eq!(models["models"][2]["id"], json!("rfdetr-specialist-local"));
        assert_eq!(models["models"][2]["enabled"], json!(false));
        assert_eq!(models["models"][2]["availability_group"], json!("labs"));
        assert_eq!(
            models["models"][2]["score_semantics"],
            json!("relative_confidence")
        );
        assert_eq!(models["models"][3]["id"], json!("sam2.1-hiera-tiny"));
        assert_eq!(models["models"][3]["role"], json!("segmentation"));
        assert_eq!(models["models"][3]["availability_group"], json!("labs"));
        assert_eq!(models["models"][4]["id"], json!("yolo-http-worker"));
        assert_eq!(models["models"][4]["availability_group"], json!("labs"));
        assert_eq!(
            request(
                &service,
                axum::http::Method::POST,
                "/api/models/locate-anything-local/test",
                None,
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );

        let sse = request(&service, axum::http::Method::GET, "/api/events", None).await;
        assert_eq!(sse.status(), StatusCode::OK);
        let mut event_stream = sse.into_body().into_data_stream();

        let started = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/review-demo/runs",
                Some(json!({})),
            )
            .await,
        )
        .await;
        let run_id = started["run_id"].as_str().expect("run id");
        let first_event = tokio::time::timeout(Duration::from_secs(2), event_stream.next())
            .await
            .expect("SSE timeout")
            .expect("SSE item")
            .expect("SSE body");
        assert!(String::from_utf8_lossy(&first_event).contains("run_"));

        wait_for_status(&application, run_id, RunStatus::CompletedWithReview).await;
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
        let accepted = request(
            &service,
            axum::http::Method::POST,
            &format!("/api/reviews/{review_id}/accept-and-next"),
            Some(json!({
                "project_id": "review-demo",
                "queue_project_id": "review-demo",
                "decision": "accept",
                "reason_code": "accepted_as_is",
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
        let budget_run = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/projects/review-demo/runs",
                Some(json!({"provider": "mock"})),
            )
            .await,
        )
        .await;
        wait_for_status(
            &application,
            budget_run["run_id"].as_str().expect("budget run id"),
            RunStatus::BudgetExceeded,
        )
        .await;
    }

    #[tokio::test]
    async fn settings_and_local_api_key_survive_server_restart() {
        let temp = tempfile::tempdir().expect("temp");
        let credential_path = temp.path().join(".annotagent/credentials/provider-api-key");
        let secrets = Arc::new(LocalSecretStore::new(credential_path.clone()));
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(
            ServerState::with_secret_store(application, secrets.clone()).expect("state"),
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
        assert_eq!(saved["credential_store"], json!("workspace_private_file"));
        assert!(saved.get("api_key").is_none());

        let settings_path = temp.path().join(".annotagent/settings.toml");
        let persisted = std::fs::read_to_string(&settings_path).expect("persisted settings");
        assert!(persisted.contains("persisted-vision-model"));
        assert!(!persisted.contains("test-secret-that-must-not-reach-disk"));
        assert_eq!(
            std::fs::read_to_string(&credential_path)
                .expect("local credential")
                .trim(),
            "test-secret-that-must-not-reach-disk"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&credential_path)
                    .expect("credential metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let restarted_application =
            Arc::new(LocalApplication::new(temp.path()).expect("restarted application"));
        let restarted = router(
            ServerState::with_secret_store(restarted_application, secrets.clone()).expect("state"),
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
        assert!(!credential_path.exists());
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
            test_state(application, Arc::new(MemorySecretStore::default())),
            None,
        );
        let response = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/duplicate-demo/runs",
            Some(json!({"provider": "mock"})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = response_json(response).await;
        assert_eq!(body["code"], json!("active_run_exists"));
        assert_eq!(body["active_run_id"], json!(active_run_id));
        assert_eq!(body["status"], json!("pending"));
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
            test_state(application, Arc::new(MemorySecretStore::default())),
            None,
        );
        let listed =
            response_json(request(&service, axum::http::Method::GET, "/api/batches", None).await)
                .await;
        assert_eq!(listed["batches"][0]["id"], json!(batch.id));
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
            Some(json!({"provider": "mock"})),
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
            Some(json!({"provider": "mock"})),
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
        let state = test_state(application.clone(), Arc::new(MemorySecretStore::default()));
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
                    "provider": "mock",
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
            test_state(application, Arc::new(MemorySecretStore::default())),
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
