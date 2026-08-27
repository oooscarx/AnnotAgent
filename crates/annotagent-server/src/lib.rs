//! Thin HTTP/SSE adapter over the shared application service.

use std::{
    collections::BTreeMap,
    convert::Infallible,
    fs::OpenOptions,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use annotagent_application::{
    ActiveRunExists, AnnotAgentApplication, DatasetCoordinator, LocalApplication, ModelBinding,
    ProjectSummary, Settings, WorkflowVersion, stable_project_id, validate_settings,
};
use annotagent_core::{
    AgentBudget, Annotation, AnnotationId, AttributeDefinition, BatchId, CorrectionFeatures,
    CorrectionRecord, DatasetExporter, EnabledSkillConfig, ExportRequest, LabelId, ProjectSchema,
    ProjectSnapshot, ReviewStatus, RunId, RunStatus, SnapshotImage, TaskKind, UsageTotals,
    WorkflowConstraints, WorkflowDraft,
};
use annotagent_export::{
    CocoExporter, LabelMeExporter, NativeExporter, YoloDetectionExporter, YoloSegmentationExporter,
};
use annotagent_storage::HistoryRun;
use anyhow::{Context, anyhow, bail};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, patch, post},
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
        if std::env::var("ANNOTAGENT_DISABLE_KEYCHAIN").as_deref() == Ok("1") {
            Self::with_secret_store(application, Arc::new(DisabledSecretStore))
        } else {
            Self::with_secret_store(application, Arc::new(SystemSecretStore))
        }
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

struct SystemSecretStore;

struct DisabledSecretStore;

impl SecretStore for DisabledSecretStore {
    fn load(&self, _account: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn save(&self, _account: &str, _secret: &str) -> anyhow::Result<()> {
        bail!("system keychain is disabled; use the configured API-key environment variable")
    }

    fn delete(&self, _account: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

impl SystemSecretStore {
    fn entry(account: &str) -> anyhow::Result<keyring::Entry> {
        keyring::Entry::new(SECRET_SERVICE, account)
            .map_err(|error| anyhow!("cannot access the system credential store: {error}"))
    }
}

impl SecretStore for SystemSecretStore {
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
        .route(
            "/api/workflow-drafts/{draft_id}",
            patch(save_workflow_draft),
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
        .route("/api/runs", get(list_run_summaries))
        .route("/api/projects/{project_id}", get(get_project))
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
        .route("/api/projects/{project_id}/export", post(export_dataset))
        .route("/api/runs/{run_id}", get(get_run))
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
        .route("/api/runs/{run_id}/annotations", post(create_annotation))
        .route("/api/reviews", get(list_reviews))
        .route("/api/reviews/{review_id}", get(get_review))
        .route("/api/reviews/{review_id}/decision", post(review_decision))
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
    }
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
    let mut validation_issue_codes = history
        .events
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
    validation_issue_codes.sort();
    validation_issue_codes.dedup();
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
    let review_queue = reviews(&state)?.len();
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
}

fn default_workflow_advisor() -> String {
    "mock".to_owned()
}

async fn suggest_workflow(
    State(state): State<ServerState>,
    Json(request): Json<SuggestWorkflowRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let settings = state.settings.read().await.clone();
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
                        &request.constraints,
                        target,
                        AgentBudget::default(),
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
            "llm" => (
                match target {
                    Some((task_id, label)) => state
                        .application
                        .suggest_label_pipeline_live(
                            &request.project_id,
                            &settings,
                            state.api_key.read().await.clone(),
                            task_id,
                            label,
                            &request.constraints,
                        )
                        .await
                        .map_err(ApiError::bad_request)?,
                    None => state
                        .application
                        .suggest_workflow_live(
                            &request.project_id,
                            &settings,
                            state.api_key.read().await.clone(),
                            &request.constraints,
                        )
                        .await
                        .map_err(ApiError::bad_request)?,
                },
                None,
            ),
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
    let model = {
        let settings = state.settings.read().await;
        workspace_model_binding(&settings)
    };
    Json(json!({
        "models": [model],
    }))
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
    let (imported, duplicates) = state
        .application
        .import_images(&project_id, &request.source)
        .map_err(ApiError::bad_request)?;
    Ok(Json(
        json!({"imported": imported, "duplicates": duplicates}),
    ))
}

async fn list_images(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let images = state
        .application
        .list_project_images(&project_id)
        .map_err(ApiError::not_found)?;
    Ok(Json(json!({
        "images": images.iter().enumerate().map(|(index, path)| json!({
            "index": index,
            "name": path.file_name().unwrap_or_default().to_string_lossy(),
            "url": format!("/api/projects/{project_id}/images/{index}/content"),
        })).collect::<Vec<_>>()
    })))
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

fn reviews(state: &ServerState) -> ApiResult<Vec<Value>> {
    let mut reviews = Vec::new();
    let projects = state
        .application
        .list_projects()
        .map_err(ApiError::internal)?;
    for run in state.application.list_runs().map_err(ApiError::internal)? {
        let summary = run_summary(state, run.clone())?;
        let inspection = state
            .application
            .inspect_run_pipeline_artifacts(run.id)
            .ok();
        let project_id = projects.iter().find_map(|project| {
            let path = state.application.project_path(&project.id).ok()?;
            (Some(stable_project_id(path.parent()?)) == run.project_id)
                .then_some(project.id.as_str())
        });
        for annotation in state
            .application
            .store()
            .list_annotations(run.id)
            .map_err(ApiError::internal)?
        {
            if annotation.review_status == ReviewStatus::NeedsReview {
                let source_artifact_id = annotation.provenance.artifact_ids.first().copied();
                let source_node = inspection.as_ref().and_then(|inspection| {
                    inspection.nodes.iter().find_map(|node| {
                        node.outputs
                            .iter()
                            .any(|artifact| {
                                source_artifact_id.is_some_and(|id| {
                                    artifact.reference().artifact_id == id.to_string()
                                })
                            })
                            .then_some(node.node_id.as_str())
                    })
                });
                reviews.push(json!({
                    "id": annotation.id,
                    "run_id": run.id,
                    "project_id": project_id,
                    "project_name": run.project_name,
                    "annotation": annotation,
                    "workflow_id": inspection.as_ref().map(|value| value.workflow_id.as_str()),
                    "workflow_version": inspection.as_ref().map_or_else(
                        || summary.workflow_version.parse().unwrap_or_default(),
                        |value| value.workflow_version,
                    ),
                    "source_node": source_node.or(summary.current_node.as_deref()),
                    "source_artifact_id": source_artifact_id,
                    "review_reason": if annotation.confidence.is_some_and(|value| value < 0.8) { "low_confidence" } else if !summary.validation_issue_codes.is_empty() { "validation_issue" } else { "review_policy" },
                    "confidence": annotation.confidence,
                    "validation_issues": summary.validation_issue_codes,
                }));
            }
        }
    }
    Ok(reviews)
}

async fn list_reviews(State(state): State<ServerState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({"reviews": reviews(&state)?})))
}

fn parse_annotation_id(value: &str) -> ApiResult<AnnotationId> {
    value.parse().map_err(ApiError::bad_request)
}

async fn get_review(
    State(state): State<ServerState>,
    AxumPath(review_id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let id = parse_annotation_id(&review_id)?;
    let item = reviews(&state)?
        .into_iter()
        .find(|item| item["id"] == json!(id))
        .ok_or_else(|| ApiError::not_found("review was not found"))?;
    Ok(Json(item))
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

#[derive(Debug, Deserialize)]
struct ReviewDecisionRequest {
    decision: String,
    project_id: String,
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
    let (_, mut annotation) = state
        .application
        .store()
        .find_annotation(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("review was not found"))?;
    let original = annotation.snapshot();
    annotation.review_status = match request.decision.as_str() {
        "accept" => ReviewStatus::HumanAccepted,
        "reject" | "delete" => ReviewStatus::Rejected,
        other => return Err(ApiError::bad_request(format!("unknown decision {other:?}"))),
    };
    if let Some(label) = request.corrected_label.clone() {
        annotation.label = Some(label);
    }
    let revision = state
        .application
        .store()
        .update_annotation(&annotation, Some(&request.reason_code))
        .map_err(ApiError::bad_request)?;
    let project_path = state
        .application
        .project_path(&request.project_id)
        .map_err(ApiError::bad_request)?;
    let project = ProjectSchema::from_yaml(
        &std::fs::read_to_string(&project_path).map_err(ApiError::bad_request)?,
    )
    .map_err(ApiError::bad_request)?;
    let configured_skills = project.project.enabled_skill_versions();
    let skill_id = request
        .skill_id
        .or_else(|| {
            (configured_skills.len() == 1)
                .then(|| configured_skills.keys().next().cloned())
                .flatten()
        })
        .filter(|id| configured_skills.contains_key(id))
        .ok_or_else(|| {
            ApiError::bad_request(
                "Review correction must identify one Skill enabled by the Project",
            )
        })?;
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
        reason_code: request.reason_code,
        original_annotation: Some(original),
        corrected_annotation: Some(annotation.snapshot()),
        note: request.note,
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
    Ok(Json(
        json!({"annotation": annotation, "revision": revision, "correction_id": record.id}),
    ))
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

async fn export_dataset(
    State(state): State<ServerState>,
    AxumPath(project_id): AxumPath<String>,
    Json(request): Json<ExportBody>,
) -> ApiResult<Json<Value>> {
    let project_path = state
        .application
        .project_path(&project_id)
        .map_err(ApiError::not_found)?;
    let project_yaml = std::fs::read_to_string(&project_path).map_err(ApiError::bad_request)?;
    let schema = ProjectSchema::from_yaml(&project_yaml).map_err(ApiError::bad_request)?;
    let runs = state
        .application
        .list_runs()
        .map_err(ApiError::internal)?
        .into_iter();
    let mut export_source = None;
    for run in runs.filter(|run| run.project_name == schema.project.name) {
        let annotations = state
            .application
            .store()
            .list_annotations(run.id)
            .map_err(ApiError::internal)?;
        if !annotations.is_empty() {
            export_source = Some((run, annotations));
            break;
        }
    }
    let (run, annotations) = export_source
        .ok_or_else(|| ApiError::bad_request("project has no run with annotations"))?;
    let image_paths = state
        .application
        .list_project_images(&project_id)
        .map_err(ApiError::bad_request)?;
    let source_sha256 = run
        .workflow_snapshot_json
        .as_deref()
        .and_then(|snapshot| serde_json::from_str::<Value>(snapshot).ok())
        .and_then(|snapshot| {
            snapshot
                .pointer("/image/sha256")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let image_path = if let Some(source_sha256) = source_sha256 {
        image_paths
            .iter()
            .find(|path| {
                annotagent_image_tools::load_image(path, 40_000_000)
                    .is_ok_and(|frame| frame.metadata.sha256 == source_sha256)
            })
            .cloned()
    } else {
        image_paths.first().cloned()
    }
    .ok_or_else(|| ApiError::bad_request("the annotated source image is unavailable"))?;
    let frame = annotagent_image_tools::load_image(&image_path, 40_000_000)
        .map_err(ApiError::bad_request)?;
    let image_id = annotations
        .first()
        .map(|annotation| annotation.image_id)
        .ok_or_else(|| ApiError::bad_request("run has no annotations"))?;
    let root = project_path
        .parent()
        .unwrap_or(state.application.workspace());
    let snapshot = ProjectSnapshot {
        schema,
        images: vec![SnapshotImage {
            id: image_id,
            relative_path: image_path
                .strip_prefix(root)
                .unwrap_or(&image_path)
                .to_path_buf(),
            metadata: frame.metadata,
        }],
        annotations,
        revisions: state
            .application
            .store()
            .history(run.id)
            .map_err(ApiError::internal)?
            .revisions,
    };
    let output = root.join("exports").join(&request.format);
    let exporter: Box<dyn DatasetExporter> = match request.format.as_str() {
        "native" => Box::new(NativeExporter),
        "coco" => Box::new(CocoExporter),
        "yolo" | "yolo_detection" => Box::new(YoloDetectionExporter),
        "yolo_segmentation" => Box::new(YoloSegmentationExporter),
        "labelme" => Box::new(LabelMeExporter),
        other => {
            return Err(ApiError::bad_request(format!(
                "unknown export format {other:?}"
            )));
        }
    };
    let report = exporter
        .export(ExportRequest {
            project: snapshot,
            output,
        })
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(json!(report)))
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
            Value::String("system_keychain".to_owned()),
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
            Some(3)
        );
        let hybrid_draft = response_json(
            request(
                &service,
                axum::http::Method::POST,
                "/api/workflow-drafts",
                Some(json!({
                    "project_id": "workflow-ui",
                    "template_id": "accurate-hybrid"
                })),
            )
            .await,
        )
        .await;
        assert_eq!(hybrid_draft["name"], json!("Accurate hybrid"));
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
        assert_eq!(project["name"], json!("RoboCup Demo Dataset"));
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
        assert!(reviews["reviews"][0]["validation_issues"].is_array());
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
        let reason_code = skill
            .correction_taxonomy()
            .into_iter()
            .next()
            .expect("correction taxonomy")
            .code;
        let decision = request(
            &service,
            axum::http::Method::POST,
            &format!("/api/reviews/{review_id}/decision"),
            Some(json!({
                "project_id": "review-demo",
                "decision": "reject",
                "reason_code": reason_code,
                "note": "deterministic server test"
            })),
        )
        .await;
        assert_eq!(decision.status(), StatusCode::OK);
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
                &format!("/api/annotations/{review_id}/revisions"),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(revisions["revisions"].as_array().map(Vec::len), Some(1));

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
    async fn settings_and_api_key_survive_server_restart() {
        let temp = tempfile::tempdir().expect("temp");
        let secrets = Arc::new(MemorySecretStore::default());
        let application = Arc::new(LocalApplication::new(temp.path()).expect("application"));
        let service = router(test_state(application, secrets.clone()), None);

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
        assert!(saved.get("api_key").is_none());

        let settings_path = temp.path().join(".annotagent/settings.toml");
        let persisted = std::fs::read_to_string(&settings_path).expect("persisted settings");
        assert!(persisted.contains("persisted-vision-model"));
        assert!(!persisted.contains("test-secret-that-must-not-reach-disk"));

        let restarted_application =
            Arc::new(LocalApplication::new(temp.path()).expect("restarted application"));
        let restarted = router(test_state(restarted_application, secrets.clone()), None);
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
            json!("workflow_advisor")
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
        let exported = request(
            &service,
            axum::http::Method::POST,
            "/api/projects/http-label/export",
            Some(json!({"format": "native"})),
        )
        .await;
        assert_eq!(exported.status(), StatusCode::OK);
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
            Some(2)
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
