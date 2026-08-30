//! Shared application service used by CLI/TUI and HTTP frontends.

mod guidance;
mod published_run;

pub use guidance::{
    GuidanceBlocker, GuidedAction, GuidedActionKind, ProjectGuidance, ProjectGuidanceInput,
    ProjectJourneyState, ProjectJourneyStep, ProjectReadinessSummary, ProjectStage,
    SampleTestState, derive_project_guidance,
};

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use annotagent_core::{
    AdditionalUsage, AgentBudget, AgentDryRunSummary, AgentKind, AgentSession, AgentSessionStatus,
    Annotation, AnnotationSource, ArtifactKind, AttributeDefinition, AttributeValue,
    BackendDescriptor, BatchBudgetLedger, BatchBudgetLimits, BatchId, BatchImageCheckpoint,
    BatchImageStatus, BatchNodeState, BatchProgress, BatchRecord, BatchStatus, BatchUsage, Budget,
    DatasetExporter, DatasetImporter, DomainSkill, EnabledSkillConfig, ExportReport, ExportRequest,
    FullRunEstimate, ImageId, ImportIssue, ImportReport, ImportRequest, LabelId, LabelPipeline,
    LabelPipelineStaticValidator, LabelWorkflowComposition, LicenseMetadata, LicensePermission,
    ModelAvailabilityStatus, ModelBinding as PipelineModelBinding, ModelInputContract,
    ModelMessage, ModelOutputContract, ModelRegistry, ModelRequest, ModelRole,
    ModelVersionMetadata, NodeRegistry, PipelineArtifact, PipelineBuilderConstraints,
    PipelineBuilderTool, PipelineBuilderToolRegistry, PipelineDraftDiff, PipelineDraftTools,
    PipelineGrammarValidator, PipelineSource, PipelineStep, PricingConfig, ProjectId,
    ProjectSchema, ProjectSnapshot, PublishedWorkflowVersion, RegistryWorkflowAdvisor,
    ResourceRequirements, RetryPolicy, ReviewGate, ReviewStatus, RunEvent, RunEventKind,
    RunEventPayload, RunId, RunStatus, RuntimeRequirements, SampleTestOutcome,
    SampleTestOutcomeStatus, SampleTestSummary, ScoreSemantics, SharedWorkflowStage,
    SkillResourceRequest, SnapshotImage, TaskConfig, TaskId, TaskKind, TaskRunStatus, TokenUsage,
    ToolDefinition, UsageSource, UsageSummary, VisionArtifactValue, VisionBackendKind,
    VisionCapability, VisionInferenceRequest, VisionInputType, VisionModelDescriptor,
    VisionModelHealth, VisionModelHealthStatus, VisionModelLimits, VisionModelProvider,
    VisionNodeDescriptor, WORKFLOW_SCHEMA_VERSION, WorkflowAdvisor, WorkflowAdvisorAgentReport,
    WorkflowAdvisorInput, WorkflowConstraints, WorkflowDataProfile, WorkflowDraft,
    WorkflowDraftStatus, WorkflowDryRunNodeResult, WorkflowDryRunReport,
    WorkflowDryRunSampleResult, WorkflowEdge, WorkflowNodeKind, WorkflowSnapshot,
    WorkflowStaticValidator, WorkflowSuggestion, WorkflowValidationIssue, WorkflowValidationReport,
    WorkflowVersionComparison, all_artifact_kinds,
};
use annotagent_export::{
    CocoExporter, CocoImporter, LabelMeExporter, LabelMeImporter, NativeExporter, NativeImporter,
    YoloDetectionExporter, YoloDetectionImporter, YoloSegmentationExporter,
    YoloSegmentationImporter,
};
use annotagent_image_tools::{generate_synthetic_robocup, load_image, sha256, to_model_image};
use annotagent_provider::{
    HttpVisionWorkerConfig, HttpVisionWorkerRegistryBackend, MockResponseSpec, MockScript,
    MockStep, MockUsage, MockVisionBackend, MockVisionProvider, OpenAiCompatibleConfig,
    OpenAiCompatibleProvider,
};
use annotagent_runtime::{
    AgentLoopConfig, AgentRuntime, DagCheckpoint, DagNodeFailure, DagNodeStatus, DagNodeUsage,
    ImageRunRequest, ImageRunResult, LayeredSkillRegistry, RunControl, RuntimeStore, SkillRegistry,
};
use annotagent_skill_robocup::{
    ROBOCUP_BALL_SKILL_ID, RoboCupBallRecoveryAgent, RoboCupBallRecoveryReport,
    RoboCupBallRecoveryRequest, RoboCupBallSkill, RoboCupPackSkill, RoboCupSkill,
};
use annotagent_storage::{
    BatchClaimResult, HistoryRun, RunStartReservation, SqliteStore, WorkflowSampleTest,
};
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
    #[serde(default = "default_detection_workers")]
    pub detection_workers: Vec<DetectionWorkerSettings>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerSettings {
    pub id: String,
    pub display_name: String,
    pub model_id: String,
    pub base_url: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allow_remote: bool,
    /// Require immutable checkpoint/dataset/class metadata before this Worker may be enabled.
    #[serde(default)]
    pub requires_checkpoint_metadata: bool,
    pub expected_capabilities: Vec<VisionCapability>,
    pub score_semantics: ScoreSemantics,
    pub version: ModelVersionMetadata,
    #[serde(default)]
    pub label_space: Vec<String>,
    #[serde(default)]
    pub runtime_requirements: RuntimeRequirements,
    #[serde(default)]
    pub license: LicenseMetadata,
    pub timeout_seconds: u64,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    #[serde(default)]
    pub max_retries: u32,
    /// User-configured estimate used for planning and display. Runtime usage remains actual-first.
    #[serde(default)]
    pub cost_per_request: rust_decimal::Decimal,
}

impl DetectionWorkerSettings {
    #[must_use]
    pub fn http_config(&self) -> HttpVisionWorkerConfig {
        HttpVisionWorkerConfig {
            id: self.id.clone(),
            base_url: self.base_url.clone(),
            expected_model_id: self.model_id.clone(),
            capabilities: self.expected_capabilities.clone(),
            expected_score_semantics: Some(self.score_semantics),
            expected_label_space: self.label_space.clone(),
            request_timeout: Duration::from_secs(self.timeout_seconds),
            max_request_bytes: self.max_request_bytes,
            max_response_bytes: self.max_response_bytes,
            max_retries: self.max_retries,
            allow_remote: self.allow_remote,
            authorization: None,
        }
    }
}

fn default_detection_workers() -> Vec<DetectionWorkerSettings> {
    vec![
        DetectionWorkerSettings {
            id: "annotagent-locate-anything".to_owned(),
            display_name: "LocateAnything Local".to_owned(),
            model_id: "locate-anything-local".to_owned(),
            base_url: "http://127.0.0.1:8791".to_owned(),
            enabled: false,
            allow_remote: false,
            requires_checkpoint_metadata: false,
            expected_capabilities: vec![
                VisionCapability::OpenVocabularyDetection,
                VisionCapability::PhraseGrounding,
            ],
            score_semantics: ScoreSemantics::NotProvided,
            version: ModelVersionMetadata {
                architecture: Some("locateanything-3b".to_owned()),
                model_version: "local-unpinned".to_owned(),
                checkpoint_sha256: None,
                training_dataset_version: None,
                backend_protocol_version: annotagent_core::VISION_WORKER_PROTOCOL_VERSION
                    .to_string(),
            },
            label_space: Vec::new(),
            runtime_requirements: RuntimeRequirements {
                devices: vec!["cuda".to_owned()],
                minimum_gpu_memory_mb: None,
                dependencies: vec![
                    "official LocateAnything worker source".to_owned(),
                    "PyTorch CUDA".to_owned(),
                    "Transformers 4.57.1".to_owned(),
                ],
                supports_batch: false,
            },
            license: LicenseMetadata {
                code_license: Some(
                    "NVIDIA source notice; verify the configured checkout".to_owned(),
                ),
                weight_license: Some(
                    "NVIDIA License — non-commercial research/evaluation".to_owned(),
                ),
                source_url: Some(
                    "https://huggingface.co/nvidia/LocateAnything-3B/blob/main/LICENSE".to_owned(),
                ),
                commercial_use: LicensePermission::Restricted,
                redistribution: LicensePermission::Restricted,
                usage_notes: vec![
                    "Use only in a setting permitted by the concrete model license.".to_owned(),
                    "This metadata is informational and is not legal advice.".to_owned(),
                ],
                verified_from_official_source: true,
            },
            timeout_seconds: 120,
            max_request_bytes: 44_000_000,
            max_response_bytes: 2_000_000,
            max_retries: 0,
            cost_per_request: rust_decimal::Decimal::ZERO,
        },
        DetectionWorkerSettings {
            id: "annotagent-rfdetr".to_owned(),
            display_name: "RF-DETR Specialist Local".to_owned(),
            model_id: "rfdetr-specialist-local".to_owned(),
            base_url: "http://127.0.0.1:8792".to_owned(),
            enabled: false,
            allow_remote: false,
            requires_checkpoint_metadata: true,
            expected_capabilities: vec![VisionCapability::ObjectDetection],
            score_semantics: ScoreSemantics::RelativeConfidence,
            version: ModelVersionMetadata {
                architecture: None,
                model_version: "unconfigured".to_owned(),
                checkpoint_sha256: None,
                training_dataset_version: None,
                backend_protocol_version: annotagent_core::VISION_WORKER_PROTOCOL_VERSION
                    .to_string(),
            },
            label_space: Vec::new(),
            runtime_requirements: RuntimeRequirements {
                devices: vec!["cuda".to_owned()],
                minimum_gpu_memory_mb: None,
                dependencies: vec![
                    "rfdetr Python package compatible with the configured checkpoint".to_owned(),
                    "PyTorch CUDA".to_owned(),
                ],
                supports_batch: false,
            },
            license: LicenseMetadata {
                code_license: Some("Apache-2.0 for the open-source rfdetr package".to_owned()),
                weight_license: None,
                source_url: Some(
                    "https://github.com/roboflow/rf-detr/blob/develop/LICENSE".to_owned(),
                ),
                commercial_use: LicensePermission::Unknown,
                redistribution: LicensePermission::Unknown,
                usage_notes: vec![
                    "Set the concrete checkpoint license before enabling; RF-DETR variants do not all share one weight license."
                        .to_owned(),
                    "This metadata is informational and is not legal advice.".to_owned(),
                ],
                verified_from_official_source: true,
            },
            timeout_seconds: 120,
            max_request_bytes: 44_000_000,
            max_response_bytes: 2_000_000,
            max_retries: 0,
            cost_per_request: rust_decimal::Decimal::ZERO,
        },
    ]
}

fn default_provider_kind() -> String {
    "mock".to_owned()
}

fn pipeline_builder_live_tools(
    input: &WorkflowAdvisorInput,
    suggestion: &WorkflowSuggestion,
) -> Vec<ToolDefinition> {
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
    let no_arguments = || {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        })
    };
    let read = |tool: PipelineBuilderTool, description: &str, parameters| ToolDefinition {
        name: tool.as_str().to_owned(),
        description: description.to_owned(),
        parameters,
        read_only: true,
    };
    let mutate = |tool: PipelineBuilderTool, description: &str, parameters| ToolDefinition {
        name: tool.as_str().to_owned(),
        description: description.to_owned(),
        parameters,
        read_only: false,
    };
    let mut tools = vec![
        read(
            PipelineBuilderTool::InspectProject,
            "Read a bounded Project summary without file paths or image bytes.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::InspectLabelSchema,
            "Read Project task kinds and declared Labels.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::InspectLabel,
            "Inspect the exact target Label for this session.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::SampleImages,
            "Read bounded dataset dimensions and MIME types; no image bytes are returned.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::ListEnabledSkills,
            "List only Skills enabled by the Project.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::ListAvailableCapabilities,
            "List registered capability-bound Pipeline nodes.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::ListAvailableNodes,
            "List registered node IDs and typed input/output contracts.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::ListAvailableModels,
            "List registered Models with capability, health, backend, and price metadata.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::InspectModel,
            "Inspect one Model selected from the Registry.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["model_id"],
                "properties": {"model_id": {"type": "string", "enum": model_ids.clone()}}
            }),
        ),
        read(
            PipelineBuilderTool::ListPipelineTemplates,
            "List compatible Registry templates and the safe default template.",
            no_arguments(),
        ),
        mutate(
            PipelineBuilderTool::CreateDraftFromTemplate,
            "Create a new editable Draft from the safe Registry template. Never publishes.",
            no_arguments(),
        ),
        mutate(
            PipelineBuilderTool::DisconnectPipelineNodes,
            "Remove one existing Draft connection so validation can guide a bounded repair.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["from_node", "to_node"],
                "properties": {
                    "from_node": {"type": "string", "enum": node_ids.clone()},
                    "to_node": {"type": "string", "enum": node_ids.clone()}
                }
            }),
        ),
        mutate(
            PipelineBuilderTool::ConnectPipelineNodes,
            "Connect two existing typed ports. Rust rejects unknown ports, type mismatches, and cycles.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["from_node", "from_port", "to_node", "to_port"],
                "properties": {
                    "from_node": {"type": "string", "enum": node_ids.clone()},
                    "from_port": {"type": "string"},
                    "to_node": {"type": "string", "enum": node_ids.clone()},
                    "to_port": {"type": "string"},
                    "route": {"type": ["string", "null"]}
                }
            }),
        ),
        mutate(
            PipelineBuilderTool::AddPipelineNode,
            "Add one controlled Guided action after Dry Run evidence. Alpha supports Crop verification only.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["guided_template"],
                "properties": {
                    "guided_template": {"type": "string", "enum": ["crop_verification"]}
                }
            }),
        ),
        mutate(
            PipelineBuilderTool::SetNodeParameter,
            "Set one allowed parameter on a node in the current Draft.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["node_id", "parameter", "value"],
                "properties": {
                    "node_id": {"type": "string", "enum": node_ids.clone()},
                    "parameter": {"type": "string", "enum": ["threshold", "minimum_confidence", "padding", "max_detections", "grounding_assist"]},
                    "value": {}
                }
            }),
        ),
        mutate(
            PipelineBuilderTool::BindModel,
            "Bind one Registry Model to an existing model node. Rust validates capability and availability.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["node_id", "model_id"],
                "properties": {
                    "node_id": {"type": "string", "enum": node_ids.clone()},
                    "model_id": {"type": "string", "enum": model_ids}
                }
            }),
        ),
        mutate(
            PipelineBuilderTool::SetLabelMapping,
            "Set a bounded class-to-Label mapping on an existing selection node.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["node_id", "class_mapping"],
                "properties": {
                    "node_id": {"type": "string", "enum": node_ids.clone()},
                    "class_mapping": {"type": "object", "additionalProperties": {"type": "string"}}
                }
            }),
        ),
        mutate(
            PipelineBuilderTool::SetDecisionPolicy,
            "Set the confidence threshold of an existing Decision node.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["node_id", "threshold"],
                "properties": {
                    "node_id": {"type": "string", "enum": node_ids},
                    "threshold": {"type": "number", "minimum": 0, "maximum": 1}
                }
            }),
        ),
        read(
            PipelineBuilderTool::ValidatePipeline,
            "Run Rust Pipeline Grammar and static validation on the current Draft.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::DryRunPipeline,
            "Run the validated Draft in a non-committing sandbox on 1 to 10 images.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "image_indices": {"type": "array", "minItems": 1, "maxItems": 10, "uniqueItems": true, "items": {"type": "integer", "minimum": 0}}
                }
            }),
        ),
        read(
            PipelineBuilderTool::InspectDryRunSummary,
            "Read bounded review rate, failures, empty results, cost, and latency from the latest Dry Run.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::InspectFailedSamples,
            "Read at most five failed sample summaries. Image bytes and complete Artifacts are never returned.",
            bounded_inspection_schema(),
        ),
        read(
            PipelineBuilderTool::InspectReviewSamples,
            "Read at most five Review sample summaries with Label, status, and confidence only.",
            bounded_inspection_schema(),
        ),
        read(
            PipelineBuilderTool::InspectNodeArtifacts,
            "Read bounded node-level Dry Run statistics and structured warnings, not Artifact bodies.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["node_id"],
                "properties": {
                    "node_id": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 5, "default": 3}
                }
            }),
        ),
        mutate(
            PipelineBuilderTool::SubmitDraftForHumanApproval,
            "Stop with a validated and Dry-Run-tested editable Draft. Never publishes or starts a formal Run.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": {"type": "string", "minLength": 1, "maxLength": 160},
                    "rationale": {"type": "array", "maxItems": 8, "items": {"type": "string", "maxLength": 400}},
                    "warnings": {"type": "array", "maxItems": 8, "items": {"type": "string", "maxLength": 400}},
                    "alternatives": {"type": "array", "maxItems": 8, "items": {"type": "string", "maxLength": 400}}
                }
            }),
        ),
    ];
    if !input.resource_ids.is_empty() {
        tools.insert(
            5,
            read(
                PipelineBuilderTool::LoadSkillResource,
                "Load one declared Advisor resource from an enabled Skill. Paths and undeclared resources are rejected by Rust.",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["skill_id", "resource_name"],
                    "properties": {
                        "skill_id": {"type": "string", "enum": input.enabled_skills},
                        "resource_name": {"type": "string", "enum": input.resource_ids}
                    }
                }),
            ),
        );
    }
    tools
}

fn pipeline_builder_constraints(
    constraints: &WorkflowConstraints,
    mut builder: PipelineBuilderConstraints,
) -> Result<PipelineBuilderConstraints> {
    if builder.max_cost_per_image.is_none() {
        builder.max_cost_per_image = constraints
            .max_cost_per_image
            .as_deref()
            .and_then(|value| value.parse().ok());
    }
    if builder.max_expected_latency_ms.is_none() {
        builder.max_expected_latency_ms = constraints.max_latency_ms;
    }
    builder.validate().map_err(|error| anyhow!(error))?;
    Ok(builder)
}

fn bounded_inspection_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "limit": {"type": "integer", "minimum": 1, "maximum": 5, "default": 3}
        }
    })
}

fn load_enabled_skill_resource(
    skills: &SkillRegistry,
    enabled_skill_ids: &[String],
    resource_id: &str,
    task_id: Option<&str>,
) -> Result<(String, String, Vec<annotagent_core::SkillResource>)> {
    for skill_id in enabled_skill_ids {
        let prefix = format!("{skill_id}.");
        let resource_name = resource_id.strip_prefix(&prefix).unwrap_or(resource_id);
        let request = SkillResourceRequest {
            task_id: task_id.map(TaskId::from),
            resource_name: Some(resource_name.to_owned()),
        };
        if let Ok(resources) = skills.load_resource(skill_id, &request)
            && !resources.is_empty()
        {
            return Ok((skill_id.clone(), resource_name.to_owned(), resources));
        }
    }
    bail!("declared Skill resource {resource_id:?} is unavailable from enabled Skills")
}

fn bounded_skill_resources(resources: &[annotagent_core::SkillResource]) -> Vec<serde_json::Value> {
    resources
        .iter()
        .take(4)
        .map(|resource| {
            let content = resource.content.chars().take(12_000).collect::<String>();
            json!({
                "name": resource.name,
                "media_type": resource.media_type,
                "content": content,
                "truncated": resource.content.chars().count() > 12_000,
            })
        })
        .collect()
}

fn bounded_inspection_limit(arguments: &serde_json::Value) -> Result<usize> {
    let limit = arguments
        .get("limit")
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| anyhow!("inspection limit must be an integer from 1 to 5"))
        })
        .transpose()?
        .unwrap_or(3);
    if !(1..=5).contains(&limit) {
        bail!("inspection limit must be from 1 to 5");
    }
    Ok(limit)
}

fn required_string_argument(arguments: &serde_json::Value, name: &str) -> Result<String> {
    arguments
        .get(name)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("Tool argument {name:?} must be a non-empty string"))
}

fn string_array_argument(arguments: &serde_json::Value, name: &str) -> Vec<String> {
    arguments
        .get(name)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .take(8)
        .map(|value| value.chars().take(400).collect())
        .collect()
}

fn bounded_image_indices(arguments: &serde_json::Value) -> Result<Vec<usize>> {
    let values = arguments
        .get("image_indices")
        .and_then(serde_json::Value::as_array);
    let Some(values) = values else {
        return Ok(vec![0]);
    };
    if values.is_empty() || values.len() > 10 {
        bail!("Dry Run image_indices must contain 1 to 10 entries");
    }
    values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| anyhow!("Dry Run image index must be a non-negative integer"))
        })
        .collect()
}

fn sync_label_step_parameter(
    draft: &mut WorkflowDraft,
    node_id: &str,
    parameter: &str,
    value: serde_json::Value,
) {
    if let Some(composition) = draft.label_pipeline.as_mut()
        && let Some(step) = composition
            .shared_stages
            .iter_mut()
            .flat_map(|stage| stage.steps.iter_mut())
            .chain(
                composition
                    .label_pipelines
                    .iter_mut()
                    .flat_map(|pipeline| pipeline.steps.iter_mut()),
            )
            .find(|step| step.id == node_id)
    {
        step.parameters.insert(parameter.to_owned(), value);
    }
}

fn sync_label_step_model(draft: &mut WorkflowDraft, node_id: &str, model_id: &str) {
    if let Some(composition) = draft.label_pipeline.as_mut()
        && let Some(binding) = composition
            .shared_stages
            .iter_mut()
            .flat_map(|stage| stage.steps.iter_mut())
            .chain(
                composition
                    .label_pipelines
                    .iter_mut()
                    .flat_map(|pipeline| pipeline.steps.iter_mut()),
            )
            .find(|step| step.id == node_id)
            .and_then(|step| step.model_binding.as_mut())
    {
        model_id.clone_into(&mut binding.model_id);
    }
}

pub fn validate_settings(settings: &Settings) -> Result<()> {
    if !matches!(
        settings.default_provider.as_str(),
        "mock" | "openai_compatible"
    ) {
        bail!("default_provider must be either \"mock\" or \"openai_compatible\"");
    }
    OpenAiCompatibleProvider::new(settings.provider.clone()).map_err(|error| anyhow!(error))?;
    let mut worker_ids = BTreeSet::new();
    let mut model_ids = BTreeSet::new();
    for worker in &settings.detection_workers {
        if worker.display_name.trim().is_empty()
            || !worker_ids.insert(worker.id.as_str())
            || !model_ids.insert(worker.model_id.as_str())
        {
            bail!("Detection Worker ids/model ids must be non-empty and unique");
        }
        if worker.enabled
            && worker.requires_checkpoint_metadata
            && (worker
                .version
                .architecture
                .as_deref()
                .is_none_or(str::is_empty)
                || worker.version.model_version.trim().is_empty()
                || worker.version.model_version == "unconfigured"
                || worker.version.checkpoint_sha256.is_none()
                || worker
                    .version
                    .training_dataset_version
                    .as_deref()
                    .is_none_or(str::is_empty)
                || worker.label_space.is_empty()
                || worker.license.weight_license.is_none())
        {
            bail!(
                "enabled versioned Detection Workers require architecture, model version, checkpoint SHA-256, training dataset version, label space, and weight license metadata"
            );
        }
        HttpVisionWorkerRegistryBackend::new(worker.http_config())
            .map_err(|error| anyhow!(error))?;
    }
    Ok(())
}

pub struct PreparedRun {
    runtime: Arc<dyn ApplicationImageRuntime>,
    request: ImageRunRequest,
    image_path: PathBuf,
}

pub struct BallRecoveryInput {
    pub candidate: Annotation,
    pub related_annotations: Vec<Annotation>,
    pub issues: Vec<annotagent_core::ValidationIssue>,
    pub image_path: Option<PathBuf>,
    pub budget: AgentBudget,
    pub cancellation: CancellationToken,
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
    pub task_count: usize,
    pub review_count: usize,
    pub readiness: ProjectReadiness,
    pub blocking_issues: Vec<ProjectBlockingIssue>,
    pub default_workflow_version: Option<WorkflowVersion>,
    pub active_batch: Option<BatchRecord>,
    pub active_batch_progress: Option<BatchProgress>,
    pub active_run: Option<HistoryRun>,
    pub last_run: Option<HistoryRun>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectWorkspaceSummary {
    pub project: ProjectSummary,
    pub guidance: ProjectGuidance,
    pub readiness: ProjectReadinessSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectReadiness {
    Incomplete,
    Ready,
    ConfigurationIssue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectBlockingIssue {
    pub code: String,
    pub message: String,
    pub next_step: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDatasetSummary {
    pub root: String,
    pub include: Vec<String>,
    pub recursive: bool,
    pub image_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageImportIssue {
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageImportReport {
    pub source: String,
    pub discovered: u64,
    pub imported: u64,
    pub duplicates: u64,
    pub corrupt: Vec<ImageImportIssue>,
    pub unsupported_files: u64,
    pub supported_formats: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnnotationTaskSummary {
    pub id: String,
    pub display_name: String,
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
    pub availability_group: ModelAvailabilityGroup,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_semantics: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub label_space: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_request: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAvailabilityGroup {
    Ready,
    ConfiguredUnavailable,
    Labs,
    Disabled,
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

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowDraftApplyReport {
    pub draft: WorkflowDraft,
    pub previous_draft: WorkflowDraft,
    pub diff: PipelineDraftDiff,
    pub selected_change_ids: Vec<String>,
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
        VisionCapability::OpenVocabularyDetection,
        VisionCapability::PhraseGrounding,
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
        capabilities: vec![
            VisionCapability::VisionLanguage,
            VisionCapability::Classification,
        ],
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
            max_images: Some(2),
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
    models.register_model(VisionModelDescriptor {
        id: "mock-open-vocabulary".to_owned(),
        display_name: "Offline mock open-vocabulary grounding".to_owned(),
        backend_id: "workspace-provider-adapter".to_owned(),
        capabilities: vec![
            VisionCapability::OpenVocabularyDetection,
            VisionCapability::PhraseGrounding,
        ],
        input_types: vec![VisionInputType::Image, VisionInputType::Text],
        output_types: vec![ArtifactKind::DetectionSet],
        model: "mock-open-vocabulary".to_owned(),
        model_version: "1".to_owned(),
        input_contract: ModelInputContract {
            input_types: vec![VisionInputType::Image, VisionInputType::Text],
            supports_multiple_queries: true,
            supports_visual_prompt: false,
            max_queries: Some(100),
        },
        output_contract: ModelOutputContract {
            output_types: vec![ArtifactKind::DetectionSet],
            normalized_coordinates: true,
            allows_empty: true,
            label_space: Vec::new(),
        },
        score_semantics: ScoreSemantics::NotProvided,
        health: VisionModelHealth {
            status: VisionModelHealthStatus::Healthy,
            detail: Some("offline deterministic Grounding fixture available".to_owned()),
            checked_at: Some(chrono::Utc::now()),
        },
        ..VisionModelDescriptor::default()
    })?;
    for worker in &settings.detection_workers {
        let supports_text_queries = worker.expected_capabilities.iter().any(|capability| {
            matches!(
                capability,
                VisionCapability::OpenVocabularyDetection | VisionCapability::PhraseGrounding
            )
        });
        let worker_input_types = if supports_text_queries {
            vec![VisionInputType::Image, VisionInputType::Text]
        } else {
            vec![VisionInputType::Image]
        };
        models.register_backend(Arc::new(HttpVisionWorkerRegistryBackend::new(
            worker.http_config(),
        )?))?;
        models.register_model(VisionModelDescriptor {
            id: worker.model_id.clone(),
            display_name: worker.display_name.clone(),
            backend_id: worker.id.clone(),
            provider: "http_vision".to_owned(),
            backend: BackendDescriptor {
                kind: Some(VisionBackendKind::HttpVision),
                protocol_version: Some(worker.version.backend_protocol_version.clone()),
                endpoint: Some(worker.base_url.clone()),
            },
            capabilities: worker.expected_capabilities.clone(),
            input_types: worker_input_types.clone(),
            output_types: vec![ArtifactKind::DetectionSet],
            model: worker.model_id.clone(),
            model_version: worker.version.model_version.clone(),
            version: worker.version.clone(),
            endpoint_or_path: Some(worker.base_url.clone()),
            input_contract: ModelInputContract {
                input_types: worker_input_types,
                supports_multiple_queries: supports_text_queries,
                supports_visual_prompt: false,
                max_queries: supports_text_queries.then_some(100),
            },
            output_contract: ModelOutputContract {
                output_types: vec![ArtifactKind::DetectionSet],
                normalized_coordinates: true,
                allows_empty: true,
                label_space: worker.label_space.clone(),
            },
            score_semantics: worker.score_semantics,
            runtime_requirements: worker.runtime_requirements.clone(),
            license: worker.license.clone(),
            status: if worker.enabled {
                ModelAvailabilityStatus::Unknown
            } else {
                ModelAvailabilityStatus::Disabled
            },
            health: VisionModelHealth {
                status: if worker.enabled {
                    VisionModelHealthStatus::Unknown
                } else {
                    VisionModelHealthStatus::Unavailable
                },
                detail: Some(if worker.enabled {
                    "configured; health and capabilities are discovered from the Worker".to_owned()
                } else {
                    "disabled in workspace Settings".to_owned()
                }),
                checked_at: None,
            },
            limits: VisionModelLimits {
                max_images: Some(1),
                max_input_artifacts: Some(0),
                max_request_bytes: Some(worker.max_request_bytes as u64),
                timeout_seconds: Some(worker.timeout_seconds),
            },
            configuration: BTreeMap::from([
                ("allow_remote".to_owned(), json!(worker.allow_remote)),
                ("worker_enabled".to_owned(), json!(worker.enabled)),
            ]),
            ..VisionModelDescriptor::default()
        })?;
    }
    for (id, display_name, capability, output_type) in [
        (
            "mock-classifier",
            "Offline mock classifier",
            VisionCapability::Classification,
            ArtifactKind::ClassificationSet,
        ),
        (
            "mock-detector",
            "Offline mock detector",
            VisionCapability::ObjectDetection,
            ArtifactKind::DetectionSet,
        ),
        (
            "mock-object-detector",
            "Offline mock trained detector",
            VisionCapability::ObjectDetection,
            ArtifactKind::DetectionSet,
        ),
    ] {
        models.register_model(VisionModelDescriptor {
            id: id.to_owned(),
            display_name: display_name.to_owned(),
            backend_id: "workspace-provider-adapter".to_owned(),
            capabilities: vec![capability],
            input_types: vec![VisionInputType::Image],
            output_types: vec![output_type],
            model: id.to_owned(),
            model_version: "1".to_owned(),
            health: VisionModelHealth {
                status: VisionModelHealthStatus::Healthy,
                detail: Some("offline deterministic fixture available".to_owned()),
                checked_at: Some(chrono::Utc::now()),
            },
            ..VisionModelDescriptor::default()
        })?;
    }

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
        (
            "annotation_refiner",
            None,
            vec![ArtifactKind::DetectionSet],
            false,
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
    for descriptor in [
        annotagent_skill_classification::node_descriptor(),
        annotagent_skill_classification::verifier_node_descriptor(),
        annotagent_skill_open_vocabulary::open_vocabulary_node_descriptor(),
        annotagent_skill_open_vocabulary::phrase_grounding_node_descriptor(),
        annotagent_skill_object_detection::node_descriptor(),
        annotagent_skill_vlm_detection::node_descriptor(),
        annotagent_skill_yolo::node_descriptor(),
        VisionNodeDescriptor {
            id: annotagent_core::IMAGE_INPUT_OPERATION.to_owned(),
            display_name: "Image Input".to_owned(),
            required_capabilities: Vec::new(),
            accepts: Vec::new(),
            produces: vec![ArtifactKind::Image],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_ARTIFACT_CACHE.to_owned(),
            display_name: "Artifact Cache".to_owned(),
            required_capabilities: Vec::new(),
            accepts: all_artifact_kinds().to_vec(),
            produces: all_artifact_kinds().to_vec(),
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_IMAGE_STATISTICS.to_owned(),
            display_name: "Compute Image Statistics".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::Image],
            produces: vec![ArtifactKind::Attributes],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_CROP.to_owned(),
            display_name: "Crop".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::Image, ArtifactKind::DetectionSet],
            produces: vec![ArtifactKind::CropSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_FILTER.to_owned(),
            display_name: "Select detections".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::DetectionSet],
            produces: vec![ArtifactKind::DetectionSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_MAP_LABEL.to_owned(),
            display_name: "Select detections · label mapping".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::DetectionSet],
            produces: vec![ArtifactKind::DetectionSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_ATTACH_RESULT.to_owned(),
            display_name: "Combine model evidence".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::DetectionSet, ArtifactKind::ClassificationSet],
            produces: vec![ArtifactKind::AnnotationCandidateSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_ATTACH_ATTRIBUTE.to_owned(),
            display_name: "Attach Attribute".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::AnnotationCandidateSet],
            produces: vec![ArtifactKind::AnnotationCandidateSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_CONFIDENCE_GATE.to_owned(),
            display_name: "Decision".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![
                ArtifactKind::DetectionSet,
                ArtifactKind::ClassificationSet,
                ArtifactKind::AnnotationCandidateSet,
            ],
            produces: vec![
                ArtifactKind::DetectionSet,
                ArtifactKind::ClassificationSet,
                ArtifactKind::AnnotationCandidateSet,
            ],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_CANDIDATE_MATCH.to_owned(),
            display_name: "Combine model evidence".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::DetectionSet],
            produces: vec![ArtifactKind::CandidateClusterSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_EVIDENCE_GATE.to_owned(),
            display_name: "Decision · evidence".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::CandidateClusterSet],
            produces: vec![ArtifactKind::CandidateClusterSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_PROJECT_CANDIDATES.to_owned(),
            display_name: "Select detections".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::CandidateClusterSet],
            produces: vec![ArtifactKind::DetectionSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_REJECT.to_owned(),
            display_name: "Reject Candidates".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![
                ArtifactKind::DetectionSet,
                ArtifactKind::CandidateClusterSet,
                ArtifactKind::ClassificationSet,
                ArtifactKind::AnnotationCandidateSet,
            ],
            produces: vec![
                ArtifactKind::DetectionSet,
                ArtifactKind::CandidateClusterSet,
                ArtifactKind::ClassificationSet,
                ArtifactKind::AnnotationCandidateSet,
            ],
            deterministic: true,
        },
        annotagent_runtime::detection_recovery_node_descriptor(),
    ] {
        nodes.register(descriptor)?;
    }
    Ok((nodes, models))
}

fn controlled_label_composition(
    project: &ProjectSchema,
    target_task_id: &str,
    target_label: &str,
    constraints: &WorkflowConstraints,
    models: &ModelRegistry,
) -> Result<LabelWorkflowComposition> {
    let task = project
        .tasks
        .iter()
        .find(|task| task.id.as_str() == target_task_id)
        .ok_or_else(|| anyhow!("target task {target_task_id:?} is not in Project Schema"))?;
    if !task.labels.iter().any(|label| label == target_label) {
        bail!("target Label {target_label:?} is not declared by task {target_task_id:?}");
    }
    let target_task = TaskId::from(target_task_id);
    let target = LabelId::from(target_label);
    let threshold = project.review.auto_accept_confidence;
    let gate = |input: PipelineSource, artifact_type: ArtifactKind| PipelineStep {
        id: format!("{target_task_id}.{target_label}.confidence"),
        node_type: annotagent_runtime::CORE_CONFIDENCE_GATE.to_owned(),
        kind: WorkflowNodeKind::Gate,
        inputs: BTreeMap::from([("candidates".to_owned(), input)]),
        outputs: BTreeMap::from([("candidates".to_owned(), artifact_type)]),
        model_binding: None,
        skill_binding: None,
        parameters: BTreeMap::from([("threshold".to_owned(), json!(threshold))]),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        retry_policy: RetryPolicy::default(),
        review_gate: ReviewGate {
            required: true,
            allow_manual_override: true,
        },
        resources: ResourceRequirements::default(),
    };
    let commit = |input: PipelineSource| PipelineStep {
        id: format!("{target_task_id}.{target_label}.commit"),
        node_type: "commit".to_owned(),
        kind: WorkflowNodeKind::Commit,
        inputs: BTreeMap::from([("candidates".to_owned(), input)]),
        outputs: BTreeMap::new(),
        model_binding: None,
        skill_binding: None,
        parameters: BTreeMap::from([
            ("task_id".to_owned(), json!(target_task_id)),
            ("target_label".to_owned(), json!(target_label)),
        ]),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        retry_policy: RetryPolicy::default(),
        review_gate: ReviewGate::default(),
        resources: ResourceRequirements::default(),
    };

    let (shared_stages, steps) = match task.kind {
        TaskKind::Classification => {
            let classifier_id = format!("{target_task_id}.{target_label}.classifier");
            let gate_id = format!("{target_task_id}.{target_label}.confidence");
            let classifier = PipelineStep {
                id: classifier_id.clone(),
                node_type: annotagent_skill_classification::CLASSIFICATION_OPERATION.to_owned(),
                kind: WorkflowNodeKind::VisionModel,
                inputs: BTreeMap::from([("subjects".to_owned(), PipelineSource::Image)]),
                outputs: BTreeMap::from([(
                    "classifications".to_owned(),
                    ArtifactKind::ClassificationSet,
                )]),
                model_binding: Some(PipelineModelBinding {
                    model_id: preferred_model_for(
                        models,
                        constraints.preferred_model_id.as_deref(),
                        &[VisionCapability::Classification],
                    )?
                    .map_or_else(|| "mock-classifier".to_owned(), |(model, _)| model),
                    capability: VisionCapability::Classification,
                    configuration: BTreeMap::new(),
                }),
                skill_binding: None,
                parameters: BTreeMap::from([
                    ("labels".to_owned(), json!(task.labels)),
                    ("mock_label".to_owned(), json!(target_label)),
                    ("target_label".to_owned(), json!(target_label)),
                ]),
                validators: task.validators.clone(),
                refiners: task.refiners.clone(),
                fallback: None,
                retry_policy: RetryPolicy {
                    max_attempts: project.runtime.max_retries.saturating_add(1),
                },
                review_gate: ReviewGate::default(),
                resources: ResourceRequirements {
                    timeout_seconds: Some(project.runtime.task_timeout_seconds),
                    ..ResourceRequirements::default()
                },
            };
            let gate_step = gate(
                PipelineSource::Step {
                    step_id: classifier_id,
                    port: "classifications".to_owned(),
                    artifact_type: ArtifactKind::ClassificationSet,
                },
                ArtifactKind::ClassificationSet,
            );
            let commit_step = commit(PipelineSource::Step {
                step_id: gate_id,
                port: "candidates".to_owned(),
                artifact_type: ArtifactKind::ClassificationSet,
            });
            (Vec::new(), vec![classifier, gate_step, commit_step])
        }
        TaskKind::BoundingBox => {
            let detector_id = "shared.detector".to_owned();
            let filter_id = format!("{target_task_id}.{target_label}.filter");
            let gate_id = format!("{target_task_id}.{target_label}.confidence");
            let preferred = preferred_model_for(
                models,
                constraints.preferred_model_id.as_deref(),
                &[
                    VisionCapability::ObjectDetection,
                    VisionCapability::VisionLanguage,
                    VisionCapability::OpenVocabularyDetection,
                ],
            )?;
            let (model_id, capability, node_type, kind, parameters) =
                if let Some((model_id, capability)) = preferred {
                    let (node_type, kind, parameters) = match capability {
                        VisionCapability::VisionLanguage => (
                            annotagent_skill_vlm_detection::VLM_DETECTION_OPERATION.to_owned(),
                            WorkflowNodeKind::VisionLanguageModel,
                            BTreeMap::from([
                                ("labels".to_owned(), json!([target_label])),
                                (
                                    "target_description".to_owned(),
                                    json!(format!("the {target_label} object itself")),
                                ),
                            ]),
                        ),
                        VisionCapability::OpenVocabularyDetection => (
                            annotagent_skill_open_vocabulary::OPEN_VOCABULARY_DETECTION_OPERATION
                                .to_owned(),
                            WorkflowNodeKind::VisionModel,
                            BTreeMap::from([(
                                "queries".to_owned(),
                                json!([{
                                    "id": target_label,
                                    "text": target_label.replace(['_', '-'], " "),
                                    "target_label": target_label,
                                }]),
                            )]),
                        ),
                        VisionCapability::ObjectDetection => (
                            annotagent_skill_object_detection::OBJECT_DETECTION_OPERATION
                                .to_owned(),
                            WorkflowNodeKind::VisionModel,
                            BTreeMap::from([
                                ("target_labels".to_owned(), json!([target_label])),
                                (
                                    "class_mapping".to_owned(),
                                    json!(BTreeMap::from([(
                                        target_label.to_owned(),
                                        target_label.to_owned(),
                                    )])),
                                ),
                            ]),
                        ),
                        _ => unreachable!("preferred_model_for returns an allowed capability"),
                    };
                    (model_id, capability, node_type, kind, parameters)
                } else {
                    (
                        "mock-detector".to_owned(),
                        VisionCapability::ObjectDetection,
                        annotagent_skill_yolo::YOLO_DETECTION_OPERATION.to_owned(),
                        WorkflowNodeKind::VisionModel,
                        BTreeMap::from([
                            ("mock_label".to_owned(), json!(target_label)),
                            ("mock_class_id".to_owned(), json!(target_label)),
                        ]),
                    )
                };
            let detector = PipelineStep {
                id: detector_id.clone(),
                node_type,
                kind,
                inputs: BTreeMap::from([("image".to_owned(), PipelineSource::Image)]),
                outputs: BTreeMap::from([("detections".to_owned(), ArtifactKind::DetectionSet)]),
                model_binding: Some(PipelineModelBinding {
                    model_id,
                    capability,
                    configuration: BTreeMap::new(),
                }),
                skill_binding: None,
                parameters,
                validators: Vec::new(),
                refiners: Vec::new(),
                fallback: None,
                retry_policy: RetryPolicy {
                    max_attempts: project.runtime.max_retries.saturating_add(1),
                },
                review_gate: ReviewGate::default(),
                resources: ResourceRequirements {
                    timeout_seconds: Some(project.runtime.task_timeout_seconds),
                    ..ResourceRequirements::default()
                },
            };
            let filter = PipelineStep {
                id: filter_id.clone(),
                node_type: annotagent_runtime::CORE_FILTER.to_owned(),
                kind: WorkflowNodeKind::Transform,
                inputs: BTreeMap::from([(
                    "detections".to_owned(),
                    PipelineSource::SharedStage {
                        stage_id: "shared-vision".to_owned(),
                        step_id: detector_id,
                        port: "detections".to_owned(),
                        artifact_type: ArtifactKind::DetectionSet,
                    },
                )]),
                outputs: BTreeMap::from([("detections".to_owned(), ArtifactKind::DetectionSet)]),
                model_binding: None,
                skill_binding: None,
                parameters: BTreeMap::from([
                    ("labels".to_owned(), json!([target_label])),
                    ("minimum_confidence".to_owned(), json!(0.0)),
                ]),
                validators: task.validators.clone(),
                refiners: task.refiners.clone(),
                fallback: None,
                retry_policy: RetryPolicy::default(),
                review_gate: ReviewGate::default(),
                resources: ResourceRequirements::default(),
            };
            let gate_step = gate(
                PipelineSource::Step {
                    step_id: filter_id,
                    port: "detections".to_owned(),
                    artifact_type: ArtifactKind::DetectionSet,
                },
                ArtifactKind::DetectionSet,
            );
            let commit_step = commit(PipelineSource::Step {
                step_id: gate_id,
                port: "candidates".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
            });
            (
                vec![SharedWorkflowStage {
                    id: "shared-vision".to_owned(),
                    name: "Shared detector".to_owned(),
                    steps: vec![detector],
                }],
                vec![filter, gate_step, commit_step],
            )
        }
        other => bail!(
            "Label Pipeline Advisor currently supports classification and bounding_box tasks, not {other:?}"
        ),
    };
    Ok(LabelWorkflowComposition {
        schema_version: annotagent_core::LABEL_PIPELINE_SCHEMA_VERSION,
        shared_stages,
        label_pipelines: vec![LabelPipeline {
            id: format!("{target_task_id}.{target_label}"),
            target_task_id: target_task,
            target_label: target,
            steps,
        }],
    })
}

fn preferred_model_for(
    models: &ModelRegistry,
    preferred_model_id: Option<&str>,
    allowed_capabilities: &[VisionCapability],
) -> Result<Option<(String, VisionCapability)>> {
    let Some(model_id) = preferred_model_id else {
        return Ok(None);
    };
    let (model, _) = models.resolve(model_id).map_err(|error| anyhow!(error))?;
    if model.status != ModelAvailabilityStatus::Available
        && model.health.status != VisionModelHealthStatus::Healthy
    {
        bail!(
            "preferred Model {model_id:?} is not ready: status={:?}, health={:?}",
            model.status,
            model.health.status
        );
    }
    let capability = allowed_capabilities
        .iter()
        .copied()
        .find(|capability| model.capabilities.contains(capability))
        .ok_or_else(|| {
            anyhow!(
                "preferred Model {model_id:?} does not provide any allowed capability: {allowed_capabilities:?}"
            )
        })?;
    Ok(Some((model.id.clone(), capability)))
}

fn compile_label_projection(draft: WorkflowDraft, project: &ProjectSchema) -> WorkflowDraft {
    let Some(composition) = draft.label_pipeline.clone() else {
        return draft;
    };
    let mut compiled = composition.compile_draft(
        draft.project_id.clone(),
        draft.name.clone(),
        project.project.enabled_skill_versions(),
        draft.created_at,
    );
    compiled.id = draft.id;
    compiled.status = draft.status;
    compiled.resource_versions = draft.resource_versions;
    compiled.allow_unvalidated_commit = draft.allow_unvalidated_commit;
    compiled.created_at = draft.created_at;
    compiled.updated_at = draft.updated_at;
    compiled
}

fn label_projection_issues(
    draft: &WorkflowDraft,
    project: &ProjectSchema,
    nodes: &NodeRegistry,
    models: &ModelRegistry,
) -> Vec<WorkflowValidationIssue> {
    draft
        .label_pipeline
        .as_ref()
        .map_or_else(Vec::new, |composition| {
            LabelPipelineStaticValidator
                .validate(composition, project, nodes, models)
                .issues
                .into_iter()
                .map(|issue| WorkflowValidationIssue {
                    code: issue.code,
                    path: format!("label_pipeline.{}", issue.path),
                    message: issue.message,
                    blocking: issue.blocking,
                })
                .collect()
        })
}

fn node_artifact_inspection(
    run_id: RunId,
    workflow: &PublishedWorkflowVersion,
    checkpoint: &DagCheckpoint,
    image_index: Option<usize>,
) -> RunNodeArtifactInspection {
    let nodes = workflow
        .draft
        .nodes
        .iter()
        .filter_map(|configuration| {
            let trace = checkpoint
                .traces
                .iter()
                .rev()
                .find(|trace| trace.node_id == configuration.id)?;
            Some(NodeArtifactInspection {
                node_id: trace.node_id.clone(),
                operation: trace.operation.clone(),
                status: trace.status,
                configuration: configuration.clone(),
                inputs: trace.input_pipeline_artifacts.clone(),
                outputs: trace.output_pipeline_artifacts.clone(),
                latency_ms: (trace.finished_at - trace.started_at)
                    .num_milliseconds()
                    .max(0)
                    .try_into()
                    .unwrap_or(u64::MAX),
                attempts: trace.attempt_count,
                cache_hit: trace.cache_hit,
                usage: trace.usage.clone(),
                route: trace.route.clone(),
                metadata: checkpoint
                    .node_outputs
                    .get(&trace.node_id)
                    .map_or_else(BTreeMap::new, |output| output.metadata.clone()),
                error: trace.error.clone(),
            })
        })
        .collect();
    RunNodeArtifactInspection {
        run_id,
        workflow_id: workflow.workflow_id.clone(),
        workflow_version: workflow.version,
        content_hash: workflow.content_hash.clone(),
        project_id: workflow.project_id.clone(),
        image_index,
        nodes,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StartedRun {
    pub run_id: RunId,
    pub image_path: PathBuf,
    pub status: RunStatus,
    pub idempotent: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunNodeArtifactInspection {
    pub run_id: RunId,
    pub workflow_id: String,
    pub workflow_version: u32,
    pub content_hash: String,
    pub project_id: String,
    pub image_index: Option<usize>,
    pub nodes: Vec<NodeArtifactInspection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunAnnotationInspection {
    pub run_id: RunId,
    pub project_id: String,
    pub image_index: Option<usize>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResultSummary {
    pub run_id: RunId,
    pub project_id: String,
    pub status: RunStatus,
    pub image_count: usize,
    pub result_count: usize,
    pub ready_count: usize,
    pub needs_review_count: usize,
    pub no_target_count: usize,
    pub failed_count: usize,
    pub fallback_count: usize,
    pub cache_hit_count: usize,
    pub duration_ms: u64,
    pub usage: UsageSummary,
    pub image_index: Option<usize>,
    pub labels: Vec<RunResultLabelSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResultLabelSummary {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunDebugSummary {
    pub run_id: RunId,
    pub workflow_id: Option<String>,
    pub workflow_version: Option<u32>,
    pub node_count: usize,
    pub succeeded_node_count: usize,
    pub failed_node_count: usize,
    pub current_node: Option<String>,
    pub issues: Vec<RunDebugIssue>,
    pub duration_ms: u64,
    pub usage: UsageSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunDebugIssue {
    pub node_id: String,
    pub code: String,
    pub summary: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportReadiness {
    pub project_id: String,
    pub ready: bool,
    pub image_count: u64,
    pub processed_image_count: u64,
    pub accepted_annotations: u64,
    pub unresolved_reviews: u64,
    pub blocking_issues: Vec<ExportBlocker>,
    pub recommended_format: Option<String>,
    pub formats: Vec<ExportFormatCompatibility>,
    pub output_root: PathBuf,
    pub last_export: Option<ProjectExportResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBlocker {
    pub code: String,
    pub title: String,
    pub explanation: String,
    pub repair_destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFormatCompatibility {
    pub format: String,
    pub display_name: String,
    pub supported: bool,
    pub recommended: bool,
    pub summary: String,
    pub warnings: Vec<String>,
    pub unsupported_task_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectExportResult {
    pub format: String,
    pub output_path: PathBuf,
    pub completed_at: String,
    #[serde(default)]
    pub source_fingerprint: String,
    pub report: ExportReport,
}

struct ProjectExportData {
    snapshot: ProjectSnapshot,
    project_root: PathBuf,
    image_count: usize,
    processed_image_count: usize,
    unresolved_reviews: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeArtifactInspection {
    pub node_id: String,
    pub operation: String,
    pub status: DagNodeStatus,
    pub configuration: annotagent_core::WorkflowDraftNode,
    pub inputs: Vec<PipelineArtifact>,
    pub outputs: Vec<PipelineArtifact>,
    pub latency_ms: u64,
    pub attempts: u32,
    pub cache_hit: bool,
    pub usage: DagNodeUsage,
    pub route: Option<String>,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub error: Option<DagNodeFailure>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeReplayReport {
    pub source_run_id: RunId,
    pub replayed_from: String,
    pub reexecuted_nodes: Vec<String>,
    pub preserved_upstream_nodes: Vec<String>,
    pub inspection: RunNodeArtifactInspection,
    pub sandbox: bool,
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
    layered_skills: Arc<LayeredSkillRegistry>,
    event_sender: broadcast::Sender<RunEvent>,
    active: Mutex<HashMap<RunId, ManagedRun>>,
    agent_cancellations: Mutex<HashMap<uuid::Uuid, CancellationToken>>,
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
        let mut layered_skills = LayeredSkillRegistry::new();
        let classification =
            Arc::new(annotagent_skill_classification::ClassificationCapabilitySkill::default());
        registry.register_layered(classification.clone())?;
        layered_skills.register(classification)?;
        let legacy_classification = Arc::new(
            annotagent_skill_classification::LegacyClassificationCapabilitySkill::default(),
        );
        registry.register_layered(legacy_classification.clone())?;
        layered_skills.register(legacy_classification)?;
        let open_vocabulary =
            Arc::new(annotagent_skill_open_vocabulary::OpenVocabularyGroundingSkill::default());
        registry.register_layered(open_vocabulary.clone())?;
        layered_skills.register(open_vocabulary)?;
        let object_detection =
            Arc::new(annotagent_skill_object_detection::ObjectDetectionCapabilitySkill::default());
        registry.register_layered(object_detection.clone())?;
        layered_skills.register(object_detection)?;
        let legacy_object_detection = Arc::new(
            annotagent_skill_object_detection::LegacyObjectDetectionCapabilitySkill::default(),
        );
        registry.register_layered(legacy_object_detection.clone())?;
        layered_skills.register(legacy_object_detection)?;
        let vlm_detection =
            Arc::new(annotagent_skill_vlm_detection::VlmDetectionCapabilitySkill::default());
        registry.register_layered(vlm_detection.clone())?;
        layered_skills.register(vlm_detection)?;
        let segmentation =
            Arc::new(annotagent_skill_segmentation::SegmentationCapabilitySkill::default());
        registry.register_layered(segmentation.clone())?;
        layered_skills.register(segmentation)?;
        let ball = Arc::new(RoboCupBallSkill::new().map_err(|error| anyhow!(error))?);
        registry.register_layered(ball.clone())?;
        layered_skills.register(ball)?;
        let pack = Arc::new(RoboCupPackSkill::new().map_err(|error| anyhow!(error))?);
        registry.register_layered(pack.clone())?;
        layered_skills.register(pack)?;
        let (event_sender, _) = broadcast::channel(1024);
        Ok(Self {
            workspace,
            database_path,
            store,
            skills: Arc::new(registry),
            layered_skills: Arc::new(layered_skills),
            event_sender,
            active: Mutex::new(HashMap::new()),
            agent_cancellations: Mutex::new(HashMap::new()),
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

    /// Runs bounded domain recovery for one risky candidate. Correction records are selected only
    /// from the candidate's exact Project, Skill, task and Label scope. A clean candidate returns
    /// through the fast path without creating or persisting an Agent Session.
    pub async fn recover_ball_candidate(
        &self,
        project_id: &str,
        input: BallRecoveryInput,
    ) -> Result<RoboCupBallRecoveryReport> {
        let BallRecoveryInput {
            candidate,
            related_annotations,
            issues,
            image_path,
            budget,
            cancellation,
        } = input;
        let project_path = self.project_path(project_id)?;
        let project_root = project_path
            .parent()
            .unwrap_or(&self.workspace)
            .canonicalize()
            .context("cannot canonicalize recovery Project root")?;
        let memory_project_id = stable_project_id(&project_root);
        let correction_memory = self.store.query_corrections(
            memory_project_id,
            ROBOCUP_BALL_SKILL_ID,
            &candidate.task_id,
            candidate.label.as_ref(),
            20,
        )?;
        let image = image_path
            .map(|path| {
                let path = if path.is_absolute() {
                    path
                } else {
                    project_root.join(path)
                };
                let path = path
                    .canonicalize()
                    .with_context(|| format!("cannot access recovery image {}", path.display()))?;
                ensure_within(&project_root, &path)?;
                load_image(&path, 40_000_000)
                    .map(Arc::new)
                    .map_err(|error| anyhow!(error))
            })
            .transpose()?;
        let mut report = RoboCupBallRecoveryAgent
            .run(RoboCupBallRecoveryRequest {
                project_id: memory_project_id,
                project_root,
                candidate,
                related_annotations,
                issues,
                correction_memory,
                image,
                budget,
                cancellation,
            })
            .await
            .map_err(anyhow::Error::msg)?;
        if let Some(session) = report.session.as_mut() {
            session.project_id = Some(project_id.to_owned());
            self.store.save_agent_session(session)?;
        }
        Ok(report)
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

    #[must_use]
    pub fn layered_skills(&self) -> Arc<LayeredSkillRegistry> {
        self.layered_skills.clone()
    }

    pub fn list_agent_sessions(&self, project_id: &str) -> Result<Vec<AgentSession>> {
        validate_project_id(project_id)?;
        self.store
            .list_agent_sessions(Some(project_id))
            .map_err(anyhow::Error::from)
    }

    pub fn cancel_agent_session(&self, session_id: uuid::Uuid) -> Result<AgentSession> {
        let mut session = self.store.get_agent_session(session_id)?;
        if !matches!(
            session.status,
            AgentSessionStatus::Running | AgentSessionStatus::WaitingForHuman
        ) {
            bail!(
                "Agent Session is already terminal with status {:?}",
                session.status
            );
        }
        if let Some(cancellation) = self
            .agent_cancellations
            .lock()
            .map_err(|_| anyhow!("Agent cancellation registry lock poisoned"))?
            .get(&session_id)
        {
            cancellation.cancel();
        }
        session.cancel();
        self.store.save_agent_session(&session)?;
        Ok(session)
    }

    pub fn list_project_correction_memory(
        &self,
        project_id: &str,
    ) -> Result<Vec<annotagent_core::CorrectionRecord>> {
        let project_path = self.project_path(project_id)?;
        let project_root = project_path.parent().unwrap_or(&self.workspace);
        self.store
            .list_project_corrections(stable_project_id(project_root), 200)
            .map_err(anyhow::Error::from)
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

    pub fn add_project_label(
        &self,
        project_id: &str,
        task_id: &str,
        label: &str,
    ) -> Result<ProjectSummary> {
        let path = self.project_path(project_id)?;
        let (mut project, _) = load_project_schema_with_registry(&path, &self.skills)?;
        let task = project
            .tasks
            .iter_mut()
            .find(|task| task.id.as_str() == task_id)
            .ok_or_else(|| anyhow!("task {task_id:?} is not in Project Schema"))?;
        if task.labels.iter().any(|current| current == label) {
            bail!("Label {label:?} already exists on task {task_id:?}");
        }
        task.labels.push(label.to_owned());
        resolve_project_skills(&project, &self.skills)?;
        let yaml = serde_yaml::to_string(&project)?;
        std::fs::write(&path, yaml)?;
        self.get_project(project_id)
    }

    pub fn set_project_enabled_skills(
        &self,
        project_id: &str,
        enabled_skills: Vec<EnabledSkillConfig>,
    ) -> Result<ProjectSummary> {
        let path = self.project_path(project_id)?;
        let yaml = std::fs::read_to_string(&path)?;
        let mut project = ProjectSchema::from_yaml(&yaml).map_err(|error| anyhow!(error))?;
        project.project.enabled_skills = enabled_skills;
        project.project.skill.clear();
        project.project.skill_version.clear();
        resolve_project_skills(&project, &self.skills)?;
        std::fs::write(&path, serde_yaml::to_string(&project)?)?;
        self.get_project(project_id)
    }

    pub fn add_project_task(
        &self,
        project_id: &str,
        display_name: &str,
        kind: TaskKind,
        labels: Vec<String>,
        attributes: BTreeMap<String, AttributeDefinition>,
    ) -> Result<ProjectSummary> {
        let display_name = display_name.trim();
        if display_name.is_empty() {
            bail!("Label group display name cannot be empty");
        }
        let path = self.project_path(project_id)?;
        let (mut project, _) = load_project_schema_with_registry(&path, &self.skills)?;
        let base = display_name
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_owned();
        let base = if base.is_empty() {
            "label_task".to_owned()
        } else {
            base
        };
        let mut id = base.clone();
        let mut suffix = 2;
        while project.tasks.iter().any(|task| task.id.as_str() == id) {
            id = format!("{base}_{suffix}");
            suffix += 1;
        }
        project.tasks.push(TaskConfig {
            id: TaskId::new(id),
            display_name: Some(display_name.to_owned()),
            kind,
            labels,
            required: true,
            multi_label: false,
            depends_on: Vec::new(),
            validators: Vec::new(),
            refiners: Vec::new(),
            target_task: None,
            target_labels: Vec::new(),
            attributes,
        });
        resolve_project_skills(&project, &self.skills)?;
        std::fs::write(&path, serde_yaml::to_string(&project)?)?;
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
        let default_workflow_version = available_workflow_versions
            .iter()
            .find(|workflow| workflow.is_default && workflow.status == WorkflowStatus::Published)
            .cloned()
            .or_else(|| {
                available_workflow_versions
                    .iter()
                    .rev()
                    .find(|workflow| workflow.status == WorkflowStatus::Published)
                    .cloned()
            });
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
                    availability_group: ModelAvailabilityGroup::ConfiguredUnavailable,
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
                }]
            })
            .unwrap_or_default();
        let review_count = project_runs
            .iter()
            .map(|run| self.store.list_annotations(run.id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .filter(|annotation| annotation.review_status == ReviewStatus::NeedsReview)
            .count();
        let mut blocking_issues = Vec::new();
        if image_count == 0 {
            blocking_issues.push(ProjectBlockingIssue {
                code: "no_images".to_owned(),
                message: "Import at least one supported image.".to_owned(),
                next_step: "data".to_owned(),
            });
        }
        if project.tasks.is_empty() || project.tasks.iter().all(|task| task.labels.is_empty()) {
            blocking_issues.push(ProjectBlockingIssue {
                code: "no_labels".to_owned(),
                message: "Define at least one Label in the Project Schema.".to_owned(),
                next_step: "labels".to_owned(),
            });
        }
        if default_workflow_version.is_none() {
            blocking_issues.push(ProjectBlockingIssue {
                code: "no_published_pipeline".to_owned(),
                message: "Publish a valid Pipeline Version before starting a Run.".to_owned(),
                next_step: "pipeline".to_owned(),
            });
        }
        let configuration_issue = available_workflow_versions.iter().any(|workflow| {
            workflow.is_default
                && (workflow.status != WorkflowStatus::Published
                    || workflow.validation_status != "valid")
        });
        if configuration_issue {
            blocking_issues.push(ProjectBlockingIssue {
                code: "invalid_default_pipeline".to_owned(),
                message: "The default Pipeline has validation issues.".to_owned(),
                next_step: "pipeline".to_owned(),
            });
        }
        let readiness = if configuration_issue {
            ProjectReadiness::ConfigurationIssue
        } else if blocking_issues.is_empty() {
            ProjectReadiness::Ready
        } else {
            ProjectReadiness::Incomplete
        };
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
                    display_name: task
                        .display_name
                        .clone()
                        .unwrap_or_else(|| task.id.to_string()),
                    kind: task_kind_name(task.kind),
                    labels: task.labels.clone(),
                    required: task.required,
                })
                .collect(),
            enabled_skills: project
                .project
                .enabled_skill_versions()
                .into_iter()
                .map(|(id, version)| {
                    let catalog = self.skills.catalog_entry(&id)?;
                    Ok(EnabledSkill {
                        id,
                        display_name: catalog.display_name,
                        version,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
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
            task_count: project.tasks.len(),
            review_count,
            readiness,
            blocking_issues,
            default_workflow_version,
            active_batch,
            active_batch_progress,
            active_run,
            last_run,
        })
    }

    pub fn project_guidance(
        &self,
        project_id: &str,
        settings: &Settings,
        workspace_model_connected: bool,
    ) -> Result<ProjectGuidance> {
        let summary = self.get_project(project_id)?;
        let project_path = self.project_path(project_id)?;
        let mut updated_at = project_path
            .metadata()
            .and_then(|metadata| metadata.modified())
            .map_or_else(
                |_| chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
                chrono::DateTime::<chrono::Utc>::from,
            );
        let mut drafts = self
            .store
            .list_workflow_drafts(Some(project_id))?
            .into_iter()
            .filter(|draft| draft.status != WorkflowDraftStatus::Archived)
            .collect::<Vec<_>>();
        drafts.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        if let Some(draft) = drafts.first() {
            updated_at = updated_at.max(draft.updated_at);
        }
        let published = self
            .store
            .list_published_workflow_versions(Some(project_id))?
            .into_iter()
            .max_by_key(|version| version.published_at);
        if let Some(version) = &published {
            updated_at = updated_at.max(version.published_at);
        }
        for run in summary.active_run.iter().chain(summary.last_run.iter()) {
            if let Ok(value) = chrono::DateTime::parse_from_rfc3339(&run.updated_at) {
                updated_at = updated_at.max(value.with_timezone(&chrono::Utc));
            }
        }

        let editable_draft = drafts.iter().find(|draft| {
            matches!(
                draft.status,
                WorkflowDraftStatus::Suggested
                    | WorkflowDraftStatus::Editing
                    | WorkflowDraftStatus::Validated
            )
        });
        let automation = published
            .as_ref()
            .map(|version| &version.draft)
            .or(editable_draft);
        let has_automation = automation.is_some();
        let automation_valid = if published.is_some() {
            true
        } else if let Some(draft) = editable_draft {
            self.validate_workflow_draft(draft, settings, false)?
                .issues
                .iter()
                .all(|issue| !issue.blocking || issue.code == "unresolved_model_binding")
        } else {
            true
        };
        let model_nodes = automation
            .into_iter()
            .flat_map(|draft| draft.nodes.iter())
            .filter(|node| {
                matches!(
                    node.kind,
                    WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
                )
            })
            .collect::<Vec<_>>();
        let all_model_nodes_bound = model_nodes.iter().all(|node| node.model_binding.is_some());
        let needs_workspace_connection = model_nodes.iter().any(|node| {
            node.model_binding
                .as_deref()
                .is_some_and(|binding| !binding.starts_with("mock"))
        });
        let has_model_binding =
            all_model_nodes_bound && (!needs_workspace_connection || workspace_model_connected);

        let sample_test = if published.is_some() {
            SampleTestState::Passed
        } else if let Some(draft) = editable_draft {
            self.store.get_workflow_sample_test(&draft.id)?.map_or(
                SampleTestState::NotRun,
                |record| {
                    updated_at = updated_at.max(record.completed_at);
                    if record.report.validation.valid
                        && record.report.summary.failed_count == 0
                        && record.report.summary.needs_review_count == 0
                    {
                        SampleTestState::Passed
                    } else {
                        SampleTestState::NeedsAttention
                    }
                },
            )
        } else {
            SampleTestState::NotRun
        };
        let project_root = project_path.parent().unwrap_or(&self.workspace);
        let stable_id = stable_project_id(project_root);
        let project_runs = self
            .store
            .list_runs()?
            .into_iter()
            .filter(|run| {
                run.project_id == Some(stable_id)
                    || (run.project_id.is_none() && run.project_name == summary.name)
            })
            .collect::<Vec<_>>();
        let has_completed_run = project_runs.iter().any(|run| {
            matches!(
                run.status,
                RunStatus::Completed | RunStatus::CompletedWithReview | RunStatus::Partial
            )
        });
        let has_labels = !summary.annotation_schema.is_empty()
            && summary
                .annotation_schema
                .iter()
                .any(|task| !task.labels.is_empty());
        let guidance = derive_project_guidance(ProjectGuidanceInput {
            project_id: project_id.to_owned(),
            image_count: summary.image_count,
            has_labels,
            has_automation,
            has_model_binding,
            automation_valid,
            sample_test,
            automation_activated: published.is_some(),
            active_run_id: summary.active_run.as_ref().map(|run| run.id.to_string()),
            active_batch_id: summary
                .active_batch
                .as_ref()
                .map(|batch| batch.id.to_string()),
            review_count: summary.review_count,
            has_completed_run,
            updated_at,
        });
        Ok(guidance)
    }

    pub fn project_workspace_summary(
        &self,
        project_id: &str,
        settings: &Settings,
        workspace_model_connected: bool,
    ) -> Result<ProjectWorkspaceSummary> {
        let project = self.get_project(project_id)?;
        let guidance = self.project_guidance(project_id, settings, workspace_model_connected)?;
        let readiness = guidance.readiness_summary();
        Ok(ProjectWorkspaceSummary {
            project,
            guidance,
            readiness,
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

    pub fn run_result_summary(&self, run_id: RunId) -> Result<RunResultSummary> {
        let history = self.store.history(run_id)?;
        let inspection = self.inspect_run_annotations(run_id)?;
        let pipeline_inspection = self.inspect_run_pipeline_artifacts(run_id).ok();
        let results = inspection
            .annotations
            .iter()
            .filter(|annotation| annotation.review_status != ReviewStatus::Rejected)
            .collect::<Vec<_>>();
        let mut result_count = results.len();
        let mut ready_count = results
            .iter()
            .filter(|annotation| {
                matches!(
                    annotation.review_status,
                    ReviewStatus::Draft | ReviewStatus::AutoAccepted | ReviewStatus::HumanAccepted
                )
            })
            .count();
        let mut needs_review_count = results
            .iter()
            .filter(|annotation| annotation.review_status == ReviewStatus::NeedsReview)
            .count();
        let failed_count = usize::from(matches!(
            history.run.status,
            RunStatus::Partial
                | RunStatus::BudgetExceeded
                | RunStatus::Failed
                | RunStatus::Interrupted
        ));
        let mut label_counts = BTreeMap::<String, usize>::new();
        for annotation in &results {
            let label = annotation
                .label
                .as_ref()
                .map_or_else(|| annotation.task_id.to_string(), ToString::to_string);
            *label_counts.entry(label).or_default() += 1;
        }
        if result_count == 0 {
            let mut detections = BTreeMap::new();
            let mut classifications = BTreeMap::new();
            let mut candidates = BTreeMap::new();
            if let Some(pipeline) = pipeline_inspection.as_ref() {
                for artifact in pipeline.nodes.iter().flat_map(|node| &node.outputs) {
                    match artifact {
                        PipelineArtifact::DetectionSet(set) => {
                            for detection in &set.detections {
                                detections.insert(
                                    detection.detection_id.clone(),
                                    (
                                        detection.project_label.as_ref().map_or_else(
                                            || {
                                                detection
                                                    .model_label
                                                    .clone()
                                                    .unwrap_or_else(|| "unlabeled".to_owned())
                                            },
                                            ToString::to_string,
                                        ),
                                        sample_test_outcome_status(Some(set.validation_state)),
                                    ),
                                );
                            }
                        }
                        PipelineArtifact::ClassificationSet(set) => {
                            for classification in &set.classifications {
                                classifications.insert(
                                    classification.id.clone(),
                                    (
                                        classification.label.to_string(),
                                        sample_test_outcome_status(Some(set.validation_state)),
                                    ),
                                );
                            }
                        }
                        PipelineArtifact::AnnotationCandidateSet(set) => {
                            for candidate in &set.candidates {
                                candidates.insert(
                                    candidate.id.clone(),
                                    (
                                        candidate.label.to_string(),
                                        sample_test_outcome_status(candidate.validation_state),
                                    ),
                                );
                            }
                        }
                        PipelineArtifact::CandidateClusterSet(set) => {
                            for candidate in &set.candidates {
                                candidates.insert(
                                    candidate.id.clone(),
                                    (
                                        candidate.target_label.to_string(),
                                        sample_test_outcome_status(Some(set.validation_state)),
                                    ),
                                );
                            }
                        }
                        PipelineArtifact::Image(_) | PipelineArtifact::CropSet(_) => {}
                    }
                }
            }
            let outcomes = if !candidates.is_empty() {
                candidates
            } else if !classifications.is_empty() {
                classifications
            } else {
                detections
            };
            result_count = outcomes.len();
            ready_count = outcomes
                .values()
                .filter(|(_, status)| *status == SampleTestOutcomeStatus::ReadyToAccept)
                .count();
            needs_review_count = outcomes
                .values()
                .filter(|(_, status)| *status == SampleTestOutcomeStatus::NeedsReview)
                .count();
            for (label, _) in outcomes.into_values() {
                *label_counts.entry(label).or_default() += 1;
            }
        }
        let no_target_count = usize::from(
            result_count == 0
                && failed_count == 0
                && matches!(
                    history.run.status,
                    RunStatus::Completed | RunStatus::CompletedWithReview
                ),
        );
        let fallback_count = history
            .run
            .workflow_snapshot_json
            .as_deref()
            .and_then(|snapshot| serde_json::from_str::<serde_json::Value>(snapshot).ok())
            .and_then(|snapshot| {
                snapshot
                    .pointer("/checkpoint/activated_fallbacks")
                    .and_then(serde_json::Value::as_array)
                    .map(Vec::len)
            })
            .unwrap_or(0);
        let cache_hit_count = pipeline_inspection.as_ref().map_or(0, |pipeline| {
            pipeline.nodes.iter().filter(|node| node.cache_hit).count()
        });
        Ok(RunResultSummary {
            run_id,
            project_id: inspection.project_id,
            status: history.run.status,
            image_count: 1,
            result_count,
            ready_count,
            needs_review_count,
            no_target_count,
            failed_count,
            fallback_count,
            cache_hit_count,
            duration_ms: history_run_duration_ms(&history.run),
            usage: history_usage_summary(&history),
            image_index: inspection.image_index,
            labels: label_counts
                .into_iter()
                .map(|(label, count)| RunResultLabelSummary { label, count })
                .collect(),
        })
    }

    pub fn run_debug_summary(&self, run_id: RunId) -> Result<RunDebugSummary> {
        let history = self.store.history(run_id)?;
        let inspection = self.inspect_run_pipeline_artifacts(run_id).ok();
        let nodes = inspection
            .as_ref()
            .map_or(&[][..], |inspection| inspection.nodes.as_slice());
        let issues = nodes
            .iter()
            .filter_map(|node| {
                node.error.as_ref().map(|error| RunDebugIssue {
                    node_id: node.node_id.clone(),
                    code: error.code.clone(),
                    summary: error.summary.clone(),
                    retryable: error.retryable,
                })
            })
            .collect::<Vec<_>>();
        let current_node = nodes
            .iter()
            .find(|node| {
                matches!(
                    node.status,
                    DagNodeStatus::Running | DagNodeStatus::AwaitingReview
                )
            })
            .or_else(|| nodes.iter().find(|node| node.error.is_some()))
            .map(|node| node.node_id.clone());
        Ok(RunDebugSummary {
            run_id,
            workflow_id: inspection
                .as_ref()
                .map(|inspection| inspection.workflow_id.clone()),
            workflow_version: inspection
                .as_ref()
                .map(|inspection| inspection.workflow_version),
            node_count: nodes.len(),
            succeeded_node_count: nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.status,
                        DagNodeStatus::Succeeded
                            | DagNodeStatus::Cached
                            | DagNodeStatus::Skipped
                            | DagNodeStatus::FailedWithFallback
                    )
                })
                .count(),
            failed_node_count: nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.status,
                        DagNodeStatus::Failed | DagNodeStatus::Cancelled
                    )
                })
                .count(),
            current_node,
            issues,
            duration_ms: history_run_duration_ms(&history.run),
            usage: history_usage_summary(&history),
        })
    }

    pub fn export_readiness(&self, project_id: &str) -> Result<ExportReadiness> {
        let data = self.project_export_data(project_id)?;
        export_readiness_from_data(project_id, &data)
    }

    pub async fn export_project_dataset(
        &self,
        project_id: &str,
        requested_format: &str,
    ) -> Result<ProjectExportResult> {
        let data = self.project_export_data(project_id)?;
        let readiness = export_readiness_from_data(project_id, &data)?;
        if !readiness.ready {
            bail!(
                "dataset is not ready to export: {}",
                readiness
                    .blocking_issues
                    .iter()
                    .map(|issue| issue.title.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        let format = canonical_export_format(requested_format);
        let compatibility = readiness
            .formats
            .iter()
            .find(|candidate| candidate.format == format)
            .ok_or_else(|| anyhow!("format {format:?} is not enabled by the Project Schema"))?;
        if !compatibility.supported {
            bail!(
                "format {format:?} is incompatible with task kinds: {}",
                compatibility.unsupported_task_kinds.join(", ")
            );
        }
        let output_path = data.project_root.join("exports").join(&format);
        let source_fingerprint = sha256(&serde_json::to_vec(&data.snapshot)?);
        let mut report = dataset_exporter(&format)?
            .export(ExportRequest {
                project: data.snapshot,
                output: output_path.clone(),
            })
            .await
            .map_err(|error| anyhow!(error))?;
        let report_path = output_path.join("export-report.json");
        report.output_files.push(report_path.clone());
        let result = ProjectExportResult {
            format,
            output_path,
            completed_at: chrono::Utc::now().to_rfc3339(),
            source_fingerprint,
            report,
        };
        std::fs::write(&report_path, serde_json::to_vec_pretty(&result)?)
            .with_context(|| format!("cannot write export report {}", report_path.display()))?;
        Ok(result)
    }

    fn project_export_data(&self, project_id: &str) -> Result<ProjectExportData> {
        let project_path = self.project_path(project_id)?;
        let (schema, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let project_root = project_path
            .parent()
            .unwrap_or(&self.workspace)
            .to_path_buf();
        let stable_id = stable_project_id(&project_root);
        let image_paths = self.list_project_images(project_id)?;
        let mut unresolved_reviews = 0_usize;
        let mut selected_runs = BTreeMap::<usize, (HistoryRun, Vec<Annotation>)>::new();
        for run in self.store.list_runs()? {
            let belongs_to_project = run.project_id.as_ref().map_or_else(
                || run.project_name == schema.project.name,
                |run_project_id| *run_project_id == stable_id,
            );
            if !belongs_to_project {
                continue;
            }
            let annotations = self.store.list_annotations(run.id)?;
            unresolved_reviews += annotations
                .iter()
                .filter(|annotation| annotation.review_status == ReviewStatus::NeedsReview)
                .count();
            if !matches!(
                run.status,
                RunStatus::Completed | RunStatus::CompletedWithReview | RunStatus::Partial
            ) {
                continue;
            }
            let image_index = self
                .inspect_run_annotations(run.id)
                .ok()
                .and_then(|inspection| inspection.image_index)
                .or_else(|| (image_paths.len() == 1).then_some(0));
            if let Some(image_index) = image_index.filter(|index| *index < image_paths.len()) {
                selected_runs
                    .entry(image_index)
                    .or_insert((run, annotations));
            }
        }

        let processed_image_count = selected_runs.len();
        let mut annotations = Vec::new();
        let mut image_ids = BTreeMap::<usize, BTreeSet<ImageId>>::new();
        let mut revisions = Vec::new();
        for (image_index, (run, candidates)) in selected_runs {
            let accepted = candidates
                .into_iter()
                .filter(|annotation| {
                    matches!(
                        annotation.review_status,
                        ReviewStatus::AutoAccepted | ReviewStatus::HumanAccepted
                    ) || (annotation.review_status == ReviewStatus::Draft
                        && run.status == RunStatus::Completed)
                })
                .collect::<Vec<_>>();
            let accepted_ids = accepted
                .iter()
                .map(|annotation| annotation.id)
                .collect::<BTreeSet<_>>();
            image_ids
                .entry(image_index)
                .or_default()
                .extend(accepted.iter().map(|annotation| annotation.image_id));
            revisions.extend(
                self.store
                    .history(run.id)?
                    .revisions
                    .into_iter()
                    .filter(|revision| accepted_ids.contains(&revision.annotation_id)),
            );
            annotations.extend(accepted);
        }

        let mut images = Vec::new();
        for (index, image_path) in image_paths.iter().enumerate() {
            let frame = load_image(image_path, 40_000_000)
                .with_context(|| format!("cannot load export image {}", image_path.display()))?;
            let relative_path = image_path
                .strip_prefix(&project_root)
                .unwrap_or(image_path)
                .to_path_buf();
            let ids = image_ids
                .remove(&index)
                .filter(|ids| !ids.is_empty())
                .unwrap_or_else(|| {
                    BTreeSet::from([ImageId(uuid::Uuid::new_v5(
                        &stable_id.0,
                        relative_path.to_string_lossy().as_bytes(),
                    ))])
                });
            for id in ids {
                images.push(SnapshotImage {
                    id,
                    relative_path: relative_path.clone(),
                    metadata: frame.metadata.clone(),
                });
            }
        }
        Ok(ProjectExportData {
            snapshot: ProjectSnapshot {
                schema,
                images,
                annotations,
                revisions,
            },
            project_root,
            image_count: image_paths.len(),
            processed_image_count,
            unresolved_reviews,
        })
    }

    pub fn inspect_run_pipeline_artifacts(
        &self,
        run_id: RunId,
    ) -> Result<RunNodeArtifactInspection> {
        let history = self
            .store
            .list_runs()?
            .into_iter()
            .find(|run| run.id == run_id)
            .ok_or_else(|| anyhow!("run {run_id} was not found"))?;
        let snapshot: serde_json::Value = serde_json::from_str(
            history
                .workflow_snapshot_json
                .as_deref()
                .ok_or_else(|| anyhow!("run {run_id} has no Workflow checkpoint"))?,
        )?;
        let image_index = snapshot
            .pointer("/image/sha256")
            .and_then(serde_json::Value::as_str)
            .and_then(|sha256| {
                snapshot
                    .pointer("/selected_workflow/project_id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|project_id| self.image_index_by_sha256(project_id, sha256).ok())
            })
            .flatten();
        self.inspect_run_pipeline_artifacts_from_history(&history, image_index)
    }

    pub fn inspect_run_pipeline_artifacts_from_history(
        &self,
        history: &HistoryRun,
        image_index: Option<usize>,
    ) -> Result<RunNodeArtifactInspection> {
        let run_id = history.id;
        let snapshot: serde_json::Value = serde_json::from_str(
            history
                .workflow_snapshot_json
                .as_deref()
                .ok_or_else(|| anyhow!("run {run_id} has no Workflow checkpoint"))?,
        )?;
        let workflow: PublishedWorkflowVersion = serde_json::from_value(
            snapshot
                .get("selected_workflow")
                .cloned()
                .ok_or_else(|| anyhow!("run {run_id} did not select a Published Workflow"))?,
        )?;
        let checkpoint: DagCheckpoint = serde_json::from_value(
            snapshot
                .get("checkpoint")
                .cloned()
                .ok_or_else(|| anyhow!("run {run_id} has no completed node checkpoint"))?,
        )?;
        Ok(node_artifact_inspection(
            run_id,
            &workflow,
            &checkpoint,
            image_index,
        ))
    }

    pub fn project_image_indices_by_sha256(
        &self,
        project_id: &str,
    ) -> Result<BTreeMap<String, usize>> {
        self.list_project_images(project_id)?
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let bytes = std::fs::read(path)
                    .with_context(|| format!("cannot read image {}", path.display()))?;
                Ok((sha256(&bytes), index))
            })
            .collect()
    }

    pub fn inspect_run_annotations(&self, run_id: RunId) -> Result<RunAnnotationInspection> {
        let history = self.store.history(run_id)?;
        let stable_id = history
            .run
            .project_id
            .ok_or_else(|| anyhow!("run {run_id} has no Project identity"))?;
        let project_id = std::fs::read_dir(&self.workspace)?
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .find_map(|entry| {
                let root = entry.path();
                (root.join("project.yaml").is_file() && stable_project_id(&root) == stable_id)
                    .then(|| entry.file_name().to_string_lossy().into_owned())
            })
            .ok_or_else(|| anyhow!("Project for run {run_id} is no longer available"))?;
        let annotation_image_id = history
            .annotations
            .first()
            .map(|annotation| annotation.image_id);
        let image_index = self
            .inspect_run_pipeline_artifacts(run_id)
            .ok()
            .and_then(|inspection| inspection.image_index)
            .or_else(|| {
                history
                    .model_messages
                    .iter()
                    .filter(|entry| {
                        annotation_image_id.is_none() || entry.image_id == annotation_image_id
                    })
                    .find_map(|entry| extract_sha256(&entry.message.content))
                    .and_then(|sha256| {
                        self.image_index_by_sha256(&project_id, sha256)
                            .ok()
                            .flatten()
                    })
            });
        Ok(RunAnnotationInspection {
            run_id,
            project_id,
            image_index,
            annotations: history.annotations,
        })
    }

    /// Resume the frozen `HumanReview` and `Commit` nodes of a Published Workflow without
    /// re-running any model or upstream refiner. The reviewed geometry is patched into the
    /// persisted typed Artifact before the checkpoint is resumed.
    pub async fn resume_published_review(
        &self,
        run_id: RunId,
        annotation: &Annotation,
        settings: &Settings,
    ) -> Result<bool> {
        let history = self
            .store
            .list_runs()?
            .into_iter()
            .find(|run| run.id == run_id)
            .ok_or_else(|| anyhow!("run {run_id} was not found"))?;
        let Some(snapshot_json) = history.workflow_snapshot_json.as_deref() else {
            return Ok(false);
        };
        let mut snapshot: serde_json::Value = serde_json::from_str(snapshot_json)?;
        let Some(workflow_value) = snapshot.get("selected_workflow").cloned() else {
            return Ok(false);
        };
        let workflow: PublishedWorkflowVersion = serde_json::from_value(workflow_value)?;
        let Some(checkpoint_value) = snapshot.get("checkpoint").cloned() else {
            return Ok(false);
        };
        let mut checkpoint: DagCheckpoint = serde_json::from_value(checkpoint_value)?;
        let approved_review_nodes = checkpoint
            .node_statuses
            .iter()
            .filter_map(|(node_id, status)| {
                (*status == DagNodeStatus::AwaitingReview).then_some(node_id.clone())
            })
            .collect::<BTreeSet<_>>();
        if approved_review_nodes.is_empty() {
            return Ok(false);
        }

        let detection_id = annotation
            .attributes
            .get("pipeline_detection_id")
            .and_then(|value| match value {
                AttributeValue::String(value) => Some(value.as_str()),
                _ => None,
            });
        let artifact_ref = annotation
            .attributes
            .get("pipeline_artifact_ref")
            .and_then(|value| match value {
                AttributeValue::String(value) => Some(value.as_str()),
                _ => None,
            });
        if let annotagent_core::AnnotationValue::BoundingBox { rect } = annotation.value {
            for output in checkpoint.node_outputs.values_mut() {
                for artifact in &mut output.pipeline_artifacts {
                    let PipelineArtifact::DetectionSet(set) = artifact else {
                        continue;
                    };
                    if artifact_ref.is_some_and(|reference| reference != set.reference.artifact_id)
                    {
                        continue;
                    }
                    for detection in &mut set.detections {
                        if detection_id.is_none_or(|id| id == detection.detection_id) {
                            detection.bbox = rect;
                            if let Some(confidence) = annotation.confidence {
                                detection.score =
                                    annotagent_core::DetectionScore::relative(confidence)
                                        .map_err(anyhow::Error::msg)?;
                            }
                        }
                    }
                }
            }
        }

        let sha256 = snapshot
            .pointer("/image/sha256")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("run {run_id} has no replayable image identity"))?;
        let image_index = self
            .image_index_by_sha256(&workflow.project_id, sha256)?
            .ok_or_else(|| anyhow!("the source image for run {run_id} is no longer available"))?;
        let images = self.list_project_images(&workflow.project_id)?;
        let image_path = images
            .get(image_index)
            .ok_or_else(|| anyhow!("source image index {image_index} is no longer available"))?;
        let project_path = self.project_path(&workflow.project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let enabled_ids = project
            .project
            .enabled_skill_versions()
            .into_keys()
            .collect::<Vec<_>>();
        let (validators, refiners) =
            workflow_extension_implementations(&self.skills, &enabled_ids)?;
        let runtime = PublishedWorkflowRuntime::new(
            workflow.clone(),
            "mock",
            settings,
            None,
            self.store.clone(),
            validators,
            refiners,
        )?;
        let image = Arc::new(load_image(image_path, 40_000_000).map_err(|error| anyhow!(error))?);
        let model_image = to_model_image("label-pipeline-review-resume", &image, 1280)
            .map_err(|error| anyhow!(error))?;
        let image_id = annotation.image_id;
        let project_root = project_path
            .parent()
            .unwrap_or(&self.workspace)
            .to_path_buf();
        let request = ImageRunRequest {
            run_id,
            project_id: stable_project_id(&project_root),
            project_root,
            project: Arc::new(project),
            image_id,
            image,
            model_image: Some(model_image),
        };
        self.store
            .set_run_status(run_id, RunStatus::Running, Some("human review approved"))
            .await
            .map_err(|error| anyhow!(error))?;
        let result = match runtime
            .resume_review_sandbox(&request, checkpoint, approved_review_nodes)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                self.store
                    .set_run_status(
                        run_id,
                        RunStatus::CompletedWithReview,
                        Some("human review resume failed; retry remains available"),
                    )
                    .await
                    .map_err(|store_error| anyhow!(store_error))?;
                return Err(error);
            }
        };
        if result.status != annotagent_runtime::DagRunStatus::Completed {
            bail!("reviewed Published Workflow did not reach Commit");
        }
        snapshot["checkpoint"] = serde_json::to_value(&result.checkpoint)?;
        self.store
            .update_run_workflow_snapshot(run_id, &serde_json::to_string(&snapshot)?)?;
        for trace in result.checkpoint.traces.iter().rev().take_while(|trace| {
            matches!(
                trace.status,
                DagNodeStatus::Succeeded | DagNodeStatus::Cached
            ) && (trace.node_id == "review" || trace.node_id == "commit")
        }) {
            self.store
                .set_task_run_status(
                    run_id,
                    image_id,
                    &TaskId::from(trace.node_id.as_str()),
                    TaskRunStatus::Succeeded,
                    None,
                )
                .await
                .map_err(|error| anyhow!(error))?;
        }
        Ok(true)
    }

    pub async fn replay_run_from_node(
        &self,
        run_id: RunId,
        node_id: &str,
        settings: &Settings,
    ) -> Result<NodeReplayReport> {
        let history = self
            .store
            .list_runs()?
            .into_iter()
            .find(|run| run.id == run_id)
            .ok_or_else(|| anyhow!("run {run_id} was not found"))?;
        let snapshot: serde_json::Value = serde_json::from_str(
            history
                .workflow_snapshot_json
                .as_deref()
                .ok_or_else(|| anyhow!("run {run_id} has no Workflow checkpoint"))?,
        )?;
        let workflow: PublishedWorkflowVersion = serde_json::from_value(
            snapshot
                .get("selected_workflow")
                .cloned()
                .ok_or_else(|| anyhow!("run {run_id} did not select a Published Workflow"))?,
        )?;
        let mut replayed_node_ids = BTreeSet::from([node_id.to_owned()]);
        loop {
            let descendants = workflow
                .draft
                .edges
                .iter()
                .filter(|edge| replayed_node_ids.contains(&edge.from_node))
                .map(|edge| edge.to_node.clone())
                .collect::<Vec<_>>();
            let before = replayed_node_ids.len();
            replayed_node_ids.extend(descendants);
            if replayed_node_ids.len() == before {
                break;
            }
        }
        let has_live_model_binding = workflow.draft.nodes.iter().any(|node| {
            replayed_node_ids.contains(&node.id)
                && node
                    .model_binding
                    .as_deref()
                    .is_some_and(|model| !model.starts_with("mock-"))
        });
        if history.provider != "mock" && has_live_model_binding {
            bail!(
                "Replay of live model nodes requires an explicit current binding; credentials are never recovered from Run history"
            );
        }
        let checkpoint: DagCheckpoint = serde_json::from_value(
            snapshot
                .get("checkpoint")
                .cloned()
                .ok_or_else(|| anyhow!("run {run_id} has no completed node checkpoint"))?,
        )?;
        let sha256 = snapshot
            .pointer("/image/sha256")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("run {run_id} predates replayable image identity"))?;
        let image_index = self
            .image_index_by_sha256(&workflow.project_id, sha256)?
            .ok_or_else(|| {
                anyhow!("the source image for run {run_id} is no longer in the Project")
            })?;
        let images = self.list_project_images(&workflow.project_id)?;
        let image_path = images
            .get(image_index)
            .ok_or_else(|| anyhow!("source image index {image_index} is no longer available"))?;
        let project_path = self.project_path(&workflow.project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let enabled_ids = project
            .project
            .enabled_skill_versions()
            .into_keys()
            .collect::<Vec<_>>();
        let (validators, refiners) =
            workflow_extension_implementations(&self.skills, &enabled_ids)?;
        let runtime = PublishedWorkflowRuntime::new(
            workflow.clone(),
            "mock",
            settings,
            None,
            self.store.clone(),
            validators,
            refiners,
        )?;
        let image = Arc::new(load_image(image_path, 40_000_000).map_err(|error| anyhow!(error))?);
        let model_image = to_model_image("label-pipeline-replay", &image, 1280)
            .map_err(|error| anyhow!(error))?;
        let image_id = checkpoint
            .node_outputs
            .values()
            .flat_map(|output| output.pipeline_artifacts.iter())
            .find_map(|artifact| match artifact {
                PipelineArtifact::Image(image) => Some(image.image_id),
                _ => None,
            })
            .ok_or_else(|| anyhow!("run {run_id} checkpoint has no Image Artifact"))?;
        let before_trace_count = checkpoint.traces.len();
        let before_outputs = checkpoint.node_outputs.clone();
        let request = ImageRunRequest {
            run_id: RunId::new(),
            project_id: stable_project_id(project_path.parent().unwrap_or(&self.workspace)),
            project_root: project_path
                .parent()
                .unwrap_or(&self.workspace)
                .to_path_buf(),
            project: Arc::new(project),
            image_id,
            image,
            model_image: Some(model_image),
        };
        let result = runtime
            .replay_sandbox(&request, checkpoint, node_id)
            .await?;
        let reexecuted_nodes = result.checkpoint.traces[before_trace_count..]
            .iter()
            .map(|trace| trace.node_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let preserved_upstream_nodes = before_outputs
            .iter()
            .filter(|(id, output)| {
                result.checkpoint.node_outputs.get(*id) == Some(*output)
                    && !reexecuted_nodes.contains(id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        Ok(NodeReplayReport {
            source_run_id: run_id,
            replayed_from: node_id.to_owned(),
            reexecuted_nodes,
            preserved_upstream_nodes,
            inspection: node_artifact_inspection(
                run_id,
                &workflow,
                &result.checkpoint,
                Some(image_index),
            ),
            sandbox: true,
        })
    }

    fn image_index_by_sha256(&self, project_id: &str, sha256: &str) -> Result<Option<usize>> {
        for (index, path) in self.list_project_images(project_id)?.iter().enumerate() {
            let bytes = std::fs::read(path)
                .with_context(|| format!("cannot read image {}", path.display()))?;
            if annotagent_image_tools::sha256(&bytes) == sha256 {
                return Ok(Some(index));
            }
        }
        Ok(None)
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
        self.workflow_advisor_input_for_label(project_id, settings, constraints, None, None)
    }

    pub fn workflow_advisor_input_for_label(
        &self,
        project_id: &str,
        settings: &Settings,
        constraints: WorkflowConstraints,
        target_task_id: Option<&str>,
        target_label: Option<&str>,
    ) -> Result<WorkflowAdvisorInput> {
        let project_path = self.project_path(project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let (nodes, models) = workflow_catalog(settings)?;
        let enabled_skills = project
            .project
            .enabled_skill_versions()
            .into_keys()
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
        let workflow_templates = workflow_templates_for(&self.skills, &enabled_skills)?;
        Ok(WorkflowAdvisorInput {
            project_id: project_id.to_owned(),
            project_schema: project,
            target_task_id: target_task_id.map(TaskId::from),
            target_label: target_label.map(LabelId::from),
            enabled_skills,
            node_catalog: nodes.nodes(),
            model_registry: models.models(),
            validator_ids: extensions.validators.into_iter().collect(),
            refiner_ids: extensions.refiners.into_iter().collect(),
            resource_ids: extensions.resources.into_iter().collect(),
            workflow_templates,
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
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let now = chrono::Utc::now();
        let draft = if let Some(template_id) = template_id {
            let enabled_ids = project
                .project
                .enabled_skill_versions()
                .into_keys()
                .collect::<Vec<_>>();
            let available = workflow_templates_for(&self.skills, &enabled_ids)?;
            let template = available
                .iter()
                .find(|template| template.id == template_id)
                .ok_or_else(|| {
                    anyhow!("workflow template {template_id:?} is not provided by an enabled Skill")
                })?;
            let mut draft =
                template.instantiate(project_id, project.project.enabled_skill_versions(), now);
            let (nodes, _) = workflow_catalog(settings)?;
            apply_project_capability_bindings(&mut draft, &project, &nodes)?;
            draft
        } else if from_template {
            let (nodes, models) = workflow_catalog(settings)?;
            let mut draft = RegistryWorkflowAdvisor
                .suggest_workflow(
                    project_id,
                    &project,
                    &project
                        .project
                        .enabled_skill_versions()
                        .into_keys()
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
                label_pipeline: None,
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
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let (nodes, models) = workflow_catalog(settings)?;
        let suggestion = RegistryWorkflowAdvisor.suggest_workflow(
            project_id,
            &project,
            &project
                .project
                .enabled_skill_versions()
                .into_keys()
                .collect::<Vec<_>>(),
            &nodes,
            &models,
            constraints,
        );
        self.store.save_workflow_draft(&suggestion.draft)?;
        Ok(suggestion)
    }

    /// Runs the offline, deterministic Workflow Advisor through the same observable tool sequence
    /// used by a model-backed policy. It intentionally validates an invalid proposal first so the
    /// session proves revision rather than wrapping one static suggestion.
    pub async fn run_workflow_advisor_agent(
        &self,
        project_id: &str,
        settings: &Settings,
        constraints: &WorkflowConstraints,
        target: Option<(&str, &str)>,
        builder_constraints: PipelineBuilderConstraints,
        cancellation: CancellationToken,
    ) -> Result<WorkflowAdvisorAgentReport> {
        let builder_constraints = pipeline_builder_constraints(constraints, builder_constraints)?;
        let mut session = AgentSession::start(
            AgentKind::PipelineBuilder,
            builder_constraints.agent_budget(),
        )
        .with_builder_constraints(builder_constraints.clone())
        .with_project(project_id);
        self.agent_cancellations
            .lock()
            .map_err(|_| anyhow!("Agent cancellation registry lock poisoned"))?
            .insert(session.id, cancellation.clone());
        self.store.save_agent_session(&session)?;
        let abort = |session: AgentSession| WorkflowAdvisorAgentReport {
            session,
            suggestion: None,
            validation: None,
            dry_run: None,
            approval_required: false,
        };
        let record = |session: &mut AgentSession,
                      name: &str,
                      arguments: serde_json::Value,
                      result: serde_json::Value| {
            if PipelineBuilderToolRegistry.resolve(name).is_err() {
                session.fail(format!("unregistered Pipeline Builder tool {name:?}"));
                return false;
            }
            let result =
                annotagent_core::AgentToolResult::summary(format!("{name} completed"), result);
            let recorded = session
                .record_tool(
                    name,
                    arguments,
                    serde_json::to_value(result).unwrap_or_else(|error| {
                        json!({"display_summary": "tool result serialization failed", "error": error.to_string()})
                    }),
                    true,
                )
                .is_ok();
            if recorded && self.store.save_agent_session(session).is_err() {
                session.fail("could not persist Pipeline Builder progress");
                return false;
            }
            if cancellation.is_cancelled() {
                session.cancel();
                let _ignored = self.store.save_agent_session(session);
                return false;
            }
            recorded
        };

        if cancellation.is_cancelled() {
            session.cancel();
            self.store.save_agent_session(&session)?;
            return Ok(abort(session));
        }
        let input = self.workflow_advisor_input_for_label(
            project_id,
            settings,
            constraints.clone(),
            target.map(|value| value.0),
            target.map(|value| value.1),
        )?;
        if !record(
            &mut session,
            "inspect_project",
            json!({"project_id": project_id}),
            json!({"task_count": input.project_schema.tasks.len(), "target": target}),
        ) {
            return Ok(abort(session));
        }
        if !record(
            &mut session,
            "list_enabled_skills",
            json!({}),
            json!({"enabled_skills": input.enabled_skills}),
        ) {
            return Ok(abort(session));
        }
        if let Some(resource_id) = input
            .resource_ids
            .iter()
            .find(|resource| resource.ends_with("advisor.md"))
        {
            let (skill_id, resource_name, resources) = load_enabled_skill_resource(
                &self.skills,
                &input.enabled_skills,
                resource_id,
                target.map(|value| value.0),
            )?;
            if !record(
                &mut session,
                "load_skill_resource",
                json!({"skill_id": skill_id, "resource_name": resource_name}),
                json!({
                    "resource_id": resource_id,
                    "resources": bounded_skill_resources(&resources),
                }),
            ) {
                return Ok(abort(session));
            }
        }
        if !record(
            &mut session,
            "list_available_capabilities",
            json!({"skills": input.enabled_skills}),
            json!({"nodes": input.node_catalog}),
        ) || !record(
            &mut session,
            "list_available_models",
            json!({}),
            json!({"models": input.model_registry}),
        ) || !record(
            &mut session,
            "list_pipeline_templates",
            json!({"skills": input.enabled_skills}),
            json!({"template_ids": input.workflow_templates.iter().map(|template| template.id.as_str()).collect::<Vec<_>>() }),
        ) {
            return Ok(abort(session));
        }

        let suggestion = if let Some((task_id, label)) = target {
            self.suggest_label_pipeline_preview(project_id, settings, task_id, label, constraints)?
        } else {
            self.suggest_workflow_preview(project_id, settings, constraints)?
        };
        let mut invalid = suggestion.draft.clone();
        if !record(
            &mut session,
            "create_draft_from_template",
            json!({"strategy": "registry_bounded_initial_proposal"}),
            json!({"draft_id": invalid.id, "node_count": invalid.nodes.len()}),
        ) {
            return Ok(abort(session));
        }
        let commit_id = invalid
            .nodes
            .iter()
            .find(|node| node.kind == WorkflowNodeKind::Commit)
            .map(|node| node.id.clone())
            .ok_or_else(|| anyhow!("Pipeline Builder template has no Commit"))?;
        let incoming = invalid
            .edges
            .iter()
            .find(|edge| edge.to_node == commit_id)
            .cloned()
            .ok_or_else(|| anyhow!("Pipeline Builder template has no connection into Commit"))?;
        let removed =
            PipelineDraftTools.disconnect(&mut invalid, &incoming.from_node, &incoming.to_node)?;
        if !record(
            &mut session,
            "disconnect_pipeline_nodes",
            json!({"from_node": incoming.from_node, "to_node": incoming.to_node}),
            json!({"draft_id": invalid.id, "removed_connections": removed.len()}),
        ) {
            return Ok(abort(session));
        }
        let (nodes, models) = workflow_catalog(settings)?;
        let enabled_skills = invalid
            .enabled_skills
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let extensions = self
            .skills
            .validation_catalog_for(&enabled_skills.iter().cloned().collect::<Vec<_>>())?;
        let validate = |draft: &WorkflowDraft| {
            if target.is_some() {
                PipelineGrammarValidator.validate(
                    draft,
                    &nodes,
                    &models,
                    &extensions,
                    &enabled_skills,
                    &builder_constraints,
                )
            } else {
                WorkflowStaticValidator.validate_for_publish(
                    draft,
                    &nodes,
                    &models,
                    &extensions,
                    &enabled_skills,
                    false,
                )
            }
        };
        let invalid_report = validate(&invalid);
        if !record(
            &mut session,
            "validate_pipeline",
            json!({"draft_id": invalid.id}),
            json!({"valid": invalid_report.valid, "issues": invalid_report.issues}),
        ) {
            return Ok(abort(session));
        }
        if invalid_report.valid {
            session.fail("the intentionally invalid Advisor Draft unexpectedly validated");
            return Ok(abort(session));
        }

        let mut revised = suggestion;
        PipelineDraftTools.connect(&mut invalid, incoming.clone())?;
        invalid.status = WorkflowDraftStatus::Suggested;
        revised.draft = invalid;
        if !record(
            &mut session,
            "connect_pipeline_nodes",
            json!({"from_node": incoming.from_node, "to_node": incoming.to_node}),
            json!({"draft_id": revised.draft.id, "restored_connection": true}),
        ) {
            return Ok(abort(session));
        }
        let mut validation = validate(&revised.draft);
        if !record(
            &mut session,
            "validate_pipeline",
            json!({"draft_id": revised.draft.id, "revision": 2}),
            json!({"valid": validation.valid, "issues": validation.issues}),
        ) {
            return Ok(abort(session));
        }
        if !validation.valid {
            session.fail("revised Advisor Draft did not pass static validation");
            return Ok(WorkflowAdvisorAgentReport {
                session,
                suggestion: Some(revised),
                validation: Some(validation),
                dry_run: None,
                approval_required: false,
            });
        }
        self.store.save_workflow_draft(&revised.draft)?;
        let mut dry_run = self
            .dry_run_workflow_samples(&revised.draft.id, settings, &[0])
            .await?;
        let first_observation = agent_dry_run_summary(&dry_run, &revised.draft);
        if !record(
            &mut session,
            "dry_run_pipeline",
            json!({"draft_id": revised.draft.id, "image_limit": 1}),
            json!({"sandbox": dry_run.sandbox, "summary": first_observation}),
        ) || !record(
            &mut session,
            "inspect_dry_run_summary",
            json!({"draft_id": revised.draft.id}),
            json!({
                "summary": first_observation,
                "review_rate": first_observation.review_rate(),
            }),
        ) {
            return Ok(WorkflowAdvisorAgentReport {
                session,
                suggestion: Some(revised),
                validation: Some(validation),
                dry_run: Some(dry_run),
                approval_required: false,
            });
        }
        let target_is_detection = target.is_some_and(|(task_id, _)| {
            input
                .project_schema
                .tasks
                .iter()
                .any(|task| task.id.as_str() == task_id && task.kind == TaskKind::BoundingBox)
        });
        if target_is_detection
            && first_observation.review_rate()
                > builder_constraints.target_review_rate.unwrap_or(0.25)
            && let Some((task_id, label)) = target
            && let Ok(classifier_model_id) = available_model_for_capability(
                &input.model_registry,
                VisionCapability::Classification,
            )
            && add_crop_verification_revision(
                &mut revised,
                &input.project_schema,
                task_id,
                label,
                &classifier_model_id,
                &first_observation,
            )?
        {
            if !record(
                &mut session,
                "add_pipeline_node",
                json!({
                    "draft_id": revised.draft.id,
                    "guided_action": "crop_verification",
                    "evidence": {
                        "review_count": first_observation.review_count,
                        "review_rate": first_observation.review_rate(),
                    }
                }),
                json!({
                    "added": ["core.crop", "classification.classify", "core.attach_result"],
                    "reason": "review_rate_above_target"
                }),
            ) {
                return Ok(abort(session));
            }
            validation = validate(&revised.draft);
            if !record(
                &mut session,
                "validate_pipeline",
                json!({"draft_id": revised.draft.id, "revision": 3}),
                json!({"valid": validation.valid, "issues": validation.issues}),
            ) {
                return Ok(abort(session));
            }
            if !validation.valid {
                session.fail("Crop verification revision did not pass static validation");
                self.store.save_agent_session(&session)?;
                return Ok(WorkflowAdvisorAgentReport {
                    session,
                    suggestion: Some(revised),
                    validation: Some(validation),
                    dry_run: Some(dry_run),
                    approval_required: false,
                });
            }
            if builder_constraints.maximum_dry_runs < 2 {
                session.fail("maximum Pipeline Builder Dry Runs reached before quality target");
                self.store.save_agent_session(&session)?;
                return Ok(WorkflowAdvisorAgentReport {
                    session,
                    suggestion: Some(revised),
                    validation: Some(validation),
                    dry_run: Some(dry_run),
                    approval_required: false,
                });
            }
            self.store.save_workflow_draft(&revised.draft)?;
            dry_run = self
                .dry_run_workflow_samples(&revised.draft.id, settings, &[0])
                .await?;
            let second_observation = agent_dry_run_summary(&dry_run, &revised.draft);
            if !record(
                &mut session,
                "dry_run_pipeline",
                json!({"draft_id": revised.draft.id, "image_limit": 1, "revision": 3}),
                json!({
                    "sandbox": dry_run.sandbox,
                    "summary": second_observation,
                    "previous_review_rate": first_observation.review_rate(),
                    "review_rate": second_observation.review_rate(),
                }),
            ) {
                return Ok(abort(session));
            }
        }
        if revise_draft_after_failed_dry_run(&mut revised, dry_run.summary.failed_count) {
            let _recorded = record(
                &mut session,
                "set_node_parameter",
                json!({"draft_id": revised.draft.id, "cause": "dry_run_metrics"}),
                json!({
                    "retry_policy_hardened": true,
                    "publish_approval_requested": false
                }),
            );
            self.store.save_workflow_draft(&revised.draft)?;
            if session.status == AgentSessionStatus::Running {
                session.wait_for_human("edit_failed_dry_run");
            }
            self.store.save_agent_session(&session)?;
            return Ok(WorkflowAdvisorAgentReport {
                session,
                suggestion: Some(revised),
                validation: Some(validation),
                dry_run: Some(dry_run),
                approval_required: false,
            });
        }
        if cancellation.is_cancelled() {
            session.cancel();
            self.store.save_agent_session(&session)?;
            return Ok(WorkflowAdvisorAgentReport {
                session,
                suggestion: Some(revised),
                validation: Some(validation),
                dry_run: Some(dry_run),
                approval_required: false,
            });
        }
        revised.draft.status = WorkflowDraftStatus::Suggested;
        self.store.save_workflow_draft(&revised.draft)?;
        if session.status == AgentSessionStatus::Running
            && record(
                &mut session,
                "submit_draft_for_human_approval",
                json!({"draft_id": revised.draft.id}),
                json!({"published": false, "requires_human": true}),
            )
        {
            session.wait_for_human("approve_pipeline_draft");
        }
        self.store.save_agent_session(&session)?;
        Ok(WorkflowAdvisorAgentReport {
            approval_required: session.status == AgentSessionStatus::WaitingForHuman,
            session,
            suggestion: Some(revised),
            validation: Some(validation),
            dry_run: Some(dry_run),
        })
    }

    pub fn suggest_label_pipeline(
        &self,
        project_id: &str,
        settings: &Settings,
        target_task_id: &str,
        target_label: &str,
        constraints: &WorkflowConstraints,
    ) -> Result<WorkflowSuggestion> {
        let suggestion = self.suggest_label_pipeline_preview(
            project_id,
            settings,
            target_task_id,
            target_label,
            constraints,
        )?;
        self.store.save_workflow_draft(&suggestion.draft)?;
        Ok(suggestion)
    }

    fn suggest_label_pipeline_preview(
        &self,
        project_id: &str,
        settings: &Settings,
        target_task_id: &str,
        target_label: &str,
        constraints: &WorkflowConstraints,
    ) -> Result<WorkflowSuggestion> {
        let project_path = self.project_path(project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let (nodes, models) = workflow_catalog(settings)?;
        let composition = controlled_label_composition(
            &project,
            target_task_id,
            target_label,
            constraints,
            &models,
        )?;
        let label_report =
            LabelPipelineStaticValidator.validate(&composition, &project, &nodes, &models);
        if !label_report.valid {
            bail!(
                "controlled Label Pipeline template failed registry validation: {}",
                label_report
                    .issues
                    .iter()
                    .map(|issue| format!("{}: {}", issue.path, issue.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        let now = chrono::Utc::now();
        let mut draft = composition.compile_draft(
            project_id,
            format!("{target_label} Label Pipeline"),
            project.project.enabled_skill_versions(),
            now,
        );
        draft.status = WorkflowDraftStatus::Suggested;
        let estimated_model_calls_per_image = draft
            .nodes
            .iter()
            .filter(|node| node.model_binding.is_some())
            .count();
        Ok(WorkflowSuggestion {
            draft,
            rationale: vec![
                format!(
                    "The Draft targets only {target_task_id}.{target_label} and uses registered nodes and Models."
                ),
                "Shared model stages are compiled once and referenced by Label Pipelines rather than duplicated."
                    .to_owned(),
            ],
            estimated_model_calls_per_image,
            estimated_latency_ms: Some(estimated_model_calls_per_image as u64 * 1_200),
            estimated_cost_tier: if estimated_model_calls_per_image <= 1 {
                "low"
            } else if estimated_model_calls_per_image <= 3 {
                "medium"
            } else {
                "high"
            }
            .to_owned(),
            unresolved_model_bindings: Vec::new(),
            warnings: vec![
                "This suggestion is an editable Draft. Dry Run and static validation are required before publish."
                    .to_owned(),
            ],
            alternatives: match project
                .tasks
                .iter()
                .find(|task| task.id.as_str() == target_task_id)
                .map(|task| task.kind)
            {
                Some(TaskKind::BoundingBox) => vec![
                    "Add Core Crop after the shared detector and bind a Classification Skill for crop attributes."
                        .to_owned(),
                ],
                _ => vec![
                    "Bind a generic HTTP JSON classifier when the mock binding is no longer appropriate."
                        .to_owned(),
                ],
            },
        })
    }

    pub async fn suggest_workflow_live(
        &self,
        project_id: &str,
        settings: &Settings,
        temporary_api_key: Option<String>,
        constraints: &WorkflowConstraints,
    ) -> Result<WorkflowSuggestion> {
        self.run_workflow_advisor_live_agent(
            project_id,
            settings,
            temporary_api_key,
            constraints,
            None,
            PipelineBuilderConstraints::default(),
            CancellationToken::new(),
        )
        .await?
        .suggestion
        .ok_or_else(|| anyhow!("Pipeline Builder stopped without an editable Draft"))
    }

    pub async fn suggest_label_pipeline_live(
        &self,
        project_id: &str,
        settings: &Settings,
        temporary_api_key: Option<String>,
        target_task_id: &str,
        target_label: &str,
        constraints: &WorkflowConstraints,
    ) -> Result<WorkflowSuggestion> {
        self.run_workflow_advisor_live_agent(
            project_id,
            settings,
            temporary_api_key,
            constraints,
            Some((target_task_id, target_label)),
            PipelineBuilderConstraints::default(),
            CancellationToken::new(),
        )
        .await?
        .suggestion
        .ok_or_else(|| anyhow!("Pipeline Builder stopped without an editable Draft"))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_workflow_advisor_live_agent(
        &self,
        project_id: &str,
        settings: &Settings,
        temporary_api_key: Option<String>,
        constraints: &WorkflowConstraints,
        target: Option<(&str, &str)>,
        builder_constraints: PipelineBuilderConstraints,
        cancellation: CancellationToken,
    ) -> Result<WorkflowAdvisorAgentReport> {
        let input = self.workflow_advisor_input_for_label(
            project_id,
            settings,
            constraints.clone(),
            target.map(|value| value.0),
            target.map(|value| value.1),
        )?;
        let suggestion = if let Some((task_id, label)) = target {
            self.suggest_label_pipeline_preview(project_id, settings, task_id, label, constraints)?
        } else {
            self.suggest_workflow_preview(project_id, settings, constraints)?
        };
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            settings.provider.clone(),
            temporary_api_key,
        )
        .map_err(|error| anyhow!(error))?;
        self.run_workflow_advisor_with_provider(
            project_id,
            settings,
            constraints,
            target,
            input,
            suggestion,
            &provider,
            builder_constraints,
            cancellation,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_workflow_advisor_with_provider(
        &self,
        project_id: &str,
        settings: &Settings,
        constraints: &WorkflowConstraints,
        target: Option<(&str, &str)>,
        input: WorkflowAdvisorInput,
        safe_suggestion: WorkflowSuggestion,
        provider: &dyn VisionModelProvider,
        builder_constraints: PipelineBuilderConstraints,
        cancellation: CancellationToken,
    ) -> Result<WorkflowAdvisorAgentReport> {
        let builder_constraints = pipeline_builder_constraints(constraints, builder_constraints)?;
        let mut session = AgentSession::start(
            AgentKind::PipelineBuilder,
            builder_constraints.agent_budget(),
        )
        .with_builder_constraints(builder_constraints.clone())
        .with_project(project_id);
        self.agent_cancellations
            .lock()
            .map_err(|_| anyhow!("Agent cancellation registry lock poisoned"))?
            .insert(session.id, cancellation.clone());
        self.store.save_agent_session(&session)?;
        let mut messages = vec![
            ModelMessage {
                role: ModelRole::System,
                content: "You are AnnotAgent's constrained Pipeline Builder. Call exactly one registered tool at a time. Inspect the Project and Registry before creating a Draft. You may modify only the current editable Draft through tools, must validate it, must run a sandbox Dry Run, and may finish only with submit_draft_for_human_approval. Never publish, start a formal Run, emit code, request Shell/Python/package/download/arbitrary URL tools, or reveal hidden reasoning. Use short tool arguments only.".to_owned(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            ModelMessage {
                role: ModelRole::User,
                content: serde_json::to_string(&json!({
                    "project": {
                        "id": project_id,
                        "name": input.project_schema.project.name,
                        "task_count": input.project_schema.tasks.len(),
                        "image_count": input.data_profile.image_count,
                    },
                    "target": {"task_id": input.target_task_id, "label": input.target_label},
                    "workflow_constraints": constraints,
                    "builder_constraints": builder_constraints,
                    "enabled_skill_summaries": input.enabled_skills,
                    "rule": "Inspect details with tools; do not assume Registry identities."
                }))?,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
        ];
        let tools = pipeline_builder_live_tools(&input, &safe_suggestion);
        let (nodes, models) = workflow_catalog(settings)?;
        let enabled_skills = safe_suggestion
            .draft
            .enabled_skills
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let extensions = self
            .skills
            .validation_catalog_for(&enabled_skills.iter().cloned().collect::<Vec<_>>())?;
        let mut current: Option<WorkflowSuggestion> = None;
        let mut validation: Option<WorkflowValidationReport> = None;
        let mut dry_run: Option<WorkflowDryRunReport> = None;
        let mut inspected_dry_run = false;
        let mut inspected_project = false;
        let mut inspected_label = false;
        let mut inspected_skills = false;
        let mut inspected_nodes = false;
        let mut inspected_models = false;
        let required_advisor_resource = input
            .resource_ids
            .iter()
            .find(|resource| resource.ends_with("advisor.md"))
            .cloned();
        let mut loaded_resources = BTreeSet::new();

        while session.status == AgentSessionStatus::Running {
            if cancellation.is_cancelled() {
                session.cancel();
                break;
            }
            if session.usage.steps >= session.budget.max_steps {
                session.fail("maximum Pipeline Builder turns reached");
                break;
            }
            let response = match provider
                .complete(
                    ModelRequest {
                        model: settings.provider.model.clone(),
                        task_id: "pipeline_builder".into(),
                        messages: messages.clone(),
                        images: Vec::new(),
                        tools: tools.clone(),
                        max_output_tokens: settings.provider.max_output_tokens,
                        temperature: 0.0,
                        extra: BTreeMap::new(),
                    },
                    cancellation.clone(),
                )
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    session.fail(format!("Pipeline Builder provider error: {error}"));
                    break;
                }
            };
            session.add_model_usage(
                response.usage.input_tokens.unwrap_or_default(),
                response.usage.output_tokens.unwrap_or_default(),
                rust_decimal::Decimal::ZERO,
            );
            if session.status != AgentSessionStatus::Running {
                break;
            }
            if response.tool_calls.is_empty() {
                session.fail("Pipeline Builder provider returned no registered Tool Call");
                break;
            }
            messages.push(ModelMessage {
                role: ModelRole::Assistant,
                content: response.content.unwrap_or_default(),
                tool_call_id: None,
                tool_calls: response.tool_calls.clone(),
            });

            for call in response.tool_calls {
                if cancellation.is_cancelled() {
                    session.cancel();
                    break;
                }
                let resolved = PipelineBuilderToolRegistry.resolve(&call.name);
                let outcome: Result<annotagent_core::AgentToolResult> = async {
                    match resolved {
                    Ok(PipelineBuilderTool::InspectProject) => {
                        inspected_project = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Inspected Project",
                            json!({
                                "project_id": project_id,
                                "name": input.project_schema.project.name,
                                "task_count": input.project_schema.tasks.len(),
                                "image_count": input.data_profile.image_count,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectLabelSchema) => {
                        inspected_label = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Inspected Label Schema",
                            json!({"tasks": input.project_schema.tasks.iter().map(|task| json!({"id": task.id, "kind": task.kind, "labels": task.labels})).collect::<Vec<_>>() }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectLabel) => {
                        let (task_id, label) = target.ok_or_else(|| {
                            anyhow!("inspect_label requires a target Label session")
                        })?;
                        let task = input
                            .project_schema
                            .tasks
                            .iter()
                            .find(|task| task.id.as_str() == task_id)
                            .ok_or_else(|| anyhow!("target task is no longer in Project Schema"))?;
                        inspected_label = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected Label {label}"),
                            json!({"task_id": task_id, "kind": task.kind, "label": label, "declared": task.labels.iter().any(|candidate| candidate == label)}),
                        ))
                    }
                    Ok(PipelineBuilderTool::SampleImages) => Ok(
                        annotagent_core::AgentToolResult::summary(
                            "Inspected bounded image sample metadata",
                            json!({"image_count": input.data_profile.image_count, "sample_width": input.data_profile.sample_width, "sample_height": input.data_profile.sample_height, "mime_types": input.data_profile.mime_types}),
                        ),
                    ),
                    Ok(PipelineBuilderTool::ListEnabledSkills) => {
                        inspected_skills = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Listed enabled Skills",
                            json!({"skill_ids": input.enabled_skills}),
                        ))
                    }
                    Ok(PipelineBuilderTool::LoadSkillResource) => {
                        let skill_id = required_string_argument(&call.arguments, "skill_id")?;
                        let resource_id =
                            required_string_argument(&call.arguments, "resource_name")?;
                        if !input.enabled_skills.contains(&skill_id) {
                            bail!("Skill {skill_id:?} is not enabled by this Project");
                        }
                        if !input.resource_ids.contains(&resource_id) {
                            bail!("Skill resource {resource_id:?} is not declared for this Project");
                        }
                        let (_, resource_name, resources) = load_enabled_skill_resource(
                            &self.skills,
                            std::slice::from_ref(&skill_id),
                            &resource_id,
                            target.map(|value| value.0),
                        )?;
                        loaded_resources.insert(resource_id.clone());
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Loaded Advisor resource {resource_name}"),
                            json!({
                                "skill_id": skill_id,
                                "resource_id": resource_id,
                                "resources": bounded_skill_resources(&resources),
                            }),
                        ))
                    }
                    Ok(
                        PipelineBuilderTool::ListAvailableCapabilities
                        | PipelineBuilderTool::ListAvailableNodes,
                    ) => {
                        inspected_nodes = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Listed available Pipeline nodes",
                            json!({"nodes": input.node_catalog.iter().map(|node| json!({"id": node.id, "name": node.display_name, "required_capabilities": node.required_capabilities, "accepts": node.accepts, "produces": node.produces})).collect::<Vec<_>>() }),
                        ))
                    }
                    Ok(PipelineBuilderTool::ListAvailableModels) => {
                        inspected_models = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Listed registered Models",
                            json!({"models": input.model_registry.iter().map(|model| json!({"id": model.id, "name": model.display_name, "capabilities": model.capabilities, "status": model.status, "backend": model.backend.kind, "pricing": model.pricing})).collect::<Vec<_>>() }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectModel) => {
                        let model_id = required_string_argument(&call.arguments, "model_id")?;
                        let model = input
                            .model_registry
                            .iter()
                            .find(|model| model.id == model_id)
                            .ok_or_else(|| anyhow!("model {model_id:?} is not registered"))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected Model {model_id}"),
                            json!({"id": model.id, "capabilities": model.capabilities, "status": model.status, "input_contract": model.input_contract, "output_contract": model.output_contract, "pricing": model.pricing}),
                        ))
                    }
                    Ok(PipelineBuilderTool::ListPipelineTemplates) => Ok(
                        annotagent_core::AgentToolResult::summary(
                            "Listed compatible Pipeline templates",
                            json!({"templates": input.workflow_templates.iter().map(|template| json!({"id": template.id, "name": template.name, "description": template.description})).collect::<Vec<_>>(), "safe_default_available": true}),
                        ),
                    ),
                    Ok(PipelineBuilderTool::CreateDraftFromTemplate) => {
                        if !inspected_project
                            || !inspected_label
                            || !inspected_skills
                            || !inspected_nodes
                            || !inspected_models
                        {
                            bail!("inspect Project, target Label, enabled Skills, available nodes, and Models before creating a Draft");
                        }
                        if required_advisor_resource
                            .as_ref()
                            .is_some_and(|resource| !loaded_resources.contains(resource))
                        {
                            bail!("load the enabled Domain Advisor resource before creating a Draft");
                        }
                        let mut created = safe_suggestion.clone();
                        created.draft.id = uuid::Uuid::new_v4().to_string();
                        created.draft.status = WorkflowDraftStatus::Suggested;
                        created.draft.created_at = chrono::Utc::now();
                        created.draft.updated_at = created.draft.created_at;
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&created.draft)?;
                        let result = annotagent_core::AgentToolResult::summary(
                            "Created an editable Draft from a Registry template",
                            json!({"draft_id": created.draft.id, "node_count": created.draft.nodes.len(), "published": false}),
                        );
                        current = Some(created);
                        Ok(result)
                    }
                    Ok(PipelineBuilderTool::DisconnectPipelineNodes) => {
                        let from_node =
                            required_string_argument(&call.arguments, "from_node")?;
                        let to_node = required_string_argument(&call.arguments, "to_node")?;
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before editing connections"))?;
                        let removed = PipelineDraftTools.disconnect(
                            &mut suggestion.draft,
                            &from_node,
                            &to_node,
                        )?;
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Disconnected {from_node} from {to_node}"),
                            json!({"draft_id": suggestion.draft.id, "removed_connections": removed}),
                        ))
                    }
                    Ok(PipelineBuilderTool::ConnectPipelineNodes) => {
                        let edge = WorkflowEdge {
                            from_node: required_string_argument(&call.arguments, "from_node")?,
                            from_port: required_string_argument(&call.arguments, "from_port")?,
                            to_node: required_string_argument(&call.arguments, "to_node")?,
                            to_port: required_string_argument(&call.arguments, "to_port")?,
                            route: call
                                .arguments
                                .get("route")
                                .and_then(serde_json::Value::as_str)
                                .map(ToOwned::to_owned),
                        };
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before editing connections"))?;
                        PipelineDraftTools.connect(&mut suggestion.draft, edge.clone())?;
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Connected {} to {}", edge.from_node, edge.to_node),
                            json!({"draft_id": suggestion.draft.id, "connection": edge}),
                        ))
                    }
                    Ok(PipelineBuilderTool::AddPipelineNode) => {
                        let template =
                            required_string_argument(&call.arguments, "guided_template")?;
                        if template != "crop_verification" {
                            bail!("unsupported Guided node template {template:?}");
                        }
                        let (task_id, label) = target.ok_or_else(|| {
                            anyhow!("Crop verification requires a target Label session")
                        })?;
                        if !inspected_dry_run {
                            bail!("inspect the latest Dry Run before adding Crop verification");
                        }
                        let report = dry_run.as_ref().ok_or_else(|| {
                            anyhow!("run and inspect a Dry Run before adding Crop verification")
                        })?;
                        let draft = current
                            .as_ref()
                            .ok_or_else(|| anyhow!("Dry Run has no current Draft"))?;
                        let observation = agent_dry_run_summary(report, &draft.draft);
                        let classifier_model_id = available_model_for_capability(
                            &input.model_registry,
                            VisionCapability::Classification,
                        )?;
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before revising it"))?;
                        if !add_crop_verification_revision(
                            suggestion,
                            &input.project_schema,
                            task_id,
                            label,
                            &classifier_model_id,
                            &observation,
                        )? {
                            bail!("Crop verification is not applicable or is already present");
                        }
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Added Crop verification from Dry Run evidence",
                            json!({
                                "draft_id": suggestion.draft.id,
                                "guided_action": "crop_verification",
                                "evidence": {
                                    "review_count": observation.review_count,
                                    "review_rate": observation.review_rate(),
                                }
                            }),
                        ))
                    }
                    Ok(
                        tool @ (PipelineBuilderTool::SetNodeParameter
                        | PipelineBuilderTool::SetDecisionPolicy
                        | PipelineBuilderTool::SetLabelMapping),
                    ) => {
                        let node_id = required_string_argument(&call.arguments, "node_id")?;
                        let (parameter, value) = match tool {
                            PipelineBuilderTool::SetDecisionPolicy => (
                                "threshold".to_owned(),
                                call.arguments
                                    .get("threshold")
                                    .cloned()
                                    .ok_or_else(|| anyhow!("threshold is required"))?,
                            ),
                            PipelineBuilderTool::SetLabelMapping => (
                                "class_mapping".to_owned(),
                                call.arguments
                                    .get("class_mapping")
                                    .cloned()
                                    .ok_or_else(|| anyhow!("class_mapping is required"))?,
                            ),
                            _ => (
                                required_string_argument(&call.arguments, "parameter")?,
                                call.arguments
                                    .get("value")
                                    .cloned()
                                    .ok_or_else(|| anyhow!("value is required"))?,
                            ),
                        };
                        if !matches!(
                            parameter.as_str(),
                            "threshold"
                                | "minimum_confidence"
                                | "padding"
                                | "max_detections"
                                | "grounding_assist"
                                | "class_mapping"
                        ) {
                            bail!("Pipeline Builder parameter {parameter:?} is not mutable");
                        }
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before editing it"))?;
                        PipelineDraftTools.set_parameter(
                            &mut suggestion.draft,
                            &node_id,
                            parameter.clone(),
                            value.clone(),
                        )?;
                        sync_label_step_parameter(
                            &mut suggestion.draft,
                            &node_id,
                            &parameter,
                            value,
                        );
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Updated {parameter} on {node_id}"),
                            json!({"draft_id": suggestion.draft.id, "node_id": node_id, "parameter": parameter}),
                        ))
                    }
                    Ok(PipelineBuilderTool::BindModel) => {
                        let node_id = required_string_argument(&call.arguments, "node_id")?;
                        let model_id = required_string_argument(&call.arguments, "model_id")?;
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before binding a Model"))?;
                        PipelineDraftTools.bind_model(
                            &mut suggestion.draft,
                            &node_id,
                            &model_id,
                            &nodes,
                            &models,
                            &enabled_skills,
                        )?;
                        sync_label_step_model(&mut suggestion.draft, &node_id, &model_id);
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Bound {model_id} to {node_id}"),
                            json!({"draft_id": suggestion.draft.id, "node_id": node_id, "model_id": model_id}),
                        ))
                    }
                    Ok(PipelineBuilderTool::ValidatePipeline) => {
                        let suggestion = current
                            .as_ref()
                            .ok_or_else(|| anyhow!("create a Draft before validating it"))?;
                        let report = if target.is_some() {
                            PipelineGrammarValidator.validate(
                                &suggestion.draft,
                                &nodes,
                                &models,
                                &extensions,
                                &enabled_skills,
                                &builder_constraints,
                            )
                        } else {
                            WorkflowStaticValidator.validate_for_publish(
                                &suggestion.draft,
                                &nodes,
                                &models,
                                &extensions,
                                &enabled_skills,
                                false,
                            )
                        };
                        let result = annotagent_core::AgentToolResult::summary(
                            if report.valid {
                                "Draft passed Rust static validation"
                            } else {
                                "Draft has blocking validation issues"
                            },
                            json!({"valid": report.valid, "issues": report.issues, "execution_order": report.execution_order}),
                        );
                        validation = Some(report);
                        Ok(result)
                    }
                    Ok(PipelineBuilderTool::DryRunPipeline) => {
                        let completed_dry_runs = session
                            .steps
                            .iter()
                            .filter(|step| {
                                step.success
                                    && step.tool_name
                                        == PipelineBuilderTool::DryRunPipeline.as_str()
                            })
                            .count();
                        if completed_dry_runs
                            >= builder_constraints.maximum_dry_runs as usize
                        {
                            bail!("maximum Pipeline Builder Dry Runs reached");
                        }
                        if !validation.as_ref().is_some_and(|report| report.valid) {
                            bail!("validate_pipeline must pass before Dry Run");
                        }
                        let suggestion = current
                            .as_ref()
                            .ok_or_else(|| anyhow!("create a Draft before Dry Run"))?;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        let image_indices = bounded_image_indices(&call.arguments)?;
                        let report = self
                            .dry_run_workflow_samples(
                                &suggestion.draft.id,
                                settings,
                                &image_indices,
                            )
                            .await?;
                        let observation = agent_dry_run_summary(&report, &suggestion.draft);
                        let result = annotagent_core::AgentToolResult::summary(
                            "Completed sandbox Dry Run",
                            json!({"sandbox": report.sandbox, "summary": observation, "review_rate": observation.review_rate()}),
                        );
                        dry_run = Some(report);
                        inspected_dry_run = false;
                        Ok(result)
                    }
                    Ok(PipelineBuilderTool::InspectDryRunSummary) => {
                        let report = dry_run
                            .as_ref()
                            .ok_or_else(|| anyhow!("run dry_run_pipeline before inspecting it"))?;
                        let suggestion = current
                            .as_ref()
                            .ok_or_else(|| anyhow!("Dry Run has no current Draft"))?;
                        let observation = agent_dry_run_summary(report, &suggestion.draft);
                        inspected_dry_run = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Inspected Dry Run quality, cost, and latency",
                            json!({"summary": observation, "review_rate": observation.review_rate()}),
                        ))
                    }
                    Ok(
                        tool @ (PipelineBuilderTool::InspectFailedSamples
                        | PipelineBuilderTool::InspectReviewSamples),
                    ) => {
                        let report = dry_run
                            .as_ref()
                            .ok_or_else(|| anyhow!("run dry_run_pipeline before inspecting samples"))?;
                        let limit = bounded_inspection_limit(&call.arguments)?;
                        let samples = report
                            .samples
                            .iter()
                            .filter(|sample| match tool {
                                PipelineBuilderTool::InspectFailedSamples => sample.failed,
                                PipelineBuilderTool::InspectReviewSamples => {
                                    sample.review_count > 0
                                        || sample.outcomes.iter().any(|outcome| {
                                            outcome.status == SampleTestOutcomeStatus::NeedsReview
                                        })
                                }
                                _ => false,
                            })
                            .take(limit)
                            .map(|sample| {
                                json!({
                                    "image_index": sample.image_index,
                                    "image_name": sample.image_name,
                                    "failed": sample.failed,
                                    "empty": sample.empty,
                                    "result_count": sample.result_count,
                                    "review_count": sample.review_count,
                                    "outcomes": sample.outcomes.iter().map(|outcome| json!({
                                        "id": outcome.id,
                                        "label": outcome.label,
                                        "confidence": outcome.confidence,
                                        "status": outcome.status,
                                    })).collect::<Vec<_>>(),
                                })
                            })
                            .collect::<Vec<_>>();
                        inspected_dry_run = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected {} bounded sample(s)", samples.len()),
                            json!({"sample_count": samples.len(), "limit": limit, "samples": samples}),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectNodeArtifacts) => {
                        let report = dry_run.as_ref().ok_or_else(|| {
                            anyhow!("run dry_run_pipeline before inspecting node results")
                        })?;
                        let node_id = required_string_argument(&call.arguments, "node_id")?;
                        let limit = bounded_inspection_limit(&call.arguments)?;
                        let node_results = report
                            .samples
                            .iter()
                            .filter_map(|sample| {
                                sample
                                    .nodes
                                    .iter()
                                    .find(|node| node.node_id == node_id)
                                    .map(|node| {
                                        json!({
                                            "image_index": sample.image_index,
                                            "node_id": node.node_id,
                                            "status": node.status,
                                            "output_types": node.output_types,
                                            "latency_ms": node.latency_ms,
                                            "estimated_cost": node.estimated_cost,
                                            "issues": node.issues,
                                        })
                                    })
                            })
                            .take(limit)
                            .collect::<Vec<_>>();
                        if node_results.is_empty() {
                            bail!("node {node_id:?} has no result in the latest Dry Run");
                        }
                        inspected_dry_run = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected {} bounded result(s) for {node_id}", node_results.len()),
                            json!({"node_id": node_id, "result_count": node_results.len(), "limit": limit, "results": node_results}),
                        ))
                    }
                    Ok(PipelineBuilderTool::SubmitDraftForHumanApproval) => {
                        if !validation.as_ref().is_some_and(|report| report.valid) {
                            bail!("a valid static report is required before human approval");
                        }
                        if dry_run.is_none() {
                            bail!("a sandbox Dry Run is required before human approval");
                        }
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before submission"))?;
                        if let Some(name) = call.arguments.get("name").and_then(|value| value.as_str())
                            && !name.trim().is_empty()
                            && name.len() <= 160
                        {
                            name.clone_into(&mut suggestion.draft.name);
                        }
                        suggestion
                            .rationale
                            .extend(string_array_argument(&call.arguments, "rationale"));
                        suggestion
                            .warnings
                            .extend(string_array_argument(&call.arguments, "warnings"));
                        suggestion
                            .alternatives
                            .extend(string_array_argument(&call.arguments, "alternatives"));
                        suggestion.rationale.dedup();
                        suggestion.warnings.dedup();
                        suggestion.alternatives.dedup();
                        suggestion.draft.status = WorkflowDraftStatus::Suggested;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Draft is ready for explicit human approval",
                            json!({"draft_id": suggestion.draft.id, "published": false, "formal_run_started": false, "requires_human": true}),
                        ))
                    }
                    Ok(other) => Err(anyhow!(
                        "registered tool {:?} is unavailable in the current bounded live context",
                        other.as_str()
                    )),
                        Err(error) => Err(anyhow!(error)),
                    }
                }
                .await;

                let (result, success) = match outcome {
                    Ok(result) => (result, true),
                    Err(error) => (
                        annotagent_core::AgentToolResult::summary(
                            format!("{} failed", call.name),
                            json!({"error": error.to_string(), "retryable": true}),
                        ),
                        false,
                    ),
                };
                let model_payload = result.model_payload.clone();
                if session
                    .record_tool(
                        &call.name,
                        call.arguments.clone(),
                        serde_json::to_value(&result)?,
                        success,
                    )
                    .is_err()
                {
                    break;
                }
                self.store.save_agent_session(&session)?;
                messages.push(ModelMessage {
                    role: ModelRole::Tool,
                    content: serde_json::to_string(&model_payload)?,
                    tool_call_id: Some(call.id),
                    tool_calls: Vec::new(),
                });
                if success && call.name == PipelineBuilderTool::SubmitDraftForHumanApproval.as_str()
                {
                    session.wait_for_human("approve_pipeline_draft");
                    break;
                }
            }
        }
        if session.status == AgentSessionStatus::Running {
            session.fail("Pipeline Builder stopped without requesting human approval");
        }
        self.store.save_agent_session(&session)?;
        self.agent_cancellations
            .lock()
            .map_err(|_| anyhow!("Agent cancellation registry lock poisoned"))?
            .remove(&session.id);
        Ok(WorkflowAdvisorAgentReport {
            approval_required: session.status == AgentSessionStatus::WaitingForHuman,
            session,
            suggestion: current,
            validation,
            dry_run,
        })
    }

    fn suggest_workflow_preview(
        &self,
        project_id: &str,
        settings: &Settings,
        constraints: &WorkflowConstraints,
    ) -> Result<WorkflowSuggestion> {
        let project_path = self.project_path(project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let (nodes, models) = workflow_catalog(settings)?;
        Ok(RegistryWorkflowAdvisor.suggest_workflow(
            project_id,
            &project,
            &project
                .project
                .enabled_skill_versions()
                .into_keys()
                .collect::<Vec<_>>(),
            &nodes,
            &models,
            constraints,
        ))
    }

    pub fn save_workflow_draft(&self, mut draft: WorkflowDraft) -> Result<WorkflowDraft> {
        let project_path = self.project_path(&draft.project_id)?;
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
        if draft.label_pipeline.is_some() {
            let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
            draft = compile_label_projection(draft, &project);
        }
        self.store.save_workflow_draft(&draft)?;
        Ok(draft)
    }

    pub fn diff_workflow_drafts(
        &self,
        base_draft_id: &str,
        proposed_draft_id: &str,
    ) -> Result<PipelineDraftDiff> {
        let base = self.store.get_workflow_draft(base_draft_id)?;
        let proposed = self.store.get_workflow_draft(proposed_draft_id)?;
        self.project_path(&base.project_id)?;
        PipelineDraftDiff::between(&base, &proposed).map_err(|error| anyhow!(error))
    }

    pub fn apply_workflow_draft_diff(
        &self,
        base_draft_id: &str,
        proposed_draft_id: &str,
        selected_change_ids: &[String],
    ) -> Result<WorkflowDraftApplyReport> {
        let previous_draft = self.store.get_workflow_draft(base_draft_id)?;
        let proposed = self.store.get_workflow_draft(proposed_draft_id)?;
        self.project_path(&previous_draft.project_id)?;
        let diff = PipelineDraftDiff::between(&previous_draft, &proposed)
            .map_err(|error| anyhow!(error))?;
        let selected = selected_change_ids.iter().cloned().collect::<BTreeSet<_>>();
        let applied = diff
            .apply_selected(&previous_draft, &proposed, &selected)
            .map_err(|error| anyhow!(error))?;
        let draft = self.save_workflow_draft(applied)?;
        Ok(WorkflowDraftApplyReport {
            draft,
            previous_draft,
            diff,
            selected_change_ids: selected.into_iter().collect(),
        })
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

    fn validate_workflow_draft(
        &self,
        draft: &WorkflowDraft,
        settings: &Settings,
        require_publish_ready: bool,
    ) -> Result<WorkflowValidationReport> {
        let project_path = self.project_path(&draft.project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let (nodes, models) = workflow_catalog(settings)?;
        let enabled_skills = draft
            .enabled_skills
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let enabled_skill_ids = enabled_skills.iter().cloned().collect::<Vec<_>>();
        let validation_catalog = self.skills.validation_catalog_for(&enabled_skill_ids)?;
        let mut report = WorkflowStaticValidator.validate_for_publish(
            draft,
            &nodes,
            &models,
            &validation_catalog,
            &enabled_skills,
            require_publish_ready,
        );
        if draft.label_pipeline.is_some() {
            let grammar = PipelineGrammarValidator.validate(
                draft,
                &nodes,
                &models,
                &validation_catalog,
                &enabled_skills,
                &PipelineBuilderConstraints {
                    allow_external_models: true,
                    ..PipelineBuilderConstraints::default()
                },
            );
            let existing = report
                .issues
                .iter()
                .map(|issue| (issue.code.clone(), issue.path.clone()))
                .collect::<BTreeSet<_>>();
            report.issues.extend(
                grammar
                    .issues
                    .into_iter()
                    .filter(|issue| issue.code.starts_with("builder_"))
                    .filter(|issue| !existing.contains(&(issue.code.clone(), issue.path.clone()))),
            );
        }
        report
            .issues
            .extend(label_projection_issues(draft, &project, &nodes, &models));
        report.valid = report.issues.iter().all(|issue| !issue.blocking);
        Ok(report)
    }

    pub fn dry_run_workflow(
        &self,
        draft_id: &str,
        settings: &Settings,
    ) -> Result<WorkflowValidationReport> {
        let draft = self.store.get_workflow_draft(draft_id)?;
        let report = self.validate_workflow_draft(&draft, settings, false)?;
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
        self.dry_run_workflow_samples_with_api_key(draft_id, settings, image_indices, None)
            .await
    }

    pub async fn dry_run_workflow_samples_with_api_key(
        &self,
        draft_id: &str,
        settings: &Settings,
        image_indices: &[usize],
        temporary_api_key: Option<String>,
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
        if draft.label_pipeline.is_some() && validation.valid {
            let project_id = draft.project_id.clone();
            let report = self
                .dry_run_label_pipeline_samples(
                    draft,
                    settings,
                    &images,
                    &selected,
                    started,
                    temporary_api_key,
                )
                .await?;
            self.store.save_workflow_sample_test(&WorkflowSampleTest {
                draft_id: draft_id.to_owned(),
                project_id,
                report: report.clone(),
                completed_at: chrono::Utc::now(),
            })?;
            return Ok(report);
        }
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
                    image_index: index,
                    image_name: path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("image")
                        .to_owned(),
                    width: image.metadata.width,
                    height: image.metadata.height,
                    result_count: 0,
                    auto_accepted_count: 0,
                    review_count: 0,
                    failed: node_results.iter().any(|node| !node.issues.is_empty()),
                    empty: node_results.iter().all(|node| node.issues.is_empty()),
                    outcomes: Vec::new(),
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
        let total_latency_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        let mut summary = SampleTestSummary {
            image_count: samples.len(),
            failed_count: samples.iter().filter(|sample| sample.failed).count(),
            empty_count: samples.iter().filter(|sample| sample.empty).count(),
            ..SampleTestSummary::default()
        };
        finish_sample_test_summary(&mut summary, images.len(), total_latency_ms, "0");
        let report = WorkflowDryRunReport {
            sandbox: true,
            validation,
            summary,
            samples,
            total_latency_ms,
            estimated_cost: "0".to_owned(),
        };
        self.store.save_workflow_sample_test(&WorkflowSampleTest {
            draft_id: draft_id.to_owned(),
            project_id: draft.project_id,
            report: report.clone(),
            completed_at: chrono::Utc::now(),
        })?;
        Ok(report)
    }

    async fn dry_run_label_pipeline_samples(
        &self,
        draft: WorkflowDraft,
        settings: &Settings,
        images: &[PathBuf],
        selected: &[usize],
        started: std::time::Instant,
        temporary_api_key: Option<String>,
    ) -> Result<WorkflowDryRunReport> {
        let project_path = self.project_path(&draft.project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let (_, models) = workflow_catalog(settings)?;
        let snapshot =
            WorkflowSnapshot::frozen(&draft, &models, project.project.enabled_skill_versions());
        let content_hash = annotagent_image_tools::sha256(&snapshot.content_hash_material()?);
        let published = PublishedWorkflowVersion {
            workflow_id: format!("dry-run:{}", draft.id),
            version: 0,
            project_id: draft.project_id.clone(),
            source_draft_id: draft.id.clone(),
            content_hash,
            draft,
            snapshot,
            published_at: chrono::Utc::now(),
        };
        let execution_order = published
            .draft
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        let enabled_ids = project
            .project
            .enabled_skill_versions()
            .into_keys()
            .collect::<Vec<_>>();
        let (validators, refiners) =
            workflow_extension_implementations(&self.skills, &enabled_ids)?;
        let runtime = PublishedWorkflowRuntime::new(
            published,
            &settings.default_provider,
            settings,
            temporary_api_key,
            self.store.clone(),
            validators,
            refiners,
        )?;
        let project = Arc::new(project);
        let project_root = project_path
            .parent()
            .unwrap_or(&self.workspace)
            .to_path_buf();
        let mut samples = Vec::new();
        let mut execution_issues = Vec::new();
        let mut summary = SampleTestSummary::default();
        let mut total_cost = rust_decimal::Decimal::ZERO;
        for index in selected {
            let path = images
                .get(*index)
                .ok_or_else(|| anyhow!("image index {index} was not found"))?;
            let image = Arc::new(load_image(path, 40_000_000).map_err(|error| anyhow!(error))?);
            let model_image = to_model_image("label-pipeline-dry-run", &image, 1280)
                .map_err(|error| anyhow!(error))?;
            let request = ImageRunRequest {
                run_id: RunId::new(),
                project_id: stable_project_id(&project_root),
                project_root: project_root.clone(),
                project: project.clone(),
                image_id: ImageId::new(),
                image: image.clone(),
                model_image: Some(model_image),
            };
            let result = runtime.execute_sandbox(&request).await?;
            summary.image_count += 1;
            summary.fallback_count += result.checkpoint.activated_fallbacks.len();
            summary.cache_hit_count += result
                .checkpoint
                .traces
                .iter()
                .filter(|trace| trace.cache_hit)
                .count();
            let mut sample_detections = 0;
            let mut sample_candidates = 0;
            let mut sample_failed = false;
            let mut detection_outcomes = BTreeMap::new();
            let mut classification_outcomes = BTreeMap::new();
            let mut candidate_outcomes = BTreeMap::new();
            for trace in &result.checkpoint.traces {
                summary.input_tokens = summary
                    .input_tokens
                    .saturating_add(trace.usage.input_tokens);
                summary.output_tokens = summary
                    .output_tokens
                    .saturating_add(trace.usage.output_tokens);
                total_cost += trace.usage.cost;
                sample_failed |= trace.error.is_some();
                for artifact in &trace.output_pipeline_artifacts {
                    match artifact {
                        PipelineArtifact::DetectionSet(set) => {
                            sample_detections = sample_detections.max(set.detections.len());
                            for detection in &set.detections {
                                detection_outcomes.insert(
                                    detection.detection_id.clone(),
                                    SampleTestOutcome {
                                        id: detection.detection_id.clone(),
                                        label: detection.project_label.as_ref().map_or_else(
                                            || {
                                                detection
                                                    .model_label
                                                    .clone()
                                                    .unwrap_or_else(|| "unlabeled".to_owned())
                                            },
                                            ToString::to_string,
                                        ),
                                        confidence: detection.score.comparable_confidence(),
                                        status: sample_test_outcome_status(Some(
                                            set.validation_state,
                                        )),
                                        value: Some(VisionArtifactValue::BoundingBox {
                                            rect: detection.bbox,
                                        }),
                                    },
                                );
                            }
                        }
                        PipelineArtifact::AnnotationCandidateSet(set) => {
                            sample_candidates = sample_candidates.max(set.candidates.len());
                            for candidate in &set.candidates {
                                candidate_outcomes.insert(
                                    candidate.id.clone(),
                                    SampleTestOutcome {
                                        id: candidate.id.clone(),
                                        label: candidate.label.to_string(),
                                        confidence: candidate.confidence,
                                        status: sample_test_outcome_status(
                                            candidate.validation_state,
                                        ),
                                        value: candidate.value.clone(),
                                    },
                                );
                            }
                        }
                        PipelineArtifact::ClassificationSet(set) => {
                            for classification in &set.classifications {
                                classification_outcomes.insert(
                                    classification.id.clone(),
                                    SampleTestOutcome {
                                        id: classification.id.clone(),
                                        label: classification.label.to_string(),
                                        confidence: Some(classification.confidence),
                                        status: sample_test_outcome_status(Some(
                                            set.validation_state,
                                        )),
                                        value: Some(VisionArtifactValue::Classification {
                                            labels: vec![classification.label.clone()],
                                        }),
                                    },
                                );
                            }
                        }
                        PipelineArtifact::CandidateClusterSet(set) => {
                            sample_candidates = sample_candidates.max(set.candidates.len());
                            for candidate in &set.candidates {
                                let source_models = candidate
                                    .members
                                    .iter()
                                    .map(|member| &member.source_model_id)
                                    .collect::<BTreeSet<_>>();
                                candidate_outcomes.insert(
                                    candidate.id.clone(),
                                    SampleTestOutcome {
                                        id: candidate.id.clone(),
                                        label: candidate.target_label.to_string(),
                                        confidence: (source_models.len() == 1)
                                            .then(|| {
                                                candidate.members.first().and_then(|member| {
                                                    member.score.comparable_confidence()
                                                })
                                            })
                                            .flatten(),
                                        status: sample_test_outcome_status(Some(
                                            set.validation_state,
                                        )),
                                        value: Some(VisionArtifactValue::BoundingBox {
                                            rect: candidate.representative_bbox,
                                        }),
                                    },
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            summary.detection_count += sample_detections;
            summary.candidate_count += sample_candidates;
            summary.failed_count += usize::from(sample_failed);
            let outcomes = if candidate_outcomes.is_empty() {
                if classification_outcomes.is_empty() {
                    detection_outcomes.into_values().collect::<Vec<_>>()
                } else {
                    classification_outcomes.into_values().collect::<Vec<_>>()
                }
            } else {
                candidate_outcomes.into_values().collect::<Vec<_>>()
            };
            let sample_auto_accepted = outcomes
                .iter()
                .filter(|outcome| outcome.status == SampleTestOutcomeStatus::ReadyToAccept)
                .count();
            let sample_review = outcomes
                .iter()
                .filter(|outcome| outcome.status == SampleTestOutcomeStatus::NeedsReview)
                .count();
            let sample_empty = outcomes.is_empty() && !sample_failed;
            summary.auto_accepted_count += sample_auto_accepted;
            summary.needs_review_count += sample_review;
            summary.empty_count += usize::from(sample_empty);
            let nodes = result
                .checkpoint
                .traces
                .iter()
                .map(|trace| {
                    let issues = trace
                        .error
                        .iter()
                        .map(|error| WorkflowValidationIssue {
                            code: error.code.clone(),
                            path: format!("nodes.{}", trace.node_id),
                            message: error.summary.clone(),
                            blocking: true,
                        })
                        .collect::<Vec<_>>();
                    execution_issues.extend(issues.clone());
                    WorkflowDryRunNodeResult {
                        node_id: trace.node_id.clone(),
                        status: format!("{:?}", trace.status).to_ascii_lowercase(),
                        output_types: trace
                            .output_pipeline_artifacts
                            .iter()
                            .map(PipelineArtifact::artifact_type)
                            .collect(),
                        latency_ms: (trace.finished_at - trace.started_at)
                            .num_milliseconds()
                            .max(0)
                            .try_into()
                            .unwrap_or(u64::MAX),
                        estimated_cost: trace.usage.cost.to_string(),
                        issues,
                    }
                })
                .collect();
            samples.push(WorkflowDryRunSampleResult {
                image_index: *index,
                image_name: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("image")
                    .to_owned(),
                width: image.metadata.width,
                height: image.metadata.height,
                result_count: outcomes.len(),
                auto_accepted_count: sample_auto_accepted,
                review_count: sample_review,
                failed: sample_failed,
                empty: sample_empty,
                outcomes,
                nodes,
            });
        }
        let total_latency_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        let total_cost = total_cost.to_string();
        finish_sample_test_summary(&mut summary, images.len(), total_latency_ms, &total_cost);
        Ok(WorkflowDryRunReport {
            sandbox: true,
            validation: WorkflowValidationReport {
                valid: execution_issues.is_empty(),
                issues: execution_issues,
                execution_order,
            },
            summary,
            samples,
            total_latency_ms,
            estimated_cost: total_cost,
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
        let publish_report = self.validate_workflow_draft(&draft, settings, true)?;
        if !publish_report.valid {
            bail!("workflow has unresolved bindings and cannot be published");
        }
        let (_, models) = workflow_catalog(settings)?;
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
        let report = self.import_images_with_report(project_id, source)?;
        Ok((report.imported, report.duplicates))
    }

    pub fn import_images_with_report(
        &self,
        project_id: &str,
        source: &Path,
    ) -> Result<ImageImportReport> {
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
        let candidates = if canonical_source.is_file() {
            vec![canonical_source.clone()]
        } else {
            WalkDir::new(&canonical_source)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file() && !entry.file_type().is_symlink())
                .map(walkdir::DirEntry::into_path)
                .collect::<Vec<_>>()
        };
        let mut report = ImageImportReport {
            source: canonical_source.to_string_lossy().into_owned(),
            discovered: 0,
            imported: 0,
            duplicates: 0,
            corrupt: Vec::new(),
            unsupported_files: 0,
            supported_formats: vec!["PNG".to_owned(), "JPEG".to_owned()],
        };
        for source in candidates {
            if !is_supported_image(&source) {
                report.unsupported_files += 1;
                continue;
            }
            report.discovered += 1;
            let bytes = match std::fs::read(&source) {
                Ok(bytes) => bytes,
                Err(error) => {
                    report.corrupt.push(ImageImportIssue {
                        name: source
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        message: format!("cannot read image: {error}"),
                    });
                    continue;
                }
            };
            if let Err(error) = annotagent_image_tools::load_image(&source, 100_000_000) {
                report.corrupt.push(ImageImportIssue {
                    name: source
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    message: error.to_string(),
                });
                continue;
            }
            if !hashes.insert(annotagent_image_tools::sha256(&bytes)) {
                report.duplicates += 1;
                continue;
            }
            let name = source.file_name().context("image has no file name")?;
            let target = unique_target(&destination, name);
            std::fs::copy(source, target)?;
            report.imported += 1;
        }
        Ok(report)
    }

    pub fn remove_project_image(&self, project_id: &str, index: usize) -> Result<String> {
        let path = self
            .list_project_images(project_id)?
            .get(index)
            .cloned()
            .context("image index was not found")?;
        let project_path = self.project_path(project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let dataset = project_path
            .parent()
            .unwrap_or(&self.workspace)
            .join(project.dataset.root)
            .canonicalize()
            .context("cannot access Project dataset")?;
        let canonical = path.canonicalize().context("cannot access Project image")?;
        ensure_within(&dataset, &canonical)?;
        let name = canonical
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        std::fs::remove_file(canonical)?;
        Ok(name)
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
    let mut settings: Settings =
        toml::from_str(&contents).context("invalid provider/pricing/budget config")?;
    for default_worker in default_detection_workers() {
        if !settings.detection_workers.iter().any(|worker| {
            worker.id == default_worker.id || worker.model_id == default_worker.model_id
        }) {
            settings.detection_workers.push(default_worker);
        }
    }
    Ok(settings)
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

fn canonical_export_format(format: &str) -> String {
    match format.trim().to_ascii_lowercase().as_str() {
        "yolo" => "yolo_detection".to_owned(),
        canonical => canonical.to_owned(),
    }
}

fn dataset_exporter(format: &str) -> Result<Box<dyn DatasetExporter>> {
    match canonical_export_format(format).as_str() {
        "native" => Ok(Box::new(NativeExporter)),
        "coco" => Ok(Box::new(CocoExporter)),
        "yolo_detection" => Ok(Box::new(YoloDetectionExporter)),
        "yolo_segmentation" => Ok(Box::new(YoloSegmentationExporter)),
        "labelme" => Ok(Box::new(LabelMeExporter)),
        other => bail!("unknown export format {other:?}"),
    }
}

fn export_format_display_name(format: &str) -> &'static str {
    match format {
        "native" => "AnnotAgent Native",
        "coco" => "COCO",
        "yolo_detection" => "YOLO Detection",
        "yolo_segmentation" => "YOLO Segmentation",
        "labelme" => "LabelMe",
        _ => "Dataset",
    }
}

fn export_format_summary(format: &str, supported: bool) -> String {
    if !supported {
        return "This format cannot represent every task in the current Project Schema.".to_owned();
    }
    match format {
        "native" => "Preserves the full schema, provenance, review state, and revision history.",
        "coco" => "A standardized computer-vision interchange format for compatible geometry.",
        "yolo_detection" => "Training-ready labels for bounding-box detection models.",
        "yolo_segmentation" => {
            "Training-ready labels for polygon and instance segmentation models."
        }
        "labelme" => "Editable per-image annotations for compatible geometry.",
        _ => "A configured Project Schema export format.",
    }
    .to_owned()
}

fn recommended_export_format(
    snapshot: &ProjectSnapshot,
    formats: &[ExportFormatCompatibility],
) -> Option<String> {
    let task_kinds = snapshot
        .schema
        .tasks
        .iter()
        .map(|task| task.kind)
        .collect::<Vec<_>>();
    let all_bbox =
        !task_kinds.is_empty() && task_kinds.iter().all(|kind| *kind == TaskKind::BoundingBox);
    let all_segmentation = !task_kinds.is_empty()
        && task_kinds
            .iter()
            .all(|kind| matches!(kind, TaskKind::Polygon | TaskKind::InstanceMask));
    let preference = if all_bbox {
        [
            "yolo_detection",
            "coco",
            "labelme",
            "native",
            "yolo_segmentation",
        ]
    } else if all_segmentation {
        [
            "yolo_segmentation",
            "coco",
            "labelme",
            "native",
            "yolo_detection",
        ]
    } else {
        [
            "native",
            "coco",
            "labelme",
            "yolo_detection",
            "yolo_segmentation",
        ]
    };
    preference.into_iter().find_map(|preferred| {
        formats
            .iter()
            .find(|format| format.format == preferred && format.supported)
            .map(|format| format.format.clone())
    })
}

fn latest_project_export(
    output_root: &Path,
    formats: &[ExportFormatCompatibility],
    source_fingerprint: &str,
) -> Option<ProjectExportResult> {
    formats
        .iter()
        .filter_map(|format| {
            let report_path = output_root.join(&format.format).join("export-report.json");
            let contents = std::fs::read_to_string(report_path).ok()?;
            serde_json::from_str::<ProjectExportResult>(&contents)
                .ok()
                .filter(|result| result.source_fingerprint == source_fingerprint)
        })
        .max_by(|left, right| left.completed_at.cmp(&right.completed_at))
}

fn export_readiness_from_data(
    project_id: &str,
    data: &ProjectExportData,
) -> Result<ExportReadiness> {
    let configured = if data.snapshot.schema.export.formats.is_empty() {
        vec!["native".to_owned()]
    } else {
        data.snapshot.schema.export.formats.clone()
    };
    let mut seen = BTreeSet::new();
    let mut formats = Vec::new();
    for configured_format in configured {
        let format = canonical_export_format(&configured_format);
        if !seen.insert(format.clone()) {
            continue;
        }
        let (supported, warnings, unsupported_task_kinds) = match dataset_exporter(&format) {
            Ok(exporter) => {
                let compatibility = exporter.compatibility(&data.snapshot);
                (
                    compatibility.supported,
                    compatibility.warnings,
                    compatibility.unsupported_task_kinds,
                )
            }
            Err(_) => (
                false,
                vec![format!(
                    "No exporter is registered for {configured_format:?}."
                )],
                Vec::new(),
            ),
        };
        formats.push(ExportFormatCompatibility {
            display_name: export_format_display_name(&format).to_owned(),
            summary: export_format_summary(&format, supported),
            format,
            supported,
            recommended: false,
            warnings,
            unsupported_task_kinds,
        });
    }
    let recommended_format = recommended_export_format(&data.snapshot, &formats);
    if let Some(recommended) = recommended_format.as_deref()
        && let Some(format) = formats
            .iter_mut()
            .find(|format| format.format == recommended)
    {
        format.recommended = true;
    }

    let mut blocking_issues = Vec::new();
    if data.image_count == 0 {
        blocking_issues.push(ExportBlocker {
            code: "images_missing".to_owned(),
            title: "Add images before exporting".to_owned(),
            explanation: "The dataset has no images to include in an export.".to_owned(),
            repair_destination: format!("/projects/{project_id}/build/data"),
        });
    } else if data.processed_image_count < data.image_count {
        blocking_issues.push(ExportBlocker {
            code: "images_not_processed".to_owned(),
            title: "Run the remaining images".to_owned(),
            explanation: format!(
                "{} of {} images have a completed annotation run.",
                data.processed_image_count, data.image_count
            ),
            repair_destination: format!("/projects/{project_id}"),
        });
    }
    if data.unresolved_reviews > 0 {
        blocking_issues.push(ExportBlocker {
            code: "reviews_unresolved".to_owned(),
            title: "Resolve pending reviews".to_owned(),
            explanation: format!(
                "{} annotation{} still {} a human decision.",
                data.unresolved_reviews,
                if data.unresolved_reviews == 1 {
                    ""
                } else {
                    "s"
                },
                if data.unresolved_reviews == 1 {
                    "requires"
                } else {
                    "require"
                }
            ),
            repair_destination: format!("/review?project_id={project_id}"),
        });
    }
    if recommended_format.is_none() {
        blocking_issues.push(ExportBlocker {
            code: "no_compatible_format".to_owned(),
            title: "Choose a compatible export format".to_owned(),
            explanation:
                "None of the formats enabled by the Project Schema can represent every task."
                    .to_owned(),
            repair_destination: format!("/projects/{project_id}/build/labels"),
        });
    }
    let output_root = data.project_root.join("exports");
    let source_fingerprint = sha256(&serde_json::to_vec(&data.snapshot)?);
    let last_export = blocking_issues
        .is_empty()
        .then(|| latest_project_export(&output_root, &formats, &source_fingerprint))
        .flatten();
    Ok(ExportReadiness {
        project_id: project_id.to_owned(),
        ready: blocking_issues.is_empty(),
        image_count: data.image_count as u64,
        processed_image_count: data.processed_image_count as u64,
        accepted_annotations: data.snapshot.annotations.len() as u64,
        unresolved_reviews: data.unresolved_reviews as u64,
        blocking_issues,
        recommended_format,
        formats,
        output_root,
        last_export,
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
        let enabled_ids = project
            .project
            .enabled_skill_versions()
            .into_keys()
            .collect::<Vec<_>>();
        let (validators, refiners) = workflow_extension_implementations(skills, &enabled_ids)?;
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
    let enabled_versions = project.project.enabled_skill_versions();
    if !enabled_ids.is_empty() && enabled_ids.iter().all(|id| skills.get(id).is_err()) {
        skills.resolve_layered_enabled(&enabled_versions)?;
    }
    let mut project_skills = Vec::new();
    for id in &enabled_ids {
        if let Ok(skill) = skills.get(id) {
            project_skills.push(skill);
        } else {
            skills.get_layered(id)?;
        }
    }
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

fn workflow_templates_for(
    skills: &SkillRegistry,
    enabled_ids: &[String],
) -> Result<Vec<annotagent_core::WorkflowTemplate>> {
    let mut templates = Vec::new();
    for id in enabled_ids {
        if let Ok(skill) = skills.get(id) {
            templates.extend(skill.workflow_templates());
        } else {
            templates.extend(skills.get_layered(id)?.workflow_templates());
        }
    }
    Ok(templates)
}

type WorkflowValidators = BTreeMap<String, Arc<dyn annotagent_core::AnnotationValidator>>;
type WorkflowRefiners = BTreeMap<String, Arc<dyn annotagent_core::AnnotationRefiner>>;

fn workflow_extension_implementations(
    skills: &SkillRegistry,
    enabled_ids: &[String],
) -> Result<(WorkflowValidators, WorkflowRefiners)> {
    let use_namespace = enabled_ids.len() > 1;
    let mut validators = BTreeMap::new();
    let mut refiners = BTreeMap::new();
    for skill_id in enabled_ids {
        if let Ok(skill) = skills.get(skill_id) {
            for validator in skill.validators() {
                let id = if use_namespace {
                    format!("{skill_id}.{}", validator.id())
                } else {
                    validator.id().to_owned()
                };
                validators.insert(id, validator);
            }
            for refiner in skill.refiners() {
                let id = if use_namespace {
                    format!("{skill_id}.{}", refiner.id())
                } else {
                    refiner.id().to_owned()
                };
                refiners.insert(id, refiner);
            }
        } else {
            let skill = skills.get_layered(skill_id)?;
            for validator in skill.validators() {
                let id = if use_namespace {
                    format!("{skill_id}.{}", validator.id())
                } else {
                    validator.id().to_owned()
                };
                validators.insert(id, validator);
            }
            for refiner in skill.refiners() {
                let id = if use_namespace {
                    format!("{skill_id}.{}", refiner.id())
                } else {
                    refiner.id().to_owned()
                };
                refiners.insert(id, refiner);
            }
        }
    }
    Ok((validators, refiners))
}

fn apply_project_capability_bindings(
    draft: &mut WorkflowDraft,
    project: &ProjectSchema,
    nodes: &NodeRegistry,
) -> Result<()> {
    let mut configured = BTreeMap::new();
    for skill in &project.project.enabled_skills {
        for (key, model_id) in &skill.configuration {
            let Some(capability) = key.strip_prefix("capability.") else {
                continue;
            };
            if let Some(existing) = configured.insert(capability.to_owned(), model_id.clone())
                && existing != *model_id
            {
                bail!(
                    "Project capability {capability:?} has conflicting model bindings {existing:?} and {model_id:?}"
                );
            }
        }
    }
    for node in &mut draft.nodes {
        if node.model_binding.is_some() {
            continue;
        }
        let Some(descriptor) = nodes.get(&node.node_type) else {
            continue;
        };
        if descriptor.required_capabilities.len() != 1 {
            continue;
        }
        let capability = serde_json::to_value(descriptor.required_capabilities[0])
            .ok()
            .and_then(|value| value.as_str().map(ToOwned::to_owned));
        if let Some(model_id) = capability.and_then(|capability| configured.get(&capability)) {
            node.model_binding = Some(model_id.clone());
        }
    }
    Ok(())
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

fn extract_sha256(content: &str) -> Option<&str> {
    let value = content.split_once("sha256=")?.1.split(';').next()?.trim();
    (value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(value)
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
                    {"label":"ball","value":{"kind":"bounding_box","rect":[0.547,0.75,0.06,0.01]},"attributes":{},"confidence":0.98}
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

fn sample_test_outcome_status(
    state: Option<annotagent_core::ArtifactValidationState>,
) -> SampleTestOutcomeStatus {
    match state {
        Some(annotagent_core::ArtifactValidationState::Valid) => {
            SampleTestOutcomeStatus::ReadyToAccept
        }
        Some(annotagent_core::ArtifactValidationState::Invalid) => SampleTestOutcomeStatus::Invalid,
        Some(
            annotagent_core::ArtifactValidationState::NeedsReview
            | annotagent_core::ArtifactValidationState::Unvalidated,
        )
        | None => SampleTestOutcomeStatus::NeedsReview,
    }
}

fn finish_sample_test_summary(
    summary: &mut SampleTestSummary,
    full_image_count: usize,
    duration_ms: u64,
    estimated_cost: &str,
) {
    summary.duration_ms = duration_ms;
    summary.usage = UsageSummary {
        input_tokens: summary.input_tokens,
        output_tokens: summary.output_tokens,
        estimated_cost: estimated_cost.to_owned(),
    };
    if summary.image_count == 0 {
        summary.estimated_full_run = None;
        return;
    }
    let sample_count = summary.image_count;
    let projected_duration = duration_ms
        .saturating_mul(full_image_count.try_into().unwrap_or(u64::MAX))
        .saturating_add(
            sample_count
                .saturating_sub(1)
                .try_into()
                .unwrap_or(u64::MAX),
        )
        / u64::try_from(sample_count).unwrap_or(u64::MAX);
    let projected_cost = estimated_cost
        .parse::<rust_decimal::Decimal>()
        .unwrap_or_default()
        * rust_decimal::Decimal::from(u64::try_from(full_image_count).unwrap_or(u64::MAX))
        / rust_decimal::Decimal::from(u64::try_from(sample_count).unwrap_or(u64::MAX));
    let review_numerator = summary.needs_review_count.saturating_mul(full_image_count);
    summary.estimated_full_run = Some(FullRunEstimate {
        image_count: full_image_count,
        duration_ms: projected_duration,
        estimated_cost: projected_cost.to_string(),
        review_count_min: review_numerator / sample_count,
        review_count_max: review_numerator.saturating_add(sample_count.saturating_sub(1))
            / sample_count,
    });
}

fn history_run_duration_ms(run: &HistoryRun) -> u64 {
    let started = chrono::DateTime::parse_from_rfc3339(&run.created_at).ok();
    let finished = chrono::DateTime::parse_from_rfc3339(&run.updated_at).ok();
    started
        .zip(finished)
        .map(|(started, finished)| {
            (finished - started)
                .num_milliseconds()
                .max(0)
                .try_into()
                .unwrap_or(u64::MAX)
        })
        .unwrap_or_default()
}

fn history_usage_summary(history: &annotagent_storage::HistoryDocument) -> UsageSummary {
    let mut totals = annotagent_core::UsageTotals::default();
    for record in &history.usage {
        totals.add(record);
    }
    UsageSummary {
        input_tokens: totals.input_tokens,
        output_tokens: totals.output_tokens,
        estimated_cost: totals.cost.to_string(),
    }
}

fn revise_draft_after_failed_dry_run(
    suggestion: &mut WorkflowSuggestion,
    failed_count: usize,
) -> bool {
    if failed_count == 0 {
        return false;
    }
    let Some(model_node) = suggestion.draft.nodes.iter_mut().find(|node| {
        matches!(
            node.kind,
            WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
        )
    }) else {
        suggestion.warnings.push(format!(
            "Dry Run reported {failed_count} failed sample(s); no model node was available for a bounded retry revision."
        ));
        suggestion.draft.status = WorkflowDraftStatus::Editing;
        suggestion.draft.updated_at = chrono::Utc::now();
        return true;
    };
    let model_node_id = model_node.id.clone();
    let revised_attempts = model_node
        .retry_policy
        .max_attempts
        .max(1)
        .saturating_add(1)
        .min(3);
    model_node.retry_policy.max_attempts = revised_attempts;
    if let Some(composition) = suggestion.draft.label_pipeline.as_mut() {
        for step in composition
            .shared_stages
            .iter_mut()
            .flat_map(|stage| stage.steps.iter_mut())
            .chain(
                composition
                    .label_pipelines
                    .iter_mut()
                    .flat_map(|pipeline| pipeline.steps.iter_mut()),
            )
            .filter(|step| step.id == model_node_id)
        {
            step.retry_policy.max_attempts = revised_attempts;
        }
    }
    suggestion.draft.status = WorkflowDraftStatus::Editing;
    suggestion.draft.updated_at = chrono::Utc::now();
    suggestion.warnings.push(format!(
        "Dry Run reported {failed_count} failed sample(s); {model_node_id} retry attempts were bounded at {revised_attempts} and the Draft requires human editing before another Dry Run."
    ));
    true
}

fn agent_dry_run_summary(
    report: &WorkflowDryRunReport,
    draft: &WorkflowDraft,
) -> AgentDryRunSummary {
    let mut warning_counts = BTreeMap::new();
    let mut rejected_count = 0usize;
    for sample in &report.samples {
        for node in &sample.nodes {
            for issue in &node.issues {
                *warning_counts.entry(issue.code.clone()).or_insert(0) += 1;
            }
        }
        rejected_count += sample
            .outcomes
            .iter()
            .filter(|outcome| outcome.status == SampleTestOutcomeStatus::Invalid)
            .count();
    }
    let as_u32 = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    let image_count = as_u32(report.summary.image_count);
    let failed_images = as_u32(report.summary.failed_count);
    let model_nodes = draft
        .nodes
        .iter()
        .filter(|node| node.model_binding.is_some())
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    let model_calls = report
        .samples
        .iter()
        .flat_map(|sample| sample.nodes.iter())
        .filter(|node| model_nodes.contains(node.node_id.as_str()) && node.status != "skipped")
        .count();
    AgentDryRunSummary {
        image_count,
        successful_images: image_count.saturating_sub(failed_images),
        empty_images: as_u32(report.summary.empty_count),
        failed_images,
        detection_count: as_u32(report.summary.detection_count),
        auto_accepted_count: as_u32(report.summary.auto_accepted_count),
        review_count: as_u32(report.summary.needs_review_count),
        rejected_count: as_u32(rejected_count),
        warning_counts,
        model_calls: as_u32(model_calls),
        duration_ms: report.total_latency_ms,
        cost: report
            .estimated_cost
            .parse()
            .unwrap_or(rust_decimal::Decimal::ZERO),
    }
}

fn available_model_for_capability(
    models: &[VisionModelDescriptor],
    capability: VisionCapability,
) -> Result<String> {
    models
        .iter()
        .find(|model| {
            model.capabilities.contains(&capability)
                && (model.status == ModelAvailabilityStatus::Available
                    || model.health.status == VisionModelHealthStatus::Healthy)
        })
        .map(|model| model.id.clone())
        .ok_or_else(|| anyhow!("no available Registry Model supports {capability:?}"))
}

fn add_crop_verification_revision(
    suggestion: &mut WorkflowSuggestion,
    project: &ProjectSchema,
    target_task_id: &str,
    target_label: &str,
    classifier_model_id: &str,
    evidence: &AgentDryRunSummary,
) -> Result<bool> {
    let Some(mut composition) = suggestion.draft.label_pipeline.clone() else {
        return Ok(false);
    };
    let Some(pipeline_index) = composition.label_pipelines.iter().position(|pipeline| {
        pipeline.target_task_id.as_str() == target_task_id
            && pipeline.target_label.as_str() == target_label
    }) else {
        return Ok(false);
    };
    let pipeline = &composition.label_pipelines[pipeline_index];
    if pipeline
        .steps
        .iter()
        .any(|step| step.node_type == annotagent_runtime::CORE_CROP)
    {
        return Ok(false);
    }
    let filter = pipeline
        .steps
        .iter()
        .find(|step| step.node_type == annotagent_runtime::CORE_FILTER)
        .cloned()
        .ok_or_else(|| anyhow!("Crop verification requires a Select detections step"))?;
    let gate = pipeline
        .steps
        .iter()
        .find(|step| step.kind == WorkflowNodeKind::Gate)
        .cloned()
        .ok_or_else(|| anyhow!("Crop verification requires a Decision step"))?;
    let commit = pipeline
        .steps
        .iter()
        .find(|step| step.kind == WorkflowNodeKind::Commit)
        .cloned()
        .ok_or_else(|| anyhow!("Crop verification requires a Commit step"))?;
    let prefix = format!("{target_task_id}.{target_label}.crop_verify");
    let crop = PipelineStep {
        id: format!("{prefix}.crop"),
        node_type: annotagent_runtime::CORE_CROP.to_owned(),
        kind: WorkflowNodeKind::Transform,
        inputs: BTreeMap::from([
            ("image".to_owned(), PipelineSource::Image),
            (
                "detections".to_owned(),
                PipelineSource::Step {
                    step_id: filter.id.clone(),
                    port: "detections".to_owned(),
                    artifact_type: ArtifactKind::DetectionSet,
                },
            ),
        ]),
        outputs: BTreeMap::from([("crops".to_owned(), ArtifactKind::CropSet)]),
        model_binding: None,
        skill_binding: None,
        parameters: BTreeMap::from([("padding".to_owned(), json!(0.08))]),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        retry_policy: RetryPolicy::default(),
        review_gate: ReviewGate::default(),
        resources: ResourceRequirements::default(),
    };
    let classifier = PipelineStep {
        id: format!("{prefix}.classifier"),
        node_type: annotagent_skill_classification::CLASSIFICATION_OPERATION.to_owned(),
        kind: WorkflowNodeKind::VisionModel,
        inputs: BTreeMap::from([(
            "subjects".to_owned(),
            PipelineSource::Step {
                step_id: crop.id.clone(),
                port: "crops".to_owned(),
                artifact_type: ArtifactKind::CropSet,
            },
        )]),
        outputs: BTreeMap::from([(
            "classifications".to_owned(),
            ArtifactKind::ClassificationSet,
        )]),
        model_binding: Some(PipelineModelBinding {
            model_id: classifier_model_id.to_owned(),
            capability: VisionCapability::Classification,
            configuration: BTreeMap::new(),
        }),
        skill_binding: None,
        parameters: BTreeMap::from([
            ("labels".to_owned(), json!([target_label])),
            ("mock_label".to_owned(), json!(target_label)),
            ("target_label".to_owned(), json!(target_label)),
        ]),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        retry_policy: RetryPolicy::default(),
        review_gate: ReviewGate::default(),
        resources: ResourceRequirements::default(),
    };
    let attach = PipelineStep {
        id: format!("{prefix}.attach"),
        node_type: annotagent_runtime::CORE_ATTACH_RESULT.to_owned(),
        kind: WorkflowNodeKind::CandidateMerge,
        inputs: BTreeMap::from([
            (
                "detections".to_owned(),
                PipelineSource::Step {
                    step_id: filter.id.clone(),
                    port: "detections".to_owned(),
                    artifact_type: ArtifactKind::DetectionSet,
                },
            ),
            (
                "classifications".to_owned(),
                PipelineSource::Step {
                    step_id: classifier.id.clone(),
                    port: "classifications".to_owned(),
                    artifact_type: ArtifactKind::ClassificationSet,
                },
            ),
        ]),
        outputs: BTreeMap::from([(
            "candidates".to_owned(),
            ArtifactKind::AnnotationCandidateSet,
        )]),
        model_binding: None,
        skill_binding: None,
        parameters: BTreeMap::from([
            ("task_id".to_owned(), json!(target_task_id)),
            (
                "class_mapping".to_owned(),
                json!(BTreeMap::from([(
                    target_label.to_owned(),
                    target_label.to_owned(),
                )])),
            ),
        ]),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        retry_policy: RetryPolicy::default(),
        review_gate: ReviewGate::default(),
        resources: ResourceRequirements::default(),
    };
    let mut revised_gate = gate;
    revised_gate.inputs = BTreeMap::from([(
        "candidates".to_owned(),
        PipelineSource::Step {
            step_id: attach.id.clone(),
            port: "candidates".to_owned(),
            artifact_type: ArtifactKind::AnnotationCandidateSet,
        },
    )]);
    revised_gate.outputs = BTreeMap::from([(
        "candidates".to_owned(),
        ArtifactKind::AnnotationCandidateSet,
    )]);
    // The first Dry Run supplied evidence for adding verification; keep acceptance conservative
    // but avoid an impossible threshold that would route every verified candidate to Review.
    revised_gate.parameters.insert(
        "threshold".to_owned(),
        json!(project.review.auto_accept_confidence.min(0.9)),
    );
    let mut revised_commit = commit;
    revised_commit.inputs = BTreeMap::from([(
        "candidates".to_owned(),
        PipelineSource::Step {
            step_id: revised_gate.id.clone(),
            port: "candidates".to_owned(),
            artifact_type: ArtifactKind::AnnotationCandidateSet,
        },
    )]);
    let revised_gate_id = revised_gate.id.clone();
    let revised_commit_id = revised_commit.id.clone();
    let pipeline = &mut composition.label_pipelines[pipeline_index];
    pipeline.steps = pipeline
        .steps
        .iter()
        .filter(|step| step.id != revised_gate_id && step.id != revised_commit_id)
        .cloned()
        .chain([crop, classifier, attach, revised_gate, revised_commit])
        .collect();

    let old = suggestion.draft.clone();
    let mut compiled = composition.compile_draft(
        old.project_id.clone(),
        old.name.clone(),
        old.enabled_skills.clone(),
        old.created_at,
    );
    compiled.id = old.id;
    compiled.status = WorkflowDraftStatus::Editing;
    compiled.resource_versions = old.resource_versions;
    compiled.allow_unvalidated_commit = old.allow_unvalidated_commit;
    compiled.updated_at = chrono::Utc::now();
    suggestion.draft = compiled;
    let decided = evidence
        .auto_accepted_count
        .saturating_add(evidence.review_count)
        .saturating_add(evidence.rejected_count);
    suggestion.rationale.push(format!(
        "The first Dry Run routed {} of {} decided candidate(s) to Review ({:.0}%), so the Draft now crops each detection and verifies the crop before Decision.",
        evidence.review_count,
        decided,
        evidence.review_rate() * 100.0
    ));
    suggestion.warnings.push(
        "Crop verification adds one model call per candidate and remains an editable Draft change."
            .to_owned(),
    );
    Ok(true)
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
    use annotagent_core::{
        LabelId, NodePort, TaskId, WorkflowDraftNode, WorkflowEdge, WorkflowNodeKind,
    };

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

    const HIGH_REVIEW_BBOX_PROJECT: &str = r"
version: 1
project:
  name: High review component inspection
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
  auto_accept_confidence: 0.99
  force_review_below: 0.5
export:
  formats: [native, coco]
";

    const GENERIC_CLASSIFICATION_PROJECT: &str = r"
version: 1
project:
  name: Generic scene classification
  language: en
dataset:
  root: images
runtime: {}
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

    const OPEN_VOCABULARY_PROJECT: &str = r#"
version: 1
project:
  name: Open vocabulary demo
  language: en
  enabled_skills:
    - id: annotagent.open_vocabulary_grounding
      version: "1"
dataset:
  root: images
runtime: {}
tasks:
  - id: objects
    kind: bounding_box
    labels: [target]
    required: false
review:
  auto_accept_confidence: 0.95
  force_review_below: 0.5
export:
  formats: [native]
"#;

    const OBJECT_DETECTION_PROJECT: &str = r#"
version: 1
project:
  name: Generic trained detector demo
  language: en
  enabled_skills:
    - id: annotagent.object_detection
      version: "1"
dataset:
  root: images
runtime: {}
tasks:
  - id: objects
    kind: bounding_box
    labels: [ball]
    required: false
review:
  auto_accept_confidence: 0.95
  force_review_below: 0.5
export:
  formats: [native]
"#;

    const MIXED_DETECTION_PROJECT: &str = r#"
version: 1
project:
  name: Generic mixed evidence demo
  language: en
  enabled_skills:
    - id: annotagent.object_detection
      version: "1"
    - id: annotagent.open_vocabulary_grounding
      version: "1"
dataset:
  root: images
runtime: {}
tasks:
  - id: objects
    kind: bounding_box
    labels: [ball]
    required: false
review:
  auto_accept_confidence: 0.95
  force_review_below: 0.5
export:
  formats: [native]
"#;

    #[test]
    fn open_vocabulary_skill_and_models_are_registered_without_contacting_worker() {
        let settings = load_settings(None).expect("default Settings");
        assert_eq!(settings.detection_workers.len(), 2);
        assert!(!settings.detection_workers[0].enabled);
        assert!(!settings.detection_workers[1].enabled);
        validate_settings(&settings).expect("offline Worker configuration remains valid");
        let (nodes, models) = workflow_catalog(&settings).expect("catalog");
        assert!(
            nodes
                .get(annotagent_skill_open_vocabulary::OPEN_VOCABULARY_DETECTION_OPERATION)
                .is_some()
        );
        assert!(
            nodes
                .get(annotagent_skill_open_vocabulary::PHRASE_GROUNDING_OPERATION)
                .is_some()
        );
        assert!(models.resolve("mock-open-vocabulary").is_ok());
        assert!(
            models
                .models()
                .iter()
                .any(|model| model.id == "locate-anything-local"
                    && model.status == ModelAvailabilityStatus::Disabled)
        );
        assert!(models.resolve("locate-anything-local").is_err());
    }

    #[test]
    fn object_detection_skill_and_versioned_specialist_profile_are_registered_offline() {
        let mut settings = load_settings(None).expect("default Settings");
        let temporary = tempfile::tempdir().expect("temporary settings");
        let settings_path = temporary.path().join("legacy-settings.toml");
        let mut legacy = settings.clone();
        legacy
            .detection_workers
            .retain(|worker| worker.model_id == "locate-anything-local");
        std::fs::write(
            &settings_path,
            toml::to_string_pretty(&legacy).expect("legacy Settings TOML"),
        )
        .expect("legacy Settings file");
        let migrated =
            load_settings(Some(&settings_path)).expect("additive Worker profile migration");
        assert!(
            migrated
                .detection_workers
                .iter()
                .any(|worker| worker.model_id == "rfdetr-specialist-local")
        );
        let specialist_index = settings
            .detection_workers
            .iter()
            .position(|worker| worker.model_id == "rfdetr-specialist-local")
            .expect("specialist profile");
        settings.detection_workers[specialist_index].enabled = true;
        assert!(validate_settings(&settings).is_err());
        let specialist = &mut settings.detection_workers[specialist_index];
        specialist.version.architecture = Some("rfdetr-small".to_owned());
        specialist.version.model_version = "robocup-ball-v1".to_owned();
        specialist.version.checkpoint_sha256 = Some("a".repeat(64));
        specialist.version.training_dataset_version = Some("robocup-ball-v3".to_owned());
        specialist.label_space = vec!["football".to_owned(), "robot".to_owned()];
        specialist.license.weight_license = Some("checkpoint-owner-supplied".to_owned());
        validate_settings(&settings).expect("complete immutable specialist metadata");
        let (nodes, models) = workflow_catalog(&settings).expect("catalog");
        assert!(
            nodes
                .get(annotagent_skill_object_detection::OBJECT_DETECTION_OPERATION)
                .is_some()
        );
        assert!(models.resolve("mock-object-detector").is_ok());
        let descriptor = models
            .models()
            .into_iter()
            .find(|model| model.id == "rfdetr-specialist-local")
            .expect("specialist descriptor");
        assert_eq!(
            descriptor.version.checkpoint_sha256.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(
            descriptor.version.training_dataset_version.as_deref(),
            Some("robocup-ball-v3")
        );
        assert_eq!(
            descriptor.output_contract.label_space,
            vec!["football", "robot"]
        );
        assert!(!descriptor.input_contract.supports_multiple_queries);
    }

    #[tokio::test]
    async fn generic_open_vocabulary_template_dry_runs_and_executes_offline() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("open-vocabulary", OPEN_VOCABULARY_PROJECT)
            .expect("Generic Project");
        generate_synthetic_robocup(&temporary.path().join("open-vocabulary/images/sample.png"))
            .expect("sample image");
        let settings = load_settings(None).expect("settings");
        let draft = application
            .create_workflow_draft_with_template(
                "open-vocabulary",
                &settings,
                false,
                Some("open-vocabulary.text-query-review"),
            )
            .expect("Grounding template");
        let dry_run = application
            .dry_run_workflow_samples(&draft.id, &settings, &[0])
            .await
            .expect("offline Grounding Dry Run");
        assert!(dry_run.validation.valid, "{:#?}", dry_run.validation.issues);
        assert_eq!(dry_run.samples.len(), 1);
        assert!(
            dry_run.samples[0].nodes.iter().any(|node| {
                node.node_id == "grounding" && node.status == "completed_in_sandbox"
            })
        );
        let published = application
            .publish_workflow(&draft.id, &settings)
            .expect("publish Grounding Workflow");
        let started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("open-vocabulary/project.yaml"),
                "mock",
                settings,
                None,
                Some("open-vocabulary-offline"),
                Some((&published.workflow_id, published.version)),
            )
            .expect("start Grounding Run");
        let result = application
            .wait_run(started.run_id)
            .await
            .expect("complete Grounding Run");
        assert_eq!(
            result.status,
            RunStatus::CompletedWithReview,
            "{:#?}",
            result.issues
        );
        let inspection = application
            .inspect_run_pipeline_artifacts(started.run_id)
            .expect("persisted Pipeline Artifact inspection");
        let grounding = inspection
            .nodes
            .iter()
            .find(|node| node.node_id == "grounding")
            .expect("Grounding node inspection");
        let serialized = serde_json::to_string(&grounding.outputs).expect("Pipeline Artifacts");
        assert!(serialized.contains("grounding-0"));
        assert!(serialized.contains("not_provided"));
    }

    #[tokio::test]
    async fn generic_object_detection_template_maps_classes_and_executes_offline() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("object-detection", OBJECT_DETECTION_PROJECT)
            .expect("Generic Project");
        generate_synthetic_robocup(&temporary.path().join("object-detection/images/sample.png"))
            .expect("sample image");
        let settings = load_settings(None).expect("settings");
        let mut draft = application
            .create_workflow_draft_with_template(
                "object-detection",
                &settings,
                false,
                Some("object-detection.specialist-review"),
            )
            .expect("Object Detection template");
        let detector = draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "detector")
            .expect("detector node");
        detector
            .parameters
            .insert("target_labels".to_owned(), json!(["ball"]));
        detector
            .parameters
            .insert("class_mapping".to_owned(), json!({"football": "ball"}));
        detector
            .parameters
            .insert("mock_model_label".to_owned(), json!("football"));
        detector
            .parameters
            .insert("mock_confidence".to_owned(), json!(0.87));
        application
            .save_workflow_draft(draft.clone())
            .expect("save mapped Draft");
        let dry_run = application
            .dry_run_workflow_samples(&draft.id, &settings, &[0])
            .await
            .expect("offline specialist Dry Run");
        assert!(dry_run.validation.valid, "{:#?}", dry_run.validation.issues);
        assert!(
            dry_run.samples[0].nodes.iter().any(|node| {
                node.node_id == "detector" && node.status == "completed_in_sandbox"
            })
        );
        let published = application
            .publish_workflow(&draft.id, &settings)
            .expect("publish specialist Workflow");
        let started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("object-detection/project.yaml"),
                "mock",
                settings,
                None,
                Some("object-detection-offline"),
                Some((&published.workflow_id, published.version)),
            )
            .expect("start specialist Run");
        let result = application
            .wait_run(started.run_id)
            .await
            .expect("complete specialist Run");
        assert_eq!(result.status, RunStatus::CompletedWithReview);
        let inspection = application
            .inspect_run_pipeline_artifacts(started.run_id)
            .expect("persisted specialist Artifact inspection");
        let detector = inspection
            .nodes
            .iter()
            .find(|node| node.node_id == "detector")
            .expect("detector inspection");
        let serialized = serde_json::to_string(&detector.outputs).expect("DetectionSet JSON");
        assert!(serialized.contains("football"));
        assert!(serialized.contains("ball"));
        assert!(serialized.contains("relative_confidence"));
    }

    #[tokio::test]
    async fn mixed_detection_nodes_execute_and_persist_explainable_evidence_offline() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("mixed-evidence", MIXED_DETECTION_PROJECT)
            .expect("Generic Project");
        generate_synthetic_robocup(&temporary.path().join("mixed-evidence/images/sample.png"))
            .expect("sample image");
        let settings = load_settings(None).expect("settings");
        let mut draft = application
            .create_workflow_draft("mixed-evidence", &settings, false)
            .expect("blank Draft");
        let port = |id: &str, artifact_type, multiple| NodePort {
            id: id.to_owned(),
            artifact_type,
            required: true,
            multiple,
        };
        let node =
            |id: &str, node_type: &str, kind, inputs, outputs, parameters| WorkflowDraftNode {
                id: id.to_owned(),
                node_type: node_type.to_owned(),
                kind,
                inputs,
                outputs,
                parameters,
                ..WorkflowDraftNode::default()
            };
        let mut specialist = node(
            "specialist",
            annotagent_skill_object_detection::OBJECT_DETECTION_OPERATION,
            WorkflowNodeKind::VisionModel,
            vec![port("image", ArtifactKind::Image, false)],
            vec![port("detections", ArtifactKind::DetectionSet, false)],
            BTreeMap::from([
                ("target_labels".to_owned(), json!(["ball"])),
                ("class_mapping".to_owned(), json!({"football": "ball"})),
                ("mock_model_label".to_owned(), json!("football")),
                ("mock_confidence".to_owned(), json!(0.93)),
            ]),
        );
        specialist.model_binding = Some("mock-object-detector".to_owned());
        specialist.required_skills = vec!["annotagent.object_detection".to_owned()];
        let mut grounding = node(
            "grounding",
            annotagent_skill_open_vocabulary::OPEN_VOCABULARY_DETECTION_OPERATION,
            WorkflowNodeKind::VisionModel,
            vec![port("image", ArtifactKind::Image, false)],
            vec![port("detections", ArtifactKind::DetectionSet, false)],
            BTreeMap::from([(
                "queries".to_owned(),
                json!([{"id": "ball-query", "text": "ball", "target_label": "ball"}]),
            )]),
        );
        grounding.model_binding = Some("mock-open-vocabulary".to_owned());
        grounding.required_skills = vec!["annotagent.open_vocabulary_grounding".to_owned()];
        let image = node(
            "image",
            annotagent_core::IMAGE_INPUT_OPERATION,
            WorkflowNodeKind::ImageInput,
            Vec::new(),
            vec![port("image", ArtifactKind::Image, false)],
            BTreeMap::new(),
        );
        let matcher = node(
            "match",
            annotagent_runtime::CORE_CANDIDATE_MATCH,
            WorkflowNodeKind::CandidateMerge,
            vec![
                port("left", ArtifactKind::DetectionSet, false),
                port("right", ArtifactKind::DetectionSet, false),
            ],
            vec![port("candidates", ArtifactKind::CandidateClusterSet, false)],
            BTreeMap::from([
                ("method".to_owned(), json!("iou")),
                ("minimum_iou".to_owned(), json!(0.6)),
                ("preserve_unmatched".to_owned(), json!(true)),
            ]),
        );
        let gate = node(
            "evidence",
            annotagent_runtime::CORE_EVIDENCE_GATE,
            WorkflowNodeKind::Gate,
            vec![port("candidates", ArtifactKind::CandidateClusterSet, false)],
            vec![port("candidates", ArtifactKind::CandidateClusterSet, false)],
            BTreeMap::from([
                (
                    "accept_when".to_owned(),
                    json!([{"minimum_sources": 2, "minimum_iou": 0.6}]),
                ),
                ("review_when".to_owned(), json!([{"score_missing": true}])),
            ]),
        );
        let review = node(
            "review",
            "review_gate",
            WorkflowNodeKind::HumanReview,
            vec![port("candidates", ArtifactKind::CandidateClusterSet, false)],
            vec![port("candidates", ArtifactKind::CandidateClusterSet, false)],
            BTreeMap::from([("task_id".to_owned(), json!("objects"))]),
        );
        let commit = node(
            "commit",
            "commit",
            WorkflowNodeKind::Commit,
            vec![port("candidates", ArtifactKind::CandidateClusterSet, false)],
            Vec::new(),
            BTreeMap::from([("task_id".to_owned(), json!("objects"))]),
        );
        let edge = |from_node: &str,
                    from_port: &str,
                    to_node: &str,
                    to_port: &str,
                    route: Option<&str>| WorkflowEdge {
            from_node: from_node.to_owned(),
            from_port: from_port.to_owned(),
            to_node: to_node.to_owned(),
            to_port: to_port.to_owned(),
            route: route.map(ToOwned::to_owned),
        };
        draft.nodes = vec![image, specialist, grounding, matcher, gate, review, commit];
        draft.edges = vec![
            edge("image", "image", "specialist", "image", None),
            edge("image", "image", "grounding", "image", None),
            edge("specialist", "detections", "match", "left", None),
            edge("grounding", "detections", "match", "right", None),
            edge("match", "candidates", "evidence", "candidates", None),
            edge(
                "evidence",
                "candidates",
                "review",
                "candidates",
                Some("review"),
            ),
            edge("review", "candidates", "commit", "candidates", None),
        ];
        let draft = application
            .save_workflow_draft(draft)
            .expect("save mixed-evidence Draft");
        let dry_run = application
            .dry_run_workflow_samples(&draft.id, &settings, &[0])
            .await
            .expect("mixed-evidence Dry Run");
        assert!(dry_run.validation.valid, "{:#?}", dry_run.validation.issues);
        assert_eq!(dry_run.summary.failed_count, 0);
        let published = application
            .publish_workflow(&draft.id, &settings)
            .expect("publish mixed-evidence Workflow");
        let started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("mixed-evidence/project.yaml"),
                "mock",
                settings,
                None,
                Some("mixed-evidence-offline"),
                Some((&published.workflow_id, published.version)),
            )
            .expect("start mixed-evidence Run");
        let result = application
            .wait_run(started.run_id)
            .await
            .expect("complete mixed-evidence Run");
        assert_eq!(result.status, RunStatus::CompletedWithReview);
        let inspection = application
            .inspect_run_pipeline_artifacts(started.run_id)
            .expect("persisted evidence inspection");
        let match_inspection = inspection
            .nodes
            .iter()
            .find(|node| node.node_id == "match")
            .expect("match inspection");
        let PipelineArtifact::CandidateClusterSet(clusters) = &match_inspection.outputs[0] else {
            panic!("CandidateClusterSet")
        };
        assert_eq!(clusters.candidates.len(), 1);
        assert_eq!(clusters.candidates[0].members.len(), 2);
        assert!(clusters.candidates[0].members.iter().any(|member| {
            member.source_model_id == "mock-object-detector" && member.score.value == Some(0.93)
        }));
        assert!(clusters.candidates[0].members.iter().any(|member| {
            member.source_model_id == "mock-open-vocabulary" && member.score.value.is_none()
        }));
        let evidence = inspection
            .nodes
            .iter()
            .find(|node| node.node_id == "evidence")
            .expect("Evidence Gate inspection");
        assert_eq!(evidence.route.as_deref(), Some("review"));
        assert_eq!(evidence.metadata["evidence_gate"]["decision"], "review");
        assert_eq!(
            evidence.metadata["evidence_gate"]["reasons"][0]["code"],
            "score_not_comparable"
        );
    }

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
        assert!(history.artifacts.is_empty());
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
        let inspection = application
            .inspect_run_pipeline_artifacts(started.run_id)
            .expect("Detection Recovery inspection");
        let recovery = inspection
            .nodes
            .iter()
            .find(|node| node.node_id == "recovery")
            .expect("Recovery Agent node");
        assert_eq!(
            recovery.metadata["recovery_agent"]["fallback_invoked"],
            false
        );
        assert_eq!(
            recovery.metadata["recovery_agent"]["stop_condition"],
            "primary_accepted"
        );
        let sessions = application
            .list_agent_sessions("generic-dag")
            .expect("persisted Detection Recovery trace");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].kind, AgentKind::AnnotationRecovery);
        assert!(
            history
                .events
                .iter()
                .any(|event| event.kind == RunEventKind::ArtifactCommitted
                    || event.kind == RunEventKind::ArtifactCreated)
        );
    }

    #[tokio::test]
    async fn published_recovery_invokes_open_vocabulary_only_for_empty_specialist_evidence() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("generic-fallback", GENERIC_BBOX_PROJECT)
            .expect("generic Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary
                .path()
                .join("generic-fallback/images/component.png"),
        )
        .expect("generic image");
        let settings = load_settings(None).expect("settings");
        let mut suggestion = application
            .suggest_workflow(
                "generic-fallback",
                &settings,
                &WorkflowConstraints::default(),
            )
            .expect("specialist-first Draft");
        suggestion
            .draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "specialist")
            .expect("specialist node")
            .parameters
            .insert("mock_empty".to_owned(), json!(true));
        let draft = application
            .save_workflow_draft(suggestion.draft)
            .expect("save fallback fixture");
        let published = application
            .publish_workflow(&draft.id, &settings)
            .expect("publish fallback Workflow");
        let started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("generic-fallback/project.yaml"),
                "mock",
                settings,
                None,
                Some("generic-fallback-run"),
                Some((&published.workflow_id, published.version)),
            )
            .expect("start fallback Run");
        let result = application.wait_run(started.run_id).await.expect("DAG Run");
        assert_eq!(result.status, RunStatus::CompletedWithReview);
        let inspection = application
            .inspect_run_pipeline_artifacts(started.run_id)
            .expect("Recovery inspection");
        let recovery = inspection
            .nodes
            .iter()
            .find(|node| node.node_id == "recovery")
            .expect("Recovery Agent node");
        assert_eq!(
            recovery.metadata["recovery_agent"]["fallback_invoked"],
            true
        );
        assert_eq!(
            recovery.metadata["recovery_agent"]["fallback_call_count"],
            1
        );
        assert_eq!(
            recovery.metadata["recovery_agent"]["initial_evidence"]["decision"],
            "fallback"
        );
        let sessions = application
            .list_agent_sessions("generic-fallback")
            .expect("persisted Recovery trace");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].steps.len(), 2);
        assert_eq!(sessions[0].steps[1].tool_name, "invoke_fallback_detection");
    }

    #[tokio::test]
    async fn published_label_pipeline_executes_and_persists_typed_checkpoint() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("label-classification", GENERIC_CLASSIFICATION_PROJECT)
            .expect("classification Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary
                .path()
                .join("label-classification/images/sample.png"),
        )
        .expect("sample image");
        let now = chrono::Utc::now();
        let port = |id: &str, artifact_type| annotagent_core::NodePort {
            id: id.to_owned(),
            artifact_type,
            required: true,
            multiple: false,
        };
        let draft = WorkflowDraft {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            id: "label-classification-draft".to_owned(),
            project_id: "label-classification".to_owned(),
            name: "Whole-image Classification Demo".to_owned(),
            status: WorkflowDraftStatus::Editing,
            nodes: vec![
                WorkflowDraftNode {
                    id: "image".to_owned(),
                    node_type: annotagent_core::IMAGE_INPUT_OPERATION.to_owned(),
                    kind: WorkflowNodeKind::ImageInput,
                    outputs: vec![port("image", ArtifactKind::Image)],
                    ..WorkflowDraftNode::default()
                },
                WorkflowDraftNode {
                    id: "classifier".to_owned(),
                    node_type: annotagent_skill_classification::CLASSIFICATION_OPERATION.to_owned(),
                    kind: WorkflowNodeKind::VisionModel,
                    inputs: vec![port("image", ArtifactKind::Image)],
                    outputs: vec![port("classifications", ArtifactKind::ClassificationSet)],
                    model_binding: Some("mock-classifier".to_owned()),
                    parameters: BTreeMap::from([
                        ("labels".to_owned(), json!(["day", "night"])),
                        ("mock_label".to_owned(), json!("day")),
                    ]),
                    ..WorkflowDraftNode::default()
                },
                WorkflowDraftNode {
                    id: "commit".to_owned(),
                    node_type: "commit".to_owned(),
                    kind: WorkflowNodeKind::Commit,
                    inputs: vec![port("classifications", ArtifactKind::ClassificationSet)],
                    parameters: BTreeMap::from([("task_id".to_owned(), json!("scene"))]),
                    ..WorkflowDraftNode::default()
                },
            ],
            edges: vec![
                WorkflowEdge {
                    from_node: "image".to_owned(),
                    from_port: "image".to_owned(),
                    to_node: "classifier".to_owned(),
                    to_port: "image".to_owned(),
                    route: None,
                },
                WorkflowEdge {
                    from_node: "classifier".to_owned(),
                    from_port: "classifications".to_owned(),
                    to_node: "commit".to_owned(),
                    to_port: "classifications".to_owned(),
                    route: None,
                },
            ],
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            allow_unvalidated_commit: true,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        };
        application
            .save_workflow_draft(draft)
            .expect("save Label Pipeline Draft");
        let settings = load_settings(None).expect("settings");
        let published = application
            .publish_workflow("label-classification-draft", &settings)
            .expect("publish Label Pipeline");
        let started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("label-classification/project.yaml"),
                "mock",
                settings.clone(),
                None,
                Some("label-pipeline-app-run"),
                Some((&published.workflow_id, published.version)),
            )
            .expect("start Label Pipeline Run");
        let result = application.wait_run(started.run_id).await.expect("Run");
        assert_eq!(result.status, RunStatus::Completed, "{:#?}", result.issues);
        assert_eq!(result.committed.len(), 1);
        assert_eq!(result.committed[0].task_id, TaskId::from("scene"));
        assert_eq!(result.committed[0].label, Some(LabelId::from("day")));
        let history = application.store.history(started.run_id).expect("history");
        assert_eq!(history.annotations.len(), 1);
        let snapshot: serde_json::Value = serde_json::from_str(
            history
                .run
                .workflow_snapshot_json
                .as_deref()
                .expect("snapshot"),
        )
        .expect("snapshot JSON");
        assert_eq!(
            snapshot["checkpoint"]["node_outputs"]["classifier"]["pipeline_artifacts"][0]["kind"],
            json!("classification_set")
        );
        let inspection = application
            .inspect_run_pipeline_artifacts(started.run_id)
            .expect("Pipeline Artifact inspection");
        assert_eq!(inspection.image_index, Some(0));
        let classifier_inspection = inspection
            .nodes
            .iter()
            .find(|node| node.node_id == "classifier")
            .expect("classifier inspection");
        assert_eq!(
            classifier_inspection.inputs[0].artifact_type(),
            ArtifactKind::Image
        );
        assert_eq!(
            classifier_inspection.outputs[0].artifact_type(),
            ArtifactKind::ClassificationSet
        );
        assert_eq!(classifier_inspection.attempts, 1);
        assert!(classifier_inspection.error.is_none());
        let result_summary = application
            .run_result_summary(started.run_id)
            .expect("Run result summary");
        assert_eq!(result_summary.result_count, 1);
        assert_eq!(result_summary.ready_count, 1);
        assert_eq!(result_summary.needs_review_count, 0);
        assert_eq!(result_summary.no_target_count, 0);
        assert_eq!(result_summary.labels[0].label, "day");
        let debug_summary = application
            .run_debug_summary(started.run_id)
            .expect("Run debug summary");
        assert_eq!(debug_summary.node_count, 3);
        assert_eq!(debug_summary.succeeded_node_count, 3);
        assert_eq!(debug_summary.failed_node_count, 0);
        assert!(debug_summary.issues.is_empty());
        let replay = application
            .replay_run_from_node(started.run_id, "classifier", &settings)
            .await
            .expect("classifier Replay");
        assert!(replay.sandbox);
        assert!(replay.reexecuted_nodes.contains(&"classifier".to_owned()));
        assert!(
            replay
                .preserved_upstream_nodes
                .contains(&"image".to_owned())
        );
        let readiness = application
            .export_readiness("label-classification")
            .expect("Export Readiness");
        assert!(readiness.ready, "{:#?}", readiness.blocking_issues);
        assert_eq!(readiness.image_count, 1);
        assert_eq!(readiness.processed_image_count, 1);
        assert_eq!(readiness.accepted_annotations, 1);
        assert_eq!(readiness.unresolved_reviews, 0);
        assert_eq!(readiness.recommended_format.as_deref(), Some("native"));
        let export = application
            .export_project_dataset("label-classification", "native")
            .await
            .expect("native Project export");
        assert_eq!(export.report.exported_count, 1);
        assert!(export.output_path.join("annotagent-native.json").is_file());
        assert!(export.output_path.join("export-report.json").is_file());
        let persisted = application
            .export_readiness("label-classification")
            .expect("persisted Export result");
        assert_eq!(
            persisted
                .last_export
                .as_ref()
                .map(|result| &result.completed_at),
            Some(&export.completed_at)
        );

        let image_root = temporary.path().join("label-classification/images");
        for index in 0..99 {
            annotagent_image_tools::generate_synthetic_inspection(
                &image_root.join(format!("batch-{index:03}.png")),
            )
            .expect("batch image");
        }
        let coordinator = DatasetCoordinator::new(&application);
        let batch = coordinator
            .create_with_workflow(
                &temporary.path().join("label-classification/project.yaml"),
                "mock",
                None,
                Some(100),
                Some((&published.workflow_id, published.version)),
            )
            .expect("Label Pipeline Batch");
        let execution = coordinator
            .execute(batch.id, None)
            .await
            .expect("Label Pipeline Batch execution");
        assert_eq!(execution.batch.status, BatchStatus::Completed);
        assert_eq!(execution.results.len(), 100);
        assert!(execution.results.iter().all(|result| {
            result.result.status == RunStatus::Completed && result.result.committed.len() == 1
        }));
    }

    #[tokio::test]
    async fn target_label_advisor_draft_is_editable_dry_runnable_and_publish_blocking() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("label-advisor", GENERIC_CLASSIFICATION_PROJECT)
            .expect("classification Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary.path().join("label-advisor/images/sample.png"),
        )
        .expect("sample image");
        let settings = load_settings(None).expect("settings");
        let suggestion = application
            .suggest_label_pipeline(
                "label-advisor",
                &settings,
                "scene",
                "day",
                &WorkflowConstraints::default(),
            )
            .expect("controlled Label Pipeline suggestion");
        assert_eq!(suggestion.draft.status, WorkflowDraftStatus::Suggested);
        assert!(suggestion.estimated_model_calls_per_image > 0);
        assert!(suggestion.estimated_latency_ms.is_some());
        assert!(matches!(
            suggestion.estimated_cost_tier.as_str(),
            "low" | "medium" | "high"
        ));
        let composition = suggestion
            .draft
            .label_pipeline
            .as_ref()
            .expect("Label Pipeline authoring projection");
        assert_eq!(composition.label_pipelines.len(), 1);
        assert_eq!(
            composition.label_pipelines[0].target_task_id,
            TaskId::from("scene")
        );
        assert_eq!(
            composition.label_pipelines[0].target_label,
            LabelId::from("day")
        );
        assert!(suggestion.unresolved_model_bindings.is_empty());

        let mut edited = suggestion.draft;
        edited
            .label_pipeline
            .as_mut()
            .expect("composition")
            .label_pipelines[0]
            .steps
            .iter_mut()
            .find(|step| step.node_type == annotagent_runtime::CORE_CONFIDENCE_GATE)
            .expect("confidence gate")
            .parameters
            .insert("threshold".to_owned(), json!(0.8));
        let saved = application
            .save_workflow_draft(edited)
            .expect("human-edited Draft");
        assert_eq!(saved.status, WorkflowDraftStatus::Editing);
        assert_eq!(
            saved
                .nodes
                .iter()
                .find(|node| node.node_type == annotagent_runtime::CORE_CONFIDENCE_GATE)
                .and_then(|node| node.parameters.get("threshold")),
            Some(&json!(0.8))
        );
        let dry_run = application
            .dry_run_workflow_samples(&saved.id, &settings, &[0])
            .await
            .expect("real Label Pipeline Dry Run");
        assert!(dry_run.sandbox);
        assert!(dry_run.validation.valid, "{:#?}", dry_run.validation.issues);
        assert_eq!(dry_run.samples.len(), 1);
        assert_eq!(dry_run.samples[0].image_index, 0);
        assert_eq!(dry_run.samples[0].result_count, 1);
        assert_eq!(dry_run.samples[0].outcomes[0].label, "day");
        assert_eq!(
            dry_run.samples[0].outcomes[0].status,
            SampleTestOutcomeStatus::ReadyToAccept
        );
        assert_eq!(dry_run.summary.auto_accepted_count, 1);
        assert_eq!(dry_run.summary.empty_count, 0);
        assert_eq!(
            dry_run.summary.usage.input_tokens,
            dry_run.summary.input_tokens
        );
        assert_eq!(
            dry_run
                .summary
                .estimated_full_run
                .as_ref()
                .expect("full Run estimate")
                .image_count,
            1
        );
        assert!(dry_run.samples[0].nodes.iter().any(|node| {
            node.node_id.ends_with("classifier")
                && node.output_types.contains(&ArtifactKind::ClassificationSet)
        }));
        assert!(
            application
                .list_runs()
                .expect("no formal Dry Run")
                .is_empty()
        );
        let published = application
            .publish_workflow(&saved.id, &settings)
            .expect("publish validated Label Pipeline");
        assert_eq!(published.version, 1);
        assert!(published.draft.label_pipeline.is_some());
        assert!(application.save_workflow_draft(published.draft).is_err());

        let mut invalid = application
            .suggest_label_pipeline(
                "label-advisor",
                &settings,
                "scene",
                "night",
                &WorkflowConstraints::default(),
            )
            .expect("second Draft")
            .draft;
        invalid
            .label_pipeline
            .as_mut()
            .expect("composition")
            .label_pipelines[0]
            .steps[0]
            .model_binding
            .as_mut()
            .expect("model binding")
            .model_id = "not-in-registry".to_owned();
        let invalid = application
            .save_workflow_draft(invalid)
            .expect("invalid Draft remains editable");
        let invalid_report = application
            .dry_run_workflow(&invalid.id, &settings)
            .expect("static report");
        assert!(!invalid_report.valid);
        assert!(invalid_report.issues.iter().any(|issue| {
            issue.code == "unknown_model" && issue.path.starts_with("label_pipeline.")
        }));
        assert!(
            application
                .publish_workflow(&invalid.id, &settings)
                .is_err()
        );
    }

    #[tokio::test]
    async fn iterative_advisor_revises_invalid_draft_and_stops_for_publish_approval() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("advisor-agent", GENERIC_CLASSIFICATION_PROJECT)
            .expect("classification Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary.path().join("advisor-agent/images/sample.png"),
        )
        .expect("sample image");
        let settings = load_settings(None).expect("settings");
        let report = application
            .run_workflow_advisor_agent(
                "advisor-agent",
                &settings,
                &WorkflowConstraints::default(),
                Some(("scene", "day")),
                PipelineBuilderConstraints::default(),
                CancellationToken::new(),
            )
            .await
            .expect("Advisor Agent");
        assert_eq!(report.session.status, AgentSessionStatus::WaitingForHuman);
        assert_eq!(report.session.kind, AgentKind::PipelineBuilder);
        assert!(report.approval_required);
        assert!(report.validation.as_ref().is_some_and(|value| value.valid));
        assert!(report.dry_run.as_ref().is_some_and(|value| value.sandbox));
        let tools = report
            .session
            .steps
            .iter()
            .map(|step| step.tool_name.as_str())
            .collect::<Vec<_>>();
        assert!(tools.starts_with(&[
            "inspect_project",
            "list_enabled_skills",
            "list_available_capabilities",
            "list_available_models",
            "list_pipeline_templates",
            "create_draft_from_template",
            "disconnect_pipeline_nodes",
            "validate_pipeline",
            "connect_pipeline_nodes",
            "validate_pipeline",
            "dry_run_pipeline",
            "inspect_dry_run_summary",
            "submit_draft_for_human_approval",
        ]));
        assert!(report.session.steps.iter().all(|step| {
            PipelineBuilderToolRegistry.resolve(&step.tool_name).is_ok()
                && step.result.get("display_summary").is_some()
                && step.result.get("model_payload").is_some()
        }));
        let validation_outcomes = report
            .session
            .steps
            .iter()
            .filter(|step| step.tool_name == "validate_pipeline")
            .filter_map(|step| step.result["model_payload"]["valid"].as_bool())
            .collect::<Vec<_>>();
        assert_eq!(validation_outcomes, vec![false, true]);
        assert_eq!(
            application
                .store
                .list_published_workflow_versions(Some("advisor-agent"))
                .expect("published versions")
                .len(),
            0,
            "Advisor must never auto-publish"
        );
        assert_eq!(
            application
                .store
                .list_agent_sessions(Some("advisor-agent"))
                .expect("persisted Advisor sessions")
                .first()
                .map(|session| session.id),
            Some(report.session.id)
        );

        let mut dry_run_revision = report.suggestion.clone().expect("Advisor suggestion");
        let (model_node_id, previous_attempts) = dry_run_revision
            .draft
            .nodes
            .iter()
            .find(|node| {
                matches!(
                    node.kind,
                    WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
                )
            })
            .map(|node| (node.id.clone(), node.retry_policy.max_attempts))
            .expect("model node");
        assert!(revise_draft_after_failed_dry_run(&mut dry_run_revision, 2));
        assert_eq!(dry_run_revision.draft.status, WorkflowDraftStatus::Editing);
        assert_eq!(
            dry_run_revision
                .draft
                .nodes
                .iter()
                .find(|node| node.id == model_node_id)
                .map(|node| node.retry_policy.max_attempts),
            Some(previous_attempts.max(1).saturating_add(1).min(3))
        );
        assert!(
            dry_run_revision
                .warnings
                .iter()
                .any(|warning| warning.contains("failed sample") && warning.contains("human"))
        );

        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let cancelled_report = application
            .run_workflow_advisor_agent(
                "advisor-agent",
                &settings,
                &WorkflowConstraints::default(),
                None,
                PipelineBuilderConstraints::default(),
                cancelled,
            )
            .await
            .expect("cancelled Advisor report");
        assert_eq!(
            cancelled_report.session.status,
            AgentSessionStatus::Cancelled
        );
    }

    #[test]
    fn workflow_draft_diff_applies_selected_changes_and_undo_uses_the_saved_snapshot() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("draft-diff", GENERIC_CLASSIFICATION_PROJECT)
            .expect("classification Project");
        let settings = load_settings(None).expect("settings");
        let base = application
            .create_workflow_draft("draft-diff", &settings, true)
            .expect("base Draft");
        let mut proposed = base.clone();
        proposed.id = uuid::Uuid::new_v4().to_string();
        proposed.name = "Agent proposal".to_owned();
        proposed.status = WorkflowDraftStatus::Suggested;
        proposed.nodes[0]
            .parameters
            .insert("agent_selected".to_owned(), json!(true));
        application
            .store
            .save_workflow_draft(&proposed)
            .expect("proposal Draft");

        let diff = application
            .diff_workflow_drafts(&base.id, &proposed.id)
            .expect("structured Draft Diff");
        let parameter_change = diff
            .modified_nodes
            .iter()
            .find(|change| change.change_id.starts_with("node:parameters:"))
            .expect("parameter change")
            .change_id
            .clone();
        let report = application
            .apply_workflow_draft_diff(
                &base.id,
                &proposed.id,
                std::slice::from_ref(&parameter_change),
            )
            .expect("selected apply");

        assert_eq!(report.draft.id, base.id);
        assert_eq!(report.previous_draft, base);
        assert_eq!(report.selected_change_ids, vec![parameter_change]);
        assert_eq!(
            report.draft.nodes[0].parameters["agent_selected"],
            json!(true)
        );
        assert_eq!(
            application
                .store
                .get_workflow_draft(&proposed.id)
                .expect("proposal remains auditable")
                .status,
            WorkflowDraftStatus::Suggested
        );

        let restored = application
            .save_workflow_draft(report.previous_draft)
            .expect("Undo through the normal Draft save boundary");
        assert!(!restored.nodes[0].parameters.contains_key("agent_selected"));
        assert!(
            application
                .store
                .list_published_workflow_versions(Some("draft-diff"))
                .expect("published versions")
                .is_empty(),
            "Apply and Undo must never publish"
        );
        assert!(application.list_runs().expect("formal Runs").is_empty());
    }

    #[tokio::test]
    async fn advisor_uses_real_dry_run_evidence_to_add_crop_verification() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("review-revision", HIGH_REVIEW_BBOX_PROJECT)
            .expect("detection Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary.path().join("review-revision/images/sample.png"),
        )
        .expect("sample image");
        let settings = load_settings(None).expect("settings");
        let report = application
            .run_workflow_advisor_agent(
                "review-revision",
                &settings,
                &WorkflowConstraints::default(),
                Some(("components", "component")),
                PipelineBuilderConstraints::default(),
                CancellationToken::new(),
            )
            .await
            .expect("Advisor revision loop");

        assert_eq!(report.session.status, AgentSessionStatus::WaitingForHuman);
        assert!(report.approval_required);
        assert!(report.validation.as_ref().is_some_and(|value| value.valid));
        assert!(report.dry_run.as_ref().is_some_and(|value| {
            value.sandbox
                && value.summary.auto_accepted_count == 1
                && value.summary.needs_review_count == 0
        }));
        assert_eq!(
            report
                .session
                .steps
                .iter()
                .map(|step| step.tool_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "inspect_project",
                "list_enabled_skills",
                "list_available_capabilities",
                "list_available_models",
                "list_pipeline_templates",
                "create_draft_from_template",
                "disconnect_pipeline_nodes",
                "validate_pipeline",
                "connect_pipeline_nodes",
                "validate_pipeline",
                "dry_run_pipeline",
                "inspect_dry_run_summary",
                "add_pipeline_node",
                "validate_pipeline",
                "dry_run_pipeline",
                "submit_draft_for_human_approval",
            ]
        );
        let validations = report
            .session
            .steps
            .iter()
            .filter(|step| step.tool_name == "validate_pipeline")
            .filter_map(|step| step.result["model_payload"]["valid"].as_bool())
            .collect::<Vec<_>>();
        assert_eq!(validations, vec![false, true, true]);
        let dry_runs = report
            .session
            .steps
            .iter()
            .filter(|step| step.tool_name == "dry_run_pipeline")
            .collect::<Vec<_>>();
        assert_eq!(dry_runs.len(), 2);
        assert_eq!(
            dry_runs[0].result["model_payload"]["summary"]["review_count"],
            json!(1)
        );
        assert_eq!(
            dry_runs[1].result["model_payload"]["summary"]["review_count"],
            json!(0)
        );

        let suggestion = report.suggestion.expect("revised Draft");
        let composition = suggestion.draft.label_pipeline.expect("Label Pipeline");
        let steps = &composition.label_pipelines[0].steps;
        assert!(
            steps
                .iter()
                .any(|step| step.node_type == annotagent_runtime::CORE_CROP)
        );
        assert!(steps.iter().any(|step| {
            step.node_type == annotagent_skill_classification::CLASSIFICATION_OPERATION
                && step.model_binding.as_ref().is_some_and(|binding| {
                    binding.capability == VisionCapability::Classification
                        && !binding.model_id.is_empty()
                })
        }));
        assert!(
            suggestion
                .rationale
                .iter()
                .any(|reason| reason.contains("1 of 1") && reason.contains("100%"))
        );
        assert!(
            application
                .store
                .list_published_workflow_versions(Some("review-revision"))
                .expect("published versions")
                .is_empty()
        );
        assert!(application.list_runs().expect("formal Runs").is_empty());
    }

    #[tokio::test]
    async fn live_pipeline_builder_uses_multi_turn_tool_results_and_never_publishes() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("live-builder", GENERIC_CLASSIFICATION_PROJECT)
            .expect("classification Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary.path().join("live-builder/images/sample.png"),
        )
        .expect("sample image");
        let settings = load_settings(None).expect("settings");
        let constraints = WorkflowConstraints::default();
        let input = application
            .workflow_advisor_input_for_label(
                "live-builder",
                &settings,
                constraints.clone(),
                Some("scene"),
                Some("day"),
            )
            .expect("Advisor input");
        let safe = application
            .suggest_label_pipeline_preview("live-builder", &settings, "scene", "day", &constraints)
            .expect("safe Draft");
        let scripted_step =
            |name: &str, arguments: serde_json::Value, expect_message_contains: Option<&str>| {
                MockStep {
                    expect_task: Some("pipeline_builder".to_owned()),
                    expect_message_contains: expect_message_contains.map(ToOwned::to_owned),
                    response: MockResponseSpec::ToolCall {
                        name: name.to_owned(),
                        arguments,
                    },
                    usage: MockUsage {
                        input_tokens: 10,
                        output_tokens: 5,
                    },
                }
            };
        let provider = MockVisionProvider::new(MockScript {
            steps: vec![
                scripted_step("create_draft_from_template", json!({}), None),
                scripted_step("inspect_project", json!({}), Some("inspect Project")),
                scripted_step("inspect_label", json!({}), Some("project_id")),
                scripted_step("list_enabled_skills", json!({}), Some("declared")),
                scripted_step("list_available_nodes", json!({}), Some("skill_ids")),
                scripted_step("list_available_models", json!({}), Some("nodes")),
                scripted_step("create_draft_from_template", json!({}), Some("models")),
                scripted_step("validate_pipeline", json!({}), Some("draft_id")),
                scripted_step(
                    "dry_run_pipeline",
                    json!({"image_indices": [0]}),
                    Some("valid"),
                ),
                scripted_step("inspect_dry_run_summary", json!({}), Some("sandbox")),
                scripted_step(
                    "submit_draft_for_human_approval",
                    json!({
                        "name": "Day classification proposal",
                        "rationale": ["The registered mock classifier proves the offline path."],
                        "warnings": ["Replace Mock before claiming live inference."],
                        "alternatives": []
                    }),
                    Some("review_rate"),
                ),
            ],
        });

        let report = application
            .run_workflow_advisor_with_provider(
                "live-builder",
                &settings,
                &constraints,
                Some(("scene", "day")),
                input,
                safe,
                &provider,
                PipelineBuilderConstraints::default(),
                CancellationToken::new(),
            )
            .await
            .expect("multi-turn Pipeline Builder");
        assert_eq!(provider.remaining_steps(), 0);
        assert_eq!(report.session.status, AgentSessionStatus::WaitingForHuman);
        assert!(report.approval_required);
        assert!(report.validation.as_ref().is_some_and(|value| value.valid));
        assert!(report.dry_run.as_ref().is_some_and(|value| value.sandbox));
        assert_eq!(report.session.usage.input_tokens, 110);
        assert_eq!(report.session.usage.output_tokens, 55);
        assert!(!report.session.steps[0].success);
        assert!(
            report.session.steps[0].result["model_payload"]["error"]
                .as_str()
                .is_some_and(|error| error.contains("inspect Project"))
        );
        assert_eq!(
            report
                .session
                .steps
                .iter()
                .map(|step| step.tool_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "create_draft_from_template",
                "inspect_project",
                "inspect_label",
                "list_enabled_skills",
                "list_available_nodes",
                "list_available_models",
                "create_draft_from_template",
                "validate_pipeline",
                "dry_run_pipeline",
                "inspect_dry_run_summary",
                "submit_draft_for_human_approval",
            ]
        );
        assert_eq!(
            application
                .store
                .list_published_workflow_versions(Some("live-builder"))
                .expect("published versions")
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn live_pipeline_builder_repairs_then_revises_from_bounded_dry_run_feedback() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("live-revision", HIGH_REVIEW_BBOX_PROJECT)
            .expect("detection Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary.path().join("live-revision/images/sample.png"),
        )
        .expect("sample image");
        let settings = load_settings(None).expect("settings");
        let constraints = WorkflowConstraints::default();
        let input = application
            .workflow_advisor_input_for_label(
                "live-revision",
                &settings,
                constraints.clone(),
                Some("components"),
                Some("component"),
            )
            .expect("Advisor input");
        let safe = application
            .suggest_label_pipeline_preview(
                "live-revision",
                &settings,
                "components",
                "component",
                &constraints,
            )
            .expect("safe Draft");
        let scripted_step = |name: &str, arguments: serde_json::Value| MockStep {
            expect_task: Some("pipeline_builder".to_owned()),
            expect_message_contains: None,
            response: MockResponseSpec::ToolCall {
                name: name.to_owned(),
                arguments,
            },
            usage: MockUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
        };
        let gate = "components.component.confidence";
        let commit = "components.component.commit";
        let provider = MockVisionProvider::new(MockScript {
            steps: vec![
                scripted_step("inspect_project", json!({})),
                scripted_step("inspect_label", json!({})),
                scripted_step("list_enabled_skills", json!({})),
                scripted_step("list_available_nodes", json!({})),
                scripted_step("list_available_models", json!({})),
                scripted_step("create_draft_from_template", json!({})),
                scripted_step(
                    "disconnect_pipeline_nodes",
                    json!({"from_node": gate, "to_node": commit}),
                ),
                scripted_step("validate_pipeline", json!({})),
                scripted_step(
                    "connect_pipeline_nodes",
                    json!({
                        "from_node": gate,
                        "from_port": "candidates",
                        "to_node": commit,
                        "to_port": "candidates"
                    }),
                ),
                scripted_step("validate_pipeline", json!({})),
                scripted_step("dry_run_pipeline", json!({"image_indices": [0]})),
                scripted_step("inspect_dry_run_summary", json!({})),
                scripted_step("inspect_review_samples", json!({"limit": 1})),
                scripted_step(
                    "add_pipeline_node",
                    json!({"guided_template": "crop_verification"}),
                ),
                scripted_step("validate_pipeline", json!({})),
                scripted_step("dry_run_pipeline", json!({"image_indices": [0]})),
                scripted_step(
                    "submit_draft_for_human_approval",
                    json!({
                        "name": "Component crop verification proposal",
                        "rationale": ["The bounded Dry Run reported one Review candidate."],
                        "warnings": ["This remains an editable Draft."],
                        "alternatives": []
                    }),
                ),
            ],
        });
        let builder_constraints = PipelineBuilderConstraints {
            maximum_agent_turns: 20,
            maximum_tool_calls: 20,
            ..PipelineBuilderConstraints::default()
        };

        let report = application
            .run_workflow_advisor_with_provider(
                "live-revision",
                &settings,
                &constraints,
                Some(("components", "component")),
                input,
                safe,
                &provider,
                builder_constraints,
                CancellationToken::new(),
            )
            .await
            .expect("live revision loop");

        assert_eq!(provider.remaining_steps(), 0);
        assert_eq!(report.session.status, AgentSessionStatus::WaitingForHuman);
        assert!(report.approval_required);
        assert_eq!(report.session.usage.tool_calls, 17);
        assert_eq!(report.session.usage.input_tokens, 170);
        assert_eq!(report.session.usage.output_tokens, 85);
        assert_eq!(
            report
                .session
                .steps
                .iter()
                .filter(|step| step.tool_name == "validate_pipeline")
                .filter_map(|step| step.result["model_payload"]["valid"].as_bool())
                .collect::<Vec<_>>(),
            vec![false, true, true]
        );
        let review_inspection = report
            .session
            .steps
            .iter()
            .find(|step| step.tool_name == "inspect_review_samples")
            .expect("bounded Review sample inspection");
        assert_eq!(
            review_inspection.result["model_payload"]["sample_count"],
            json!(1)
        );
        assert!(
            review_inspection.result["model_payload"]["samples"][0]["outcomes"][0]
                .get("value")
                .is_none(),
            "the model must not receive full Artifact bodies"
        );
        let suggestion = report.suggestion.expect("revised suggestion");
        assert!(
            suggestion
                .rationale
                .iter()
                .any(|reason| reason.contains("1 of 1") && reason.contains("100%"))
        );
        assert!(suggestion.draft.nodes.iter().any(|node| {
            node.node_type == annotagent_runtime::CORE_CROP && node.id.contains("crop_verify")
        }));
        assert!(
            application
                .store
                .list_published_workflow_versions(Some("live-revision"))
                .expect("published versions")
                .is_empty()
        );
        assert!(application.list_runs().expect("formal Runs").is_empty());
    }

    #[tokio::test]
    async fn recovery_uses_scoped_memory_persists_trace_and_keeps_clean_fast_path() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "recovery-agent",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("RoboCup Project");
        let project_root = temporary
            .path()
            .join("recovery-agent")
            .canonicalize()
            .expect("canonical Project root");
        let project_scope = stable_project_id(&project_root);
        let candidate = Annotation {
            id: annotagent_core::AnnotationId::new(),
            image_id: ImageId::new(),
            task_id: TaskId::from("objects"),
            label: Some(LabelId::from("ball")),
            value: annotagent_core::AnnotationValue::BoundingBox {
                rect: annotagent_core::NormalizedRect::new(0.2, 0.6, 0.04, 0.03)
                    .expect("valid bbox"),
            },
            attributes: BTreeMap::new(),
            confidence: Some(0.91),
            source: AnnotationSource::Model,
            review_status: ReviewStatus::Draft,
            provenance: annotagent_core::AnnotationProvenance::default(),
            created_at: chrono::Utc::now(),
        };
        let fast = application
            .recover_ball_candidate(
                "recovery-agent",
                BallRecoveryInput {
                    candidate: candidate.clone(),
                    related_annotations: Vec::new(),
                    issues: Vec::new(),
                    image_path: None,
                    budget: AgentBudget::default(),
                    cancellation: CancellationToken::new(),
                },
            )
            .await
            .expect("clean fast path");
        assert!(fast.fast_path && fast.session.is_none());
        assert!(
            application
                .store
                .list_agent_sessions(Some("recovery-agent"))
                .expect("sessions")
                .is_empty()
        );

        let issue = annotagent_core::ValidationIssue {
            code: "inaccurate_ball_bbox".to_owned(),
            severity: annotagent_core::IssueSeverity::Warning,
            annotation_ids: vec![candidate.id],
            message: "candidate needs bounded recovery".to_owned(),
            suggested_action: annotagent_core::SuggestedAction::HumanReview,
            evidence: annotagent_core::ValidationEvidence::Rule {
                facts: BTreeMap::new(),
            },
        };
        let first = application
            .recover_ball_candidate(
                "recovery-agent",
                BallRecoveryInput {
                    candidate: candidate.clone(),
                    related_annotations: Vec::new(),
                    issues: vec![issue.clone()],
                    image_path: None,
                    budget: AgentBudget::default(),
                    cancellation: CancellationToken::new(),
                },
            )
            .await
            .expect("first risky decision");
        assert_eq!(
            first.disposition,
            annotagent_skill_robocup::RecoveryDisposition::HumanReview
        );

        application
            .store
            .save_correction(&annotagent_core::CorrectionRecord {
                id: uuid::Uuid::new_v4(),
                project_id: project_scope,
                skill_id: ROBOCUP_BALL_SKILL_ID.to_owned(),
                task_id: candidate.task_id.clone(),
                predicted_label: candidate.label.clone(),
                corrected_label: None,
                reason_code: "white_shoe_as_ball".to_owned(),
                original_annotation: Some(candidate.snapshot()),
                corrected_annotation: None,
                note: Some("structured operator evidence, never a system instruction".to_owned()),
                image_features: annotagent_core::CorrectionFeatures {
                    geometry: BTreeMap::new(),
                    colors: BTreeMap::new(),
                },
                created_at: chrono::Utc::now(),
            })
            .expect("correction memory");
        let second = application
            .recover_ball_candidate(
                "recovery-agent",
                BallRecoveryInput {
                    candidate,
                    related_annotations: Vec::new(),
                    issues: vec![issue],
                    image_path: None,
                    budget: AgentBudget::default(),
                    cancellation: CancellationToken::new(),
                },
            )
            .await
            .expect("memory-guided decision");
        assert_eq!(
            second.disposition,
            annotagent_skill_robocup::RecoveryDisposition::Reject
        );
        assert!(second.memory_changed_decision);
        let sessions = application
            .store
            .list_agent_sessions(Some("recovery-agent"))
            .expect("persisted Recovery sessions");
        assert_eq!(sessions.len(), 2);
        assert!(
            sessions[0]
                .steps
                .iter()
                .any(|step| step.tool_name == "query_correction_memory")
        );
        assert!(
            !serde_json::to_string(&sessions)
                .expect("session JSON")
                .contains("system instruction")
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
    fn image_import_reports_quality_and_supports_removing_project_copy() {
        let temp = tempfile::tempdir().expect("temp");
        let workspace = temp.path().join("workspace");
        let app = LocalApplication::new(&workspace).expect("app");
        app.create_project("generic", GENERIC_PROJECT)
            .expect("project");
        let incoming = workspace.join("incoming");
        std::fs::create_dir_all(&incoming).expect("incoming");
        annotagent_image_tools::generate_synthetic_inspection(&incoming.join("valid.png"))
            .expect("valid image");
        std::fs::copy(incoming.join("valid.png"), incoming.join("duplicate.png"))
            .expect("duplicate");
        std::fs::write(incoming.join("corrupt.png"), b"not an image").expect("corrupt fixture");
        std::fs::write(incoming.join("notes.txt"), b"ignore me").expect("unsupported fixture");

        let report = app
            .import_images_with_report("generic", &incoming)
            .expect("import report");
        assert_eq!(report.discovered, 3);
        assert_eq!(report.imported, 1);
        assert_eq!(report.duplicates, 1);
        assert_eq!(report.corrupt.len(), 1);
        assert_eq!(report.unsupported_files, 1);
        assert_eq!(app.list_project_images("generic").expect("images").len(), 1);

        let removed = app
            .remove_project_image("generic", 0)
            .expect("remove image");
        assert!(
            Path::new(&removed)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        );
        assert!(
            app.list_project_images("generic")
                .expect("images")
                .is_empty()
        );
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

        assert_eq!(summary.name, "RoboCup Ball Demo");
        assert_eq!(summary.annotation_schema.len(), 1);
        assert_eq!(summary.annotation_schema[0].id, "objects");
        assert_eq!(summary.annotation_schema[0].labels, ["ball"]);
        assert_eq!(summary.readiness, ProjectReadiness::Incomplete);
        assert_eq!(summary.task_count, summary.annotation_schema.len());
        assert_eq!(summary.review_count, 0);
        assert!(
            summary
                .blocking_issues
                .iter()
                .any(|issue| issue.code == "no_images" && issue.next_step == "data")
        );
        assert!(summary.default_workflow_version.is_some());
        assert_eq!(summary.enabled_skills.len(), 1);
        assert_eq!(summary.enabled_skills[0].id, "robocup");
        assert_eq!(summary.enabled_skills[0].display_name, "RoboCup Ball");
        assert_eq!(summary.active_workflow.name, "Configured task graph");
        assert_eq!(summary.active_workflow.status, WorkflowStatus::Published);
        assert_eq!(
            summary.workflows[0].node_count,
            summary.annotation_schema.len()
        );
        assert_eq!(summary.active_workflow.nodes.len(), 1);
        assert_eq!(summary.active_workflow.nodes[0].id, "objects");
        assert!(summary.model_bindings.is_empty());
        let settings = load_settings(None).expect("settings");
        let catalog = application
            .workflow_advisor_input("robocup-demo", &settings, WorkflowConstraints::default())
            .expect("RoboCup catalog");
        assert_eq!(catalog.workflow_templates.len(), 1);
        let draft = application
            .create_workflow_draft_with_template(
                "robocup-demo",
                &settings,
                false,
                Some("robocup.ball.vlm-bootstrap"),
            )
            .expect("Ball draft");
        assert_eq!(draft.name, "RoboCup Ball · VLM bootstrap");
        assert_eq!(draft.enabled_skills.get("robocup"), Some(&"1".to_owned()));
        let updated = application
            .add_project_task(
                "robocup-demo",
                "Quality Check",
                TaskKind::Classification,
                vec!["usable".to_owned(), "reject".to_owned()],
                BTreeMap::new(),
            )
            .expect("guided Label group");
        assert!(
            updated
                .annotation_schema
                .iter()
                .any(|task| { task.id == "quality_check" && task.display_name == "Quality Check" })
        );
    }

    #[tokio::test]
    async fn robocup_pipeline_builder_loads_domain_advice_and_keeps_the_default_flow_lean() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "robocup-lean",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("RoboCup Ball Project");
        generate_synthetic_robocup(&temporary.path().join("robocup-lean/images/synthetic.png"))
            .expect("synthetic image");
        let settings = load_settings(None).expect("settings");
        let report = application
            .run_workflow_advisor_agent(
                "robocup-lean",
                &settings,
                &WorkflowConstraints::default(),
                Some(("objects", "ball")),
                PipelineBuilderConstraints {
                    target_review_rate: Some(1.0),
                    ..PipelineBuilderConstraints::default()
                },
                CancellationToken::new(),
            )
            .await
            .expect("RoboCup Pipeline Builder");

        assert_eq!(report.session.status, AgentSessionStatus::WaitingForHuman);
        let tools = report
            .session
            .steps
            .iter()
            .map(|step| step.tool_name.as_str())
            .collect::<Vec<_>>();
        let resource_index = tools
            .iter()
            .position(|tool| *tool == "load_skill_resource")
            .expect("Domain Advisor resource Tool");
        let create_index = tools
            .iter()
            .position(|tool| *tool == "create_draft_from_template")
            .expect("Draft creation Tool");
        assert!(resource_index < create_index);
        let resource_step = &report.session.steps[resource_index];
        assert_eq!(
            resource_step.result["model_payload"]["resource_id"],
            json!("resources/advisor.md")
        );
        assert!(
            resource_step.result["model_payload"]["resources"][0]["content"]
                .as_str()
                .is_some_and(|content| content.contains("smallest Pipeline"))
        );

        let suggestion = report.suggestion.expect("lean RoboCup Draft");
        let model_nodes = suggestion
            .draft
            .nodes
            .iter()
            .filter(|node| node.model_binding.is_some())
            .collect::<Vec<_>>();
        assert_eq!(model_nodes.len(), 1);
        assert!(suggestion.draft.nodes.iter().all(|node| {
            !node.node_type.contains("sam")
                && !node.node_type.contains("recovery")
                && node.node_type != annotagent_runtime::CORE_CROP
        }));
        let selection = suggestion
            .draft
            .nodes
            .iter()
            .find(|node| node.node_type == annotagent_runtime::CORE_FILTER)
            .expect("Select football candidates");
        assert_eq!(
            selection.validators,
            ["ball_hard_negative", "robocup_ball_field_relation"]
        );
        assert!(
            application
                .store
                .list_published_workflow_versions(Some("robocup-lean"))
                .expect("published versions")
                .is_empty()
        );
        assert!(application.list_runs().expect("formal Runs").is_empty());
    }

    #[test]
    fn robocup_advisor_uses_a_ready_workspace_vlm_and_rejects_unready_labs_models() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "robocup-model-choice",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("RoboCup Ball Project");
        let mut settings = load_settings(None).expect("settings");
        let ready = application
            .suggest_label_pipeline_preview(
                "robocup-model-choice",
                &settings,
                "objects",
                "ball",
                &WorkflowConstraints {
                    preferred_model_id: Some("default-vision".to_owned()),
                    ..WorkflowConstraints::default()
                },
            )
            .expect("ready workspace VLM Draft");
        let detector = ready
            .draft
            .nodes
            .iter()
            .find(|node| node.model_binding.as_deref() == Some("default-vision"))
            .expect("workspace VLM binding");
        assert_eq!(
            detector.node_type,
            annotagent_skill_vlm_detection::VLM_DETECTION_OPERATION
        );
        assert!(
            ready
                .draft
                .nodes
                .iter()
                .all(|node| node.node_type != annotagent_skill_yolo::YOLO_DETECTION_OPERATION)
        );

        settings.detection_workers[0].enabled = true;
        let labs_model = settings.detection_workers[0].model_id.clone();
        let error = application
            .suggest_label_pipeline_preview(
                "robocup-model-choice",
                &settings,
                "objects",
                "ball",
                &WorkflowConstraints {
                    preferred_model_id: Some(labs_model.clone()),
                    ..WorkflowConstraints::default()
                },
            )
            .expect_err("unverified Labs Worker must not be recommended");
        assert!(error.to_string().contains("not ready"));

        let input = application
            .workflow_advisor_input_for_label(
                "robocup-model-choice",
                &settings,
                WorkflowConstraints::default(),
                Some("objects"),
                Some("ball"),
            )
            .expect("Advisor input");
        let tools = pipeline_builder_live_tools(&input, &ready);
        assert!(tools.iter().any(|tool| tool.name == "load_skill_resource"));
        assert!(
            input
                .model_registry
                .iter()
                .find(|model| model.id == labs_model)
                .is_some_and(|model| model.status != ModelAvailabilityStatus::Available)
        );
    }

    #[tokio::test]
    async fn robocup_hybrid_template_resolves_capabilities_and_runs_fast_path_offline() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "robocup-hybrid-mock",
                include_str!("../../../examples/robocup-ball-hybrid-mock/project.yaml"),
            )
            .expect("hybrid Project");
        let settings = load_settings(None).expect("settings");
        let mut draft = application
            .create_workflow_draft_with_template(
                "robocup-hybrid-mock",
                &settings,
                false,
                Some("robocup.ball.specialist_with_open_vocab_fallback"),
            )
            .expect("hybrid Draft");
        let binding = |node_id: &str| {
            draft
                .nodes
                .iter()
                .find(|node| node.id == node_id)
                .and_then(|node| node.model_binding.as_deref())
        };
        assert_eq!(binding("specialist"), Some("mock-object-detector"));
        assert_eq!(binding("recovery"), Some("mock-open-vocabulary"));
        assert_eq!(binding("classify_crop"), Some("mock-classifier"));
        let specialist = draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "specialist")
            .expect("specialist node");
        specialist
            .parameters
            .insert("mock_confidence".to_owned(), json!(0.92));
        specialist
            .parameters
            .insert("mock_bbox".to_owned(), json!([0.55, 0.72, 0.04, 0.04]));
        application
            .save_workflow_draft(draft.clone())
            .expect("save deterministic fixture");
        let report = application
            .dry_run_workflow(&draft.id, &settings)
            .expect("static validation");
        assert!(report.valid, "{:#?}", report.issues);
        generate_synthetic_robocup(
            &temporary
                .path()
                .join("robocup-hybrid-mock/images/sample.png"),
        )
        .expect("sample image");
        let published = application
            .publish_workflow(&draft.id, &settings)
            .expect("publish hybrid Workflow");
        let started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("robocup-hybrid-mock/project.yaml"),
                "mock",
                settings,
                None,
                Some("hybrid-fast-path"),
                Some((&published.workflow_id, published.version)),
            )
            .expect("start hybrid Run");
        let run = application
            .wait_run(started.run_id)
            .await
            .expect("hybrid Run");
        assert_eq!(run.status, RunStatus::Completed, "{:#?}", run.issues);
        let inspection = application
            .inspect_run_pipeline_artifacts(started.run_id)
            .expect("pipeline inspection");
        assert!(
            inspection
                .nodes
                .iter()
                .any(|node| node.node_id == "commit_evidence")
        );
        assert!(
            !inspection
                .nodes
                .iter()
                .any(|node| node.node_id == "classify_crop"),
            "{:#?}",
            inspection.nodes
        );
        let replay = application
            .replay_run_from_node(
                started.run_id,
                "commit_evidence",
                &load_settings(None).expect("Replay settings"),
            )
            .await
            .expect("hybrid Commit Replay");
        assert!(replay.sandbox);
        assert_eq!(replay.reexecuted_nodes, ["commit_evidence"]);
        for upstream in ["specialist", "validate_primary", "recovery"] {
            assert!(
                replay
                    .preserved_upstream_nodes
                    .contains(&upstream.to_owned()),
                "Replay did not preserve {upstream}: {:#?}",
                replay.preserved_upstream_nodes
            );
        }
        assert_eq!(
            application
                .store
                .history(started.run_id)
                .expect("history after sandbox Replay")
                .annotations
                .len(),
            1,
            "sandbox Replay must not duplicate a committed Annotation"
        );

        let mut fallback_draft = application
            .create_workflow_draft_with_template(
                "robocup-hybrid-mock",
                &load_settings(None).expect("fallback settings"),
                false,
                Some("robocup.ball.specialist_with_open_vocab_fallback"),
            )
            .expect("fallback Draft");
        fallback_draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "specialist")
            .expect("specialist")
            .parameters
            .insert("mock_empty".to_owned(), json!(true));
        fallback_draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "classify_crop")
            .expect("classifier")
            .parameters
            .insert("mock_label".to_owned(), json!("football"));
        application
            .save_workflow_draft(fallback_draft.clone())
            .expect("save fallback fixture");
        let fallback_settings = load_settings(None).expect("fallback settings");
        let fallback_published = application
            .publish_workflow(&fallback_draft.id, &fallback_settings)
            .expect("publish fallback Workflow");
        let fallback_started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("robocup-hybrid-mock/project.yaml"),
                "mock",
                fallback_settings,
                None,
                Some("hybrid-fallback-path"),
                Some((&fallback_published.workflow_id, fallback_published.version)),
            )
            .expect("start fallback Run");
        let fallback_run = application
            .wait_run(fallback_started.run_id)
            .await
            .expect("fallback Run");
        assert_eq!(
            fallback_run.status,
            RunStatus::CompletedWithReview,
            "{:#?}",
            fallback_run.issues
        );
        let fallback_inspection = application
            .inspect_run_pipeline_artifacts(fallback_started.run_id)
            .expect("fallback pipeline inspection");
        for expected in [
            "recovery",
            "project_candidates",
            "crop_verify",
            "classify_crop",
            "review_verified",
        ] {
            assert!(
                fallback_inspection
                    .nodes
                    .iter()
                    .any(|node| node.node_id == expected),
                "missing {expected}: {:#?}",
                fallback_inspection.nodes
            );
        }

        let mut reject_draft = application
            .create_workflow_draft_with_template(
                "robocup-hybrid-mock",
                &load_settings(None).expect("reject settings"),
                false,
                Some("robocup.ball.specialist_with_open_vocab_fallback"),
            )
            .expect("reject Draft");
        reject_draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "specialist")
            .expect("specialist")
            .parameters
            .insert("mock_empty".to_owned(), json!(true));
        reject_draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "classify_crop")
            .expect("classifier")
            .parameters
            .insert("mock_label".to_owned(), json!("not_football"));
        application
            .save_workflow_draft(reject_draft.clone())
            .expect("save reject fixture");
        let reject_settings = load_settings(None).expect("reject settings");
        let reject_published = application
            .publish_workflow(&reject_draft.id, &reject_settings)
            .expect("publish reject Workflow");
        let reject_started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("robocup-hybrid-mock/project.yaml"),
                "mock",
                reject_settings,
                None,
                Some("hybrid-hard-negative-path"),
                Some((&reject_published.workflow_id, reject_published.version)),
            )
            .expect("start reject Run");
        let reject_run = application
            .wait_run(reject_started.run_id)
            .await
            .expect("reject Run");
        assert_ne!(
            reject_run.status,
            RunStatus::Failed,
            "{:#?}",
            reject_run.issues
        );
        let reject_inspection = application
            .inspect_run_pipeline_artifacts(reject_started.run_id)
            .expect("reject pipeline inspection");
        let rejected = reject_inspection
            .nodes
            .iter()
            .find(|node| node.node_id == "reject_hard_negative")
            .expect("explicit reject node");
        assert_eq!(rejected.metadata.get("decision"), Some(&json!("reject")));
        assert!(!rejected.outputs.is_empty());
        assert!(
            !reject_inspection
                .nodes
                .iter()
                .any(|node| node.node_id.starts_with("commit_")),
            "{:#?}",
            reject_inspection.nodes
        );

        let mut crashed_draft = application
            .create_workflow_draft_with_template(
                "robocup-hybrid-mock",
                &load_settings(None).expect("crash settings"),
                false,
                Some("robocup.ball.specialist_with_open_vocab_fallback"),
            )
            .expect("crash Draft");
        crashed_draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "specialist")
            .expect("specialist")
            .parameters
            .insert("mock_backend_error".to_owned(), json!(true));
        crashed_draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "classify_crop")
            .expect("classifier")
            .parameters
            .insert("mock_label".to_owned(), json!("football"));
        application
            .save_workflow_draft(crashed_draft.clone())
            .expect("save crash fixture");
        let crashed_settings = load_settings(None).expect("crash settings");
        let crashed_published = application
            .publish_workflow(&crashed_draft.id, &crashed_settings)
            .expect("publish crash Workflow");
        let crashed_started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("robocup-hybrid-mock/project.yaml"),
                "mock",
                crashed_settings,
                None,
                Some("hybrid-specialist-crash"),
                Some((&crashed_published.workflow_id, crashed_published.version)),
            )
            .expect("start crash Run");
        let crashed_run = application
            .wait_run(crashed_started.run_id)
            .await
            .expect("crash Run");
        assert_eq!(
            crashed_run.status,
            RunStatus::CompletedWithReview,
            "{:#?}",
            crashed_run.issues
        );
        let crashed_inspection = application
            .inspect_run_pipeline_artifacts(crashed_started.run_id)
            .expect("crash pipeline inspection");
        let specialist = crashed_inspection
            .nodes
            .iter()
            .find(|node| node.node_id == "specialist")
            .expect("specialist trace");
        assert_eq!(
            specialist
                .metadata
                .get("backend_error")
                .and_then(|error| error.get("code")),
            Some(&json!("detection_backend"))
        );
        for expected in ["recovery", "classify_crop", "review_verified"] {
            assert!(
                crashed_inspection
                    .nodes
                    .iter()
                    .any(|node| node.node_id == expected),
                "missing {expected}: {:#?}",
                crashed_inspection.nodes
            );
        }
    }

    #[tokio::test]
    async fn project_guidance_uses_persisted_sample_test_and_published_state() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let settings = load_settings(None).expect("settings");
        let draft_id = {
            let application = LocalApplication::new(temporary.path()).expect("application");
            application
                .create_project("guided", GENERIC_BBOX_PROJECT)
                .expect("Project");
            let guidance = application
                .project_guidance("guided", &settings, true)
                .expect("data guidance");
            assert_eq!(guidance.stage, ProjectStage::NeedsData);

            let image_root = temporary.path().join("guided/images");
            annotagent_image_tools::generate_synthetic_inspection(&image_root.join("sample.png"))
                .expect("sample image");
            let guidance = application
                .project_guidance("guided", &settings, true)
                .expect("automation guidance");
            assert_eq!(guidance.stage, ProjectStage::NeedsAutomation);

            let suggestion = application
                .suggest_workflow("guided", &settings, &WorkflowConstraints::default())
                .expect("Workflow Draft");
            let guidance = application
                .project_guidance("guided", &settings, true)
                .expect("test guidance");
            assert_eq!(guidance.stage, ProjectStage::ReadyForSampleTest);
            let report = application
                .dry_run_workflow_samples(&suggestion.draft.id, &settings, &[0])
                .await
                .expect("sample test");
            assert!(report.validation.valid);
            assert_eq!(report.summary.failed_count, 0);
            assert_eq!(report.summary.empty_count, 1);
            assert_eq!(report.summary.usage.estimated_cost, "0");
            assert_eq!(
                report
                    .summary
                    .estimated_full_run
                    .as_ref()
                    .expect("full Run estimate")
                    .image_count,
                1
            );
            assert_eq!(
                application
                    .project_guidance("guided", &settings, true)
                    .expect("activation guidance")
                    .stage,
                ProjectStage::ReadyToActivate
            );
            suggestion.draft.id
        };

        let restarted = LocalApplication::new(temporary.path()).expect("restarted application");
        assert_eq!(
            restarted
                .project_guidance("guided", &settings, true)
                .expect("restored guidance")
                .stage,
            ProjectStage::ReadyToActivate
        );
        restarted
            .publish_workflow(&draft_id, &settings)
            .expect("activate Automation");
        let summary = restarted
            .project_workspace_summary("guided", &settings, true)
            .expect("workspace summary");
        assert_eq!(summary.guidance.stage, ProjectStage::ReadyToRun);
        assert_eq!(summary.readiness.readiness, ProjectReadiness::Ready);
        assert_eq!(summary.guidance.primary_action.label, "Run dataset");
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
        let project_yaml = include_str!("../../../examples/robocup-ball-hybrid-mock/project.yaml");
        let app = Arc::new(LocalApplication::new(&workspace).expect("app"));
        app.create_project("batch-demo", project_yaml)
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
        let settings = load_settings(Some(&config_path)).expect("hybrid batch settings");
        let mut draft = app
            .create_workflow_draft_with_template(
                "batch-demo",
                &settings,
                false,
                Some("robocup.ball.specialist_with_open_vocab_fallback"),
            )
            .expect("hybrid batch Draft");
        draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "specialist")
            .expect("specialist")
            .parameters
            .insert("mock_confidence".to_owned(), json!(0.92));
        draft
            .nodes
            .iter_mut()
            .find(|node| node.id == "specialist")
            .expect("specialist")
            .parameters
            .insert("mock_bbox".to_owned(), json!([0.55, 0.72, 0.04, 0.04]));
        app.save_workflow_draft(draft.clone())
            .expect("save hybrid batch Draft");
        let published = app
            .publish_workflow(&draft.id, &settings)
            .expect("publish hybrid batch Workflow");
        let coordinator = DatasetCoordinator::new(app.as_ref());
        let batch = coordinator
            .create_with_workflow(
                &workspace.join("batch-demo/project.yaml"),
                "mock",
                Some(&config_path),
                None,
                Some((&published.workflow_id, published.version)),
            )
            .expect("batch");
        assert_eq!(batch.max_concurrency, 1);
        let task_app = app.clone();
        let batch_id = batch.id;
        let execution = tokio::spawn(async move {
            DatasetCoordinator::new(task_app.as_ref())
                .execute(batch_id, None)
                .await
        });
        let mut observed_progress = false;
        for _ in 0..1_000 {
            let images = app.store.list_batch_images(batch_id).expect("batch images");
            let completed = images
                .iter()
                .filter(|image| image.status == BatchImageStatus::Completed)
                .count();
            let running = images
                .iter()
                .any(|image| image.status == BatchImageStatus::Running);
            if running && completed < 100 {
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
        assert!(runs.iter().all(|run| {
            run.workflow_snapshot_json
                .as_deref()
                .is_some_and(|snapshot| {
                    snapshot.contains(&published.content_hash)
                        && snapshot.contains("published_dag_runtime")
                })
        }));
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
