//! Shared application service used by CLI/TUI and HTTP frontends.

mod published_run;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use annotagent_core::{
    AdditionalUsage, Annotation, AnnotationSource, ArtifactKind, BatchBudgetLedger,
    BatchBudgetLimits, BatchId, BatchImageCheckpoint, BatchImageStatus, BatchNodeState,
    BatchProgress, BatchRecord, BatchStatus, BatchUsage, Budget, DatasetImporter, DomainSkill,
    ImageId, ImportIssue, ImportReport, ImportRequest, ModelMessage, ModelRegistry, ModelRequest,
    ModelRole, NodeRegistry, PricingConfig, ProjectId, ProjectSchema, PublishedWorkflowVersion,
    RegistryWorkflowAdvisor, RunEvent, RunEventKind, RunEventPayload, RunId, RunStatus,
    SnapshotImage, TaskRunStatus, TokenUsage, ToolDefinition, UsageSource, VisionCapability,
    VisionInferenceRequest, VisionInputType, VisionModelDescriptor, VisionModelHealth,
    VisionModelHealthStatus, VisionModelLimits, VisionModelProvider, VisionNodeDescriptor,
    WORKFLOW_SCHEMA_VERSION, WorkflowAdvisor, WorkflowAdvisorInput, WorkflowConstraints,
    WorkflowDataProfile, WorkflowDraft, WorkflowDraftStatus, WorkflowDryRunNodeResult,
    WorkflowDryRunReport, WorkflowDryRunSampleResult, WorkflowSnapshot, WorkflowStaticValidator,
    WorkflowSuggestion, WorkflowValidationIssue, WorkflowValidationReport,
    WorkflowVersionComparison, all_artifact_kinds,
};
use annotagent_export::{
    CocoImporter, LabelMeImporter, NativeImporter, YoloDetectionImporter, YoloSegmentationImporter,
};
use annotagent_image_tools::{generate_synthetic_robocup, load_image, to_model_image};
use annotagent_provider::{
    MockResponseSpec, MockScript, MockStep, MockUsage, MockVisionBackend, MockVisionProvider,
    OpenAiCompatibleConfig, OpenAiCompatibleProvider,
};
use annotagent_runtime::{
    AgentLoopConfig, AgentRuntime, ImageRunRequest, ImageRunResult, RunControl, RuntimeStore,
    SkillRegistry,
};
use annotagent_skill_robocup::RoboCupSkill;
use annotagent_storage::{BatchClaimResult, HistoryRun, RunStartReservation, SqliteStore};
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use futures::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{broadcast, watch};
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

use published_run::{ApplicationImageRuntime, PublishedWorkflowRuntime};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default = "default_provider_kind")]
    pub default_provider: String,
    pub provider: OpenAiCompatibleConfig,
    pub pricing: PricingConfig,
    pub budget: Budget,
}

#[derive(Debug, Deserialize)]
struct LiveWorkflowAdvice {
    name: String,
    #[serde(default)]
    model_bindings: Vec<LiveWorkflowBinding>,
    #[serde(default)]
    review_gate_node_ids: Vec<String>,
    #[serde(default)]
    rationale: Vec<String>,
    #[serde(default)]
    unresolved_model_bindings: Vec<String>,
    #[serde(default)]
    warnings: Vec<String>,
    #[serde(default)]
    alternatives: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LiveWorkflowBinding {
    node_id: String,
    model_id: String,
}

fn default_provider_kind() -> String {
    "mock".to_owned()
}

pub fn validate_settings(settings: &Settings) -> Result<()> {
    if !matches!(
        settings.default_provider.as_str(),
        "mock" | "openai_compatible"
    ) {
        bail!("default_provider must be either \"mock\" or \"openai_compatible\"");
    }
    OpenAiCompatibleProvider::new(settings.provider.clone()).map_err(|error| anyhow!(error))?;
    Ok(())
}

pub struct PreparedRun {
    runtime: Arc<dyn ApplicationImageRuntime>,
    request: ImageRunRequest,
    image_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub dataset: ProjectDatasetSummary,
    pub annotation_schema: Vec<AnnotationTaskSummary>,
    pub enabled_skills: Vec<EnabledSkill>,
    pub workflows: Vec<WorkflowSummary>,
    pub active_workflow: WorkflowVersion,
    pub available_workflow_versions: Vec<WorkflowVersion>,
    pub model_bindings: Vec<ModelBinding>,
    pub export_formats: Vec<String>,
    /// Compatibility field for v1 clients. New clients use `enabled_skills`.
    pub skill_id: String,
    pub image_count: usize,
    pub active_batch: Option<BatchRecord>,
    pub active_batch_progress: Option<BatchProgress>,
    pub active_run: Option<HistoryRun>,
    pub last_run: Option<HistoryRun>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDatasetSummary {
    pub root: String,
    pub include: Vec<String>,
    pub recursive: bool,
    pub image_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnnotationTaskSummary {
    pub id: String,
    pub kind: String,
    pub labels: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnabledSkill {
    pub id: String,
    pub display_name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelBinding {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub role: String,
    pub scope: String,
    pub health_status: String,
    pub health_detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Draft,
    Valid,
    Published,
    Archived,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowNodeSummary {
    pub id: String,
    pub node_type: String,
    pub depends_on: Vec<String>,
    pub model_binding: Option<String>,
    pub validators: Vec<String>,
    pub refiners: Vec<String>,
    pub human_review_gate: bool,
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowVersion {
    pub workflow_id: String,
    pub name: String,
    pub version: String,
    pub status: WorkflowStatus,
    pub validation_status: String,
    pub is_default: bool,
    pub source: String,
    pub nodes: Vec<WorkflowNodeSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowSummary {
    pub id: String,
    pub name: String,
    pub current_version: String,
    pub status: WorkflowStatus,
    pub validation_status: String,
    pub is_default: bool,
    pub node_count: usize,
}

fn task_kind_name(kind: annotagent_core::TaskKind) -> String {
    match kind {
        annotagent_core::TaskKind::Classification => "classification",
        annotagent_core::TaskKind::BoundingBox => "bounding_box",
        annotagent_core::TaskKind::Keypoints => "keypoints",
        annotagent_core::TaskKind::Polyline => "polyline",
        annotagent_core::TaskKind::Polygon => "polygon",
        annotagent_core::TaskKind::SemanticMask => "semantic_mask",
        annotagent_core::TaskKind::InstanceMask => "instance_mask",
        annotagent_core::TaskKind::Attributes => "attributes",
        annotagent_core::TaskKind::Relations => "relations",
    }
    .to_owned()
}

fn compatibility_workflow(
    project: &ProjectSchema,
    skills: &[Arc<dyn DomainSkill>],
) -> WorkflowVersion {
    let graph_nodes = skills
        .iter()
        .flat_map(|skill| skill.workflow().nodes)
        .collect::<Vec<_>>();
    let dependency_map: HashMap<_, _> = graph_nodes
        .into_iter()
        .map(|node| (node.id, node.depends_on))
        .collect();
    let nodes = project
        .tasks
        .iter()
        .map(|task| WorkflowNodeSummary {
            id: task.id.to_string(),
            node_type: task_kind_name(task.kind),
            depends_on: dependency_map
                .get(&task.id)
                .unwrap_or(&task.depends_on)
                .iter()
                .map(ToString::to_string)
                .collect(),
            model_binding: Some("default-vision".to_owned()),
            validators: task.validators.clone(),
            refiners: task.refiners.clone(),
            human_review_gate: true,
            fallback: Some("bounded retry, then human review".to_owned()),
        })
        .collect();
    WorkflowVersion {
        workflow_id: format!(
            "{}-configured",
            if skills.is_empty() {
                "generic".to_owned()
            } else {
                skills
                    .iter()
                    .map(|skill| skill.id())
                    .collect::<Vec<_>>()
                    .join("+")
            }
        ),
        name: "Configured task graph".to_owned(),
        version: project.version.to_string(),
        status: WorkflowStatus::Published,
        validation_status: "valid".to_owned(),
        is_default: true,
        source: if skills.is_empty() {
            "project tasks".to_owned()
        } else {
            "project tasks + registered Skill graphs".to_owned()
        },
        nodes,
    }
}

fn published_workflow_summary(
    version: &PublishedWorkflowVersion,
    is_default: bool,
) -> WorkflowVersion {
    WorkflowVersion {
        workflow_id: version.workflow_id.clone(),
        name: version.draft.name.clone(),
        version: version.version.to_string(),
        status: WorkflowStatus::Published,
        validation_status: "valid".to_owned(),
        is_default,
        source: format!("published draft {}", version.source_draft_id),
        nodes: version
            .draft
            .nodes
            .iter()
            .map(|node| WorkflowNodeSummary {
                id: node.id.clone(),
                node_type: node.node_type.clone(),
                depends_on: node.depends_on.clone(),
                model_binding: node.model_binding.clone(),
                validators: node.validators.clone(),
                refiners: node.refiners.clone(),
                human_review_gate: node.review_gate || node.gate.required,
                fallback: node
                    .fallback_policy
                    .target_node
                    .clone()
                    .or_else(|| node.fallback.clone()),
            })
            .collect(),
    }
}

fn workflow_catalog(settings: &Settings) -> Result<(NodeRegistry, ModelRegistry)> {
    let capabilities = vec![
        VisionCapability::VisionLanguage,
        VisionCapability::ObjectDetection,
        VisionCapability::SemanticSegmentation,
        VisionCapability::InstanceSegmentation,
        VisionCapability::PromptedSegmentation,
        VisionCapability::Classification,
        VisionCapability::KeypointDetection,
    ];
    let mut models = ModelRegistry::new();
    models.register_backend(Arc::new(MockVisionBackend::new(
        "workspace-provider-adapter",
        capabilities,
        Vec::new(),
    )))?;
    models.register_model(VisionModelDescriptor {
        id: "default-vision".to_owned(),
        display_name: "Workspace default vision model".to_owned(),
        backend_id: "workspace-provider-adapter".to_owned(),
        capabilities: vec![VisionCapability::VisionLanguage],
        input_types: vec![VisionInputType::Image, VisionInputType::Text],
        output_types: all_artifact_kinds().to_vec(),
        model: settings.provider.model.clone(),
        model_version: "provider-managed".to_owned(),
        endpoint_or_path: Some(settings.provider.endpoint.clone()),
        health: VisionModelHealth {
            status: if settings.default_provider == "mock" {
                VisionModelHealthStatus::Healthy
            } else {
                VisionModelHealthStatus::Unknown
            },
            detail: Some(if settings.default_provider == "mock" {
                "offline deterministic fixture available".to_owned()
            } else {
                "configured; health is verified on the next request".to_owned()
            }),
            checked_at: Some(chrono::Utc::now()),
        },
        limits: VisionModelLimits {
            max_images: Some(1),
            timeout_seconds: Some(settings.provider.request_timeout_seconds),
            ..VisionModelLimits::default()
        },
        secret_reference: (settings.default_provider != "mock")
            .then(|| format!("env:{}", settings.provider.api_key_env)),
        configuration: BTreeMap::from([
            ("provider".to_owned(), json!(settings.default_provider)),
            ("model".to_owned(), json!(settings.provider.model)),
        ]),
        ..VisionModelDescriptor::default()
    })?;

    let mut nodes = NodeRegistry::new();
    let artifact_kinds = all_artifact_kinds().to_vec();
    for (id, capability, produces, deterministic) in [
        (
            "vision_language",
            Some(VisionCapability::VisionLanguage),
            artifact_kinds.clone(),
            false,
        ),
        (
            "object_detection",
            Some(VisionCapability::ObjectDetection),
            vec![ArtifactKind::BoundingBox],
            false,
        ),
        (
            "semantic_segmentation",
            Some(VisionCapability::SemanticSegmentation),
            vec![ArtifactKind::SemanticMask],
            false,
        ),
        (
            "instance_segmentation",
            Some(VisionCapability::InstanceSegmentation),
            vec![ArtifactKind::InstanceMask],
            false,
        ),
        (
            "prompted_segmentation",
            Some(VisionCapability::PromptedSegmentation),
            vec![ArtifactKind::InstanceMask],
            false,
        ),
        (
            "classification",
            Some(VisionCapability::Classification),
            vec![ArtifactKind::Classification],
            false,
        ),
        (
            "keypoint_detection",
            Some(VisionCapability::KeypointDetection),
            vec![ArtifactKind::Keypoints],
            false,
        ),
        (
            "deterministic_cv",
            None,
            vec![ArtifactKind::Polygon, ArtifactKind::Polyline],
            true,
        ),
        (
            "field_line_refiner",
            None,
            vec![ArtifactKind::Polyline],
            true,
        ),
        ("static_validator", None, artifact_kinds.clone(), true),
        ("review_gate", None, artifact_kinds.clone(), true),
        ("commit", None, artifact_kinds, true),
    ] {
        nodes.register(VisionNodeDescriptor {
            id: id.to_owned(),
            display_name: id.replace('_', " "),
            required_capabilities: capability.into_iter().collect(),
            accepts: all_artifact_kinds().to_vec(),
            produces,
            deterministic,
        })?;
    }
    Ok((nodes, models))
}

#[derive(Debug, Clone, Serialize)]
pub struct StartedRun {
    pub run_id: RunId,
    pub image_path: PathBuf,
    pub status: RunStatus,
    pub idempotent: bool,
}

#[derive(Debug, Clone)]
pub struct ActiveRunExists {
    pub active_run_id: RunId,
    pub status: RunStatus,
}

impl std::fmt::Display for ActiveRunExists {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "active run {} already exists with status {:?}",
            self.active_run_id, self.status
        )
    }
}

impl std::error::Error for ActiveRunExists {}

#[derive(Debug, Clone)]
pub struct DatasetImageResult {
    pub image_path: PathBuf,
    pub result: ImageRunResult,
}

#[derive(Debug, Clone)]
pub struct DatasetBatchExecution {
    pub batch: BatchRecord,
    pub results: Vec<DatasetImageResult>,
}

pub struct DatasetCoordinator<'a> {
    application: &'a LocalApplication,
}

impl<'a> DatasetCoordinator<'a> {
    #[must_use]
    pub const fn new(application: &'a LocalApplication) -> Self {
        Self { application }
    }

    pub async fn run(
        &self,
        project_path: &Path,
        provider: &str,
        config_path: Option<&Path>,
        limit: Option<usize>,
    ) -> Result<Vec<DatasetImageResult>> {
        let batch = self.create(project_path, provider, config_path, limit)?;
        Ok(self.execute(batch.id, None).await?.results)
    }

    pub fn create(
        &self,
        project_path: &Path,
        provider: &str,
        config_path: Option<&Path>,
        limit: Option<usize>,
    ) -> Result<BatchRecord> {
        self.create_with_workflow(project_path, provider, config_path, limit, None)
    }

    pub fn create_with_workflow(
        &self,
        project_path: &Path,
        provider: &str,
        config_path: Option<&Path>,
        limit: Option<usize>,
        workflow: Option<(&str, u32)>,
    ) -> Result<BatchRecord> {
        let project_path = project_path.canonicalize()?;
        ensure_within(&self.application.workspace, &project_path)?;
        let (project, project_skills) =
            load_project_schema_with_registry(&project_path, &self.application.skills)?;
        let settings = load_settings(config_path)?;
        let mut images = self
            .application
            .list_images_for_project_path(&project_path)?;
        if let Some(limit) = limit {
            images.truncate(limit);
        }
        if images.is_empty() {
            bail!("project has no supported images");
        }
        let relative_project_path = project_path
            .strip_prefix(&self.application.workspace)
            .context("project path is outside the workspace")?
            .to_string_lossy()
            .into_owned();
        let image_records = images
            .iter()
            .map(|path| {
                Ok((
                    ImageId::new(),
                    path.strip_prefix(&self.application.workspace)
                        .context("image path is outside the workspace")?
                        .to_string_lossy()
                        .into_owned(),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let project_id = project_path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_owned();
        let published_workflow = workflow
            .map(|(workflow_id, version)| {
                let published = self
                    .application
                    .store
                    .get_published_workflow_version(workflow_id, version)?;
                if published.project_id != project_id {
                    bail!(
                        "published workflow {workflow_id:?} version {version} belongs to project {:?}, not {project_id:?}",
                        published.project_id
                    );
                }
                Ok(published)
            })
            .transpose()?;
        let compatibility = compatibility_workflow(&project, &project_skills);
        let workflow_version = published_workflow.as_ref().map_or_else(
            || compatibility.version.clone(),
            |published| format!("{}@{}", published.workflow_id, published.version),
        );
        let workflow_snapshot = published_workflow.as_ref().map_or_else(
            || {
                json!({
                    "workflow": compatibility,
                    "settings": settings,
                })
            },
            |published| {
                json!({
                    "published_workflow": published,
                    "settings": settings,
                })
            },
        );
        let now = chrono::Utc::now();
        if self
            .application
            .store
            .list_batches(true)?
            .iter()
            .any(|batch| batch.project_id == project_id)
        {
            bail!("project {project_id:?} already has an active dataset batch");
        }
        let stable_id =
            stable_project_id(project_path.parent().unwrap_or(&self.application.workspace));
        if self.application.store.list_runs()?.iter().any(|run| {
            run.project_id == Some(stable_id)
                && matches!(
                    run.status,
                    RunStatus::Pending
                        | RunStatus::Running
                        | RunStatus::Paused
                        | RunStatus::AwaitingReview
                )
        }) {
            bail!("project {project_id:?} already has an active image Run");
        }
        let record = BatchRecord {
            id: BatchId::new(),
            project_id,
            project_path: relative_project_path,
            provider: provider.to_owned(),
            status: BatchStatus::Pending,
            max_concurrency: u32::try_from(project.runtime.max_parallel_images.max(1))
                .unwrap_or(u32::MAX),
            workflow_version,
            workflow_snapshot,
            project_snapshot: serde_json::to_value(&project)?,
            budget_limits: batch_budget_limits(&settings.budget, now),
            budget_ledger: BatchBudgetLedger::default(),
            lease_owner: None,
            lease_expires_at: None,
            event_sequence: 0,
            created_at: now,
            updated_at: now,
        };
        Ok(self
            .application
            .store
            .create_batch(record, &image_records)?)
    }

    pub async fn execute(
        &self,
        batch_id: BatchId,
        temporary_api_key: Option<String>,
    ) -> Result<DatasetBatchExecution> {
        const LEASE_DURATION: std::time::Duration = std::time::Duration::from_secs(30);
        let stored = self.application.store.get_batch(batch_id)?;
        let settings: Settings = serde_json::from_value(
            stored
                .workflow_snapshot
                .get("settings")
                .cloned()
                .context("batch snapshot lacks settings")?,
        )?;
        let published_workflow = stored
            .workflow_snapshot
            .get("published_workflow")
            .cloned()
            .map(serde_json::from_value::<PublishedWorkflowVersion>)
            .transpose()
            .context("batch snapshot contains an invalid Published Workflow Version")?;
        let project_path = self.application.workspace.join(&stored.project_path);
        let owner = format!("worker-{}", uuid::Uuid::new_v4());
        let batch = self.application.store.acquire_batch_lease(
            batch_id,
            &owner,
            LEASE_DURATION,
            chrono::Utc::now(),
        )?;
        let heartbeat_cancel = CancellationToken::new();
        let heartbeat_store = self.application.store.clone();
        let heartbeat_owner = owner.clone();
        let heartbeat_token = heartbeat_cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
            interval.tick().await;
            loop {
                tokio::select! {
                    () = heartbeat_token.cancelled() => break,
                    _ = interval.tick() => {
                        if heartbeat_store.renew_batch_lease(
                            batch_id,
                            &heartbeat_owner,
                            LEASE_DURATION,
                            chrono::Utc::now(),
                        ).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let concurrency = usize::try_from(batch.max_concurrency.max(1)).unwrap_or(usize::MAX);
        let worker_results = stream::iter(0..concurrency)
            .map(|_| {
                self.run_batch_worker(
                    batch_id,
                    &owner,
                    &project_path,
                    &settings,
                    temporary_api_key.clone(),
                    published_workflow.clone(),
                )
            })
            .buffer_unordered(concurrency)
            .collect::<Vec<_>>()
            .await;
        heartbeat_cancel.cancel();
        let mut results = Vec::new();
        for worker_result in worker_results {
            results.extend(worker_result?);
        }
        let current = self.application.store.get_batch(batch_id)?;
        let batch = if current.status == BatchStatus::Running {
            self.application
                .store
                .finalize_batch(batch_id, &owner, chrono::Utc::now())?
        } else {
            current
        };
        Ok(DatasetBatchExecution { batch, results })
    }

    pub async fn resume(
        &self,
        batch_id: BatchId,
        temporary_api_key: Option<String>,
    ) -> Result<DatasetBatchExecution> {
        let batch = self.application.store.get_batch(batch_id)?;
        match batch.status {
            BatchStatus::Paused | BatchStatus::Pending => {
                if batch.status == BatchStatus::Paused {
                    self.application.store.set_batch_status(
                        batch_id,
                        BatchStatus::Pending,
                        chrono::Utc::now(),
                    )?;
                }
            }
            BatchStatus::Failed | BatchStatus::Partial => {
                self.application
                    .store
                    .retry_failed_batch_images(batch_id, chrono::Utc::now())?;
            }
            status => bail!("batch {batch_id} cannot resume from {status:?}"),
        }
        self.execute(batch_id, temporary_api_key).await
    }

    pub fn pause(&self, batch_id: BatchId) -> Result<BatchRecord> {
        Ok(self.application.store.set_batch_status(
            batch_id,
            BatchStatus::Paused,
            chrono::Utc::now(),
        )?)
    }

    pub fn cancel(&self, batch_id: BatchId) -> Result<BatchRecord> {
        let child_run_ids = self
            .application
            .store
            .list_batch_images(batch_id)?
            .into_iter()
            .filter_map(|image| image.child_run_id)
            .collect::<BTreeSet<_>>();
        let batch = self.application.store.set_batch_status(
            batch_id,
            BatchStatus::Cancelled,
            chrono::Utc::now(),
        )?;
        if let Ok(active) = self.application.active.lock() {
            for (_, managed) in active
                .iter()
                .filter(|(run_id, managed)| child_run_ids.contains(run_id) && managed.is_active())
            {
                let _ignored = managed.control.cancel();
            }
        }
        Ok(batch)
    }

    async fn run_batch_worker(
        &self,
        batch_id: BatchId,
        owner: &str,
        project_path: &Path,
        settings: &Settings,
        temporary_api_key: Option<String>,
        published_workflow: Option<PublishedWorkflowVersion>,
    ) -> Result<Vec<DatasetImageResult>> {
        let reservation = batch_image_reservation(settings);
        let mut completed = Vec::new();
        loop {
            let claim = match self.application.store.claim_batch_image(
                batch_id,
                owner,
                &reservation,
                chrono::Utc::now(),
            ) {
                Ok(claim) => claim,
                Err(annotagent_storage::StorageError::BatchLeaseConflict(_)) => break,
                Err(error) => return Err(error.into()),
            };
            let image = match claim {
                BatchClaimResult::Claimed(image) => image,
                BatchClaimResult::Empty | BatchClaimResult::BudgetExceeded(_) => break,
            };
            let image_path = self.application.workspace.join(&image.image_path);
            let prepared = prepare_run_with_settings(
                project_path,
                &self.application.store.get_batch(batch_id)?.provider,
                settings.clone(),
                temporary_api_key.clone(),
                self.application.store.clone(),
                &self.application.skills,
                Some(&image_path),
                Some(image.image_id),
                published_workflow.clone(),
            );
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(error) => {
                    self.application.store.finish_batch_image(
                        batch_id,
                        image.image_id,
                        owner,
                        BatchImageStatus::Failed,
                        &BatchUsage::default(),
                        &BatchImageCheckpoint::default(),
                        Some(&error.to_string()),
                        chrono::Utc::now(),
                    )?;
                    continue;
                }
            };
            let child_run_id = prepared.request.run_id;
            if let Err(error) = self.application.store.mark_batch_image_running(
                batch_id,
                image.image_id,
                owner,
                child_run_id,
                chrono::Utc::now(),
            ) {
                if matches!(
                    error,
                    annotagent_storage::StorageError::BatchLeaseConflict(_)
                ) {
                    break;
                }
                return Err(error.into());
            }
            let started = self.application.start_prepared(prepared, false, None)?;
            match self.application.wait_run(started.run_id).await {
                Ok(result) => {
                    let usage = batch_usage(&result.usage);
                    let checkpoint = self.batch_image_checkpoint(started.run_id, &result)?;
                    let image_status = batch_image_status(result.status);
                    let error = matches!(image_status, BatchImageStatus::Failed).then(|| {
                        result
                            .issues
                            .iter()
                            .map(|issue| format!("{}: {}", issue.code, issue.message))
                            .collect::<Vec<_>>()
                            .join("; ")
                    });
                    self.application.store.finish_batch_image(
                        batch_id,
                        image.image_id,
                        owner,
                        image_status,
                        &usage,
                        &checkpoint,
                        error.as_deref(),
                        chrono::Utc::now(),
                    )?;
                    completed.push(DatasetImageResult { image_path, result });
                }
                Err(error) => {
                    self.application.store.finish_batch_image(
                        batch_id,
                        image.image_id,
                        owner,
                        BatchImageStatus::Failed,
                        &BatchUsage::default(),
                        &BatchImageCheckpoint::default(),
                        Some(&error.to_string()),
                        chrono::Utc::now(),
                    )?;
                }
            }
            let _ignored = self.application.store.renew_batch_lease(
                batch_id,
                owner,
                std::time::Duration::from_secs(30),
                chrono::Utc::now(),
            );
            let batch = self.application.store.get_batch(batch_id)?;
            if batch.status != BatchStatus::Running {
                break;
            }
        }
        Ok(completed)
    }

    fn batch_image_checkpoint(
        &self,
        run_id: RunId,
        result: &ImageRunResult,
    ) -> Result<BatchImageCheckpoint> {
        let task_runs = self.application.store.list_task_runs(run_id)?;
        let events = self.application.store.list_events(run_id)?;
        let artifacts = self.application.store.list_artifacts(run_id)?;
        let mut node_states = BTreeMap::new();
        let mut retry_counters = BTreeMap::new();
        let mut review_suspensions = BTreeSet::new();
        for task in task_runs {
            let status = serde_json::to_value(task.status)?
                .as_str()
                .unwrap_or("unknown")
                .to_owned();
            let retries = events
                .iter()
                .filter(|event| {
                    event.task_id.as_ref() == Some(&task.task_id)
                        && event.kind == RunEventKind::RetryScheduled
                })
                .count();
            let retries = u32::try_from(retries).unwrap_or(u32::MAX);
            if task.status == TaskRunStatus::NeedsReview {
                review_suspensions.insert(task.task_id.to_string());
            }
            retry_counters.insert(task.task_id.to_string(), retries);
            node_states.insert(
                task.task_id.to_string(),
                BatchNodeState {
                    status,
                    artifact_references: artifacts
                        .iter()
                        .filter(|artifact| artifact.task_id.as_ref() == Some(&task.task_id))
                        .map(|artifact| artifact.id)
                        .collect(),
                    retry_count: retries,
                    review_suspended: task.status == TaskRunStatus::NeedsReview,
                },
            );
        }
        Ok(BatchImageCheckpoint {
            node_states,
            artifact_references: artifacts.iter().map(|artifact| artifact.id).collect(),
            retry_counters,
            review_suspensions,
            runtime_checkpoint: Some(json!({
                "run_id": run_id,
                "status": result.status,
                "committed": result.committed.len(),
                "review": result.review_queue.len(),
            })),
        })
    }
}

fn batch_budget_limits(
    budget: &Budget,
    started_at: chrono::DateTime<chrono::Utc>,
) -> BatchBudgetLimits {
    BatchBudgetLimits {
        max_input_tokens: budget.max_input_tokens,
        max_output_tokens: budget.max_output_tokens,
        max_total_tokens: budget.max_total_tokens,
        max_request_count: budget.max_requests,
        max_image_count: budget.max_images,
        max_cost: budget.max_cost,
        wall_clock_deadline: budget.max_wall_clock_seconds.and_then(|seconds| {
            chrono::Duration::try_seconds(i64::try_from(seconds).ok()?)
                .map(|duration| started_at + duration)
        }),
    }
}

fn batch_image_reservation(settings: &Settings) -> BatchUsage {
    let output_tokens = u64::from(settings.provider.max_output_tokens);
    let token_usage = TokenUsage::known(0, output_tokens, UsageSource::Estimated);
    let additional = AdditionalUsage {
        image_count: 1,
        request_count: 1,
        ..AdditionalUsage::default()
    };
    BatchUsage {
        output_tokens,
        total_tokens: output_tokens,
        request_count: 1,
        image_count: 1,
        cost: settings.pricing.calculate(&token_usage, &additional).total,
        ..BatchUsage::default()
    }
}

fn batch_usage(usage: &annotagent_core::UsageTotals) -> BatchUsage {
    BatchUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        total_tokens: usage.total_tokens,
        request_count: usage.requests,
        image_count: 1,
        cost: usage.cost,
    }
}

const fn batch_image_status(status: RunStatus) -> BatchImageStatus {
    match status {
        RunStatus::Completed | RunStatus::Partial => BatchImageStatus::Completed,
        RunStatus::CompletedWithReview | RunStatus::AwaitingReview => {
            BatchImageStatus::AwaitingReview
        }
        RunStatus::Cancelled => BatchImageStatus::Cancelled,
        RunStatus::Failed | RunStatus::BudgetExceeded | RunStatus::Interrupted => {
            BatchImageStatus::Failed
        }
        RunStatus::Pending | RunStatus::Running | RunStatus::Paused => BatchImageStatus::Failed,
    }
}

#[derive(Clone)]
struct ManagedRun {
    project_name: String,
    control: RunControl,
    result: watch::Receiver<Option<Result<ImageRunResult, String>>>,
}

impl ManagedRun {
    fn is_active(&self) -> bool {
        self.result.borrow().is_none()
    }
}

#[async_trait]
pub trait AnnotAgentApplication: Send + Sync {
    async fn start_run(&self, project_id: &str, provider: &str) -> Result<StartedRun>;
    async fn pause_run(&self, run_id: RunId) -> Result<()>;
    async fn resume_run(&self, run_id: RunId) -> Result<()>;
    async fn cancel_run(&self, run_id: RunId) -> Result<()>;
    async fn wait_run(&self, run_id: RunId) -> Result<ImageRunResult>;
    fn subscribe(&self) -> broadcast::Receiver<RunEvent>;
    fn list_projects(&self) -> Result<Vec<ProjectSummary>>;
    fn list_runs(&self) -> Result<Vec<HistoryRun>>;
    fn list_events(&self, run_id: RunId) -> Result<Vec<RunEvent>>;
}

pub struct LocalApplication {
    workspace: PathBuf,
    database_path: PathBuf,
    store: Arc<SqliteStore>,
    skills: Arc<SkillRegistry>,
    event_sender: broadcast::Sender<RunEvent>,
    active: Mutex<HashMap<RunId, ManagedRun>>,
}

impl LocalApplication {
    pub fn new(workspace: impl AsRef<Path>) -> Result<Self> {
        let workspace = workspace.as_ref();
        std::fs::create_dir_all(workspace)
            .with_context(|| format!("cannot create workspace {}", workspace.display()))?;
        let workspace = workspace
            .canonicalize()
            .with_context(|| format!("cannot canonicalize workspace {}", workspace.display()))?;
        let database_path = workspace.join(".annotagent/history.db");
        Self::with_database(workspace, database_path)
    }

    pub fn with_database(
        workspace: impl AsRef<Path>,
        database_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let workspace = workspace
            .as_ref()
            .canonicalize()
            .context("cannot canonicalize workspace")?;
        if let Some(parent) = database_path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        let database_path = database_path.as_ref().to_path_buf();
        let store = Arc::new(SqliteStore::open(&database_path)?);
        store.reconcile_interrupted_runs()?;
        store.recover_orphaned_batch_leases(chrono::Utc::now())?;
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(
            RoboCupSkill::new().map_err(|error| anyhow!(error))?,
        ))?;
        let (event_sender, _) = broadcast::channel(1024);
        Ok(Self {
            workspace,
            database_path,
            store,
            skills: Arc::new(registry),
            event_sender,
            active: Mutex::new(HashMap::new()),
        })
    }

    #[must_use]
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    #[must_use]
    pub fn store(&self) -> Arc<SqliteStore> {
        self.store.clone()
    }

    pub async fn create_human_annotation(
        &self,
        run_id: RunId,
        mut annotation: Annotation,
    ) -> Result<Annotation> {
        let existing = self.store.list_annotations(run_id)?;
        if !existing.is_empty()
            && !existing
                .iter()
                .any(|item| item.image_id == annotation.image_id)
        {
            bail!("new annotation image_id does not belong to this Run");
        }
        annotation.source = AnnotationSource::Human;
        annotation.review_status = annotagent_core::ReviewStatus::NeedsReview;
        annotation.confidence = None;
        self.store
            .commit_annotation(run_id, &annotation)
            .await
            .map_err(|error| anyhow!(error))?;
        Ok(annotation)
    }

    pub async fn import_project_annotations(
        &self,
        project_id: &str,
        format: &str,
        source: &Path,
        label_mapping: BTreeMap<String, String>,
        dry_run: bool,
    ) -> Result<ImportReport> {
        let source = source
            .canonicalize()
            .with_context(|| format!("cannot access import source {}", source.display()))?;
        ensure_within(&self.workspace, &source)?;
        let project_path = self.project_path(project_id)?;
        let (schema, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let mut project_runs = self
            .store
            .list_runs()?
            .into_iter()
            .filter(|run| run.project_name == schema.project.name)
            .collect::<Vec<_>>();
        project_runs.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        let mut run_by_image = BTreeMap::new();
        for run in &project_runs {
            for annotation in self.store.list_annotations(run.id)? {
                run_by_image.entry(annotation.image_id).or_insert(run.id);
            }
        }
        let mut batch_binding_by_path = BTreeMap::new();
        for batch in self
            .store
            .list_batches(false)?
            .into_iter()
            .filter(|batch| batch.project_id == project_id)
        {
            for image in self.store.list_batch_images(batch.id)? {
                if let Some(run_id) = image.child_run_id {
                    run_by_image.insert(image.image_id, run_id);
                    batch_binding_by_path
                        .insert(PathBuf::from(image.image_path), (image.image_id, run_id));
                }
            }
        }
        let discovered_paths = self.list_project_images(project_id)?;
        let single_image_fallback = (discovered_paths.len() == 1 && run_by_image.len() == 1)
            .then(|| run_by_image.iter().next().map(|(id, run)| (*id, *run)))
            .flatten();
        let root = project_path.parent().unwrap_or(&self.workspace);
        let mut known_images = Vec::new();
        for path in discovered_paths {
            let frame = load_image(&path, 40_000_000).map_err(|error| anyhow!(error))?;
            let binding = batch_binding_by_path
                .iter()
                .find(|(batch_path, _)| {
                    batch_path.as_path() == path
                        || batch_path
                            .file_name()
                            .is_some_and(|name| path.file_name() == Some(name))
                })
                .map(|(_, binding)| *binding)
                .or(single_image_fallback);
            if let Some((image_id, run_id)) = binding {
                run_by_image.insert(image_id, run_id);
            }
            known_images.push(SnapshotImage {
                id: binding.map_or_else(ImageId::new, |(image_id, _)| image_id),
                relative_path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                metadata: frame.metadata,
            });
        }
        let importer: Box<dyn DatasetImporter> = match format {
            "native" => Box::new(NativeImporter),
            "coco" => Box::new(CocoImporter),
            "labelme" => Box::new(LabelMeImporter),
            "yolo" | "yolo_detection" => Box::new(YoloDetectionImporter),
            "yolo_segmentation" => Box::new(YoloSegmentationImporter),
            other => bail!(
                "unknown annotation import format {other:?}; choose native, coco, labelme, yolo_detection, or yolo_segmentation"
            ),
        };
        let mut report = importer
            .import(ImportRequest {
                project_schema: schema,
                known_images,
                source,
                label_mapping,
                dry_run,
            })
            .await
            .map_err(|error| anyhow!(error))?;
        if dry_run {
            return Ok(report);
        }
        let mut persisted = Vec::new();
        for mut annotation in report.annotations.drain(..) {
            if self.store.find_annotation(annotation.id)?.is_some() {
                report.issues.push(ImportIssue {
                    record: annotation.id.to_string(),
                    message: "annotation id already exists in workspace history".to_owned(),
                });
                continue;
            }
            let Some(run_id) = run_by_image.get(&annotation.image_id).copied() else {
                report.issues.push(ImportIssue {
                    record: annotation.id.to_string(),
                    message: "no persisted Run owns this image; run the Project image before importing annotations"
                        .to_owned(),
                });
                continue;
            };
            annotation.review_status = annotagent_core::ReviewStatus::NeedsReview;
            if let Err(error) = self.store.commit_annotation(run_id, &annotation).await {
                report.issues.push(ImportIssue {
                    record: annotation.id.to_string(),
                    message: error,
                });
                continue;
            }
            persisted.push(annotation);
        }
        let persisted_ids = persisted
            .iter()
            .map(|annotation| annotation.id)
            .collect::<BTreeSet<_>>();
        for revision in report
            .revisions
            .iter()
            .filter(|revision| persisted_ids.contains(&revision.annotation_id))
        {
            self.store
                .record_revision(revision)
                .await
                .map_err(|error| anyhow!(error))?;
        }
        report.annotations = persisted;
        report.imported_count = report.annotations.len() as u64;
        report.skipped_count = report.issues.len() as u64;
        Ok(report)
    }

    #[must_use]
    pub fn skills(&self) -> Arc<SkillRegistry> {
        self.skills.clone()
    }

    pub fn active_run_for_project(&self, project_name: &str) -> Result<Option<RunId>> {
        Ok(self
            .active
            .lock()
            .map_err(|_| anyhow!("active run lock poisoned"))?
            .iter()
            .find_map(|(run_id, managed)| {
                (managed.project_name == project_name && managed.is_active()).then_some(*run_id)
            }))
    }

    pub fn is_run_controllable(&self, run_id: RunId) -> bool {
        self.active
            .lock()
            .ok()
            .and_then(|active| active.get(&run_id).cloned())
            .is_some_and(|managed| managed.is_active())
    }

    pub fn project_path(&self, project_id: &str) -> Result<PathBuf> {
        validate_project_id(project_id)?;
        let directory = self.workspace.join(project_id);
        let canonical = directory
            .canonicalize()
            .with_context(|| format!("project {project_id:?} does not exist"))?;
        ensure_within(&self.workspace, &canonical)?;
        let path = canonical.join("project.yaml");
        if !path.is_file() {
            bail!("project {project_id:?} has no project.yaml");
        }
        Ok(path)
    }

    pub fn create_project(&self, project_id: &str, yaml: &str) -> Result<ProjectSummary> {
        validate_project_id(project_id)?;
        let project = ProjectSchema::from_yaml(yaml).map_err(|error| anyhow!(error))?;
        resolve_project_skills(&project, &self.skills)?;
        let directory = self.workspace.join(project_id);
        if directory.exists() {
            bail!("project {project_id:?} already exists");
        }
        std::fs::create_dir_all(directory.join(&project.dataset.root))?;
        std::fs::write(directory.join("project.yaml"), yaml)?;
        self.get_project(project_id)
    }

    pub fn get_project(&self, project_id: &str) -> Result<ProjectSummary> {
        let path = self.project_path(project_id)?;
        let (project, project_skills) = load_project_schema_with_registry(&path, &self.skills)?;
        let dataset = path
            .parent()
            .unwrap_or(&self.workspace)
            .join(&project.dataset.root);
        let image_count = supported_images(&dataset).count();
        let stable_id = stable_project_id(path.parent().unwrap_or(&self.workspace));
        let project_runs = self
            .store
            .list_runs()?
            .into_iter()
            .filter(|run| {
                run.project_id == Some(stable_id)
                    || (run.project_id.is_none() && run.project_name == project.project.name)
            })
            .collect::<Vec<_>>();
        let active_run = project_runs
            .iter()
            .find(|run| {
                matches!(
                    run.status,
                    RunStatus::Pending
                        | RunStatus::Running
                        | RunStatus::Paused
                        | RunStatus::AwaitingReview
                )
            })
            .cloned();
        let active_batch = self
            .store
            .list_batches(true)?
            .into_iter()
            .find(|batch| batch.project_id == project_id);
        let active_batch_progress = active_batch
            .as_ref()
            .map(|batch| self.store.batch_progress(batch.id))
            .transpose()?;
        let last_run = project_runs
            .iter()
            .find(|run| {
                !matches!(
                    run.status,
                    RunStatus::Pending
                        | RunStatus::Running
                        | RunStatus::Paused
                        | RunStatus::AwaitingReview
                )
            })
            .cloned();
        let compatibility = compatibility_workflow(&project, &project_skills);
        let published = self
            .store
            .list_published_workflow_versions(Some(project_id))?;
        let mut available_workflow_versions = published
            .iter()
            .enumerate()
            .map(|(index, version)| {
                published_workflow_summary(version, index + 1 == published.len())
            })
            .collect::<Vec<_>>();
        if available_workflow_versions.is_empty() {
            available_workflow_versions.push(compatibility.clone());
        }
        let active_workflow = available_workflow_versions
            .last()
            .cloned()
            .unwrap_or_else(|| compatibility.clone());
        let workflows = available_workflow_versions
            .iter()
            .map(|workflow| WorkflowSummary {
                id: workflow.workflow_id.clone(),
                name: workflow.name.clone(),
                current_version: workflow.version.clone(),
                status: workflow.status,
                validation_status: workflow.validation_status.clone(),
                is_default: workflow.is_default,
                node_count: workflow.nodes.len(),
            })
            .collect();
        let model_bindings = active_run
            .as_ref()
            .or(last_run.as_ref())
            .as_ref()
            .map(|run| {
                vec![ModelBinding {
                    id: "default-vision".to_owned(),
                    provider: run.provider.clone(),
                    model: run.model.clone(),
                    role: "vision".to_owned(),
                    scope: "latest_run".to_owned(),
                    health_status: if run.status == RunStatus::Failed {
                        "degraded".to_owned()
                    } else {
                        "unknown".to_owned()
                    },
                    health_detail: Some("health at execution time was not recorded".to_owned()),
                }]
            })
            .unwrap_or_default();
        Ok(ProjectSummary {
            id: project_id.to_owned(),
            name: project.project.name.clone(),
            description: None,
            dataset: ProjectDatasetSummary {
                root: project.dataset.root.to_string_lossy().into_owned(),
                include: project.dataset.include.clone(),
                recursive: project.dataset.recursive,
                image_count,
            },
            annotation_schema: project
                .tasks
                .iter()
                .map(|task| AnnotationTaskSummary {
                    id: task.id.to_string(),
                    kind: task_kind_name(task.kind),
                    labels: task.labels.clone(),
                    required: task.required,
                })
                .collect(),
            enabled_skills: project_skills
                .iter()
                .map(|skill| EnabledSkill {
                    id: skill.id().to_owned(),
                    display_name: skill.manifest().display_name.clone(),
                    version: project
                        .project
                        .enabled_skill_versions()
                        .get(skill.id())
                        .cloned()
                        .unwrap_or_else(|| skill.manifest().version.to_string()),
                })
                .collect(),
            workflows,
            active_workflow,
            available_workflow_versions,
            model_bindings,
            export_formats: project.export.formats.clone(),
            skill_id: project
                .project
                .enabled_skill_versions()
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(","),
            image_count,
            active_batch,
            active_batch_progress,
            active_run,
            last_run,
        })
    }

    pub fn list_project_images(&self, project_id: &str) -> Result<Vec<PathBuf>> {
        let project_path = self.project_path(project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let root = project_path
            .parent()
            .unwrap_or(&self.workspace)
            .join(project.dataset.root);
        let mut images: Vec<_> = supported_images(&root).collect();
        images.sort();
        Ok(images)
    }

    pub fn list_workflow_drafts(&self, project_id: Option<&str>) -> Result<Vec<WorkflowDraft>> {
        if let Some(project_id) = project_id {
            validate_project_id(project_id)?;
        }
        Ok(self.store.list_workflow_drafts(project_id)?)
    }

    pub fn workflow_advisor_input(
        &self,
        project_id: &str,
        settings: &Settings,
        constraints: WorkflowConstraints,
    ) -> Result<WorkflowAdvisorInput> {
        let project_path = self.project_path(project_id)?;
        let (project, project_skills) =
            load_project_schema_with_registry(&project_path, &self.skills)?;
        let (nodes, models) = workflow_catalog(settings)?;
        let enabled_skills = project_skills
            .iter()
            .map(|skill| skill.id().to_owned())
            .collect::<Vec<_>>();
        let extensions = self.skills.validation_catalog_for(&enabled_skills)?;
        let images = self.list_project_images(project_id)?;
        let sample = images
            .first()
            .map(|path| load_image(path, 40_000_000))
            .transpose()
            .map_err(|error| anyhow!(error))?;
        let mut mime_types = BTreeSet::new();
        if let Some(image) = sample.as_ref() {
            mime_types.insert(image.metadata.mime_type.clone());
        }
        Ok(WorkflowAdvisorInput {
            project_id: project_id.to_owned(),
            project_schema: project,
            enabled_skills,
            node_catalog: nodes.nodes(),
            model_registry: models.models(),
            validator_ids: extensions.validators.into_iter().collect(),
            refiner_ids: extensions.refiners.into_iter().collect(),
            resource_ids: extensions.resources.into_iter().collect(),
            workflow_templates: project_skills
                .iter()
                .flat_map(|skill| skill.workflow_templates())
                .collect(),
            constraints,
            data_profile: WorkflowDataProfile {
                image_count: images.len(),
                sample_width: sample.as_ref().map(|image| image.metadata.width),
                sample_height: sample.as_ref().map(|image| image.metadata.height),
                mime_types,
            },
        })
    }

    pub fn create_workflow_draft(
        &self,
        project_id: &str,
        settings: &Settings,
        from_template: bool,
    ) -> Result<WorkflowDraft> {
        self.create_workflow_draft_with_template(project_id, settings, from_template, None)
    }

    pub fn create_workflow_draft_with_template(
        &self,
        project_id: &str,
        settings: &Settings,
        from_template: bool,
        template_id: Option<&str>,
    ) -> Result<WorkflowDraft> {
        let project_path = self.project_path(project_id)?;
        let (project, project_skills) =
            load_project_schema_with_registry(&project_path, &self.skills)?;
        let now = chrono::Utc::now();
        let draft = if let Some(template_id) = template_id {
            let available = project_skills
                .iter()
                .flat_map(|skill| skill.workflow_templates())
                .collect::<Vec<_>>();
            let template = available
                .iter()
                .find(|template| template.id == template_id)
                .ok_or_else(|| {
                    anyhow!("workflow template {template_id:?} is not provided by an enabled Skill")
                })?;
            template.instantiate(project_id, project.project.enabled_skill_versions(), now)
        } else if from_template {
            let (nodes, models) = workflow_catalog(settings)?;
            let mut draft = RegistryWorkflowAdvisor
                .suggest_workflow(
                    project_id,
                    &project,
                    &project_skills
                        .iter()
                        .map(|skill| skill.id().to_owned())
                        .collect::<Vec<_>>(),
                    &nodes,
                    &models,
                    &WorkflowConstraints::default(),
                )
                .draft;
            draft.id = uuid::Uuid::new_v4().to_string();
            draft.name = format!("{} template workflow", project.project.name);
            draft.status = WorkflowDraftStatus::Editing;
            draft.created_at = now;
            draft.updated_at = now;
            draft
        } else {
            WorkflowDraft {
                schema_version: WORKFLOW_SCHEMA_VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                project_id: project_id.to_owned(),
                name: format!("{} workflow", project.project.name),
                status: WorkflowDraftStatus::Editing,
                nodes: Vec::new(),
                edges: Vec::new(),
                enabled_skills: project.project.enabled_skill_versions(),
                resource_versions: BTreeMap::new(),
                allow_unvalidated_commit: false,
                created_at: now,
                updated_at: now,
            }
        };
        self.store.save_workflow_draft(&draft)?;
        Ok(draft)
    }

    pub fn suggest_workflow(
        &self,
        project_id: &str,
        settings: &Settings,
        constraints: &WorkflowConstraints,
    ) -> Result<WorkflowSuggestion> {
        let project_path = self.project_path(project_id)?;
        let (project, project_skills) =
            load_project_schema_with_registry(&project_path, &self.skills)?;
        let (nodes, models) = workflow_catalog(settings)?;
        let suggestion = RegistryWorkflowAdvisor.suggest_workflow(
            project_id,
            &project,
            &project_skills
                .iter()
                .map(|skill| skill.id().to_owned())
                .collect::<Vec<_>>(),
            &nodes,
            &models,
            constraints,
        );
        self.store.save_workflow_draft(&suggestion.draft)?;
        Ok(suggestion)
    }

    pub async fn suggest_workflow_live(
        &self,
        project_id: &str,
        settings: &Settings,
        temporary_api_key: Option<String>,
        constraints: &WorkflowConstraints,
    ) -> Result<WorkflowSuggestion> {
        let input = self.workflow_advisor_input(project_id, settings, constraints.clone())?;
        let mut suggestion = self.suggest_workflow_preview(project_id, settings, constraints)?;
        let node_ids = suggestion
            .draft
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        let model_ids = input
            .model_registry
            .iter()
            .map(|model| model.id.clone())
            .collect::<Vec<_>>();
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            settings.provider.clone(),
            temporary_api_key,
        )
        .map_err(|error| anyhow!(error))?;
        let response = provider
            .complete(
                ModelRequest {
                    model: settings.provider.model.clone(),
                    task_id: "workflow_advisor".into(),
                    messages: vec![
                        ModelMessage {
                            role: ModelRole::System,
                            content: "You are the AnnotAgent Workflow Advisor. Return only the registered submit_workflow_advice action. Never emit code, shell commands, URLs, or unknown resource IDs. The result is always a Draft and is never executed automatically.".to_owned(),
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        },
                        ModelMessage {
                            role: ModelRole::User,
                            content: serde_json::to_string(&json!({
                                "advisor_input": input,
                                "safe_base_draft": &suggestion.draft,
                            }))?,
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        },
                    ],
                    images: Vec::new(),
                    tools: vec![ToolDefinition {
                        name: "submit_workflow_advice".to_owned(),
                        description: "Adjust only registered model bindings and review gates on the supplied safe base Draft.".to_owned(),
                        parameters: json!({
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["name", "model_bindings", "review_gate_node_ids", "rationale", "unresolved_model_bindings", "warnings", "alternatives"],
                            "properties": {
                                "name": {"type": "string", "minLength": 1, "maxLength": 160},
                                "model_bindings": {
                                    "type": "array",
                                    "uniqueItems": true,
                                    "items": {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "required": ["node_id", "model_id"],
                                        "properties": {
                                            "node_id": {"type": "string", "enum": node_ids.clone()},
                                            "model_id": {"type": "string", "enum": model_ids.clone()}
                                        }
                                    }
                                },
                                "review_gate_node_ids": {"type": "array", "items": {"type": "string", "enum": node_ids.clone()}, "uniqueItems": true},
                                "rationale": {"type": "array", "items": {"type": "string"}},
                                "unresolved_model_bindings": {"type": "array", "items": {"type": "string", "enum": node_ids.clone()}, "uniqueItems": true},
                                "warnings": {"type": "array", "items": {"type": "string"}},
                                "alternatives": {"type": "array", "items": {"type": "string"}}
                            }
                        }),
                        read_only: true,
                    }],
                    max_output_tokens: settings.provider.max_output_tokens,
                    temperature: 0.0,
                    extra: BTreeMap::new(),
                },
                CancellationToken::new(),
            )
            .await
            .map_err(|error| anyhow!(error))?;
        let call = response
            .tool_calls
            .into_iter()
            .find(|call| call.name == "submit_workflow_advice")
            .ok_or_else(|| anyhow!("Workflow Advisor did not submit a constrained Draft"))?;
        let advice: LiveWorkflowAdvice = serde_json::from_value(call.arguments)?;
        let known_nodes = node_ids.into_iter().collect::<BTreeSet<_>>();
        let known_models = model_ids.into_iter().collect::<BTreeSet<_>>();
        for binding in &advice.model_bindings {
            if !known_nodes.contains(&binding.node_id) || !known_models.contains(&binding.model_id)
            {
                bail!("Workflow Advisor referenced an unregistered node or model");
            }
        }
        if advice
            .review_gate_node_ids
            .iter()
            .any(|node_id| !known_nodes.contains(node_id))
        {
            bail!("Workflow Advisor referenced an unregistered review-gate node");
        }
        suggestion.draft.id = uuid::Uuid::new_v4().to_string();
        suggestion.draft.name = advice.name;
        suggestion.draft.status = WorkflowDraftStatus::Suggested;
        suggestion.draft.created_at = chrono::Utc::now();
        suggestion.draft.updated_at = suggestion.draft.created_at;
        for node in &mut suggestion.draft.nodes {
            if let Some(binding) = advice
                .model_bindings
                .iter()
                .find(|binding| binding.node_id == node.id)
            {
                node.model_binding = Some(binding.model_id.clone());
            }
            if advice.review_gate_node_ids.contains(&node.id) {
                node.review_gate = true;
                node.gate.required = true;
            }
        }
        suggestion.rationale = advice.rationale;
        suggestion.unresolved_model_bindings = advice.unresolved_model_bindings;
        suggestion.warnings = advice.warnings;
        suggestion.alternatives = advice.alternatives;

        let (nodes, models) = workflow_catalog(settings)?;
        let enabled_skills = suggestion
            .draft
            .enabled_skills
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let extension_ids = enabled_skills.iter().cloned().collect::<Vec<_>>();
        let extensions = self.skills.validation_catalog_for(&extension_ids)?;
        let report = WorkflowStaticValidator.validate_for_publish(
            &suggestion.draft,
            &nodes,
            &models,
            &extensions,
            &enabled_skills,
            false,
        );
        if !report.valid {
            bail!("Workflow Advisor output failed registry validation");
        }
        self.store.save_workflow_draft(&suggestion.draft)?;
        Ok(suggestion)
    }

    fn suggest_workflow_preview(
        &self,
        project_id: &str,
        settings: &Settings,
        constraints: &WorkflowConstraints,
    ) -> Result<WorkflowSuggestion> {
        let project_path = self.project_path(project_id)?;
        let (project, project_skills) =
            load_project_schema_with_registry(&project_path, &self.skills)?;
        let (nodes, models) = workflow_catalog(settings)?;
        Ok(RegistryWorkflowAdvisor.suggest_workflow(
            project_id,
            &project,
            &project_skills
                .iter()
                .map(|skill| skill.id().to_owned())
                .collect::<Vec<_>>(),
            &nodes,
            &models,
            constraints,
        ))
    }

    pub fn save_workflow_draft(&self, mut draft: WorkflowDraft) -> Result<WorkflowDraft> {
        self.project_path(&draft.project_id)?;
        if let Ok(existing) = self.store.get_workflow_draft(&draft.id)
            && matches!(
                existing.status,
                WorkflowDraftStatus::Published | WorkflowDraftStatus::Archived
            )
        {
            bail!("published or archived workflow drafts are immutable; clone it to a new draft");
        }
        draft.status = WorkflowDraftStatus::Editing;
        draft.updated_at = chrono::Utc::now();
        self.store.save_workflow_draft(&draft)?;
        Ok(draft)
    }

    pub fn archive_workflow_draft(&self, draft_id: &str) -> Result<WorkflowDraft> {
        let mut draft = self.store.get_workflow_draft(draft_id)?;
        self.project_path(&draft.project_id)?;
        draft.status = WorkflowDraftStatus::Archived;
        draft.updated_at = chrono::Utc::now();
        self.store.save_workflow_draft(&draft)?;
        Ok(draft)
    }

    pub fn clone_workflow_version(&self, workflow_id: &str, version: u32) -> Result<WorkflowDraft> {
        let published = self
            .store
            .get_published_workflow_version(workflow_id, version)?;
        self.project_path(&published.project_id)?;
        let now = chrono::Utc::now();
        let mut draft = published.draft;
        draft.id = uuid::Uuid::new_v4().to_string();
        draft.name = format!("{} (from v{version})", draft.name);
        draft.status = WorkflowDraftStatus::Editing;
        draft.created_at = now;
        draft.updated_at = now;
        self.store.save_workflow_draft(&draft)?;
        Ok(draft)
    }

    pub fn dry_run_workflow(
        &self,
        draft_id: &str,
        settings: &Settings,
    ) -> Result<WorkflowValidationReport> {
        let draft = self.store.get_workflow_draft(draft_id)?;
        let (nodes, models) = workflow_catalog(settings)?;
        let enabled_skills = draft
            .enabled_skills
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let enabled_skill_ids = enabled_skills.iter().cloned().collect::<Vec<_>>();
        let validation_catalog = self.skills.validation_catalog_for(&enabled_skill_ids)?;
        let report = WorkflowStaticValidator.validate_for_publish(
            &draft,
            &nodes,
            &models,
            &validation_catalog,
            &enabled_skills,
            false,
        );
        if report.valid
            && !matches!(
                draft.status,
                WorkflowDraftStatus::Published | WorkflowDraftStatus::Archived
            )
        {
            let mut validated = draft;
            validated.status = WorkflowDraftStatus::Validated;
            validated.updated_at = chrono::Utc::now();
            self.store.save_workflow_draft(&validated)?;
        }
        Ok(report)
    }

    pub async fn dry_run_workflow_samples(
        &self,
        draft_id: &str,
        settings: &Settings,
        image_indices: &[usize],
    ) -> Result<WorkflowDryRunReport> {
        let started = std::time::Instant::now();
        let mut validation = self.dry_run_workflow(draft_id, settings)?;
        let draft = self.store.get_workflow_draft(draft_id)?;
        let images = self.list_project_images(&draft.project_id)?;
        let selected = if image_indices.is_empty() {
            (0..images.len().min(3)).collect::<Vec<_>>()
        } else {
            image_indices.iter().copied().take(10).collect::<Vec<_>>()
        };
        let (nodes, models) = workflow_catalog(settings)?;
        let mut samples = Vec::new();
        if validation.valid {
            for index in selected {
                let path = images
                    .get(index)
                    .ok_or_else(|| anyhow!("image index {index} was not found"))?;
                let image = load_image(path, 40_000_000).map_err(|error| anyhow!(error))?;
                let model_image = to_model_image("workflow-dry-run", &image, 1280)
                    .map_err(|error| anyhow!(error))?;
                let sandbox_run_id = RunId::new();
                let sandbox_image_id = ImageId::new();
                let mut node_results = Vec::new();
                for node_id in &validation.execution_order {
                    let Some(draft_node) = draft.nodes.iter().find(|node| &node.id == node_id)
                    else {
                        continue;
                    };
                    let node_started = std::time::Instant::now();
                    let mut node_issues = Vec::new();
                    let status = if let Some(model_id) = draft_node.model_binding.as_deref() {
                        let (model, backend) =
                            models.resolve(model_id).map_err(|error| anyhow!(error))?;
                        let operation = nodes
                            .get(&draft_node.node_type)
                            .and_then(|descriptor| descriptor.required_capabilities.first())
                            .copied()
                            .unwrap_or(VisionCapability::VisionLanguage);
                        let response = backend
                            .infer(
                                VisionInferenceRequest {
                                    protocol_version:
                                        annotagent_core::VISION_WORKER_PROTOCOL_VERSION,
                                    request_id: uuid::Uuid::new_v4().to_string(),
                                    operation,
                                    run_id: sandbox_run_id,
                                    image_id: sandbox_image_id,
                                    task_id: draft_node.id.clone().into(),
                                    node_id: draft_node.id.clone(),
                                    model_id: model.id.clone(),
                                    image: Some(model_image.clone()),
                                    input_artifacts: Vec::new(),
                                    prompt: None,
                                    parameters: draft_node.parameters.clone(),
                                    timeout_ms: draft_node
                                        .resources
                                        .timeout_seconds
                                        .map(|seconds| seconds.saturating_mul(1_000)),
                                    cancellation_requested: false,
                                },
                                CancellationToken::new(),
                            )
                            .await
                            .map_err(|error| anyhow!(error))?;
                        if let Some(error) = response.error {
                            node_issues.push(WorkflowValidationIssue {
                                code: error.code,
                                path: format!("nodes.{node_id}"),
                                message: error.message,
                                blocking: true,
                            });
                            "failed_in_sandbox"
                        } else {
                            "completed_in_sandbox"
                        }
                    } else {
                        "completed_in_sandbox"
                    };
                    let output_types = if draft_node.outputs.is_empty() {
                        nodes
                            .get(&draft_node.node_type)
                            .map(|descriptor| descriptor.produces.clone())
                            .unwrap_or_default()
                    } else {
                        draft_node
                            .outputs
                            .iter()
                            .map(|port| port.artifact_type)
                            .collect()
                    };
                    node_results.push(WorkflowDryRunNodeResult {
                        node_id: node_id.clone(),
                        status: status.to_owned(),
                        output_types,
                        latency_ms: node_started
                            .elapsed()
                            .as_millis()
                            .try_into()
                            .unwrap_or(u64::MAX),
                        estimated_cost: "0".to_owned(),
                        issues: node_issues,
                    });
                }
                samples.push(WorkflowDryRunSampleResult {
                    image_name: path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("image")
                        .to_owned(),
                    width: image.metadata.width,
                    height: image.metadata.height,
                    nodes: node_results,
                });
            }
        }
        let execution_issues = samples
            .iter()
            .flat_map(|sample| sample.nodes.iter())
            .flat_map(|node| node.issues.iter().cloned())
            .collect::<Vec<_>>();
        if !execution_issues.is_empty() {
            validation.valid = false;
            validation.issues.extend(execution_issues);
        }
        Ok(WorkflowDryRunReport {
            sandbox: true,
            validation,
            samples,
            total_latency_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            estimated_cost: "0".to_owned(),
        })
    }

    pub fn compare_workflow_versions(
        &self,
        left_workflow_id: &str,
        left_version: u32,
        right_workflow_id: &str,
        right_version: u32,
    ) -> Result<WorkflowVersionComparison> {
        let left = self
            .store
            .get_published_workflow_version(left_workflow_id, left_version)?;
        let right = self
            .store
            .get_published_workflow_version(right_workflow_id, right_version)?;
        if left.project_id != right.project_id {
            bail!("workflow versions from different projects cannot be compared");
        }
        let left_nodes = left
            .draft
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node))
            .collect::<BTreeMap<_, _>>();
        let right_nodes = right
            .draft
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node))
            .collect::<BTreeMap<_, _>>();
        Ok(WorkflowVersionComparison {
            left_workflow_id: left.workflow_id,
            left_version: left.version,
            right_workflow_id: right.workflow_id,
            right_version: right.version,
            added_nodes: right_nodes
                .keys()
                .filter(|id| !left_nodes.contains_key(*id))
                .cloned()
                .collect(),
            removed_nodes: left_nodes
                .keys()
                .filter(|id| !right_nodes.contains_key(*id))
                .cloned()
                .collect(),
            changed_nodes: right_nodes
                .iter()
                .filter(|(id, node)| {
                    left_nodes
                        .get(*id)
                        .is_some_and(|left_node| *left_node != **node)
                })
                .map(|(id, _)| id.clone())
                .collect(),
            same_content: left.content_hash == right.content_hash,
        })
    }

    pub fn publish_workflow(
        &self,
        draft_id: &str,
        settings: &Settings,
    ) -> Result<PublishedWorkflowVersion> {
        let mut draft = self.store.get_workflow_draft(draft_id)?;
        if matches!(
            draft.status,
            WorkflowDraftStatus::Published | WorkflowDraftStatus::Archived
        ) {
            bail!("published or archived workflow drafts are immutable; clone it to a new draft");
        }
        let report = self.dry_run_workflow(draft_id, settings)?;
        if !report.valid {
            bail!("workflow has blocking static validation issues");
        }
        draft.status = WorkflowDraftStatus::Validated;
        draft.updated_at = chrono::Utc::now();
        let (nodes, models) = workflow_catalog(settings)?;
        let enabled_skills = draft
            .enabled_skills
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let enabled_skill_ids = enabled_skills.iter().cloned().collect::<Vec<_>>();
        let validation_catalog = self.skills.validation_catalog_for(&enabled_skill_ids)?;
        let publish_report = WorkflowStaticValidator.validate_for_publish(
            &draft,
            &nodes,
            &models,
            &validation_catalog,
            &enabled_skills,
            true,
        );
        if !publish_report.valid {
            bail!("workflow has unresolved bindings and cannot be published");
        }
        let snapshot = WorkflowSnapshot::frozen(&draft, &models, draft.enabled_skills.clone());
        let serialized = snapshot.content_hash_material()?;
        let content_hash = annotagent_image_tools::sha256(&serialized);
        Ok(self
            .store
            .publish_workflow_draft(&draft, content_hash, snapshot)?)
    }

    pub fn list_images_for_project_path(&self, project_path: &Path) -> Result<Vec<PathBuf>> {
        let canonical = project_path
            .canonicalize()
            .with_context(|| format!("cannot access {}", project_path.display()))?;
        ensure_within(&self.workspace, &canonical)?;
        let (project, _) = load_project_schema_with_registry(&canonical, &self.skills)?;
        let root = canonical
            .parent()
            .unwrap_or(&self.workspace)
            .join(project.dataset.root);
        let mut images: Vec<_> = supported_images(&root).collect();
        images.sort();
        Ok(images)
    }

    pub fn import_images(&self, project_id: &str, source: &Path) -> Result<(u64, u64)> {
        let project_path = self.project_path(project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let destination = project_path
            .parent()
            .unwrap_or(&self.workspace)
            .join(project.dataset.root);
        let canonical_source = source
            .canonicalize()
            .with_context(|| format!("cannot access import source {}", source.display()))?;
        reject_archive_source(&canonical_source)?;
        ensure_within(&self.workspace, &canonical_source).context(
            "HTTP imports may only reference workspace files; use the CLI for controlled external copies",
        )?;
        let mut hashes = BTreeSet::new();
        for path in supported_images(&destination) {
            if let Ok(bytes) = std::fs::read(path) {
                hashes.insert(annotagent_image_tools::sha256(&bytes));
            }
        }
        let mut imported = 0_u64;
        let mut duplicates = 0_u64;
        for source in supported_images(&canonical_source) {
            let bytes = std::fs::read(&source)?;
            if !hashes.insert(annotagent_image_tools::sha256(&bytes)) {
                duplicates += 1;
                continue;
            }
            let name = source.file_name().context("image has no file name")?;
            let target = unique_target(&destination, name);
            std::fs::copy(source, target)?;
            imported += 1;
        }
        Ok((imported, duplicates))
    }

    pub fn start_run_path(
        &self,
        project_path: &Path,
        provider: &str,
        config_path: Option<&Path>,
    ) -> Result<StartedRun> {
        let canonical = project_path
            .canonicalize()
            .with_context(|| format!("cannot access {}", project_path.display()))?;
        ensure_within(&self.workspace, &canonical)?;
        self.ensure_no_active_batch(&canonical)?;
        let prepared = prepare_run_with(
            &canonical,
            provider,
            config_path,
            self.store.clone(),
            &self.skills,
        )?;
        self.start_prepared(prepared, true, None)
    }

    pub fn start_run_path_with_settings(
        &self,
        project_path: &Path,
        provider: &str,
        settings: Settings,
        temporary_api_key: Option<String>,
    ) -> Result<StartedRun> {
        let canonical = project_path
            .canonicalize()
            .with_context(|| format!("cannot access {}", project_path.display()))?;
        ensure_within(&self.workspace, &canonical)?;
        self.ensure_no_active_batch(&canonical)?;
        let prepared = prepare_run_with_settings(
            &canonical,
            provider,
            settings,
            temporary_api_key,
            self.store.clone(),
            &self.skills,
            None,
            None,
            None,
        )?;
        self.start_prepared(prepared, true, None)
    }

    pub fn start_run_path_with_settings_idempotent(
        &self,
        project_path: &Path,
        provider: &str,
        settings: Settings,
        temporary_api_key: Option<String>,
        idempotency_key: Option<&str>,
    ) -> Result<StartedRun> {
        let canonical = project_path
            .canonicalize()
            .with_context(|| format!("cannot access {}", project_path.display()))?;
        ensure_within(&self.workspace, &canonical)?;
        self.ensure_no_active_batch(&canonical)?;
        let prepared = prepare_run_with_settings(
            &canonical,
            provider,
            settings,
            temporary_api_key,
            self.store.clone(),
            &self.skills,
            None,
            None,
            None,
        )?;
        self.start_prepared(prepared, true, idempotency_key)
    }

    pub fn start_run_path_with_settings_idempotent_workflow(
        &self,
        project_path: &Path,
        provider: &str,
        settings: Settings,
        temporary_api_key: Option<String>,
        idempotency_key: Option<&str>,
        workflow: Option<(&str, u32)>,
    ) -> Result<StartedRun> {
        let canonical = project_path
            .canonicalize()
            .with_context(|| format!("cannot access {}", project_path.display()))?;
        ensure_within(&self.workspace, &canonical)?;
        self.ensure_no_active_batch(&canonical)?;
        let published_workflow = workflow
            .map(|(workflow_id, version)| {
                let published = self
                    .store
                    .get_published_workflow_version(workflow_id, version)?;
                let project_id = canonical
                    .parent()
                    .and_then(Path::file_name)
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                if published.project_id != project_id {
                    bail!("selected Workflow Version belongs to a different Project");
                }
                Ok(published)
            })
            .transpose()?;
        let prepared = prepare_run_with_settings(
            &canonical,
            provider,
            settings,
            temporary_api_key,
            self.store.clone(),
            &self.skills,
            None,
            None,
            published_workflow,
        )?;
        self.start_prepared(prepared, true, idempotency_key)
    }

    pub fn start_run_image_path(
        &self,
        project_path: &Path,
        image_path: &Path,
        provider: &str,
        config_path: Option<&Path>,
    ) -> Result<StartedRun> {
        let project_path = project_path.canonicalize()?;
        let image_path = image_path.canonicalize()?;
        ensure_within(&self.workspace, &project_path)?;
        ensure_within(&self.workspace, &image_path)?;
        self.ensure_no_active_batch(&project_path)?;
        let settings = load_settings(config_path)?;
        let prepared = prepare_run_with_settings(
            &project_path,
            provider,
            settings,
            None,
            self.store.clone(),
            &self.skills,
            Some(&image_path),
            None,
            None,
        )?;
        self.start_prepared(prepared, false, None)
    }

    fn start_prepared(
        &self,
        prepared: PreparedRun,
        enforce_project_exclusivity: bool,
        idempotency_key: Option<&str>,
    ) -> Result<StartedRun> {
        let run_id = prepared.request.run_id;
        let project_name = prepared.request.project.project.name.clone();
        let image_path = prepared.image_path.clone();
        let control = prepared.runtime.control();
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow!("active run lock poisoned"))?;
        if enforce_project_exclusivity {
            if idempotency_key.is_none()
                && let Some((active_run_id, managed)) = active.iter().find(|(_, managed)| {
                    managed.project_name == project_name && managed.is_active()
                })
            {
                return Err(anyhow!(ActiveRunExists {
                    active_run_id: *active_run_id,
                    status: managed.control.status().unwrap_or(RunStatus::Running),
                }));
            }
            match self.store.reserve_project_run(
                prepared.request.project_id,
                run_id,
                idempotency_key,
            )? {
                RunStartReservation::Reserved => {}
                RunStartReservation::Idempotent { run_id, status } => {
                    return Ok(StartedRun {
                        run_id,
                        image_path,
                        status,
                        idempotent: true,
                    });
                }
                RunStartReservation::Conflict { run_id, status } => {
                    return Err(anyhow!(ActiveRunExists {
                        active_run_id: run_id,
                        status,
                    }));
                }
            }
        }
        let mut events = prepared.runtime.subscribe();
        let event_sender = self.event_sender.clone();
        tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                let terminal = event.payload_terminal();
                let _ignored = event_sender.send(event);
                if terminal {
                    break;
                }
            }
        });
        let (result_sender, result) = watch::channel(None);
        let runtime = prepared.runtime;
        let request = prepared.request;
        tokio::spawn(async move {
            let result = runtime
                .run_image(request)
                .await
                .map_err(|error| error.to_string());
            result_sender.send_replace(Some(result));
        });
        active.insert(
            run_id,
            ManagedRun {
                project_name,
                control,
                result,
            },
        );
        Ok(StartedRun {
            run_id,
            image_path,
            status: RunStatus::Pending,
            idempotent: false,
        })
    }

    fn ensure_no_active_batch(&self, project_path: &Path) -> Result<()> {
        let project_id = project_path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("project");
        if let Some(batch) = self
            .store
            .list_batches(true)?
            .into_iter()
            .find(|batch| batch.project_id == project_id)
        {
            bail!(
                "active dataset batch {} already exists with status {:?}",
                batch.id,
                batch.status
            );
        }
        Ok(())
    }

    fn managed(&self, run_id: RunId) -> Result<ManagedRun> {
        self.active
            .lock()
            .map_err(|_| anyhow!("active run lock poisoned"))?
            .get(&run_id)
            .cloned()
            .with_context(|| format!("run {run_id} is not active in this process"))
    }

    async fn record_control_event(
        &self,
        run_id: RunId,
        kind: RunEventKind,
        from: RunStatus,
        to: RunStatus,
    ) -> Result<()> {
        let event = RunEvent::new(
            run_id,
            kind,
            RunEventPayload::State {
                from: Some(from),
                to,
                reason: Some("requested by user".to_owned()),
            },
        );
        self.store
            .set_run_status(run_id, to, Some("requested by user"))
            .await
            .map_err(anyhow::Error::msg)?;
        self.store
            .record_event(&event)
            .await
            .map_err(anyhow::Error::msg)?;
        let _ignored = self.event_sender.send(event);
        Ok(())
    }
}

#[async_trait]
impl AnnotAgentApplication for LocalApplication {
    async fn start_run(&self, project_id: &str, provider: &str) -> Result<StartedRun> {
        let path = self.project_path(project_id)?;
        self.start_run_path(&path, provider, None)
    }

    async fn pause_run(&self, run_id: RunId) -> Result<()> {
        let from = self.managed(run_id)?.control.pause()?;
        self.record_control_event(run_id, RunEventKind::RunPaused, from, RunStatus::Paused)
            .await
    }

    async fn resume_run(&self, run_id: RunId) -> Result<()> {
        let from = self.managed(run_id)?.control.resume()?;
        self.record_control_event(run_id, RunEventKind::RunResumed, from, RunStatus::Running)
            .await
    }

    async fn cancel_run(&self, run_id: RunId) -> Result<()> {
        self.managed(run_id)?.control.cancel()?;
        Ok(())
    }

    async fn wait_run(&self, run_id: RunId) -> Result<ImageRunResult> {
        let mut result = self.managed(run_id)?.result;
        loop {
            if let Some(result) = result.borrow().clone() {
                return result.map_err(anyhow::Error::msg);
            }
            result
                .changed()
                .await
                .context("run completion channel closed")?;
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<RunEvent> {
        self.event_sender.subscribe()
    }

    fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let mut projects = Vec::new();
        for entry in std::fs::read_dir(&self.workspace)? {
            let entry = entry?;
            if entry.file_type()?.is_symlink() || !entry.file_type()?.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if entry.path().join("project.yaml").is_file()
                && let Ok(project) = self.get_project(&id)
            {
                projects.push(project);
            }
        }
        projects.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(projects)
    }

    fn list_runs(&self) -> Result<Vec<HistoryRun>> {
        Ok(self.store.list_runs()?)
    }

    fn list_events(&self, run_id: RunId) -> Result<Vec<RunEvent>> {
        Ok(self.store.list_events(run_id)?)
    }
}

pub fn prepare_run(
    project_path: &Path,
    provider_kind: &str,
    config_path: Option<&Path>,
) -> Result<PreparedRun> {
    let database = default_database_path()?;
    if let Some(parent) = database.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let store = Arc::new(SqliteStore::open(database)?);
    let mut skills = SkillRegistry::new();
    skills.register(Arc::new(
        RoboCupSkill::new().map_err(|error| anyhow!(error))?,
    ))?;
    prepare_run_with(project_path, provider_kind, config_path, store, &skills)
}

pub fn load_project(path: &Path) -> Result<(ProjectSchema, Arc<dyn DomainSkill>)> {
    let mut skills = SkillRegistry::new();
    skills.register(Arc::new(
        RoboCupSkill::new().map_err(|error| anyhow!(error))?,
    ))?;
    load_project_with_registry(path, &skills)
}

pub fn load_settings(path: Option<&Path>) -> Result<Settings> {
    let contents = if let Some(path) = path {
        std::fs::read_to_string(path)
            .with_context(|| format!("cannot read config {}", path.display()))?
    } else {
        include_str!("../../../config/default.toml").to_owned()
    };
    toml::from_str(&contents).context("invalid provider/pricing/budget config")
}

pub fn default_database_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()
        .context("cannot determine current directory")?
        .join(".annotagent/history.db"))
}

#[must_use]
pub fn is_supported_image(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png"
                )
            })
}

fn prepare_run_with(
    project_path: &Path,
    provider_kind: &str,
    config_path: Option<&Path>,
    store: Arc<SqliteStore>,
    skills: &SkillRegistry,
) -> Result<PreparedRun> {
    let settings = load_settings(config_path)?;
    prepare_run_with_settings(
        project_path,
        provider_kind,
        settings,
        None,
        store,
        skills,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_run_with_settings(
    project_path: &Path,
    provider_kind: &str,
    settings: Settings,
    temporary_api_key: Option<String>,
    store: Arc<SqliteStore>,
    skills: &SkillRegistry,
    image_override: Option<&Path>,
    image_id_override: Option<ImageId>,
    published_workflow: Option<PublishedWorkflowVersion>,
) -> Result<PreparedRun> {
    let (project, project_skills) = load_project_schema_with_registry(project_path, skills)?;
    let image_path = image_override.map_or_else(
        || find_or_generate_image(project_path, &project),
        |path| Ok(path.to_path_buf()),
    )?;
    let image = Arc::new(load_image(&image_path, 40_000_000).map_err(|error| anyhow!(error))?);
    let model_image = to_model_image("full-image", &image, 1280).map_err(|error| anyhow!(error))?;
    let runtime: Arc<dyn ApplicationImageRuntime> = if let Some(published) = published_workflow {
        let use_namespace = project_skills.len() > 1;
        let mut validators = BTreeMap::new();
        let mut refiners = BTreeMap::new();
        for skill in &project_skills {
            for validator in skill.validators() {
                let id = if use_namespace {
                    format!("{}.{}", skill.id(), validator.id())
                } else {
                    validator.id().to_owned()
                };
                validators.insert(id, validator);
            }
            for refiner in skill.refiners() {
                let id = if use_namespace {
                    format!("{}.{}", skill.id(), refiner.id())
                } else {
                    refiner.id().to_owned()
                };
                refiners.insert(id, refiner);
            }
        }
        Arc::new(PublishedWorkflowRuntime::new(
            published,
            provider_kind,
            &settings,
            temporary_api_key,
            store,
            validators,
            refiners,
        )?)
    } else {
        if project_skills.len() != 1 {
            bail!("Projects with zero or multiple Skills must select a Published Workflow Version");
        }
        let skill = project_skills[0].clone();
        let provider: Arc<dyn VisionModelProvider> = match provider_kind {
            "mock" => Arc::new(MockVisionProvider::new(mock_script(
                &project,
                skill.as_ref(),
            )?)),
            "openai_compatible" => Arc::new(
                OpenAiCompatibleProvider::new_with_api_key(
                    settings.provider.clone(),
                    temporary_api_key,
                )
                .map_err(|error| anyhow!(error))?,
            ),
            other => bail!("unknown provider {other:?}; choose mock or openai_compatible"),
        };
        let compatibility_snapshot = serde_json::json!({
            "schema_version": 1,
            "engine": "legacy_agent_runtime",
            "workflow": skill.workflow(),
            "skill_manifest": skill.manifest(),
            "project": &project,
            "model_binding": {
                "provider": provider.name(),
                "model": &settings.provider.model,
            }
        });
        Arc::new(
            AgentRuntime::new(
                skill,
                provider,
                store,
                settings.pricing,
                settings.budget,
                AgentLoopConfig {
                    model: settings.provider.model,
                    max_model_turns_per_task: project.runtime.max_model_turns_per_task,
                    max_tool_calls_per_task: project.runtime.max_tool_calls_per_task,
                    max_recovery_turns_per_task: project.runtime.max_recovery_turns_per_task,
                    task_timeout: std::time::Duration::from_secs(
                        project.runtime.task_timeout_seconds,
                    ),
                    provider_request_timeout: std::time::Duration::from_secs(
                        project
                            .runtime
                            .provider_request_timeout_seconds
                            .min(settings.provider.request_timeout_seconds),
                    ),
                    max_retries: project.runtime.max_retries,
                    max_output_tokens: settings.provider.max_output_tokens,
                    temperature: settings.provider.temperature,
                },
            )
            .with_workflow_snapshot_json(Some(serde_json::to_string(&compatibility_snapshot)?)),
        )
    };
    let project_root = project_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .context("cannot canonicalize project root")?;
    let project_id = stable_project_id(&project_root);
    Ok(PreparedRun {
        runtime,
        request: ImageRunRequest {
            run_id: RunId::new(),
            project_id,
            project_root,
            project: Arc::new(project),
            image_id: image_id_override.unwrap_or_default(),
            image,
            model_image: Some(model_image),
        },
        image_path,
    })
}

fn load_project_with_registry(
    path: &Path,
    skills: &SkillRegistry,
) -> Result<(ProjectSchema, Arc<dyn DomainSkill>)> {
    let (project, mut project_skills) = load_project_schema_with_registry(path, skills)?;
    if project_skills.len() != 1 {
        bail!(
            "legacy agent runtime requires exactly one enabled Skill; this Project has {}. Publish and run a Workflow version instead",
            project_skills.len()
        );
    }
    Ok((project, project_skills.remove(0)))
}

fn load_project_schema_with_registry(
    path: &Path,
    skills: &SkillRegistry,
) -> Result<(ProjectSchema, Vec<Arc<dyn DomainSkill>>)> {
    let yaml = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read project {}", path.display()))?;
    let project = ProjectSchema::from_yaml(&yaml).map_err(|error| anyhow!(error))?;
    let project_skills = resolve_project_skills(&project, skills)?;
    Ok((project, project_skills))
}

fn resolve_project_skills(
    project: &ProjectSchema,
    skills: &SkillRegistry,
) -> Result<Vec<Arc<dyn DomainSkill>>> {
    let enabled_ids = project
        .project
        .enabled_skill_versions()
        .into_keys()
        .collect::<Vec<_>>();
    let project_skills = enabled_ids
        .iter()
        .map(|id| skills.get(id).map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?;
    let catalog = skills.validation_catalog_for(&enabled_ids)?;
    let issues = project.validate(&catalog);
    if !issues.is_empty() {
        bail!(
            "project validation failed:\n{}",
            issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    Ok(project_skills)
}

fn find_or_generate_image(project_path: &Path, project: &ProjectSchema) -> Result<PathBuf> {
    let root = project_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&project.dataset.root);
    std::fs::create_dir_all(&root)?;
    if let Some(path) = supported_images(&root).next() {
        return Ok(path);
    }
    if project.project.skill != "robocup" {
        bail!("dataset has no supported image; import an image before running");
    }
    let path = root.join("synthetic-robocup.png");
    generate_synthetic_robocup(&path).map_err(|error| anyhow!(error))?;
    Ok(path)
}

fn supported_images(root: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| !entry.file_type().is_symlink())
        .map(walkdir::DirEntry::into_path)
        .filter(|path| is_supported_image(path))
}

fn mock_script(project: &ProjectSchema, skill: &dyn DomainSkill) -> Result<MockScript> {
    let known: BTreeSet<_> = project.tasks.iter().map(|task| task.id.as_str()).collect();
    let ordered = skill
        .workflow()
        .topological_order()
        .map_err(anyhow::Error::msg)?;
    Ok(MockScript {
        steps: ordered
            .into_iter()
            .filter(|task| known.contains(task.as_str()))
            .flat_map(|task| {
                let mut steps = mock_steps(task.as_str());
                if project.runtime.max_retries == 0 {
                    steps.truncate(1);
                }
                steps
            })
            .collect(),
    })
}

fn mock_steps(task: &str) -> Vec<MockStep> {
    if task == "objects" {
        return vec![
            scripted_submission(
                task,
                &json!([
                    {"label":"robot","value":{"kind":"bounding_box","rect":[0.225,0.445,0.07,0.2]},"attributes":{},"confidence":0.98},
                    {"label":"ball","value":{"kind":"bounding_box","rect":[0.219,0.615,0.036,0.03]},"attributes":{},"confidence":0.97}
                ]),
            ),
            scripted_submission(
                task,
                &json!([
                    {"label":"ball","value":{"kind":"bounding_box","rect":[0.547,0.75,0.038,0.06]},"attributes":{},"confidence":0.98}
                ]),
            ),
        ];
    }
    let annotations = match task {
        "scene_type" => {
            json!([{"label":"normal_field","value":{"kind":"classification","labels":["normal_field"]},"attributes":{},"confidence":0.99}])
        }
        "field_region" => {
            json!([{"label":"field","value":{"kind":"polygon","rings":[[[0.02,0.02],[0.98,0.02],[0.98,0.98],[0.02,0.98]]]},"attributes":{},"confidence":0.98}])
        }
        "field_line" => {
            json!([{"label":"white_field_line","value":{"kind":"polyline","points":[[0.08,0.47],[0.92,0.47]]},"attributes":{},"confidence":0.96}])
        }
        "penalty_mark" => {
            json!([{"label":"penalty_mark","value":{"kind":"keypoints","points":[{"name":"center","point":[0.775,0.695],"visible":true}]},"attributes":{},"confidence":0.97}])
        }
        "robot_attributes" => {
            json!([{"label":"robot","value":{"kind":"bounding_box","rect":[0.225,0.445,0.07,0.2]},"attributes":{"team_color":"red","state":"standing"},"confidence":0.98}])
        }
        _ => return Vec::new(),
    };
    vec![scripted_submission(task, &annotations)]
}

fn scripted_submission(task: &str, annotations: &serde_json::Value) -> MockStep {
    MockStep {
        expect_task: Some(task.to_owned()),
        expect_message_contains: None,
        response: MockResponseSpec::ToolCall {
            name: "submit_annotation_candidates".to_owned(),
            arguments: json!({"annotations": annotations}),
        },
        usage: MockUsage {
            input_tokens: 180,
            output_tokens: 45,
        },
    }
}

fn validate_project_id(project_id: &str) -> Result<()> {
    if project_id.is_empty()
        || project_id == "."
        || project_id == ".."
        || !project_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!("invalid project id {project_id:?}");
    }
    Ok(())
}

#[must_use]
pub fn stable_project_id(project_root: &Path) -> ProjectId {
    ProjectId(uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        project_root.to_string_lossy().as_bytes(),
    ))
}

fn ensure_within(workspace: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(workspace) {
        bail!(
            "path {} escapes workspace {}",
            path.display(),
            workspace.display()
        );
    }
    Ok(())
}

fn reject_archive_source(path: &Path) -> Result<()> {
    if path.is_file()
        && path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        bail!(
            "ZIP image import is not supported; archives are rejected before extraction to prevent path traversal"
        );
    }
    Ok(())
}

fn unique_target(directory: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let first = directory.join(name);
    if !first.exists() {
        return first;
    }
    let stem = Path::new(name)
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("image");
    let extension = Path::new(name)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("png");
    for index in 2.. {
        let candidate = directory.join(format!("{stem}-{index}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

trait TerminalEvent {
    fn payload_terminal(&self) -> bool;
}

impl TerminalEvent for RunEvent {
    fn payload_terminal(&self) -> bool {
        matches!(
            &self.payload,
            annotagent_core::RunEventPayload::State { to, .. }
                if matches!(
                    to,
                    annotagent_core::RunStatus::CompletedWithReview
                        | annotagent_core::RunStatus::Completed
                        | annotagent_core::RunStatus::Partial
                        | annotagent_core::RunStatus::Cancelled
                        | annotagent_core::RunStatus::BudgetExceeded
                        | annotagent_core::RunStatus::Failed
                        | annotagent_core::RunStatus::Interrupted
                )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENERIC_PROJECT: &str = r"
version: 1
project:
  name: Generic inspection
  language: en
dataset:
  root: images
runtime: {}
tasks: []
review:
  auto_accept_confidence: 0.95
  force_review_below: 0.5
export:
  formats: [json]
";

    const GENERIC_BBOX_PROJECT: &str = r"
version: 1
project:
  name: Generic component inspection
  language: en
dataset:
  root: images
runtime: {}
tasks:
  - id: components
    kind: bounding_box
    labels: [component]
    required: true
review:
  auto_accept_confidence: 0.9
  force_review_below: 0.5
export:
  formats: [native, coco]
";

    #[test]
    fn generic_project_and_workflow_need_no_robocup_skill() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        let project = application
            .create_project("generic", GENERIC_PROJECT)
            .expect("generic project");
        assert!(project.enabled_skills.is_empty());
        assert!(project.skill_id.is_empty());

        let settings = load_settings(None).expect("settings");
        let catalog = application
            .workflow_advisor_input("generic", &settings, WorkflowConstraints::default())
            .expect("generic catalog");
        assert!(catalog.workflow_templates.is_empty());
        let first = application
            .suggest_workflow("generic", &settings, &WorkflowConstraints::default())
            .expect("first workflow");
        let second = application
            .suggest_workflow("generic", &settings, &WorkflowConstraints::default())
            .expect("second workflow");
        assert_ne!(first.draft.id, second.draft.id);
        assert_eq!(
            application
                .list_workflow_drafts(Some("generic"))
                .expect("drafts")
                .len(),
            2
        );
        let encoded = serde_json::to_string(&(project, first, second)).expect("JSON");
        assert!(!encoded.to_ascii_lowercase().contains("robocup"));
    }

    #[tokio::test]
    async fn selected_published_workflow_executes_the_generic_dag_and_persists_checkpoint() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("generic-dag", GENERIC_BBOX_PROJECT)
            .expect("generic Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary.path().join("generic-dag/images/component.png"),
        )
        .expect("generic image");
        let settings = load_settings(None).expect("settings");
        let suggestion = application
            .suggest_workflow(
                "generic-dag",
                &settings,
                &WorkflowConstraints {
                    require_review_gate: false,
                    ..WorkflowConstraints::default()
                },
            )
            .expect("generic suggestion");
        let validation = application
            .dry_run_workflow(&suggestion.draft.id, &settings)
            .expect("validation");
        assert!(validation.valid, "{:#?}", validation.issues);
        let published = application
            .publish_workflow(&suggestion.draft.id, &settings)
            .expect("published Workflow");
        let started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("generic-dag/project.yaml"),
                "mock",
                settings,
                None,
                Some("generic-published-dag"),
                Some((&published.workflow_id, published.version)),
            )
            .expect("selected Published Workflow Run");
        let result = application.wait_run(started.run_id).await.expect("DAG Run");
        assert_eq!(result.status, RunStatus::Completed, "{:#?}", result.issues);
        assert_eq!(result.committed.len(), 1);
        let history = application
            .store
            .history(started.run_id)
            .expect("persisted history");
        assert_eq!(history.artifacts.len(), 1);
        assert_eq!(history.annotations.len(), 1);
        let snapshot: serde_json::Value = serde_json::from_str(
            history
                .run
                .workflow_snapshot_json
                .as_deref()
                .expect("Workflow snapshot"),
        )
        .expect("snapshot JSON");
        assert_eq!(snapshot["engine"], json!("published_dag_runtime"));
        assert!(!snapshot["checkpoint"].is_null());
        assert!(!snapshot.to_string().contains("legacy_agent_runtime"));
        assert!(
            history
                .events
                .iter()
                .any(|event| event.kind == RunEventKind::ArtifactCommitted
                    || event.kind == RunEventKind::ArtifactCreated)
        );
    }

    #[tokio::test]
    async fn dataset_batch_executes_the_exact_published_workflow_for_every_child_run() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("generic-batch", GENERIC_BBOX_PROJECT)
            .expect("generic Project");
        let image_root = temporary.path().join("generic-batch/images");
        for name in ["component-one.png", "component-two.png"] {
            annotagent_image_tools::generate_synthetic_inspection(&image_root.join(name))
                .expect("generic image");
        }
        let settings = load_settings(None).expect("settings");
        let suggestion = application
            .suggest_workflow(
                "generic-batch",
                &settings,
                &WorkflowConstraints {
                    require_review_gate: false,
                    ..WorkflowConstraints::default()
                },
            )
            .expect("generic suggestion");
        let published = application
            .publish_workflow(&suggestion.draft.id, &settings)
            .expect("Published Workflow");
        let coordinator = DatasetCoordinator::new(&application);
        let batch = coordinator
            .create_with_workflow(
                &temporary.path().join("generic-batch/project.yaml"),
                "mock",
                None,
                None,
                Some((&published.workflow_id, published.version)),
            )
            .expect("Published Workflow Batch");
        assert_eq!(
            batch.workflow_version,
            format!("{}@{}", published.workflow_id, published.version)
        );
        assert_eq!(
            batch.workflow_snapshot["published_workflow"]["content_hash"],
            json!(published.content_hash)
        );

        let execution = coordinator
            .execute(batch.id, None)
            .await
            .expect("Dataset Batch execution");
        assert_eq!(execution.batch.status, BatchStatus::Completed);
        assert_eq!(execution.results.len(), 2);
        for result in execution.results {
            let history = application
                .store
                .history(result.result.run_id)
                .expect("child Run history");
            let snapshot: serde_json::Value = serde_json::from_str(
                history
                    .run
                    .workflow_snapshot_json
                    .as_deref()
                    .expect("child Workflow snapshot"),
            )
            .expect("child snapshot JSON");
            assert_eq!(snapshot["engine"], json!("published_dag_runtime"));
            assert_eq!(
                snapshot["selected_workflow"]["content_hash"],
                json!(published.content_hash)
            );
            assert!(!snapshot["checkpoint"].is_null());
        }
    }

    #[test]
    fn workspace_rejects_traversal_and_symlink_escape() {
        let temp = tempfile::tempdir().expect("temp");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let app = LocalApplication::new(&workspace).expect("app");
        assert!(app.project_path("../outside").is_err());
        assert!(app.project_path("a/b").is_err());

        #[cfg(unix)]
        {
            let outside = temp.path().join("outside");
            std::fs::create_dir(&outside).expect("outside");
            std::fs::write(outside.join("project.yaml"), GENERIC_PROJECT).expect("outside project");
            std::os::unix::fs::symlink(&outside, workspace.join("linked-project"))
                .expect("project symlink");
            assert!(app.project_path("linked-project").is_err());
        }
    }

    #[test]
    fn image_import_rejects_zip_archives_before_extraction() {
        let temp = tempfile::tempdir().expect("temp");
        let workspace = temp.path().join("workspace");
        let app = LocalApplication::new(&workspace).expect("app");
        app.create_project("generic", GENERIC_PROJECT)
            .expect("project");
        let archive = workspace.join("malicious.zip");
        std::fs::write(&archive, b"PK\x03\x04../outside.png").expect("fake archive fixture");
        let error = app
            .import_images("generic", &archive)
            .expect_err("ZIP import must be rejected");
        assert!(error.to_string().contains("path traversal"));
        assert!(!temp.path().join("outside.png").exists());
    }

    #[test]
    fn project_summary_separates_project_skill_and_workflow() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        let summary = application
            .create_project(
                "robocup-demo",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("Project summary");

        assert_eq!(summary.name, "RoboCup Demo Dataset");
        assert_eq!(summary.enabled_skills.len(), 1);
        assert_eq!(summary.enabled_skills[0].id, "robocup");
        assert_eq!(summary.enabled_skills[0].display_name, "RoboCup Perception");
        assert_eq!(summary.active_workflow.name, "Configured task graph");
        assert_eq!(summary.active_workflow.status, WorkflowStatus::Published);
        assert_eq!(
            summary.workflows[0].node_count,
            summary.annotation_schema.len()
        );
        assert!(
            summary
                .active_workflow
                .nodes
                .iter()
                .any(|node| { node.id == "field_line" && node.depends_on == ["field_region"] })
        );
        assert!(summary.model_bindings.is_empty());
        let settings = load_settings(None).expect("settings");
        let catalog = application
            .workflow_advisor_input("robocup-demo", &settings, WorkflowConstraints::default())
            .expect("RoboCup catalog");
        assert_eq!(catalog.workflow_templates.len(), 3);
        let draft = application
            .create_workflow_draft_with_template(
                "robocup-demo",
                &settings,
                false,
                Some("accurate-hybrid"),
            )
            .expect("hybrid draft");
        assert_eq!(draft.name, "Accurate hybrid");
        assert_eq!(draft.enabled_skills.get("robocup"), Some(&"1".to_owned()));
    }

    #[test]
    fn exclusive_project_start_rejects_a_second_active_run() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        let summary = application
            .create_project(
                "robocup-demo",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("Project summary");
        let active_run_id = RunId::new();
        let (_result_sender, result) = watch::channel(None::<Result<ImageRunResult, String>>);
        application.active.lock().expect("active runs").insert(
            active_run_id,
            ManagedRun {
                project_name: summary.name,
                control: RunControl::new(),
                result,
            },
        );

        let error = application
            .start_run_path(
                &temporary.path().join("robocup-demo/project.yaml"),
                "mock",
                None,
            )
            .expect_err("duplicate run must be rejected");
        assert!(error.to_string().contains(&active_run_id.to_string()));
    }

    #[tokio::test]
    async fn startup_reconciles_stale_running_run_as_interrupted() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        let summary = application
            .create_project(
                "stale-demo",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("project");
        let project_path = application
            .project_path("stale-demo")
            .expect("project path");
        let project_id = stable_project_id(project_path.parent().expect("project root"));
        let run_id = RunId::new();
        application
            .store
            .reserve_project_run(project_id, run_id, None)
            .expect("reservation");
        application
            .store
            .create_run(&annotagent_runtime::RunRecord {
                id: run_id,
                project_id,
                project_name: summary.name,
                skill_id: "robocup".to_owned(),
                provider: "mock".to_owned(),
                model: "mock".to_owned(),
                status: RunStatus::Pending,
                project_schema_json: std::fs::read_to_string(project_path).expect("project YAML"),
                workflow_snapshot_json: None,
            })
            .await
            .expect("run");
        application
            .store
            .set_run_status(run_id, RunStatus::Running, None)
            .await
            .expect("running");
        drop(application);

        let restarted = LocalApplication::new(temporary.path()).expect("restarted application");
        let run = restarted
            .list_runs()
            .expect("runs")
            .into_iter()
            .find(|run| run.id == run_id)
            .expect("stale run");
        assert_eq!(run.status, RunStatus::Interrupted);
        assert!(
            run.terminal_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("no worker lease"))
        );
        let project = restarted
            .get_project("stale-demo")
            .expect("project summary");
        assert!(project.active_run.is_none());
        assert_eq!(
            project.last_run.as_ref().map(|run| run.status),
            Some(RunStatus::Interrupted)
        );
    }

    #[test]
    fn workflow_suggestion_edit_dry_run_and_publish_are_real() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "workflow-demo",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("project");
        let settings = load_settings(None).expect("settings");
        let suggestion = application
            .suggest_workflow(
                "workflow-demo",
                &settings,
                &WorkflowConstraints {
                    require_review_gate: true,
                    ..WorkflowConstraints::default()
                },
            )
            .expect("suggestion");
        assert!(suggestion.unresolved_model_bindings.is_empty());
        assert!(suggestion.draft.nodes.iter().all(|node| matches!(
            node.node_type.as_str(),
            "vision_language" | "static_validator" | "review_gate" | "commit"
        )));
        let mut edited = suggestion.draft;
        edited.nodes[0].fallback = Some("review_gate".to_owned());
        let edited = application
            .save_workflow_draft(edited)
            .expect("saved draft");
        assert_eq!(edited.status, WorkflowDraftStatus::Editing);
        let report = application
            .dry_run_workflow(&edited.id, &settings)
            .expect("dry run");
        assert!(report.valid, "{:#?}", report.issues);
        assert_eq!(report.execution_order.len(), edited.nodes.len());
        let version = application
            .publish_workflow(&edited.id, &settings)
            .expect("publish");
        assert_eq!(version.version, 1);
        assert!(!version.content_hash.is_empty());
        assert_eq!(version.draft.status, WorkflowDraftStatus::Published);
        assert!(
            application
                .save_workflow_draft(version.draft)
                .expect_err("published draft is immutable")
                .to_string()
                .contains("immutable")
        );
    }

    #[tokio::test]
    async fn workflow_alpha_editor_journey_is_persistent_and_version_explicit() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "workflow-alpha",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("project");
        let image_path = temporary.path().join("workflow-alpha/images/sample.png");
        generate_synthetic_robocup(&image_path).expect("sample image");
        let settings = load_settings(None).expect("settings");

        let blank = application
            .create_workflow_draft("workflow-alpha", &settings, false)
            .expect("blank draft");
        assert!(blank.nodes.is_empty());
        let mut draft = application
            .suggest_workflow(
                "workflow-alpha",
                &settings,
                &WorkflowConstraints {
                    require_review_gate: true,
                    ..WorkflowConstraints::default()
                },
            )
            .expect("mock Advisor draft")
            .draft;
        let target_index = draft
            .nodes
            .iter()
            .position(|node| !node.inputs.is_empty())
            .expect("typed input node");
        let original_type = draft.nodes[target_index].inputs[0].artifact_type;
        draft.nodes[target_index].inputs[0].artifact_type = ArtifactKind::Relations;
        let draft = application
            .save_workflow_draft(draft)
            .expect("invalid edit is persisted as a Draft");
        let invalid = application
            .dry_run_workflow(&draft.id, &settings)
            .expect("precise validation report");
        assert!(invalid.issues.iter().any(|issue| {
            issue.code == "artifact_type_mismatch"
                && issue
                    .path
                    .contains(&format!("nodes[{target_index}].inputs"))
        }));

        let mut fixed = draft;
        fixed.nodes[target_index].inputs[0].artifact_type = original_type;
        let fixed = application.save_workflow_draft(fixed).expect("fixed Draft");
        let dry_run = application
            .dry_run_workflow_samples(&fixed.id, &settings, &[0])
            .await
            .expect("sample Dry Run");
        assert!(dry_run.sandbox);
        assert!(dry_run.validation.valid, "{:#?}", dry_run.validation.issues);
        assert_eq!(dry_run.samples.len(), 1);
        assert_eq!(dry_run.samples[0].image_name, "sample.png");
        assert_eq!(dry_run.samples[0].nodes.len(), fixed.nodes.len());

        let published = application
            .publish_workflow(&fixed.id, &settings)
            .expect("published version");
        let frozen = published.draft.clone();
        let mut attempted_mutation = frozen.clone();
        attempted_mutation.name = "mutated".to_owned();
        assert!(application.save_workflow_draft(attempted_mutation).is_err());
        assert_eq!(
            application
                .store
                .get_published_workflow_version(&published.workflow_id, published.version)
                .expect("immutable version")
                .draft,
            frozen
        );

        let mut cloned = application
            .clone_workflow_version(&published.workflow_id, published.version)
            .expect("editable clone");
        assert_eq!(cloned.status, WorkflowDraftStatus::Editing);
        cloned.name.push_str(" revised");
        let cloned = application
            .save_workflow_draft(cloned)
            .expect("clone remains editable");
        let revised = application
            .publish_workflow(&cloned.id, &settings)
            .expect("revised version");
        let comparison = application
            .compare_workflow_versions(
                &published.workflow_id,
                published.version,
                &revised.workflow_id,
                revised.version,
            )
            .expect("version comparison");
        assert!(
            !comparison.same_content,
            "the published name is versioned content"
        );
        assert!(comparison.changed_nodes.is_empty());

        let started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("workflow-alpha/project.yaml"),
                "mock",
                settings,
                None,
                Some("workflow-alpha-explicit-version"),
                Some((&published.workflow_id, published.version)),
            )
            .expect("explicit Workflow Version Run");
        application
            .wait_run(started.run_id)
            .await
            .expect("completed Run");
        let history = application.store.history(started.run_id).expect("history");
        let snapshot = history.run.workflow_snapshot_json.expect("snapshot");
        assert!(snapshot.contains(&published.workflow_id));
        assert!(snapshot.contains("selected_workflow"));
        assert!(snapshot.contains("published_dag_runtime"));
        assert!(snapshot.contains("checkpoint"));
        assert!(!snapshot.contains("legacy_agent_runtime"));

        let archived = application
            .archive_workflow_draft(&blank.id)
            .expect("archived Draft");
        assert_eq!(archived.status, WorkflowDraftStatus::Archived);
        assert!(application.save_workflow_draft(archived).is_err());
    }

    #[tokio::test]
    async fn dataset_coordinator_runs_each_selected_image() {
        let temp = tempfile::tempdir().expect("temp");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let app = LocalApplication::new(&workspace).expect("app");
        app.create_project(
            "demo",
            include_str!("../../../examples/robocup/project.yaml"),
        )
        .expect("project");
        let image_root = workspace.join("demo/images");
        generate_synthetic_robocup(&image_root.join("one.png")).expect("first image");
        generate_synthetic_robocup(&image_root.join("two.png")).expect("second image");

        let results = DatasetCoordinator::new(&app)
            .run(&workspace.join("demo/project.yaml"), "mock", None, Some(2))
            .await
            .expect("dataset run");

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|item| {
            matches!(
                item.result.status,
                RunStatus::Completed | RunStatus::CompletedWithReview | RunStatus::Partial
            )
        }));
        assert_eq!(app.list_runs().expect("runs").len(), 2);
    }

    #[tokio::test]
    async fn persistent_batch_pauses_restarts_and_resumes_one_hundred_images() {
        let temp = tempfile::tempdir().expect("temp");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let project_yaml = include_str!("../../../examples/robocup/project.yaml")
            .replace("max_parallel_images: 2", "max_parallel_images: 4");
        let app = Arc::new(LocalApplication::new(&workspace).expect("app"));
        app.create_project("batch-demo", &project_yaml)
            .expect("project");
        let image_root = workspace.join("batch-demo/images");
        for index in 0..100 {
            generate_synthetic_robocup(&image_root.join(format!("image-{index:03}.png")))
                .expect("synthetic image");
        }
        let config = include_str!("../../../config/default.toml")
            .replace("max_output_tokens = 4096", "max_output_tokens = 256")
            .replace("max_output_tokens = 50000", "max_output_tokens = 1000000")
            .replace("max_total_tokens = 250000", "max_total_tokens = 2000000")
            .replace("max_cost = \"2.0\"", "max_cost = \"100.0\"")
            .replace("max_requests = 500", "max_requests = 10000");
        let config_path = workspace.join("batch-config.toml");
        std::fs::write(&config_path, config).expect("config");
        let coordinator = DatasetCoordinator::new(app.as_ref());
        let batch = coordinator
            .create(
                &workspace.join("batch-demo/project.yaml"),
                "mock",
                Some(&config_path),
                None,
            )
            .expect("batch");
        assert_eq!(batch.max_concurrency, 4);
        let task_app = app.clone();
        let batch_id = batch.id;
        let execution = tokio::spawn(async move {
            DatasetCoordinator::new(task_app.as_ref())
                .execute(batch_id, None)
                .await
        });
        let mut observed_progress = false;
        for _ in 0..500 {
            let images = app.store.list_batch_images(batch_id).expect("batch images");
            let completed = images
                .iter()
                .filter(|image| image.status == BatchImageStatus::Completed)
                .count();
            if completed > 0 && completed < 100 {
                observed_progress = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(
            observed_progress,
            "batch should expose intermediate progress"
        );
        coordinator.pause(batch_id).expect("pause");
        let paused = execution
            .await
            .expect("worker task")
            .expect("paused execution");
        assert_eq!(paused.batch.status, BatchStatus::Paused);
        let completed_before_restart = app
            .store
            .batch_checkpoint(batch_id)
            .expect("paused checkpoint")
            .completed_images
            .len();
        assert!((1..100).contains(&completed_before_restart));
        drop(app);

        let restarted = Arc::new(LocalApplication::new(&workspace).expect("restarted server"));
        let project = restarted
            .get_project("batch-demo")
            .expect("project summary");
        assert_eq!(
            project.active_batch.as_ref().map(|batch| batch.id),
            Some(batch_id)
        );
        assert_eq!(
            project.active_batch.as_ref().map(|batch| batch.status),
            Some(BatchStatus::Paused)
        );
        let resumed = DatasetCoordinator::new(restarted.as_ref())
            .resume(batch_id, None)
            .await
            .expect("resume after restart");
        assert_eq!(resumed.batch.status, BatchStatus::Completed);
        let checkpoint = restarted
            .store
            .batch_checkpoint(batch_id)
            .expect("final checkpoint");
        assert_eq!(checkpoint.completed_images.len(), 100);
        assert!(checkpoint.remaining_images.is_empty());
        assert_eq!(
            checkpoint.batch.budget_ledger.reserved,
            BatchUsage::default()
        );
        assert_eq!(checkpoint.batch.budget_ledger.consumed.image_count, 100);
        let runs = restarted.list_runs().expect("child runs");
        assert_eq!(runs.len(), 100, "completed images must not execute twice");
        let mut persisted_usage = annotagent_core::UsageTotals::default();
        for run in &runs {
            for usage in restarted.store.history(run.id).expect("history").usage {
                persisted_usage.add(&usage);
            }
        }
        assert_eq!(
            checkpoint.batch.budget_ledger.consumed.input_tokens,
            persisted_usage.input_tokens
        );
        assert_eq!(
            checkpoint.batch.budget_ledger.consumed.output_tokens,
            persisted_usage.output_tokens
        );
        assert_eq!(
            checkpoint.batch.budget_ledger.consumed.request_count,
            persisted_usage.requests
        );
        assert_eq!(
            checkpoint.batch.budget_ledger.consumed.cost,
            persisted_usage.cost
        );
        let events = restarted
            .store
            .list_batch_events(batch_id)
            .expect("batch events");
        assert!(
            events
                .windows(2)
                .all(|pair| pair[1].sequence == pair[0].sequence + 1)
        );
        assert_eq!(
            events.last().map(|event| event.sequence),
            Some(checkpoint.event_sequence)
        );
        assert!(
            restarted
                .get_project("batch-demo")
                .expect("finished project")
                .active_batch
                .is_none()
        );
    }
}
