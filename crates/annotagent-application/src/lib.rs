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
    time::{Duration, Instant},
};

use annotagent_core::{
    AdditionalUsage, AgentBudget, AgentDryRunSummary, AgentKind, AgentModelCall,
    AgentModelSelection, AgentSession, AgentSessionStatus, Annotation, AnnotationFailureClass,
    AnnotationSource, ArtifactContract, ArtifactKind, AttributeDefinition, AttributeValue,
    BatchBudgetLedger, BatchBudgetLimits, BatchId, BatchImageCheckpoint, BatchImageStatus,
    BatchNodeState, BatchProgress, BatchRecord, BatchStatus, BatchUsage, Budget,
    CapabilityDeclarationSource, CheckpointIdentity, ContractDataType, CredentialReference,
    CredentialSource, DatasetExporter, DatasetImporter, DomainSkill, EnabledSkillConfig,
    ExpertModelManifest, ExportReport, ExportRequest, FullRunEstimate, GenerationDefaults,
    GeometryQualityReport, ImageId, ImportIssue, ImportReport, ImportRequest, InputModality,
    LabelId, LabelPipeline, LabelPipelineStaticValidator, LabelWorkflowComposition,
    LicenseMetadata, LicensePermission, ModelAvailability, ModelAvailabilityEvidence,
    ModelAvailabilityStatus, ModelBinding as PipelineModelBinding, ModelBindingId,
    ModelBindingMatch, ModelBindingRole, ModelBindingSource, ModelCapability, ModelConnection,
    ModelInputContract, ModelLimits, ModelMessage, ModelOutputContract, ModelPricing, ModelProfile,
    ModelProfileId, ModelProfileSnapshot, ModelProfileStatus, ModelRegistry, ModelRequest,
    ModelRole, ModelVersionMetadata, NodeCardinality, NodeCategory, NodeDefinition, NodePort,
    NodeRegistry, NodeSideEffect, PipelineArtifact, PipelineBuilderConstraints,
    PipelineBuilderProviderProfile, PipelineBuilderTool, PipelineBuilderToolRegistry,
    PipelineDraftDiff, PipelineDraftHistory, PipelineDraftTools, PipelineGrammarValidator,
    PipelineSource, PipelineStep, PortCardinality, PortDefinition, PricingConfig, PricingSource,
    ProjectId, ProjectModelBinding, ProjectSchema, ProjectSnapshot, PromptContract, PromptKind,
    ProtocolFeatures, ProviderAdapterKind, ProviderConnectionPolicy, ProviderHealthSnapshot,
    ProviderHealthStatus, ProviderId, ProviderProfile, PublishedWorkflowVersion,
    RegistryWorkflowAdvisor, ResourceRequirements, RetryPolicy, ReviewGate, ReviewStatus, RunEvent,
    RunEventKind, RunEventPayload, RunId, RunStatus, RuntimePolicyDefinition, RuntimePolicyScope,
    RuntimeRequirements, SampleTestOutcome, SampleTestOutcomeStatus, SampleTestSummary,
    ScoreSemantics, SharedWorkflowStage, SkillResourceRequest, SnapshotImage, TaskConfig, TaskId,
    TaskKind, TaskRunStatus, TokenUsage, ToolDefinition, UsageSource, UsageSummary,
    VisionArtifactValue, VisionCapability, VisionInferenceRequest, VisionInputType,
    VisionModelDescriptor, VisionModelHealth, VisionModelHealthStatus, VisionModelLimits,
    VisionModelProvider, VisionNodeDescriptor, WORKFLOW_SCHEMA_VERSION, WorkflowAdvisor,
    WorkflowAdvisorAgentReport, WorkflowAdvisorInput, WorkflowConstraints, WorkflowDataProfile,
    WorkflowDraft, WorkflowDraftNode, WorkflowDraftStatus, WorkflowDryRunNodeResult,
    WorkflowDryRunReport, WorkflowDryRunSampleResult, WorkflowEdge, WorkflowNodeKind,
    WorkflowSnapshot, WorkflowStaticValidator, WorkflowSuggestion, WorkflowValidationIssue,
    WorkflowValidationReport, WorkflowVersionComparison, all_artifact_kinds, resolve_model_binding,
};
use annotagent_export::{
    CocoExporter, CocoImporter, LabelMeExporter, LabelMeImporter, NativeExporter, NativeImporter,
    YoloDetectionExporter, YoloDetectionImporter, YoloSegmentationExporter,
    YoloSegmentationImporter,
};
use annotagent_image_tools::{generate_synthetic_robocup, load_image, sha256, to_model_image};
use annotagent_provider::{
    HttpJsonVisionBackend, HttpJsonVisionBackendConfig, HttpVisionWorkerConfig, MockResponseSpec,
    MockScript, MockStep, MockUsage, MockVisionBackend, MockVisionProvider, OpenAiCompatibleConfig,
    OpenAiCompatibleProvider, OpenAiProtocol, OpenAiVisionBackend,
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
    BatchClaimResult, HistoryRun, LegacyRegistryImport, LegacyRegistryImportReport,
    RunStartReservation, SqliteStore, WorkflowSampleTest,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyRegistryImportPreview {
    pub fingerprint: String,
    pub provider_id: ProviderId,
    pub provider_display_name: String,
    pub provider_adapter: ProviderAdapterKind,
    pub endpoint_summary: String,
    pub model_profile_id: ModelProfileId,
    pub model_display_name: String,
    pub remote_model_id: String,
    pub capability_source: CapabilityDeclarationSource,
    pub credential_source: Option<CredentialSource>,
    pub project_binding_count: usize,
    pub already_applied: bool,
    pub moves_secret: bool,
    pub modifies_historical_runs: bool,
}

/// Registry-backed model and Provider selected to execute a Pipeline Builder session.
///
/// This value is intentionally not serializable: the persisted Agent session receives only the
/// credential-free [`AgentModelSelection`] projection.
#[derive(Clone)]
pub struct PipelineBuilderModelRuntime {
    pub provider: ProviderProfile,
    pub model: ModelProfile,
    pub binding_source: ModelBindingSource,
    pub locked: bool,
}

impl PipelineBuilderModelRuntime {
    #[must_use]
    pub fn safe_selection(&self) -> AgentModelSelection {
        AgentModelSelection {
            provider_profile_id: self.provider.id,
            provider_display_name: self.provider.display_name.clone(),
            provider_adapter: self.provider.adapter,
            endpoint_summary: self.provider.endpoint_summary(),
            model_profile_id: self.model.id,
            model_profile_revision: self.model.revision,
            model_display_name: self.model.display_name.clone(),
            remote_model_id: self.model.remote_model_id.clone(),
            binding_source: self.binding_source,
            locked: self.locked,
        }
    }

    pub fn openai_compatible_config(&self) -> Result<OpenAiCompatibleConfig> {
        if self.provider.adapter != ProviderAdapterKind::OpenAiCompatible {
            bail!("selected Pipeline Builder Provider is not OpenAI-compatible");
        }
        let maximum_output_tokens = self
            .model
            .generation_defaults
            .maximum_output_tokens
            .or(self.model.limits.maximum_output_tokens)
            .unwrap_or(4_096)
            .min(u64::from(u32::MAX)) as u32;
        let temperature = self
            .model
            .generation_defaults
            .temperature
            .map_or(0.0, |value| value.to_string().parse().unwrap_or(0.0));
        Ok(OpenAiCompatibleConfig {
            endpoint: self.provider.base_url.to_string(),
            api_key_env: "ANNOTAGENT_PIPELINE_BUILDER_API_KEY".to_owned(),
            model: self.model.remote_model_id.clone(),
            protocol: OpenAiProtocol::ChatCompletions,
            request_timeout_seconds: self.provider.connection_policy.request_timeout_seconds,
            max_output_tokens: maximum_output_tokens,
            temperature,
            reasoning_mode: self.model.generation_defaults.reasoning_mode.clone(),
            supports_tool_calls: self.model.protocol_features.tool_calls,
            supports_json_schema: self.model.protocol_features.structured_output
                || self.model.protocol_features.json_schema,
            custom_headers: self.provider.safe_headers.clone(),
            extra_request_fields: BTreeMap::new(),
            max_retries: self.provider.connection_policy.maximum_retries,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionWorkerSettings {
    pub id: String,
    pub display_name: String,
    pub model_id: String,
    pub base_url: String,
    /// Optional bearer credential locator. The Alpha accepts `env:VARIABLE_NAME` only; the
    /// resolved secret is never serialized into workspace Settings or model metadata.
    #[serde(default)]
    pub authentication_reference: Option<String>,
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
    /// Last actively observed state and the evidence that produced it. Registration never
    /// upgrades a Worker to `Available` from configuration alone.
    #[serde(default)]
    pub availability: ModelAvailability,
    #[serde(default)]
    pub availability_evidence: ModelAvailabilityEvidence,
}

impl DetectionWorkerSettings {
    pub fn authorization_header(&self) -> Result<Option<String>> {
        let Some(reference) = self
            .authentication_reference
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let name = reference.strip_prefix("env:").ok_or_else(|| {
            anyhow!("Vision Worker authentication reference must use env:VARIABLE_NAME")
        })?;
        if name.is_empty()
            || !name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            })
        {
            bail!("Vision Worker environment credential locator must be a valid variable name");
        }
        let secret = std::env::var(name).with_context(|| {
            format!("Vision Worker credential environment variable {name:?} is not set")
        })?;
        if secret.trim().is_empty() {
            bail!("Vision Worker credential environment variable {name:?} is empty");
        }
        Ok(Some(format!("Bearer {}", secret.trim())))
    }

    pub fn validate_authentication_reference(&self) -> Result<()> {
        let Some(reference) = self
            .authentication_reference
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };
        let name = reference.strip_prefix("env:").ok_or_else(|| {
            anyhow!("Vision Worker authentication reference must use env:VARIABLE_NAME")
        })?;
        if name.is_empty()
            || !name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            })
        {
            bail!("Vision Worker environment credential locator must be a valid variable name");
        }
        Ok(())
    }

    pub fn http_config(&self) -> Result<HttpVisionWorkerConfig> {
        Ok(HttpVisionWorkerConfig {
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
            authorization: self.authorization_header()?,
        })
    }

    #[must_use]
    fn has_fixed_label_space(&self) -> bool {
        self.expected_capabilities
            .contains(&VisionCapability::ObjectDetection)
            && !self
                .expected_capabilities
                .contains(&VisionCapability::OpenVocabularyDetection)
    }

    #[must_use]
    fn checkpoint_identity_complete(&self) -> bool {
        let base_identity_complete = self
            .version
            .architecture
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            && !matches!(
                self.version.model_version.trim(),
                "" | "unconfigured" | "local-unpinned"
            )
            && self
                .version
                .checkpoint_sha256
                .as_deref()
                .is_some_and(|sha256| {
                    sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            && self
                .license
                .weight_license
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
        base_identity_complete
            && (!self.has_fixed_label_space()
                || (self
                    .version
                    .training_dataset_version
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
                    && !self.label_space.is_empty()))
    }

    /// Projects a configured HTTP Worker into the same capability manifest used by newly
    /// scaffolded expert models. Configuration alone never counts as live health or a smoke test.
    pub fn expert_manifest(&self) -> Result<ExpertModelManifest> {
        let capabilities = self
            .expected_capabilities
            .iter()
            .filter_map(|capability| annotagent_core::model_capability(*capability))
            .collect::<BTreeSet<_>>();
        let supports_text_queries = capabilities
            .contains(&ModelCapability::OpenVocabularyDetection)
            || capabilities.contains(&ModelCapability::PhraseGrounding);
        let supports_prompts = capabilities.contains(&ModelCapability::PromptedSegmentation);

        let mut input_contracts = vec![ArtifactContract::artifact(
            "image",
            ArtifactKind::Image,
            true,
            false,
        )];
        let mut prompt_contracts = Vec::new();
        if supports_text_queries {
            input_contracts.push(ArtifactContract {
                name: "queries".to_owned(),
                data_type: ContractDataType::Text,
                required: true,
                multiple: true,
            });
            prompt_contracts.push(PromptContract {
                kind: PromptKind::Text,
                required: true,
                multiple: true,
            });
        }
        if supports_prompts {
            input_contracts.extend([
                ArtifactContract::artifact("box_prompts", ArtifactKind::BoxPromptSet, false, true),
                ArtifactContract::artifact(
                    "point_prompts",
                    ArtifactKind::PointPromptSet,
                    false,
                    true,
                ),
            ]);
            prompt_contracts.extend([
                PromptContract {
                    kind: PromptKind::Box,
                    required: false,
                    multiple: true,
                },
                PromptContract {
                    kind: PromptKind::Point,
                    required: false,
                    multiple: true,
                },
            ]);
        }
        let output_kind = if supports_prompts {
            ArtifactKind::MaskSet
        } else if capabilities.contains(&ModelCapability::SemanticSegmentation) {
            ArtifactKind::SemanticMask
        } else if capabilities.contains(&ModelCapability::ImageClassification) {
            ArtifactKind::ClassificationSet
        } else {
            ArtifactKind::DetectionSet
        };

        let weights_ready = if self.requires_checkpoint_metadata {
            self.checkpoint_identity_complete()
        } else {
            self.availability_evidence.weights_ready
        };
        let mut availability_evidence = self.availability_evidence.clone();
        availability_evidence.weights_ready = weights_ready;
        let authentication_ready =
            self.authentication_reference.is_none() || self.authorization_header().is_ok();
        if !authentication_ready {
            availability_evidence.health_passed = false;
            availability_evidence.detail = Some(
                "Configured Vision Worker authentication reference cannot be resolved".to_owned(),
            );
        } else if availability_evidence.detail.is_none() {
            availability_evidence.detail = Some(if !weights_ready {
                "Model weights or immutable model identity are not configured".to_owned()
            } else if !self.enabled {
                "Worker is disabled in workspace Settings".to_owned()
            } else {
                "Configured; run discovery and a sample conversion before publishing".to_owned()
            });
        }
        let availability = if !weights_ready {
            ModelAvailability::MissingWeights
        } else if !self.enabled {
            ModelAvailability::Disabled
        } else if !authentication_ready {
            ModelAvailability::Unknown
        } else if availability_evidence.available() {
            ModelAvailability::Available
        } else {
            match self.availability {
                ModelAvailability::Available
                | ModelAvailability::MissingWeights
                | ModelAvailability::Disabled => ModelAvailability::Unknown,
                observed => observed,
            }
        };
        let checkpoint = self
            .version
            .checkpoint_sha256
            .as_ref()
            .map(|sha256| CheckpointIdentity {
                sha256: sha256.clone(),
                source: None,
                training_dataset_version: self.version.training_dataset_version.clone(),
            });
        let manifest = ExpertModelManifest {
            schema_version: annotagent_core::EXPERT_MODEL_MANIFEST_SCHEMA_VERSION.to_string(),
            model_id: self.model_id.clone(),
            display_name: self.display_name.clone(),
            architecture: self.version.architecture.clone(),
            model_version: self.version.model_version.clone(),
            connection: ModelConnection::VisionWorkerModel {
                worker_id: self.id.clone(),
                worker_model_id: self.model_id.clone(),
            },
            capabilities,
            input_contracts,
            output_contracts: vec![ArtifactContract::artifact(
                "output",
                output_kind,
                true,
                true,
            )],
            prompt_contracts,
            score_semantics: self.score_semantics,
            geometry_semantics: annotagent_core::default_geometry_semantics(
                &self.expected_capabilities,
            ),
            label_space: (!self.label_space.is_empty()).then(|| self.label_space.clone()),
            checkpoint,
            runtime_requirements: self.runtime_requirements.clone(),
            license: self.license.clone(),
            availability,
            availability_evidence,
            metadata: BTreeMap::from([
                ("worker_endpoint".to_owned(), json!(self.base_url)),
                ("worker_enabled".to_owned(), json!(self.enabled)),
                ("allow_remote".to_owned(), json!(self.allow_remote)),
                (
                    "authentication_reference".to_owned(),
                    json!(self.authentication_reference),
                ),
            ]),
        };
        manifest.validate().map_err(anyhow::Error::from)?;
        Ok(manifest)
    }
}

fn default_detection_workers() -> Vec<DetectionWorkerSettings> {
    vec![
        DetectionWorkerSettings {
            id: "annotagent-locate-anything".to_owned(),
            display_name: "LocateAnything Local".to_owned(),
            model_id: "locate-anything-local".to_owned(),
            base_url: "http://127.0.0.1:8791".to_owned(),
            authentication_reference: None,
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
            availability: ModelAvailability::MissingWeights,
            availability_evidence: ModelAvailabilityEvidence::default(),
        },
        DetectionWorkerSettings {
            id: "annotagent-rfdetr".to_owned(),
            display_name: "RF-DETR Specialist Local".to_owned(),
            model_id: "rfdetr-specialist-local".to_owned(),
            base_url: "http://127.0.0.1:8792".to_owned(),
            authentication_reference: None,
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
            availability: ModelAvailability::MissingWeights,
            availability_evidence: ModelAvailabilityEvidence::default(),
        },
        DetectionWorkerSettings {
            id: "annotagent-sam2".to_owned(),
            display_name: "SAM 2.1 Prompted Segmentation".to_owned(),
            model_id: "sam2.1-hiera-tiny".to_owned(),
            base_url: "http://127.0.0.1:8790".to_owned(),
            authentication_reference: None,
            enabled: false,
            allow_remote: false,
            requires_checkpoint_metadata: true,
            expected_capabilities: vec![VisionCapability::PromptedSegmentation],
            score_semantics: ScoreSemantics::NotProvided,
            version: ModelVersionMetadata {
                architecture: Some("sam2.1-hiera-tiny".to_owned()),
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
                    "SAM 2 compatible Python package".to_owned(),
                    "PyTorch CUDA".to_owned(),
                ],
                supports_batch: false,
            },
            license: LicenseMetadata {
                code_license: Some("Apache-2.0 for the official SAM 2 implementation".to_owned()),
                weight_license: None,
                source_url: Some("https://github.com/facebookresearch/sam2".to_owned()),
                commercial_use: LicensePermission::Unknown,
                redistribution: LicensePermission::Unknown,
                usage_notes: vec![
                    "Configure and verify the concrete checkpoint license before enabling."
                        .to_owned(),
                ],
                verified_from_official_source: true,
            },
            timeout_seconds: 120,
            max_request_bytes: 44_000_000,
            max_response_bytes: 16_000_000,
            max_retries: 0,
            cost_per_request: rust_decimal::Decimal::ZERO,
            availability: ModelAvailability::MissingWeights,
            availability_evidence: ModelAvailabilityEvidence::default(),
        },
        DetectionWorkerSettings {
            id: "annotagent-yolo".to_owned(),
            display_name: "YOLO HTTP Worker".to_owned(),
            model_id: "yolo-http-worker".to_owned(),
            base_url: "http://127.0.0.1:8793".to_owned(),
            authentication_reference: None,
            enabled: false,
            allow_remote: false,
            requires_checkpoint_metadata: true,
            expected_capabilities: vec![VisionCapability::ObjectDetection],
            score_semantics: ScoreSemantics::RelativeConfidence,
            version: ModelVersionMetadata {
                architecture: Some("yolo".to_owned()),
                model_version: "unconfigured".to_owned(),
                checkpoint_sha256: None,
                training_dataset_version: None,
                backend_protocol_version: annotagent_core::VISION_WORKER_PROTOCOL_VERSION
                    .to_string(),
            },
            label_space: Vec::new(),
            runtime_requirements: RuntimeRequirements {
                devices: vec!["cpu".to_owned(), "cuda".to_owned()],
                minimum_gpu_memory_mb: None,
                dependencies: vec!["A protocol-compatible YOLO implementation".to_owned()],
                supports_batch: false,
            },
            license: LicenseMetadata {
                code_license: None,
                weight_license: None,
                source_url: None,
                commercial_use: LicensePermission::Unknown,
                redistribution: LicensePermission::Unknown,
                usage_notes: vec![
                    "License depends on the configured implementation and weights.".to_owned(),
                ],
                verified_from_official_source: false,
            },
            timeout_seconds: 120,
            max_request_bytes: 44_000_000,
            max_response_bytes: 2_000_000,
            max_retries: 0,
            cost_per_request: rust_decimal::Decimal::ZERO,
            availability: ModelAvailability::MissingWeights,
            availability_evidence: ModelAvailabilityEvidence::default(),
        },
    ]
}

fn default_provider_kind() -> String {
    "mock".to_owned()
}

const PIPELINE_BUILDER_SYSTEM_PROMPT: &str = "You are AnnotAgent's constrained Pipeline Builder. \
Use only registered tools, public Node Definitions, available Model Profiles, typed Artifact contracts, \
and inspected evidence. Never create, bind, recommend, or preserve a Mock Provider, Mock Model, \
fixture backend, or test-only fallback. If a real binding is unavailable, leave it explicitly unresolved \
and explain the required Provider or Vision Worker setup. VLM semantic confidence is not geometry accuracy: a VLM bounding box is an \
uncalibrated CoarseHypothesis even when confidence is high. Provider or Worker failure is infrastructure \
evidence, never a reason to add prompted segmentation. NoCandidate has no box or point prompt, so do not \
add prompted segmentation; consider Tile, zoom/crop search, an available open-vocabulary or specialist \
detector, or Review. Semantic errors such as white footwear mistaken for a football require Crop \
Classification, a Domain Validator, a second detector, Correction Memory, or Review; segmentation may \
tighten the wrong object and is not the primary repair. Add Detection -> Box Prompt -> Prompted Segmentation \
-> Mask to BBox only when a semantically plausible candidate exists, inspected geometry evidence is poor, \
the conversion path is registered, and an Available prompted-segmentation Model Profile passes its \
contracts. For small targets consider Resize or Tile -> Detection -> Merge before refinement. Prefer an \
Available specialist whose fixed Label Space covers the target; otherwise an Available open-vocabulary \
detector may cold-start. Missing scores remain missing and require evidence decision or Review, never a \
fabricated confidence. Never bind unavailable, disabled, unconfigured, missing-weights, unreachable, \
incompatible, invalid-contract, failed-smoke, or Unknown models. Setup-only models may appear only as \
unapplied Alternatives with a setup action. Never invent a capability, score, health result, benchmark, \
Validator, Refiner, model, or node. Inspect the Project, current Pipeline, enabled Skills, Node Definitions, \
available capabilities, compatible models, model contracts, and relevant Skill resources before creating a \
Draft. Modify only the persisted editable Draft through bounded tools. Validate with Rust, run a \
non-committing Dry Run, inspect failure classes and geometry quality, revise only from that structured \
evidence, then submit for explicit human approval. Never publish, start a formal Run, set credentials, \
create or delete Providers, emit code, request Shell/Python/package/download/arbitrary URL tools, or reveal \
hidden reasoning. check_provider_availability is passive only and must not send a billable request.";

fn pipeline_builder_live_tools(input: &WorkflowAdvisorInput) -> Vec<ToolDefinition> {
    let node_definition_ids = input
        .node_catalog
        .iter()
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    let artifact_kind_ids = all_artifact_kinds()
        .into_iter()
        .filter_map(|kind| serde_json::to_value(kind).ok())
        .collect::<Vec<_>>();
    let mut template_ids = vec!["safe_default".to_owned()];
    template_ids.extend(
        input
            .workflow_templates
            .iter()
            .filter(|template| {
                template.nodes.iter().all(|node| {
                    node_definition_ids
                        .iter()
                        .any(|node_type| node_type == &node.node_type)
                })
            })
            .map(|template| template.id.clone()),
    );
    let runtime_policy_ids = input
        .runtime_policies
        .iter()
        .map(|policy| policy.id.clone())
        .collect::<Vec<_>>();
    let enabled_skill_ids = input.enabled_skills.clone();
    let resource_ids = input.resource_ids.clone();
    let enum_string_schema = |values: &[String]| {
        if values.is_empty() {
            json!({"type": "string"})
        } else {
            json!({"type": "string", "enum": values})
        }
    };
    let load_resource_description = if resource_ids.is_empty() {
        "Load one declared resource from an enabled Skill. This Project declares no Skill resources."
            .to_owned()
    } else {
        format!(
            "Load one declared resource from an enabled Skill. Use an exact resource_name from: {}.",
            resource_ids.join(", ")
        )
    };
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
    vec![
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
            PipelineBuilderTool::SampleDataset,
            "Read bounded dataset dimensions and MIME types; no image bytes are returned.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::InspectSampleImage,
            "Inspect bounded metadata for one Project image; image bytes and filesystem paths are excluded.",
            json!({"type":"object","additionalProperties":false,"required":["image_index"],"properties":{"image_index":{"type":"integer","minimum":0}}}),
        ),
        read(
            PipelineBuilderTool::InspectExistingPipeline,
            "Inspect the current editable and published Workflow summaries without changing them.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::InspectExistingAutomations,
            "Inspect the Project's published workflows and recent Run summaries without starting a Run.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::ListEnabledSkills,
            "List only Skills enabled by the Project.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::LoadSkillResource,
            &load_resource_description,
            json!({"type":"object","additionalProperties":false,"required":["skill_id","resource_name"],"properties":{"skill_id":enum_string_schema(&enabled_skill_ids),"resource_name":enum_string_schema(&resource_ids)}}),
        ),
        read(
            PipelineBuilderTool::ListNodeDefinitions,
            "List registered node IDs and typed input/output contracts.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::InspectNodeDefinition,
            "Inspect one public Node Definition and its configuration schema.",
            json!({"type":"object","additionalProperties":false,"required":["node_type"],"properties":{"node_type":{"type":"string","enum":node_definition_ids.clone()}}}),
        ),
        read(
            PipelineBuilderTool::FindArtifactConversionPath,
            "Find a legal typed Artifact conversion path using only currently registered executable nodes. Same-type paths may represent geometry refinement cycles.",
            json!({
                "type":"object",
                "additionalProperties":false,
                "required":["from","to"],
                "properties":{
                    "from":{"enum":artifact_kind_ids.clone()},
                    "to":{"enum":artifact_kind_ids}
                }
            }),
        ),
        read(
            PipelineBuilderTool::ListPipelineTemplates,
            "List compatible Registry templates and the safe default template.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::ListProviderProfiles,
            "List credential-safe Provider summaries. Secrets and credential locators are never returned.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::ListAvailableCapabilities,
            "List capabilities backed by Available models separately from setup-only alternatives.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::ListCompatibleModels,
            "List available Provider and expert Worker models compatible with an optional Node Definition; setup-only alternatives are never applied.",
            json!({"type":"object","additionalProperties":false,"properties":{"node_type":{"type":"string","enum":node_definition_ids.clone()}}}),
        ),
        read(
            PipelineBuilderTool::InspectModelProfile,
            "Inspect one revisioned Model Profile. Provider credentials are not part of this object.",
            json!({"type":"object","additionalProperties":false,"required":["model_profile_id"],"properties":{"model_profile_id":{"type":"string"}}}),
        ),
        read(
            PipelineBuilderTool::InspectWorkerHealth,
            "Inspect evidence-backed availability for one expert Worker model. No active probe is sent.",
            json!({"type":"object","additionalProperties":false,"required":["model_id"],"properties":{"model_id":{"type":"string"}}}),
        ),
        read(
            PipelineBuilderTool::InspectModelContracts,
            "Inspect typed inputs, outputs and prompt contracts for one Registry model.",
            json!({"type":"object","additionalProperties":false,"required":["model_id"],"properties":{"model_id":{"type":"string"}}}),
        ),
        read(
            PipelineBuilderTool::InspectLabelSpace,
            "Inspect the declared Label Space for one Registry model without inferring missing labels.",
            json!({"type":"object","additionalProperties":false,"required":["model_id"],"properties":{"model_id":{"type":"string"}}}),
        ),
        read(
            PipelineBuilderTool::InspectScoreSemantics,
            "Inspect score semantics for one Registry model; missing scores remain NotProvided.",
            json!({"type":"object","additionalProperties":false,"required":["model_id"],"properties":{"model_id":{"type":"string"}}}),
        ),
        read(
            PipelineBuilderTool::InspectGeometrySemantics,
            "Inspect geometry semantics independently from model confidence.",
            json!({"type":"object","additionalProperties":false,"required":["model_id"],"properties":{"model_id":{"type":"string"}}}),
        ),
        read(
            PipelineBuilderTool::CheckCapabilityPath,
            "Check whether a public capability node has an Available compatible model and satisfiable typed contracts.",
            json!({"type":"object","additionalProperties":false,"required":["node_type"],"properties":{"node_type":{"type":"string","enum":node_definition_ids.clone()}}}),
        ),
        read(
            PipelineBuilderTool::CheckProviderAvailability,
            "Read a passive Provider availability assessment only. Never sends a billable model request.",
            json!({"type":"object","additionalProperties":false,"required":["provider_id"],"properties":{"provider_id":{"type":"string"}}}),
        ),
        read(
            PipelineBuilderTool::EstimateModelCost,
            "Estimate Model Profile cost from declared Registry pricing without invoking the model.",
            json!({"type":"object","additionalProperties":false,"required":["model_profile_id"],"properties":{"model_profile_id":{"type":"string"},"image_count":{"type":"integer","minimum":0,"maximum":1_000_000},"input_tokens":{"type":"integer","minimum":0},"output_tokens":{"type":"integer","minimum":0}}}),
        ),
        mutate(
            PipelineBuilderTool::CreatePipelineDraft,
            "Create a real persisted editable Draft. Never publishes.",
            json!({"type":"object","additionalProperties":false,"properties":{"name":{"type":"string","minLength":1,"maxLength":160}}}),
        ),
        mutate(
            PipelineBuilderTool::CreateDraftFromTemplate,
            "Create a new editable Draft from safe_default or an exact compatible Registry template ID. Never publishes.",
            json!({"type":"object","additionalProperties":false,"properties":{"template_id":{"type":"string","enum":template_ids}}}),
        ),
        mutate(
            PipelineBuilderTool::AddPipelineNode,
            "Add one public Node Definition, or apply one evidence-gated guided revision after inspecting a Dry Run.",
            json!({"type":"object","additionalProperties":false,"properties":{"node_type":{"type":"string","enum":node_definition_ids},"guided_template":{"type":"string","enum":["crop_verification","prompted_segmentation_refinement"]},"node_id":{"type":["string","null"],"maxLength":120},"configuration":{"type":"object"}}}),
        ),
        mutate(
            PipelineBuilderTool::RemovePipelineNode,
            "Remove one existing node and its incident connections from the current Draft.",
            json!({"type":"object","additionalProperties":false,"required":["node_id"],"properties":{"node_id":{"type":"string"}}}),
        ),
        mutate(
            PipelineBuilderTool::DisconnectPipelineNodes,
            "Remove one existing Draft connection so validation can guide a bounded repair.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["from_node", "to_node"],
                "properties": {
                    "from_node": {"type": "string"},
                    "to_node": {"type": "string"}
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
                    "from_node": {"type": "string"},
                    "from_port": {"type": "string"},
                    "to_node": {"type": "string"},
                    "to_port": {"type": "string"},
                    "route": {"type": ["string", "null"]}
                }
            }),
        ),
        mutate(
            PipelineBuilderTool::SetNodeConfiguration,
            "Replace the configuration object of one existing public node.",
            json!({"type":"object","additionalProperties":false,"required":["node_id","configuration"],"properties":{"node_id":{"type":"string"},"configuration":{"type":"object"}}}),
        ),
        mutate(
            PipelineBuilderTool::BindModelProfile,
            "Bind one compatible revisioned Model Profile to a node.",
            json!({"type":"object","additionalProperties":false,"required":["node_id","model_profile_id"],"properties":{"node_id":{"type":"string"},"model_profile_id":{"type":"string"},"locked":{"type":"boolean","default":true}}}),
        ),
        mutate(
            PipelineBuilderTool::SetLabelMapping,
            "Set a bounded class-to-Label mapping on an existing selection node.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["node_id", "class_mapping"],
                "properties": {
                    "node_id": {"type": "string"},
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
                    "node_id": {"type": "string"},
                    "threshold": {"type": "number", "minimum": 0, "maximum": 1}
                }
            }),
        ),
        mutate(
            PipelineBuilderTool::SetRuntimePolicy,
            "Configure a registered cross-cutting Runtime Policy outside the graph.",
            json!({"type":"object","additionalProperties":false,"required":["policy_id","configuration"],"properties":{"policy_id":{"type":"string","enum":runtime_policy_ids},"configuration":{"type":"object"}}}),
        ),
        read(
            PipelineBuilderTool::ComparePipelineDrafts,
            "Compare the current persisted Draft with another persisted Draft from the same Project.",
            json!({"type":"object","additionalProperties":false,"required":["other_draft_id"],"properties":{"other_draft_id":{"type":"string"}}}),
        ),
        mutate(
            PipelineBuilderTool::UndoLastDraftChange,
            "Undo the last Builder mutation and persist the restored Draft.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::ValidatePipeline,
            "Run Rust Pipeline Grammar and static validation on the current Draft.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::EstimatePipelineCost,
            "Estimate cost for the current Draft from bound Model Profile pricing without a model call.",
            json!({"type":"object","additionalProperties":false,"properties":{"image_count":{"type":"integer","minimum":1,"maximum":1_000_000,"default":1}}}),
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
            "Read bounded Provider/Worker failures, no-candidate, semantic/geometry/domain review, missing-score, manual-correction and refiner metrics from the latest Dry Run.",
            no_arguments(),
        ),
        read(
            PipelineBuilderTool::InspectFailureClasses,
            "Inspect structured failure classes from the latest Dry Run; infrastructure, no-candidate, semantic and geometry failures stay distinct.",
            bounded_inspection_schema(),
        ),
        read(
            PipelineBuilderTool::InspectGeometryQuality,
            "Inspect aggregate and bounded per-candidate geometry quality from the latest Dry Run.",
            bounded_inspection_schema(),
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
            PipelineBuilderTool::InspectNodeStatistics,
            "Read bounded aggregate latency, cost, status, and issue counts for one node.",
            json!({"type":"object","additionalProperties":false,"required":["node_id"],"properties":{"node_id":{"type":"string"}}}),
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
        read(
            PipelineBuilderTool::CompareDryRuns,
            "Compare the latest Dry Run summary with a persisted Dry Run from another Draft in the same Project.",
            json!({"type":"object","additionalProperties":false,"required":["other_draft_id"],"properties":{"other_draft_id":{"type":"string"}}}),
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
        mutate(
            PipelineBuilderTool::FinishAgentSession,
            "Finish only after the current Draft has been submitted for explicit human approval.",
            no_arguments(),
        ),
    ]
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

fn required_usize_argument(arguments: &serde_json::Value, name: &str) -> Result<usize> {
    arguments
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| anyhow!("Tool argument {name:?} must be a non-negative integer"))
}

fn pipeline_builder_tool_error_code(message: &str) -> &'static str {
    if message.contains("incompatible_model_capability:") {
        "incompatible_model_capability"
    } else if message.contains("model_profile_unavailable:") {
        "model_profile_unavailable"
    } else if message.contains("valid resource_name values")
        || message.contains("valid skill_id values")
    {
        "invalid_declared_tool_value"
    } else {
        "tool_validation_failed"
    }
}

fn compatible_builder_models(
    input: &WorkflowAdvisorInput,
    required_capability: Option<ModelCapability>,
) -> Vec<&ModelProfile> {
    input
        .model_profiles
        .iter()
        .filter(|model| {
            let provider = input
                .provider_profiles
                .iter()
                .find(|provider| provider.id == model.provider_id);
            model.enabled
                && model.status == ModelProfileStatus::Available
                && required_capability
                    .is_none_or(|capability| model.task_capabilities.contains(&capability))
                && provider.is_some_and(|provider| {
                    provider.enabled
                        && provider.credential_configured
                        && matches!(
                            provider.health_status,
                            ProviderHealthStatus::Available | ProviderHealthStatus::Configured
                        )
                })
        })
        .collect()
}

fn estimate_model_profile_cost(
    model: &ModelProfile,
    arguments: &serde_json::Value,
) -> serde_json::Value {
    let image_count = arguments
        .get("image_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    let input_tokens = arguments
        .get("input_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    let output_tokens = arguments
        .get("output_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    estimate_model_profile_cost_for_counts(model, image_count, input_tokens, output_tokens)
}

fn estimate_model_profile_cost_for_counts(
    model: &ModelProfile,
    image_count: u64,
    input_tokens: u64,
    output_tokens: u64,
) -> serde_json::Value {
    let million = rust_decimal::Decimal::from(1_000_000_u64);
    let image_count_decimal = rust_decimal::Decimal::from(image_count);
    let request_cost = model.pricing.per_request.unwrap_or_default() * image_count_decimal;
    let image_cost = model.pricing.per_image.unwrap_or_default() * image_count_decimal;
    let input_cost = model.pricing.input_per_million_tokens.unwrap_or_default()
        * rust_decimal::Decimal::from(input_tokens)
        / million;
    let output_cost = model.pricing.output_per_million_tokens.unwrap_or_default()
        * rust_decimal::Decimal::from(output_tokens)
        / million;
    let total = request_cost + image_cost + input_cost + output_cost;
    json!({
        "currency": model.pricing.currency,
        "estimated_cost": total.to_string(),
        "image_count": image_count,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "pricing_source": model.pricing.source,
        "billable_request_sent": false,
    })
}

fn pipeline_builder_call_cost(
    model: Option<&ModelProfile>,
    input_tokens: u64,
    output_tokens: u64,
) -> (rust_decimal::Decimal, String) {
    let Some(model) = model else {
        return (rust_decimal::Decimal::ZERO, "USD".to_owned());
    };
    let million = rust_decimal::Decimal::from(1_000_000_u64);
    let input = model.pricing.input_per_million_tokens.unwrap_or_default()
        * rust_decimal::Decimal::from(input_tokens)
        / million;
    let output = model.pricing.output_per_million_tokens.unwrap_or_default()
        * rust_decimal::Decimal::from(output_tokens)
        / million;
    (
        input + output + model.pricing.per_request.unwrap_or_default(),
        model.pricing.currency.clone(),
    )
}

/// Keep only complete Assistant Tool Call + Tool result groups when an Agent conversation grows
/// beyond the selected Model Profile's context budget. The initial policy and Project snapshot,
/// plus the four most recent exchanges, are always retained.
fn compact_pipeline_builder_messages(
    messages: &mut Vec<ModelMessage>,
    context_tokens: Option<u64>,
) -> bool {
    const BASE_MESSAGES: usize = 2;
    const RECENT_GROUPS: usize = 4;
    let byte_budget = context_tokens
        .unwrap_or(32_768)
        .saturating_mul(3)
        .clamp(16_384, 2_000_000) as usize;
    let message_size = |message: &ModelMessage| {
        message.content.len()
            + message
                .tool_calls
                .iter()
                .map(|call| call.name.len() + call.arguments.to_string().len() + 64)
                .sum::<usize>()
            + 96
    };
    let mut total = messages.iter().map(message_size).sum::<usize>();
    if total <= byte_budget || messages.len() <= BASE_MESSAGES {
        return false;
    }

    let mut groups = Vec::<(usize, usize)>::new();
    let mut index = BASE_MESSAGES;
    while index < messages.len() {
        let start = index;
        index += 1;
        if messages[start].role == ModelRole::Assistant {
            while index < messages.len() && messages[index].role == ModelRole::Tool {
                index += 1;
            }
        }
        groups.push((start, index));
    }
    let removable = groups.len().saturating_sub(RECENT_GROUPS);
    let mut remove_until = BASE_MESSAGES;
    for (_, end) in groups.into_iter().take(removable) {
        if total <= byte_budget {
            break;
        }
        total = total.saturating_sub(messages[remove_until..end].iter().map(message_size).sum());
        remove_until = end;
    }
    if remove_until == BASE_MESSAGES {
        return false;
    }
    messages.drain(BASE_MESSAGES..remove_until);
    messages.insert(
        BASE_MESSAGES,
        ModelMessage {
            role: ModelRole::System,
            content: "Earlier complete tool exchanges were compacted. Rust still enforces the current Draft, validation, Dry Run, and budget state; inspect again when needed."
                .to_owned(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        },
    );
    true
}

fn workflow_node_from_definition(
    definition: &NodeDefinition,
    node_id: String,
    configuration: &serde_json::Value,
) -> Result<annotagent_core::WorkflowDraftNode> {
    let parameters = configuration
        .as_object()
        .ok_or_else(|| anyhow!("node configuration must be an object"))?
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let ports = |definitions: &[PortDefinition]| {
        definitions
            .iter()
            .map(|port| NodePort {
                id: port.name.clone(),
                artifact_type: port.artifact_type,
                required: port.required,
                multiple: port.cardinality == PortCardinality::Many,
            })
            .collect::<Vec<_>>()
    };
    let kind = match definition.id.as_str() {
        "core.image_input" => WorkflowNodeKind::ImageInput,
        "core.validate" => WorkflowNodeKind::Validator,
        "core.decision" => WorkflowNodeKind::Gate,
        "core.human_review" => WorkflowNodeKind::HumanReview,
        "core.commit" => WorkflowNodeKind::Commit,
        _ if definition.required_model_capability.is_some() => WorkflowNodeKind::VisionModel,
        _ => WorkflowNodeKind::Transform,
    };
    Ok(annotagent_core::WorkflowDraftNode {
        id: node_id,
        node_type: definition.id.clone(),
        kind,
        depends_on: Vec::new(),
        inputs: ports(&definition.input_ports),
        outputs: ports(&definition.output_ports),
        model_binding: None,
        model_profile_binding: None,
        required_skills: Vec::new(),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        max_retries: 0,
        review_gate: false,
        parameters,
        retry_policy: RetryPolicy::default(),
        fallback_policy: annotagent_core::FallbackPolicy::default(),
        gate: ReviewGate::default(),
        resources: ResourceRequirements::default(),
    })
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

fn unique_workflow_node_id(existing: &BTreeSet<String>, preferred: &str) -> String {
    if !existing.contains(preferred) {
        return preferred.to_owned();
    }
    (2..=1_000_000)
        .map(|suffix| format!("{preferred}-{suffix}"))
        .find(|candidate| !existing.contains(candidate))
        .expect("one million duplicate compatibility node ids are unsupported")
}

/// Expands the former opaque `RoboCup` SAM refiner into the public, typed Artifact chain. The
/// compatibility check intentionally lives in the Application layer so Core remains model-brand
/// agnostic. The legacy node id is retained for downstream edges and becomes Mask-to-BBox.
fn migrate_legacy_expert_workflow(draft: &mut WorkflowDraft) -> Result<bool> {
    let legacy_ids = draft
        .nodes
        .iter()
        .filter(|node| node.node_type == "sam_prompted_refiner")
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    if legacy_ids.is_empty() {
        return Ok(false);
    }
    let image_node_id = draft
        .nodes
        .iter()
        .find(|node| {
            node.kind == WorkflowNodeKind::ImageInput
                || node
                    .outputs
                    .iter()
                    .any(|port| port.artifact_type == ArtifactKind::Image)
        })
        .map(|node| node.id.clone())
        .ok_or_else(|| {
            anyhow!(
                "legacy SAM Workflow migration requires an Image input for prompted segmentation"
            )
        })?;

    for legacy_id in legacy_ids {
        let existing = draft
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<BTreeSet<_>>();
        let prompts_id = unique_workflow_node_id(&existing, &format!("{legacy_id}-box-prompts"));
        let mut with_prompts = existing;
        with_prompts.insert(prompts_id.clone());
        let segment_id = unique_workflow_node_id(&with_prompts, &format!("{legacy_id}-segment"));
        let legacy_index = draft
            .nodes
            .iter()
            .position(|node| node.id == legacy_id)
            .expect("legacy node id came from this Draft");
        let legacy = draft.nodes[legacy_index].clone();

        let incoming = draft
            .edges
            .iter()
            .filter(|edge| edge.to_node == legacy_id)
            .cloned()
            .collect::<Vec<_>>();
        draft.edges.retain(|edge| edge.to_node != legacy_id);
        let mut detection_sources = BTreeSet::new();
        let mut image_edge_present = false;
        for edge in incoming {
            let source_is_image = edge.from_node == image_node_id
                || draft
                    .nodes
                    .iter()
                    .find(|node| node.id == edge.from_node)
                    .is_some_and(|node| {
                        node.outputs
                            .iter()
                            .any(|port| port.artifact_type == ArtifactKind::Image)
                    });
            if source_is_image {
                image_edge_present = true;
                draft.edges.push(WorkflowEdge {
                    from_node: edge.from_node,
                    from_port: edge.from_port,
                    to_node: segment_id.clone(),
                    to_port: "images".to_owned(),
                    route: edge.route,
                });
            } else {
                detection_sources.insert(edge.from_node.clone());
                draft.edges.push(WorkflowEdge {
                    from_node: edge.from_node,
                    from_port: edge.from_port,
                    to_node: prompts_id.clone(),
                    to_port: "detections".to_owned(),
                    route: edge.route,
                });
            }
        }
        if detection_sources.is_empty() {
            for source in &legacy.depends_on {
                if source != &image_node_id {
                    detection_sources.insert(source.clone());
                    if let Some(source_node) = draft.nodes.iter().find(|node| node.id == *source) {
                        let from_port = source_node
                            .outputs
                            .iter()
                            .find(|port| port.artifact_type == ArtifactKind::DetectionSet)
                            .map_or_else(|| "detections".to_owned(), |port| port.id.clone());
                        draft.edges.push(WorkflowEdge {
                            from_node: source.clone(),
                            from_port,
                            to_node: prompts_id.clone(),
                            to_port: "detections".to_owned(),
                            route: None,
                        });
                    }
                }
            }
        }
        if !image_edge_present {
            let from_port = draft
                .nodes
                .iter()
                .find(|node| node.id == image_node_id)
                .and_then(|node| {
                    node.outputs
                        .iter()
                        .find(|port| port.artifact_type == ArtifactKind::Image)
                })
                .map_or_else(|| "image".to_owned(), |port| port.id.clone());
            draft.edges.push(WorkflowEdge {
                from_node: image_node_id.clone(),
                from_port,
                to_node: segment_id.clone(),
                to_port: "images".to_owned(),
                route: None,
            });
        }

        let prompt_padding = legacy
            .parameters
            .get("prompt_padding")
            .or_else(|| legacy.parameters.get("padding"))
            .cloned();
        let prompt_node = WorkflowDraftNode {
            id: prompts_id.clone(),
            node_type: annotagent_runtime::CORE_DETECTIONS_TO_BOX_PROMPTS.to_owned(),
            kind: WorkflowNodeKind::Transform,
            depends_on: detection_sources.into_iter().collect(),
            inputs: vec![NodePort {
                id: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                required: true,
                multiple: true,
            }],
            outputs: vec![NodePort {
                id: "prompts".to_owned(),
                artifact_type: ArtifactKind::BoxPromptSet,
                required: true,
                multiple: true,
            }],
            parameters: prompt_padding
                .map(|padding| BTreeMap::from([("padding".to_owned(), padding)]))
                .unwrap_or_default(),
            ..WorkflowDraftNode::default()
        };
        let segment_node = WorkflowDraftNode {
            id: segment_id.clone(),
            node_type: "capability.segment".to_owned(),
            kind: WorkflowNodeKind::VisionModel,
            depends_on: vec![image_node_id.clone(), prompts_id.clone()],
            inputs: vec![
                NodePort {
                    id: "images".to_owned(),
                    artifact_type: ArtifactKind::Image,
                    required: true,
                    multiple: true,
                },
                NodePort {
                    id: "box_prompts".to_owned(),
                    artifact_type: ArtifactKind::BoxPromptSet,
                    required: true,
                    multiple: true,
                },
            ],
            outputs: vec![NodePort {
                id: "masks".to_owned(),
                artifact_type: ArtifactKind::MaskSet,
                required: true,
                multiple: true,
            }],
            model_binding: Some(
                legacy
                    .model_binding
                    .clone()
                    .unwrap_or_else(|| "sam2.1-hiera-tiny".to_owned()),
            ),
            model_profile_binding: legacy.model_profile_binding,
            required_skills: legacy.required_skills.clone(),
            max_retries: legacy.max_retries,
            retry_policy: legacy.retry_policy,
            resources: legacy.resources.clone(),
            ..WorkflowDraftNode::default()
        };
        let final_node = &mut draft.nodes[legacy_index];
        annotagent_runtime::CORE_MASK_TO_BBOX.clone_into(&mut final_node.node_type);
        final_node.kind = WorkflowNodeKind::Transform;
        final_node.depends_on = vec![segment_id.clone(), prompts_id.clone()];
        final_node.inputs = vec![
            NodePort {
                id: "masks".to_owned(),
                artifact_type: ArtifactKind::MaskSet,
                required: true,
                multiple: true,
            },
            NodePort {
                id: "box_prompts".to_owned(),
                artifact_type: ArtifactKind::BoxPromptSet,
                required: true,
                multiple: true,
            },
        ];
        final_node.outputs = vec![NodePort {
            id: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            required: true,
            multiple: true,
        }];
        final_node.model_binding = None;
        final_node.model_profile_binding = None;
        final_node.required_skills.clear();
        final_node.validators.clear();
        final_node.refiners.clear();
        final_node.parameters.clear();
        final_node.max_retries = 0;
        final_node.retry_policy = RetryPolicy::default();
        final_node.resources = ResourceRequirements::default();

        draft.nodes.push(prompt_node);
        draft.nodes.push(segment_node);
        draft.edges.extend([
            WorkflowEdge {
                from_node: prompts_id.clone(),
                from_port: "prompts".to_owned(),
                to_node: segment_id.clone(),
                to_port: "box_prompts".to_owned(),
                route: None,
            },
            WorkflowEdge {
                from_node: prompts_id,
                from_port: "prompts".to_owned(),
                to_node: legacy_id.clone(),
                to_port: "box_prompts".to_owned(),
                route: None,
            },
            WorkflowEdge {
                from_node: segment_id,
                from_port: "masks".to_owned(),
                to_node: legacy_id,
                to_port: "masks".to_owned(),
                route: None,
            },
        ]);
    }
    draft.resource_versions.insert(
        "compatibility.sam_prompted_refiner".to_owned(),
        "2".to_owned(),
    );
    draft.schema_version = WORKFLOW_SCHEMA_VERSION;
    draft.updated_at = chrono::Utc::now();
    Ok(true)
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
            && !worker.checkpoint_identity_complete()
        {
            if worker.has_fixed_label_space() {
                bail!(
                    "enabled fixed-label Vision Workers require architecture, model version, checkpoint SHA-256, training dataset version, label space, and weight license metadata"
                );
            }
            bail!(
                "enabled Vision Workers require architecture, model version, checkpoint SHA-256, and weight license metadata"
            );
        }
        worker.validate_authentication_reference()?;
        worker.expert_manifest()?;
        HttpJsonVisionBackend::new(HttpJsonVisionBackendConfig {
            id: worker.id.clone(),
            endpoint: format!("{}/v1/infer", worker.base_url.trim_end_matches('/')),
            capabilities: worker.expected_capabilities.clone(),
            request_timeout: Duration::from_secs(worker.timeout_seconds),
            // Syntax is validated here; a missing runtime secret remains a live availability
            // condition and does not prevent saving the workspace configuration.
            authorization: None,
            expected_model_identity: Some(worker.model_id.clone()),
            max_retries: worker.max_retries,
            max_response_bytes: worker.max_response_bytes,
            allow_remote: worker.allow_remote,
        })
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
        name: "Unpublished Project task graph".to_owned(),
        version: project.version.to_string(),
        status: WorkflowStatus::Draft,
        validation_status: "publish required".to_owned(),
        is_default: false,
        source: if skills.is_empty() {
            "Project Schema only".to_owned()
        } else {
            "Project Schema + registered Skill graphs only".to_owned()
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

fn registry_requirement_for_node(
    node: &annotagent_core::WorkflowDraftNode,
) -> Option<(ModelCapability, ModelBindingRole)> {
    let requirement = match node.node_type.as_str() {
        annotagent_skill_classification::CLASSIFICATION_OPERATION | "capability.classify" => (
            ModelCapability::ImageClassification,
            ModelBindingRole::Classification,
        ),
        annotagent_skill_classification::CLASSIFICATION_VERIFY_OPERATION => (
            ModelCapability::ImageClassification,
            ModelBindingRole::Verification,
        ),
        annotagent_skill_open_vocabulary::OPEN_VOCABULARY_DETECTION_OPERATION => (
            ModelCapability::OpenVocabularyDetection,
            ModelBindingRole::Detection,
        ),
        annotagent_skill_open_vocabulary::PHRASE_GROUNDING_OPERATION => (
            ModelCapability::PhraseGrounding,
            ModelBindingRole::Detection,
        ),
        annotagent_skill_object_detection::OBJECT_DETECTION_OPERATION
        | annotagent_skill_yolo::YOLO_DETECTION_OPERATION
        | "capability.detect" => (
            ModelCapability::ObjectDetection,
            ModelBindingRole::Detection,
        ),
        annotagent_skill_vlm_detection::VLM_DETECTION_OPERATION | "vision_language" => (
            ModelCapability::VisionLanguage,
            ModelBindingRole::PrimaryInference,
        ),
        "capability.segment" | "semantic_segmentation" => (
            ModelCapability::SemanticSegmentation,
            ModelBindingRole::Segmentation,
        ),
        "prompted_segmentation" => (
            ModelCapability::PromptedSegmentation,
            ModelBindingRole::Segmentation,
        ),
        "instance_segmentation" => (
            ModelCapability::InstanceSegmentation,
            ModelBindingRole::Segmentation,
        ),
        _ if node.kind == WorkflowNodeKind::VisionLanguageModel => (
            ModelCapability::VisionLanguage,
            ModelBindingRole::PrimaryInference,
        ),
        _ if node.kind == WorkflowNodeKind::VisionModel
            && node
                .model_binding
                .as_deref()
                .is_some_and(|binding| binding.contains("classif")) =>
        {
            (
                ModelCapability::ImageClassification,
                ModelBindingRole::Classification,
            )
        }
        _ if node.kind == WorkflowNodeKind::VisionModel => (
            ModelCapability::ObjectDetection,
            ModelBindingRole::Detection,
        ),
        _ => return None,
    };
    Some(requirement)
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
    if settings.default_provider == "mock" {
        models.register_backend(Arc::new(MockVisionBackend::new(
            "workspace-provider-adapter",
            capabilities,
            Vec::new(),
        )))?;
    } else {
        let provider: Arc<dyn VisionModelProvider> = Arc::new(
            OpenAiCompatibleProvider::new(settings.provider.clone())
                .map_err(|error| anyhow!(error))?,
        );
        models.register_backend(Arc::new(OpenAiVisionBackend::new(
            "workspace-provider-adapter",
            &settings.provider.model,
            provider,
            settings.provider.max_output_tokens,
            settings.provider.temperature,
        )))?;
    }
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
    if settings.default_provider == "mock" {
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
    }
    for worker in &settings.detection_workers {
        let authorization = worker.authorization_header().ok().flatten();
        models.register_backend(Arc::new(HttpJsonVisionBackend::new(
            HttpJsonVisionBackendConfig {
                id: worker.id.clone(),
                endpoint: format!("{}/v1/infer", worker.base_url.trim_end_matches('/')),
                capabilities: worker.expected_capabilities.clone(),
                request_timeout: Duration::from_secs(worker.timeout_seconds),
                authorization,
                expected_model_identity: Some(worker.model_id.clone()),
                max_retries: worker.max_retries,
                max_response_bytes: worker.max_response_bytes,
                allow_remote: worker.allow_remote,
            },
        )?))?;
        models.register_expert_manifest(worker.expert_manifest()?)?;
    }
    if settings.default_provider == "mock" {
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
            (
                "mock-prompted-segmenter",
                "Offline mock prompted segmenter",
                VisionCapability::PromptedSegmentation,
                ArtifactKind::MaskSet,
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
            vec![ArtifactKind::MaskSet],
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
    register_public_annotation_catalog(&mut nodes)?;
    Ok((nodes, models))
}

fn catalog_port(
    name: &str,
    artifact_type: ArtifactKind,
    required: bool,
    cardinality: PortCardinality,
) -> PortDefinition {
    PortDefinition {
        name: name.to_owned(),
        artifact_type,
        required,
        cardinality,
    }
}

fn node_schema(properties: impl Into<serde_json::Value>) -> serde_json::Value {
    let properties = properties.into();
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
    })
}

fn register_public_annotation_catalog(nodes: &mut NodeRegistry) -> Result<()> {
    let any_candidates = vec![
        ArtifactKind::DetectionSet,
        ArtifactKind::ClassificationSet,
        ArtifactKind::CandidateClusterSet,
        ArtifactKind::AnnotationCandidateSet,
    ];
    for descriptor in [
        VisionNodeDescriptor {
            id: "core.existing_annotations".to_owned(),
            display_name: "Existing annotations".to_owned(),
            required_capabilities: Vec::new(),
            accepts: Vec::new(),
            produces: vec![ArtifactKind::AnnotationCandidateSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_RESIZE.to_owned(),
            display_name: "Resize image".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::Image],
            produces: vec![ArtifactKind::Image],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_TILE.to_owned(),
            display_name: "Tile image".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::Image],
            produces: vec![ArtifactKind::Image],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: "capability.detect".to_owned(),
            display_name: "Detect objects".to_owned(),
            required_capabilities: vec![VisionCapability::ObjectDetection],
            accepts: vec![ArtifactKind::Image, ArtifactKind::CropSet],
            produces: vec![ArtifactKind::DetectionSet],
            deterministic: false,
        },
        VisionNodeDescriptor {
            id: "capability.classify".to_owned(),
            display_name: "Classify images or crops".to_owned(),
            required_capabilities: vec![VisionCapability::Classification],
            accepts: vec![
                ArtifactKind::Image,
                ArtifactKind::CropSet,
                ArtifactKind::DetectionSet,
            ],
            produces: vec![ArtifactKind::ClassificationSet],
            deterministic: false,
        },
        VisionNodeDescriptor {
            id: "capability.segment".to_owned(),
            display_name: "Segment regions".to_owned(),
            required_capabilities: vec![VisionCapability::PromptedSegmentation],
            accepts: vec![
                ArtifactKind::Image,
                ArtifactKind::BoxPromptSet,
                ArtifactKind::PointPromptSet,
            ],
            produces: vec![ArtifactKind::MaskSet],
            deterministic: false,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_DETECTIONS_TO_BOX_PROMPTS.to_owned(),
            display_name: "Convert detections to box prompts".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::DetectionSet],
            produces: vec![ArtifactKind::BoxPromptSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_MASK_TO_BBOX.to_owned(),
            display_name: "Convert masks to bounding boxes".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::MaskSet, ArtifactKind::BoxPromptSet],
            produces: vec![ArtifactKind::DetectionSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_MASK_TO_POLYGON.to_owned(),
            display_name: "Convert masks to polygons".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::MaskSet],
            produces: vec![ArtifactKind::PolygonSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_SELECT_AND_MAP.to_owned(),
            display_name: "Select and map results".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::DetectionSet],
            produces: vec![ArtifactKind::DetectionSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_PROJECT_COORDINATES.to_owned(),
            display_name: "Project coordinates".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::Image, ArtifactKind::DetectionSet],
            produces: vec![ArtifactKind::DetectionSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_COMBINE_EVIDENCE.to_owned(),
            display_name: "Combine model evidence".to_owned(),
            required_capabilities: Vec::new(),
            accepts: vec![ArtifactKind::DetectionSet],
            produces: vec![ArtifactKind::CandidateClusterSet],
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: "core.validate".to_owned(),
            display_name: "Validate results".to_owned(),
            required_capabilities: Vec::new(),
            accepts: any_candidates.clone(),
            produces: any_candidates.clone(),
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: annotagent_runtime::CORE_DECISION.to_owned(),
            display_name: "Decision".to_owned(),
            required_capabilities: Vec::new(),
            accepts: any_candidates.clone(),
            produces: any_candidates.clone(),
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: "core.human_review".to_owned(),
            display_name: "Human review".to_owned(),
            required_capabilities: Vec::new(),
            accepts: any_candidates.clone(),
            produces: any_candidates.clone(),
            deterministic: true,
        },
        VisionNodeDescriptor {
            id: "core.commit".to_owned(),
            display_name: "Commit annotations".to_owned(),
            required_capabilities: Vec::new(),
            accepts: any_candidates.clone(),
            produces: any_candidates,
            deterministic: true,
        },
    ] {
        nodes.register(descriptor)?;
    }

    let one = PortCardinality::One;
    let many = PortCardinality::Many;
    let definitions = vec![
        NodeDefinition {
            id: "core.image_input".to_owned(),
            display_name: "Image input".to_owned(),
            category: NodeCategory::Input,
            input_ports: Vec::new(),
            output_ports: vec![catalog_port("image", ArtifactKind::Image, true, one)],
            config_schema: node_schema(json!({})),
            required_model_capability: None,
            cardinality: NodeCardinality::OneToOne,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        },
        NodeDefinition {
            id: "core.existing_annotations".to_owned(),
            display_name: "Existing annotations".to_owned(),
            category: NodeCategory::Input,
            input_ports: Vec::new(),
            output_ports: vec![catalog_port(
                "candidates",
                ArtifactKind::AnnotationCandidateSet,
                true,
                many,
            )],
            config_schema: node_schema(json!({
                "task_id": {"type": "string"},
                "labels": {"type": "array", "items": {"type": "string"}}
            })),
            required_model_capability: None,
            cardinality: NodeCardinality::OneToMany,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: true,
        },
        NodeDefinition {
            id: "core.resize".to_owned(),
            display_name: "Resize image".to_owned(),
            category: NodeCategory::ImagePreparation,
            input_ports: vec![catalog_port("image", ArtifactKind::Image, true, one)],
            output_ports: vec![catalog_port("image", ArtifactKind::Image, true, one)],
            config_schema: node_schema(json!({
                "target_width": {"type": "integer", "minimum": 1},
                "target_height": {"type": "integer", "minimum": 1},
                "max_edge": {"type": "integer", "minimum": 1},
                "maximum_pixels": {"type": "integer", "minimum": 1},
                "allow_upscale": {"type": "boolean", "default": false}
            })),
            required_model_capability: None,
            cardinality: NodeCardinality::OneToOne,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        },
        NodeDefinition {
            id: "core.tile".to_owned(),
            display_name: "Tile image".to_owned(),
            category: NodeCategory::ImagePreparation,
            input_ports: vec![catalog_port("image", ArtifactKind::Image, true, one)],
            output_ports: vec![catalog_port("images", ArtifactKind::Image, true, many)],
            config_schema: node_schema(json!({
                "tile_size": {"type": "integer", "minimum": 1, "default": 1024},
                "tile_width": {"type": "integer", "minimum": 1},
                "tile_height": {"type": "integer", "minimum": 1},
                "overlap": {"type": "number", "minimum": 0, "exclusiveMaximum": 0.9, "default": 0.15},
                "maximum_tiles": {"type": "integer", "minimum": 1, "default": 64},
                "merge_policy": {"type": "string", "enum": ["nms", "deduplicate", "preserve"], "default": "nms"}
            })),
            required_model_capability: None,
            cardinality: NodeCardinality::OneToMany,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        },
        NodeDefinition {
            id: "core.crop".to_owned(),
            display_name: "Crop candidates".to_owned(),
            category: NodeCategory::ImagePreparation,
            input_ports: vec![
                catalog_port("image", ArtifactKind::Image, true, one),
                catalog_port("detections", ArtifactKind::DetectionSet, true, one),
            ],
            output_ports: vec![catalog_port("crops", ArtifactKind::CropSet, true, many)],
            config_schema: node_schema(json!({
                "padding": {"type": "number", "minimum": 0, "maximum": 0.5, "default": 0}
            })),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        },
        NodeDefinition {
            id: annotagent_runtime::CORE_DETECTIONS_TO_BOX_PROMPTS.to_owned(),
            display_name: "Detections to box prompts".to_owned(),
            category: NodeCategory::ResultTransform,
            input_ports: vec![catalog_port(
                "detections",
                ArtifactKind::DetectionSet,
                true,
                many,
            )],
            output_ports: vec![catalog_port(
                "prompts",
                ArtifactKind::BoxPromptSet,
                true,
                many,
            )],
            config_schema: node_schema(json!({
                "padding": {"type": "number", "minimum": 0, "maximum": 0.5, "default": 0}
            })),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        },
        model_node_definition(
            "capability.detect",
            "Detect objects",
            vec![catalog_port("images", ArtifactKind::Image, true, many)],
            catalog_port("detections", ArtifactKind::DetectionSet, true, many),
            ModelCapability::ObjectDetection,
        ),
        model_node_definition(
            "capability.classify",
            "Classify images or crops",
            vec![
                catalog_port("images", ArtifactKind::Image, false, many),
                catalog_port("crops", ArtifactKind::CropSet, false, many),
            ],
            catalog_port(
                "classifications",
                ArtifactKind::ClassificationSet,
                true,
                many,
            ),
            ModelCapability::ImageClassification,
        ),
        model_node_definition(
            "capability.segment",
            "Segment regions",
            vec![
                catalog_port("images", ArtifactKind::Image, true, many),
                catalog_port("box_prompts", ArtifactKind::BoxPromptSet, false, many),
                catalog_port("point_prompts", ArtifactKind::PointPromptSet, false, many),
            ],
            catalog_port("masks", ArtifactKind::MaskSet, true, many),
            ModelCapability::PromptedSegmentation,
        ),
        NodeDefinition {
            id: annotagent_runtime::CORE_MASK_TO_BBOX.to_owned(),
            display_name: "Mask to bounding box".to_owned(),
            category: NodeCategory::ResultTransform,
            input_ports: vec![
                catalog_port("masks", ArtifactKind::MaskSet, true, many),
                catalog_port("box_prompts", ArtifactKind::BoxPromptSet, true, many),
            ],
            output_ports: vec![catalog_port(
                "detections",
                ArtifactKind::DetectionSet,
                true,
                many,
            )],
            config_schema: node_schema(json!({})),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        },
        NodeDefinition {
            id: annotagent_runtime::CORE_MASK_TO_POLYGON.to_owned(),
            display_name: "Mask to polygon".to_owned(),
            category: NodeCategory::ResultTransform,
            input_ports: vec![catalog_port("masks", ArtifactKind::MaskSet, true, many)],
            output_ports: vec![catalog_port(
                "polygons",
                ArtifactKind::PolygonSet,
                true,
                many,
            )],
            config_schema: node_schema(json!({
                "encoding": {"type": "string", "enum": ["polygon"], "default": "polygon"}
            })),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: true,
        },
        NodeDefinition {
            id: "core.select_and_map".to_owned(),
            display_name: "Select and map results".to_owned(),
            category: NodeCategory::ResultTransform,
            input_ports: vec![catalog_port(
                "detections",
                ArtifactKind::DetectionSet,
                true,
                many,
            )],
            output_ports: vec![catalog_port(
                "detections",
                ArtifactKind::DetectionSet,
                true,
                many,
            )],
            config_schema: node_schema(json!({
                "minimum_confidence": {"type": "number", "minimum": 0, "maximum": 1, "default": 0},
                "class_ids": {"type": "array", "items": {"type": "string"}},
                "labels": {"type": "array", "items": {"type": "string"}},
                "queries": {"type": "array", "items": {"type": "string"}},
                "class_mapping": {"type": "object", "additionalProperties": {"type": "string"}},
                "drop_unknown_labels": {"type": "boolean", "default": false}
            })),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        },
        NodeDefinition {
            id: "core.project_coordinates".to_owned(),
            display_name: "Project coordinates".to_owned(),
            category: NodeCategory::ResultTransform,
            input_ports: vec![
                catalog_port("images", ArtifactKind::Image, true, many),
                catalog_port("detections", ArtifactKind::DetectionSet, true, many),
            ],
            output_ports: vec![catalog_port(
                "detections",
                ArtifactKind::DetectionSet,
                true,
                many,
            )],
            config_schema: node_schema(json!({})),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        },
        NodeDefinition {
            id: "core.attach_result".to_owned(),
            display_name: "Attach result".to_owned(),
            category: NodeCategory::ResultTransform,
            input_ports: vec![
                catalog_port("detections", ArtifactKind::DetectionSet, true, many),
                catalog_port(
                    "classifications",
                    ArtifactKind::ClassificationSet,
                    true,
                    many,
                ),
            ],
            output_ports: vec![catalog_port(
                "candidates",
                ArtifactKind::AnnotationCandidateSet,
                true,
                many,
            )],
            config_schema: node_schema(json!({
                "task_id": {"type": "string"},
                "class_mapping": {"type": "object", "additionalProperties": {"type": "string"}}
            })),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::None,
            dry_run_supported: true,
            expert_only: false,
        },
        simple_candidate_definition(
            "core.combine_evidence",
            "Combine model evidence",
            NodeCategory::EvidenceAndValidation,
            ArtifactKind::DetectionSet,
            ArtifactKind::CandidateClusterSet,
            node_schema(json!({
                "method": {"type": "string", "enum": ["iou"], "default": "iou"},
                "minimum_iou": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.5},
                "preserve_unmatched": {"type": "boolean", "default": true}
            })),
        ),
        simple_candidate_definition(
            "core.validate",
            "Validate results",
            NodeCategory::EvidenceAndValidation,
            ArtifactKind::AnnotationCandidateSet,
            ArtifactKind::AnnotationCandidateSet,
            node_schema(json!({
                "validators": {"type": "array", "items": {"type": "string"}}
            })),
        ),
        simple_candidate_definition(
            "core.decision",
            "Decision",
            NodeCategory::EvidenceAndValidation,
            ArtifactKind::AnnotationCandidateSet,
            ArtifactKind::AnnotationCandidateSet,
            node_schema(json!({
                "mode": {"type": "string", "enum": ["confidence", "evidence", "domain_policy"], "default": "confidence"},
                "threshold": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.5}
            })),
        ),
        NodeDefinition {
            id: "core.human_review".to_owned(),
            display_name: "Human review".to_owned(),
            category: NodeCategory::HumanAndOutput,
            input_ports: vec![catalog_port(
                "candidates",
                ArtifactKind::AnnotationCandidateSet,
                true,
                many,
            )],
            output_ports: vec![catalog_port(
                "approved",
                ArtifactKind::AnnotationCandidateSet,
                true,
                many,
            )],
            config_schema: node_schema(json!({})),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::HumanSuspension,
            dry_run_supported: true,
            expert_only: false,
        },
        NodeDefinition {
            id: "core.commit".to_owned(),
            display_name: "Commit annotations".to_owned(),
            category: NodeCategory::HumanAndOutput,
            input_ports: vec![catalog_port(
                "candidates",
                ArtifactKind::AnnotationCandidateSet,
                true,
                many,
            )],
            output_ports: vec![catalog_port(
                "annotations",
                ArtifactKind::AnnotationCandidateSet,
                true,
                many,
            )],
            config_schema: node_schema(json!({})),
            required_model_capability: None,
            cardinality: NodeCardinality::ManyToMany,
            side_effect: NodeSideEffect::AnnotationCommit,
            dry_run_supported: false,
            expert_only: false,
        },
    ];
    for definition in definitions {
        nodes.register_definition(definition)?;
    }
    for (id, display_name, scope, properties) in [
        (
            "cache",
            "Artifact cache",
            RuntimePolicyScope::Runtime,
            json!({"enabled": {"type": "boolean"}}),
        ),
        (
            "replay",
            "Replay",
            RuntimePolicyScope::Runtime,
            json!({"enabled": {"type": "boolean"}}),
        ),
        (
            "retry",
            "Retry",
            RuntimePolicyScope::Node,
            json!({"maximum_attempts": {"type": "integer", "minimum": 0}}),
        ),
        (
            "timeout",
            "Timeout",
            RuntimePolicyScope::Node,
            json!({"seconds": {"type": "integer", "minimum": 1}}),
        ),
        (
            "budget",
            "Budget",
            RuntimePolicyScope::Workflow,
            json!({"maximum_cost": {"type": "string"}}),
        ),
        (
            "usage_tracking",
            "Usage tracking",
            RuntimePolicyScope::Runtime,
            json!({"enabled": {"type": "boolean"}}),
        ),
        (
            "checkpoint",
            "Checkpoint",
            RuntimePolicyScope::Runtime,
            json!({"enabled": {"type": "boolean"}}),
        ),
        (
            "run_control",
            "Pause, resume, and cancel",
            RuntimePolicyScope::Runtime,
            json!({"enabled": {"type": "boolean"}}),
        ),
        (
            "history",
            "Run history",
            RuntimePolicyScope::Runtime,
            json!({"retention_days": {"type": "integer", "minimum": 1}}),
        ),
    ] {
        nodes.register_runtime_policy(RuntimePolicyDefinition {
            id: id.to_owned(),
            display_name: display_name.to_owned(),
            scope,
            config_schema: node_schema(properties),
        })?;
    }
    Ok(())
}

fn model_node_definition(
    id: &str,
    display_name: &str,
    input_ports: Vec<PortDefinition>,
    output_port: PortDefinition,
    capability: ModelCapability,
) -> NodeDefinition {
    NodeDefinition {
        id: id.to_owned(),
        display_name: display_name.to_owned(),
        category: NodeCategory::ModelInference,
        input_ports,
        output_ports: vec![output_port],
        config_schema: node_schema(json!({
            "model_binding": {"type": "string"},
            "queries": {"type": "array", "items": {"type": "string"}}
        })),
        required_model_capability: Some(capability),
        cardinality: NodeCardinality::ManyToMany,
        side_effect: NodeSideEffect::None,
        dry_run_supported: true,
        expert_only: false,
    }
}

fn simple_candidate_definition(
    id: &str,
    display_name: &str,
    category: NodeCategory,
    input: ArtifactKind,
    output: ArtifactKind,
    config_schema: serde_json::Value,
) -> NodeDefinition {
    NodeDefinition {
        id: id.to_owned(),
        display_name: display_name.to_owned(),
        category,
        input_ports: vec![catalog_port("input", input, true, PortCardinality::Many)],
        output_ports: vec![catalog_port("output", output, true, PortCardinality::Many)],
        config_schema,
        required_model_capability: None,
        cardinality: NodeCardinality::ManyToMany,
        side_effect: NodeSideEffect::None,
        dry_run_supported: true,
        expert_only: false,
    }
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
            let classifier_binding = preferred_model_for(
                models,
                constraints.preferred_model_id.as_deref(),
                &[VisionCapability::Classification],
            )?
            .map(|(model_id, capability)| PipelineModelBinding {
                model_id,
                capability,
                configuration: BTreeMap::new(),
            });
            let classifier = PipelineStep {
                id: classifier_id.clone(),
                node_type: annotagent_skill_classification::CLASSIFICATION_OPERATION.to_owned(),
                kind: WorkflowNodeKind::VisionModel,
                inputs: BTreeMap::from([("subjects".to_owned(), PipelineSource::Image)]),
                outputs: BTreeMap::from([(
                    "classifications".to_owned(),
                    ArtifactKind::ClassificationSet,
                )]),
                model_binding: classifier_binding,
                skill_binding: None,
                parameters: BTreeMap::from([
                    ("labels".to_owned(), json!(task.labels)),
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
            let (model_binding, node_type, kind, parameters) = if let Some((model_id, capability)) =
                preferred
            {
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
                        annotagent_skill_object_detection::OBJECT_DETECTION_OPERATION.to_owned(),
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
                (
                    Some(PipelineModelBinding {
                        model_id,
                        capability,
                        configuration: BTreeMap::new(),
                    }),
                    node_type,
                    kind,
                    parameters,
                )
            } else {
                (
                    None,
                    annotagent_skill_vlm_detection::VLM_DETECTION_OPERATION.to_owned(),
                    WorkflowNodeKind::VisionLanguageModel,
                    BTreeMap::from([
                        ("labels".to_owned(), json!([target_label])),
                        (
                            "target_description".to_owned(),
                            json!(format!("the {target_label} object itself")),
                        ),
                    ]),
                )
            };
            let detector = PipelineStep {
                id: detector_id.clone(),
                node_type,
                kind,
                inputs: BTreeMap::from([("image".to_owned(), PipelineSource::Image)]),
                outputs: BTreeMap::from([("detections".to_owned(), ArtifactKind::DetectionSet)]),
                model_binding,
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
        for capability in allowed_capabilities {
            if let Some(model) = models
                .models()
                .into_iter()
                .filter(|model| {
                    model.capabilities.contains(capability)
                        && (model.status == ModelAvailabilityStatus::Available
                            || model.health.status == VisionModelHealthStatus::Healthy)
                })
                .min_by_key(|model| model.capabilities.len())
            {
                return Ok(Some((model.id, *capability)));
            }
        }
        return Ok(None);
    };
    let (model, _) = models.resolve(model_id).map_err(|error| anyhow!(error))?;
    if matches!(
        model.status,
        ModelAvailabilityStatus::Unreachable
            | ModelAvailabilityStatus::Misconfigured
            | ModelAvailabilityStatus::IncompatibleProtocol
            | ModelAvailabilityStatus::MissingWeights
            | ModelAvailabilityStatus::Disabled
    ) || matches!(
        model.health.status,
        VisionModelHealthStatus::Degraded | VisionModelHealthStatus::Unavailable
    ) {
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
                self.application
                    .validate_published_registry_models(&published)?;
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

#[derive(Clone, Copy)]
struct DryRunRuntimeProvider<'a> {
    kind: &'a str,
    api_key: Option<&'a str>,
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

    fn legacy_registry_import(&self, settings: &Settings) -> Result<LegacyRegistryImport> {
        let adapter = match settings.default_provider.as_str() {
            "mock" => ProviderAdapterKind::Mock,
            "openai_compatible" => ProviderAdapterKind::OpenAiCompatible,
            other => bail!("legacy Provider kind {other:?} cannot be imported"),
        };
        let fingerprint_material = format!(
            "{}\n{}\n{}\n{}",
            settings.default_provider,
            settings.provider.endpoint.trim_end_matches('/'),
            settings.provider.model,
            settings.provider.api_key_env,
        );
        let fingerprint = annotagent_image_tools::sha256(fingerprint_material.as_bytes());
        let provider_id = ProviderId(uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("annotagent:legacy-provider:{fingerprint}").as_bytes(),
        ));
        let model_id = ModelProfileId(uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!(
                "annotagent:legacy-model:{provider_id}:{}",
                settings.provider.model
            )
            .as_bytes(),
        ));
        let now = chrono::Utc::now();
        let legacy_credential_path = self
            .workspace
            .join(".annotagent/credentials/provider-api-key");
        let credential_ref = match adapter {
            ProviderAdapterKind::Mock => None,
            ProviderAdapterKind::OpenAiCompatible if legacy_credential_path.is_file() => {
                Some(CredentialReference {
                    provider_id,
                    source: CredentialSource::LegacyWorkspaceFile,
                    locator: "legacy-workspace-provider-api-key".to_owned(),
                })
            }
            ProviderAdapterKind::OpenAiCompatible => Some(CredentialReference {
                provider_id,
                source: CredentialSource::EnvironmentVariable,
                locator: settings.provider.api_key_env.clone(),
            }),
        };
        let provider = ProviderProfile {
            id: provider_id,
            display_name: format!("Imported {}", settings.provider.model),
            preset_id: Some("legacy-settings".to_owned()),
            adapter,
            base_url: settings
                .provider
                .endpoint
                .parse()
                .context("legacy Provider endpoint is not a valid URL")?,
            organization: None,
            workspace: Some("compatibility-settings".to_owned()),
            credential_ref,
            safe_headers: settings.provider.custom_headers.clone(),
            connection_policy: ProviderConnectionPolicy {
                request_timeout_seconds: settings.provider.request_timeout_seconds,
                maximum_retries: settings.provider.max_retries,
                ..ProviderConnectionPolicy::default()
            },
            enabled: true,
            health: ProviderHealthSnapshot {
                status: if adapter == ProviderAdapterKind::Mock {
                    ProviderHealthStatus::Available
                } else {
                    ProviderHealthStatus::Configured
                },
                safe_message: Some(
                    "Imported from compatibility Settings; run a connection check before live use."
                        .to_owned(),
                ),
                checked_at: (adapter == ProviderAdapterKind::Mock).then_some(now),
            },
            created_at: now,
            updated_at: now,
        };
        let pricing_is_known = settings.pricing.input_per_million_tokens
            != rust_decimal::Decimal::ZERO
            || settings.pricing.output_per_million_tokens != rust_decimal::Decimal::ZERO
            || settings.pricing.per_image != rust_decimal::Decimal::ZERO
            || settings.pricing.per_request != rust_decimal::Decimal::ZERO;
        let temperature = settings
            .provider
            .temperature
            .to_string()
            .parse::<rust_decimal::Decimal>()
            .ok();
        let model = ModelProfile {
            id: model_id,
            revision: 1,
            provider_id,
            display_name: format!("{} (legacy default-vision)", settings.provider.model),
            remote_model_id: settings.provider.model.clone(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures {
                tool_calls: settings.provider.supports_tool_calls,
                structured_output: settings.provider.supports_json_schema,
                json_schema: settings.provider.supports_json_schema,
                usage_reporting: true,
                ..ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([
                ModelCapability::TextGeneration,
                ModelCapability::VisionLanguage,
                ModelCapability::ImageClassification,
            ]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits {
                maximum_output_tokens: Some(u64::from(settings.provider.max_output_tokens)),
                ..ModelLimits::default()
            },
            generation_defaults: GenerationDefaults {
                temperature,
                maximum_output_tokens: Some(u64::from(settings.provider.max_output_tokens)),
                reasoning_mode: settings.provider.reasoning_mode.clone(),
                ..GenerationDefaults::default()
            },
            pricing: ModelPricing {
                currency: settings.pricing.currency.clone(),
                input_per_million_tokens: pricing_is_known
                    .then_some(settings.pricing.input_per_million_tokens),
                output_per_million_tokens: pricing_is_known
                    .then_some(settings.pricing.output_per_million_tokens),
                per_image: pricing_is_known.then_some(settings.pricing.per_image),
                per_request: pricing_is_known.then_some(settings.pricing.per_request),
                source: if pricing_is_known {
                    PricingSource::UserConfigured
                } else {
                    PricingSource::Unknown
                },
                updated_at: pricing_is_known.then_some(now),
                ..ModelPricing::default()
            },
            status: if adapter == ProviderAdapterKind::Mock {
                ModelProfileStatus::Available
            } else {
                ModelProfileStatus::Unverified
            },
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        let mut project_bindings = Vec::new();
        for entry in std::fs::read_dir(&self.workspace)? {
            let entry = entry?;
            if entry.file_type()?.is_symlink() || !entry.file_type()?.is_dir() {
                continue;
            }
            let project_path = entry.path().join("project.yaml");
            if !project_path.is_file()
                || load_project_schema_with_registry(&project_path, &self.skills).is_err()
            {
                continue;
            }
            let project_id = stable_project_id(&entry.path());
            project_bindings.push(ProjectModelBinding {
                id: ModelBindingId(uuid::Uuid::new_v5(
                    &uuid::Uuid::NAMESPACE_URL,
                    format!("annotagent:legacy-default-vision:{project_id}:{model_id}").as_bytes(),
                )),
                project_id,
                capability: ModelCapability::VisionLanguage,
                role: ModelBindingRole::PrimaryInference,
                match_kind: ModelBindingMatch::Role,
                model_profile_id: model_id,
                locked: true,
                created_at: now,
            });
        }
        project_bindings.sort_by_key(|binding| binding.project_id);
        Ok(LegacyRegistryImport {
            fingerprint,
            provider,
            model,
            project_bindings,
        })
    }

    pub fn preview_legacy_registry_import(
        &self,
        settings: &Settings,
    ) -> Result<LegacyRegistryImportPreview> {
        let import = self.legacy_registry_import(settings)?;
        let already_applied = self
            .store
            .legacy_registry_import_report(&import.fingerprint)?
            .is_some();
        Ok(LegacyRegistryImportPreview {
            fingerprint: import.fingerprint,
            provider_id: import.provider.id,
            provider_display_name: import.provider.display_name.clone(),
            provider_adapter: import.provider.adapter,
            endpoint_summary: import.provider.endpoint_summary(),
            model_profile_id: import.model.id,
            model_display_name: import.model.display_name,
            remote_model_id: import.model.remote_model_id,
            capability_source: import.model.capability_source,
            credential_source: import
                .provider
                .credential_ref
                .map(|reference| reference.source),
            project_binding_count: import.project_bindings.len(),
            already_applied,
            moves_secret: false,
            modifies_historical_runs: false,
        })
    }

    pub fn apply_legacy_registry_import(
        &self,
        settings: &Settings,
    ) -> Result<LegacyRegistryImportReport> {
        let import = self.legacy_registry_import(settings)?;
        Ok(self.store.apply_legacy_registry_import(&import)?)
    }

    /// Resolve the model that may drive a Pipeline Builder session without making a Provider
    /// request. Selection priority is explicit request, Project binding, then global default.
    pub fn resolve_pipeline_builder_model(
        &self,
        project_id: &str,
        explicit_model_profile_id: Option<ModelProfileId>,
    ) -> Result<PipelineBuilderModelRuntime> {
        let project_path = self.project_path(project_id)?;
        let stable_id = stable_project_id(project_path.parent().unwrap_or(&self.workspace));
        let project_bindings = self.store.list_project_model_bindings(stable_id)?;
        let defaults = self.store.get_global_model_defaults()?;
        let resolved = resolve_model_binding(
            explicit_model_profile_id,
            &project_bindings,
            &defaults,
            ModelCapability::TextGeneration,
            ModelBindingRole::PipelineBuilder,
        )
        .map_err(|error| {
            anyhow!(
                "Provider setup required: choose a compatible Pipeline Builder Model Profile ({error})"
            )
        })?;
        let model = self
            .store
            .get_model_profile(resolved.model_profile_id, None)?;
        let provider = self.store.get_provider_profile(model.provider_id)?;
        if !model.enabled || model.status != ModelProfileStatus::Available {
            bail!("selected Pipeline Builder Model Profile is disabled or unavailable");
        }
        if !model.input_modalities.contains(&InputModality::Text)
            || !model
                .task_capabilities
                .contains(&ModelCapability::TextGeneration)
        {
            bail!("selected Pipeline Builder Model Profile requires text input and TextGeneration");
        }
        if !model.protocol_features.tool_calls || !model.protocol_features.structured_output {
            bail!(
                "selected Pipeline Builder Model Profile requires ToolCalls and StructuredOutput"
            );
        }
        if !provider.enabled
            || !matches!(
                provider.health.status,
                ProviderHealthStatus::Available | ProviderHealthStatus::Configured
            )
        {
            bail!("selected Pipeline Builder Provider is disabled or unavailable");
        }
        if provider.adapter != ProviderAdapterKind::Mock && provider.credential_ref.is_none() {
            bail!("Provider setup required: configure a credential reference before using Agent");
        }
        Ok(PipelineBuilderModelRuntime {
            provider,
            model,
            binding_source: resolved.source,
            locked: resolved.locked,
        })
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
        drafts.sort_by_key(|draft| std::cmp::Reverse(draft.updated_at));
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
                        PipelineArtifact::Image(_)
                        | PipelineArtifact::BoxPromptSet(_)
                        | PipelineArtifact::PointPromptSet(_)
                        | PipelineArtifact::MaskSet(_)
                        | PipelineArtifact::PolygonSet(_)
                        | PipelineArtifact::CropSet(_) => {}
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
        let mut drafts = self.store.list_workflow_drafts(project_id)?;
        for draft in &mut drafts {
            if !matches!(
                draft.status,
                WorkflowDraftStatus::Published | WorkflowDraftStatus::Archived
            ) && migrate_legacy_expert_workflow(draft)?
            {
                self.store.save_workflow_draft(draft)?;
            }
        }
        Ok(drafts)
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
        let provider_profiles = self.store.list_provider_profiles()?;
        let model_profiles = self.store.list_model_profiles(None, false)?;
        let model_counts = model_profiles
            .iter()
            .fold(BTreeMap::new(), |mut counts, model| {
                *counts.entry(model.provider_id).or_insert(0_usize) += 1;
                counts
            });
        let provider_profiles = provider_profiles
            .into_iter()
            .map(|profile| {
                let endpoint_summary = profile.endpoint_summary();
                PipelineBuilderProviderProfile {
                    id: profile.id,
                    display_name: profile.display_name,
                    adapter: profile.adapter,
                    endpoint_summary,
                    enabled: profile.enabled,
                    health_status: profile.health.status,
                    credential_configured: profile.adapter == ProviderAdapterKind::Mock
                        || profile.credential_ref.is_some(),
                    model_count: model_counts.get(&profile.id).copied().unwrap_or_default(),
                }
            })
            .collect();
        Ok(WorkflowAdvisorInput {
            project_id: project_id.to_owned(),
            project_schema: project,
            target_task_id: target_task_id.map(TaskId::from),
            target_label: target_label.map(LabelId::from),
            enabled_skills,
            node_catalog: nodes.definitions(),
            runtime_policies: nodes.runtime_policies(),
            provider_profiles,
            model_profiles,
            expert_models: models.expert_manifests(),
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
            if settings.default_provider != "mock" {
                for node in &mut draft.nodes {
                    if node
                        .model_binding
                        .as_deref()
                        .is_some_and(|binding| binding.to_ascii_lowercase().starts_with("mock"))
                    {
                        node.model_binding = None;
                    }
                }
            }
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
                runtime_policies: BTreeMap::new(),
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
                let _ignored = self.store.save_agent_session(session);
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
            if self.store.save_agent_session(session).is_err() {
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
            "inspect_existing_pipeline",
            json!({}),
            json!({
                "draft_count": self.store.list_workflow_drafts(Some(project_id))?.len(),
                "published_count": self.store.list_published_workflow_versions(Some(project_id))?.len(),
            }),
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
            "list_node_definitions",
            json!({"skills": input.enabled_skills}),
            json!({"nodes": input.node_catalog}),
        ) || !record(
            &mut session,
            "list_provider_profiles",
            json!({}),
            json!({"providers": input.provider_profiles, "secrets_exposed": false}),
        ) || !record(
            &mut session,
            "list_available_capabilities",
            json!({}),
            json!({
                "available_expert_models": input.expert_models.iter()
                    .filter(|model| model.availability == ModelAvailability::Available)
                    .map(|model| json!({"model_id": model.model_id, "capabilities": model.capabilities}))
                    .collect::<Vec<_>>(),
                "setup_only_alternatives": input.expert_models.iter()
                    .filter(|model| model.availability != ModelAvailability::Available)
                    .map(|model| json!({"model_id": model.model_id, "availability": model.availability, "applied": false}))
                    .collect::<Vec<_>>(),
            }),
        ) || !record(
            &mut session,
            "list_compatible_models",
            json!({}),
            json!({"models": compatible_builder_models(&input, None)}),
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
        let dry_run_image_indices =
            (0..self.list_project_images(project_id)?.len().min(3)).collect::<Vec<_>>();
        let mut dry_run = self
            .dry_run_workflow_samples(&revised.draft.id, settings, &dry_run_image_indices)
            .await?;
        let first_observation = agent_dry_run_summary(&dry_run, &revised.draft);
        if !record(
            &mut session,
            "dry_run_pipeline",
            json!({"draft_id": revised.draft.id, "image_limit": dry_run_image_indices.len()}),
            json!({"sandbox": dry_run.sandbox, "summary": first_observation}),
        ) || !record(
            &mut session,
            "inspect_dry_run_summary",
            json!({"draft_id": revised.draft.id}),
            json!({
                "summary": first_observation,
                "review_rate": first_observation.review_rate(),
            }),
        ) || !record(
            &mut session,
            "inspect_failure_classes",
            json!({"limit": 3}),
            json!({
                "provider_failure_count": first_observation.provider_failure_count,
                "worker_failure_count": first_observation.worker_failure_count,
                "no_candidate_count": first_observation.no_candidate_count,
                "semantic_review_count": first_observation.semantic_review_count,
                "geometry_review_count": first_observation.geometry_review_count,
                "domain_risk_count": first_observation.domain_risk_count,
            }),
        ) || !record(
            &mut session,
            "inspect_geometry_quality",
            json!({"limit": 3}),
            json!({"summary": first_observation.geometry_quality}),
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
                .dry_run_workflow_samples(&revised.draft.id, settings, &dry_run_image_indices)
                .await?;
            let second_observation = agent_dry_run_summary(&dry_run, &revised.draft);
            if !record(
                &mut session,
                "dry_run_pipeline",
                json!({"draft_id": revised.draft.id, "image_limit": dry_run_image_indices.len(), "revision": 3}),
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
                "set_node_configuration",
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

    /// Run the constrained Agent loop with a Registry-resolved model and an already constructed
    /// Provider adapter. Credential resolution stays in the caller and never enters this API.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_workflow_advisor_with_selected_model(
        &self,
        project_id: &str,
        settings: &Settings,
        selected_model: &PipelineBuilderModelRuntime,
        provider: &dyn VisionModelProvider,
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
        self.run_workflow_advisor_loop(
            project_id,
            settings,
            constraints,
            target,
            input,
            suggestion,
            provider,
            Some(selected_model),
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
        self.run_workflow_advisor_loop(
            project_id,
            settings,
            constraints,
            target,
            input,
            safe_suggestion,
            provider,
            None,
            builder_constraints,
            cancellation,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_workflow_advisor_loop(
        &self,
        project_id: &str,
        settings: &Settings,
        constraints: &WorkflowConstraints,
        target: Option<(&str, &str)>,
        input: WorkflowAdvisorInput,
        safe_suggestion: WorkflowSuggestion,
        provider: &dyn VisionModelProvider,
        selected_model: Option<&PipelineBuilderModelRuntime>,
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
        if let Some(selected_model) = selected_model {
            session = session.with_model_selection(selected_model.safe_selection());
        }
        self.agent_cancellations
            .lock()
            .map_err(|_| anyhow!("Agent cancellation registry lock poisoned"))?
            .insert(session.id, cancellation.clone());
        self.store.save_agent_session(&session)?;
        let mut messages = vec![
            ModelMessage {
                role: ModelRole::System,
                content: PIPELINE_BUILDER_SYSTEM_PROMPT.to_owned(),
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
        let tools = pipeline_builder_live_tools(&input);
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
        let mut draft_history = PipelineDraftHistory::default();
        let required_advisor_resource = input
            .resource_ids
            .iter()
            .find(|resource| resource.ends_with("advisor.md"))
            .cloned();
        let mut loaded_resources = BTreeSet::new();
        let mut failed_tool_attempts = BTreeMap::<String, u32>::new();
        let mut provider_turns = 0_u32;
        let mut consecutive_no_tool_responses = 0_u32;

        while session.status == AgentSessionStatus::Running {
            if cancellation.is_cancelled() {
                session.cancel();
                break;
            }
            if session.usage.steps >= session.budget.max_steps {
                session.fail("maximum Pipeline Builder Tool Calls reached");
                break;
            }
            if provider_turns >= builder_constraints.maximum_agent_turns {
                session.fail("maximum Pipeline Builder turns reached");
                break;
            }
            compact_pipeline_builder_messages(
                &mut messages,
                selected_model.and_then(|selected| selected.model.limits.context_tokens),
            );
            let remote_model_id = selected_model.map_or_else(
                || settings.provider.model.clone(),
                |selected| selected.model.remote_model_id.clone(),
            );
            let maximum_output_tokens =
                selected_model.map_or(settings.provider.max_output_tokens, |selected| {
                    selected
                        .model
                        .generation_defaults
                        .maximum_output_tokens
                        .or(selected.model.limits.maximum_output_tokens)
                        .unwrap_or(u64::from(settings.provider.max_output_tokens))
                        .min(u64::from(u32::MAX)) as u32
                });
            let temperature = selected_model.map_or(0.0, |selected| {
                selected
                    .model
                    .generation_defaults
                    .temperature
                    .map_or(0.0, |value| value.to_string().parse().unwrap_or(0.0))
            });
            let started_at = Instant::now();
            let response = match provider
                .complete(
                    ModelRequest {
                        model: remote_model_id.clone(),
                        task_id: "pipeline_builder".into(),
                        messages: messages.clone(),
                        images: Vec::new(),
                        tools: tools.clone(),
                        max_output_tokens: maximum_output_tokens,
                        temperature,
                        extra: BTreeMap::from([(
                            "parallel_tool_calls".to_owned(),
                            serde_json::Value::Bool(false),
                        )]),
                    },
                    cancellation.clone(),
                )
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    let (cost, currency) = pipeline_builder_call_cost(
                        selected_model.map(|selected| &selected.model),
                        0,
                        0,
                    );
                    session.record_model_call(AgentModelCall {
                        sequence: 0,
                        provider_profile_id: selected_model.map(|selected| selected.provider.id),
                        model_profile_id: selected_model.map(|selected| selected.model.id),
                        model_profile_revision: selected_model
                            .map(|selected| selected.model.revision),
                        provider_name: provider.name().to_owned(),
                        remote_model_id: remote_model_id.clone(),
                        request_id: None,
                        input_tokens: 0,
                        output_tokens: 0,
                        usage_source: UsageSource::Unknown,
                        duration_ms: u64::try_from(started_at.elapsed().as_millis())
                            .unwrap_or(u64::MAX),
                        cost,
                        currency,
                        retry_count: 0,
                        succeeded: false,
                        safe_error: Some("Provider request failed".to_owned()),
                        created_at: chrono::Utc::now(),
                    });
                    session.fail(format!("Pipeline Builder provider error: {error}"));
                    break;
                }
            };
            let input_tokens = response.usage.input_tokens.unwrap_or_default();
            let output_tokens = response.usage.output_tokens.unwrap_or_default();
            provider_turns = provider_turns.saturating_add(1);
            let retry_count = response
                .provider_metadata
                .get("retry_count")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or_default();
            let (cost, currency) = pipeline_builder_call_cost(
                selected_model.map(|selected| &selected.model),
                input_tokens,
                output_tokens,
            );
            session.record_model_call(AgentModelCall {
                sequence: 0,
                provider_profile_id: selected_model.map(|selected| selected.provider.id),
                model_profile_id: selected_model.map(|selected| selected.model.id),
                model_profile_revision: selected_model.map(|selected| selected.model.revision),
                provider_name: provider.name().to_owned(),
                remote_model_id,
                request_id: response.request_id.clone(),
                input_tokens,
                output_tokens,
                usage_source: response.usage.source,
                duration_ms: u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                cost,
                currency,
                retry_count,
                succeeded: true,
                safe_error: None,
                created_at: chrono::Utc::now(),
            });
            if session.status != AgentSessionStatus::Running {
                break;
            }
            if response.tool_calls.is_empty() {
                consecutive_no_tool_responses = consecutive_no_tool_responses.saturating_add(1);
                messages.push(ModelMessage {
                    role: ModelRole::Assistant,
                    content: response.content.unwrap_or_default(),
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                });
                if consecutive_no_tool_responses >= 2 {
                    session.fail(
                        "Pipeline Builder provider returned no registered Tool Call twice in succession",
                    );
                    break;
                }
                let required_next_tool = if current.is_none() {
                    "finish the required inspection, then call create_draft_from_template with template_id safe_default"
                } else if !validation.as_ref().is_some_and(|report| report.valid) {
                    "call validate_pipeline"
                } else if dry_run.is_none() {
                    "call dry_run_pipeline with one image index"
                } else if !inspected_dry_run {
                    "call inspect_dry_run_summary"
                } else {
                    "call submit_draft_for_human_approval"
                };
                messages.push(ModelMessage {
                    role: ModelRole::User,
                    content: format!(
                        "Your previous response did not include a registered Tool Call. Do not explain or return prose; {required_next_tool}."
                    ),
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                });
                self.store.save_agent_session(&session)?;
                continue;
            }
            consecutive_no_tool_responses = 0;
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
                    Ok(PipelineBuilderTool::SampleDataset) => Ok(
                        annotagent_core::AgentToolResult::summary(
                            "Inspected bounded image sample metadata",
                            json!({"image_count": input.data_profile.image_count, "sample_width": input.data_profile.sample_width, "sample_height": input.data_profile.sample_height, "mime_types": input.data_profile.mime_types}),
                        ),
                    ),
                    Ok(PipelineBuilderTool::InspectSampleImage) => {
                        let image_index = required_usize_argument(&call.arguments, "image_index")?;
                        let image_path = self
                            .list_project_images(project_id)?
                            .get(image_index)
                            .cloned()
                            .ok_or_else(|| anyhow!("image_index is outside the Project dataset"))?;
                        let image = load_image(&image_path, 40_000_000).map_err(|error| anyhow!(error))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected sample image {image_index}"),
                            json!({
                                "image_index": image_index,
                                "width": image.metadata.width,
                                "height": image.metadata.height,
                                "mime_type": image.metadata.mime_type,
                                "byte_length": image.rgb.len(),
                                "path_exposed": false,
                                "image_bytes_exposed": false,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectExistingPipeline) => {
                        let drafts = self.store.list_workflow_drafts(Some(project_id))?;
                        let published = self
                            .store
                            .list_published_workflow_versions(Some(project_id))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Inspected existing Workflow state",
                            json!({
                                "drafts": drafts.iter().map(|draft| json!({
                                    "id": draft.id,
                                    "name": draft.name,
                                    "status": draft.status,
                                    "node_count": draft.nodes.len(),
                                    "updated_at": draft.updated_at,
                                })).collect::<Vec<_>>(),
                                "published": published.iter().map(|version| json!({
                                    "workflow_id": version.workflow_id,
                                    "version": version.version,
                                    "name": version.draft.name,
                                    "node_count": version.draft.nodes.len(),
                                })).collect::<Vec<_>>(),
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectExistingAutomations) => {
                        let versions = self
                            .store
                            .list_published_workflow_versions(Some(project_id))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Inspected existing published Workflow automations",
                            json!({
                                "published_workflows": versions.iter().map(|version| json!({
                                    "workflow_id": version.workflow_id,
                                    "version": version.version,
                                    "name": version.draft.name,
                                    "published_at": version.published_at,
                                })).collect::<Vec<_>>(),
                                "formal_run_started": false,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::ListEnabledSkills) => {
                        inspected_skills = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Listed enabled Skills",
                            json!({
                                "skill_ids": input.enabled_skills,
                                "declared_resource_ids": input.resource_ids,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::LoadSkillResource) => {
                        let skill_id = required_string_argument(&call.arguments, "skill_id")?;
                        let resource_id =
                            required_string_argument(&call.arguments, "resource_name")?;
                        if !input.enabled_skills.contains(&skill_id) {
                            bail!(
                                "Skill {skill_id:?} is not enabled by this Project; valid skill_id values are: {}",
                                input.enabled_skills.join(", ")
                            );
                        }
                        if !input.resource_ids.contains(&resource_id) {
                            bail!(
                                "Skill resource {resource_id:?} is not declared for this Project; valid resource_name values are: {}",
                                input.resource_ids.join(", ")
                            );
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
                    Ok(PipelineBuilderTool::ListNodeDefinitions) => {
                        inspected_nodes = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Listed available Pipeline nodes",
                            json!({"nodes": input.node_catalog.iter().map(|node| json!({
                                "id": node.id,
                                "name": node.display_name,
                                "category": node.category,
                                "required_model_capability": node.required_model_capability,
                                "input_ports": node.input_ports,
                                "output_ports": node.output_ports,
                                "config_schema": node.config_schema,
                                "cardinality": node.cardinality,
                                "side_effect": node.side_effect,
                                "dry_run_supported": node.dry_run_supported,
                                "expert_only": node.expert_only,
                            })).collect::<Vec<_>>() }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectNodeDefinition) => {
                        let node_type = required_string_argument(&call.arguments, "node_type")?;
                        let definition = input
                            .node_catalog
                            .iter()
                            .find(|node| node.id == node_type)
                            .ok_or_else(|| anyhow!("node definition {node_type:?} is not registered"))?;
                        inspected_nodes = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected Node Definition {node_type}"),
                            serde_json::to_value(definition)?,
                        ))
                    }
                    Ok(PipelineBuilderTool::FindArtifactConversionPath) => {
                        let from = call
                            .arguments
                            .get("from")
                            .cloned()
                            .ok_or_else(|| anyhow!("find_artifact_conversion_path requires from"))
                            .and_then(|value| serde_json::from_value::<ArtifactKind>(value).map_err(Into::into))?;
                        let to = call
                            .arguments
                            .get("to")
                            .cloned()
                            .ok_or_else(|| anyhow!("find_artifact_conversion_path requires to"))
                            .and_then(|value| serde_json::from_value::<ArtifactKind>(value).map_err(Into::into))?;
                        let paths = annotagent_core::ArtifactConversionRegistry::default()
                            .find_conversion_path(from, to, &nodes);
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Found {} legal Artifact conversion path(s)", paths.len()),
                            json!({
                                "from": from,
                                "to": to,
                                "available": !paths.is_empty(),
                                "paths": paths,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::ListProviderProfiles) => Ok(
                        annotagent_core::AgentToolResult::summary(
                            "Listed credential-safe Provider summaries",
                            json!({"providers": input.provider_profiles, "secrets_exposed": false, "credential_locators_exposed": false}),
                        ),
                    ),
                    Ok(PipelineBuilderTool::ListAvailableCapabilities) => {
                        let mut available = BTreeMap::<ModelCapability, BTreeSet<String>>::new();
                        for model in compatible_builder_models(&input, None) {
                            for capability in &model.task_capabilities {
                                available
                                    .entry(*capability)
                                    .or_default()
                                    .insert(model.id.to_string());
                            }
                        }
                        for model in &input.expert_models {
                            if model.availability == ModelAvailability::Available {
                                for capability in &model.capabilities {
                                    available
                                        .entry(*capability)
                                        .or_default()
                                        .insert(model.model_id.clone());
                                }
                            }
                        }
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Listed evidence-backed available capabilities",
                            json!({
                                "available": available,
                                "setup_only_alternatives": input.expert_models.iter()
                                    .filter(|model| model.availability != ModelAvailability::Available)
                                    .map(|model| json!({
                                        "model_id": model.model_id,
                                        "capabilities": model.capabilities,
                                        "availability": model.availability,
                                        "requires_setup": true,
                                        "applied_to_draft": false,
                                    }))
                                    .collect::<Vec<_>>(),
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::ListCompatibleModels) => {
                        inspected_models = true;
                        let required_capability = call
                            .arguments
                            .get("node_type")
                            .and_then(serde_json::Value::as_str)
                            .map(|node_type| {
                                input
                                    .node_catalog
                                    .iter()
                                    .find(|node| node.id == node_type)
                                    .ok_or_else(|| anyhow!("node definition {node_type:?} is not registered"))
                                    .map(|node| node.required_model_capability)
                            })
                            .transpose()?
                            .flatten();
                        let provider_models =
                            compatible_builder_models(&input, required_capability);
                        let expert_models = input
                            .expert_models
                            .iter()
                            .filter(|model| {
                                model.availability == ModelAvailability::Available
                                    && required_capability.is_none_or(|capability| {
                                        model.capabilities.contains(&capability)
                                    })
                            })
                            .collect::<Vec<_>>();
                        let alternatives = input
                            .expert_models
                            .iter()
                            .filter(|model| {
                                model.availability != ModelAvailability::Available
                                    && required_capability.is_none_or(|capability| {
                                        model.capabilities.contains(&capability)
                                    })
                            })
                            .map(|model| json!({
                                "model_id": model.model_id,
                                "display_name": model.display_name,
                                "capabilities": model.capabilities,
                                "availability": model.availability,
                                "requires_setup": true,
                                "applied_to_draft": false,
                            }))
                            .collect::<Vec<_>>();
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Listed compatible Model Profiles",
                            json!({
                                "models": provider_models,
                                "expert_models": expert_models,
                                "setup_only_alternatives": alternatives,
                                "required_capability": required_capability,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectModelProfile) => {
                        let model_id = required_string_argument(&call.arguments, "model_profile_id")?;
                        if let Some(model) = input
                            .model_profiles
                            .iter()
                            .find(|model| model.id.to_string() == model_id)
                        {
                            return Ok(annotagent_core::AgentToolResult::summary(
                                format!("Inspected Model Profile {model_id}"),
                                serde_json::to_value(model)?,
                            ));
                        }
                        let model = input
                            .expert_models
                            .iter()
                            .find(|model| model.model_id == model_id)
                            .ok_or_else(|| anyhow!("Model Profile {model_id:?} is not registered"))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected Expert Model Profile {model_id}"),
                            serde_json::to_value(model)?,
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectWorkerHealth) => {
                        let model_id = required_string_argument(&call.arguments, "model_id")?;
                        let model = input
                            .expert_models
                            .iter()
                            .find(|model| model.model_id == model_id)
                            .ok_or_else(|| anyhow!("Expert Model {model_id:?} is not registered"))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected Worker evidence for {model_id}"),
                            json!({
                                "model_id": model.model_id,
                                "connection": model.connection,
                                "availability": model.availability,
                                "evidence": model.availability_evidence,
                                "publishable": model.availability.publishable(),
                                "active_probe_sent": false,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectModelContracts) => {
                        let model_id = required_string_argument(&call.arguments, "model_id")?;
                        if let Some(model) = input
                            .expert_models
                            .iter()
                            .find(|model| model.model_id == model_id)
                        {
                            return Ok(annotagent_core::AgentToolResult::summary(
                                format!("Inspected contracts for {model_id}"),
                                json!({
                                    "model_id": model.model_id,
                                    "capabilities": model.capabilities,
                                    "input_contracts": model.input_contracts,
                                    "output_contracts": model.output_contracts,
                                    "prompt_contracts": model.prompt_contracts,
                                }),
                            ));
                        }
                        let model = input
                            .model_registry
                            .iter()
                            .find(|model| model.id == model_id)
                            .ok_or_else(|| anyhow!("Registry Model {model_id:?} is not registered"))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected contracts for {model_id}"),
                            json!({
                                "model_id": model.id,
                                "capabilities": model.capabilities,
                                "input_contract": model.input_contract,
                                "output_contract": model.output_contract,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectLabelSpace) => {
                        let model_id = required_string_argument(&call.arguments, "model_id")?;
                        let labels = input
                            .expert_models
                            .iter()
                            .find(|model| model.model_id == model_id)
                            .and_then(|model| model.label_space.clone())
                            .or_else(|| {
                                input
                                    .model_registry
                                    .iter()
                                    .find(|model| model.id == model_id)
                                    .map(|model| model.output_contract.label_space.clone())
                            })
                            .ok_or_else(|| anyhow!("Registry Model {model_id:?} is not registered"))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected Label Space for {model_id}"),
                            json!({"model_id": model_id, "labels": labels, "open_label_space": labels.is_empty()}),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectScoreSemantics) => {
                        let model_id = required_string_argument(&call.arguments, "model_id")?;
                        let semantics = input
                            .expert_models
                            .iter()
                            .find(|model| model.model_id == model_id)
                            .map(|model| model.score_semantics)
                            .or_else(|| {
                                input
                                    .model_registry
                                    .iter()
                                    .find(|model| model.id == model_id)
                                    .map(|model| model.score_semantics)
                            })
                            .ok_or_else(|| anyhow!("Registry Model {model_id:?} is not registered"))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected score semantics for {model_id}"),
                            json!({"model_id": model_id, "score_semantics": semantics, "missing_score_must_remain_missing": semantics == ScoreSemantics::NotProvided}),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectGeometrySemantics) => {
                        let model_id = required_string_argument(&call.arguments, "model_id")?;
                        let semantics = input
                            .expert_models
                            .iter()
                            .find(|model| model.model_id == model_id)
                            .map(|model| model.geometry_semantics)
                            .or_else(|| {
                                input
                                    .model_registry
                                    .iter()
                                    .find(|model| model.id == model_id)
                                    .map(|model| {
                                        annotagent_core::default_geometry_semantics(
                                            &model.capabilities,
                                        )
                                    })
                            })
                            .ok_or_else(|| anyhow!("Registry Model {model_id:?} is not registered"))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected geometry semantics for {model_id}"),
                            json!({"model_id": model_id, "geometry_semantics": semantics, "independent_from_score": true}),
                        ))
                    }
                    Ok(PipelineBuilderTool::CheckCapabilityPath) => {
                        let node_type = required_string_argument(&call.arguments, "node_type")?;
                        let definition = input
                            .node_catalog
                            .iter()
                            .find(|definition| definition.id == node_type)
                            .ok_or_else(|| anyhow!("node definition {node_type:?} is not registered"))?;
                        let provider_models = compatible_builder_models(
                            &input,
                            definition.required_model_capability,
                        );
                        let expert_models = input
                            .expert_models
                            .iter()
                            .filter(|model| {
                                model.availability == ModelAvailability::Available
                                    && definition.required_model_capability.is_none_or(
                                        |capability| model.capabilities.contains(&capability),
                                    )
                            })
                            .collect::<Vec<_>>();
                        let available = definition.required_model_capability.is_none()
                            || !provider_models.is_empty()
                            || !expert_models.is_empty();
                        Ok(annotagent_core::AgentToolResult::summary(
                            if available {
                                "Capability path is available"
                            } else {
                                "Capability path requires model setup"
                            },
                            json!({
                                "node": definition,
                                "available": available,
                                "provider_models": provider_models,
                                "expert_models": expert_models,
                                "setup_only_alternatives": input.expert_models.iter()
                                    .filter(|model| model.availability != ModelAvailability::Available)
                                    .filter(|model| definition.required_model_capability.is_none_or(|capability| model.capabilities.contains(&capability)))
                                    .map(|model| json!({"model_id": model.model_id, "availability": model.availability, "requires_setup": true}))
                                    .collect::<Vec<_>>(),
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::CheckProviderAvailability) => {
                        let provider_id = required_string_argument(&call.arguments, "provider_id")?;
                        let provider = input
                            .provider_profiles
                            .iter()
                            .find(|provider| provider.id.to_string() == provider_id)
                            .ok_or_else(|| anyhow!("Provider Profile {provider_id:?} is not registered"))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Completed passive Provider availability assessment",
                            json!({
                                "provider_id": provider.id,
                                "enabled": provider.enabled,
                                "health_status": provider.health_status,
                                "credential_configured": provider.credential_configured,
                                "endpoint_summary": provider.endpoint_summary,
                                "check_kind": "passive_registry_snapshot",
                                "billable_request_sent": false,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::EstimateModelCost) => {
                        let model_id = required_string_argument(&call.arguments, "model_profile_id")?;
                        let model = input
                            .model_profiles
                            .iter()
                            .find(|model| model.id.to_string() == model_id)
                            .ok_or_else(|| anyhow!("Model Profile {model_id:?} is not registered"))?;
                        let estimate = estimate_model_profile_cost(model, &call.arguments);
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Estimated cost for Model Profile {model_id}"),
                            estimate,
                        ))
                    }
                    Ok(PipelineBuilderTool::ListPipelineTemplates) => Ok(
                        annotagent_core::AgentToolResult::summary(
                            "Listed compatible Pipeline templates",
                            json!({
                                "templates": std::iter::once(json!({
                                    "id": "safe_default",
                                    "name": safe_suggestion.draft.name,
                                    "description": "Core-generated controlled Label Pipeline, statically type-checked before this Agent session.",
                                    "node_count": safe_suggestion.draft.nodes.len(),
                                }))
                                .chain(input.workflow_templates.iter().filter(|template| {
                                    template.nodes.iter().all(|node| {
                                        input.node_catalog.iter().any(|definition| definition.id == node.node_type)
                                    })
                                }).map(|template| json!({
                                    "id": template.id,
                                    "name": template.name,
                                    "description": template.description,
                                    "node_count": template.nodes.len(),
                                })))
                                .collect::<Vec<_>>(),
                                "safe_default_available": true,
                                "incompatible_templates_hidden": input.workflow_templates.iter().filter(|template| {
                                    template.nodes.iter().any(|node| {
                                        !input.node_catalog.iter().any(|definition| definition.id == node.node_type)
                                    })
                                }).count(),
                            }),
                        ),
                    ),
                    Ok(
                        tool @ (PipelineBuilderTool::CreatePipelineDraft
                        | PipelineBuilderTool::CreateDraftFromTemplate),
                    ) => {
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
                            bail!(
                                "load the enabled Domain Advisor resource {:?} before creating a Draft",
                                required_advisor_resource.as_deref().unwrap_or_default()
                            );
                        }
                        let mut created = safe_suggestion.clone();
                        if tool == PipelineBuilderTool::CreateDraftFromTemplate
                            && let Some(template_id) = call
                                .arguments
                                .get("template_id")
                                .and_then(serde_json::Value::as_str)
                            && template_id != "safe_default"
                        {
                            if !input
                                .workflow_templates
                                .iter()
                                .any(|template| {
                                    template.id == template_id
                                        && template.nodes.iter().all(|node| {
                                            input.node_catalog.iter().any(|definition| {
                                                definition.id == node.node_type
                                            })
                                        })
                                })
                            {
                                bail!(
                                    "workflow template {template_id:?} is not Pipeline Builder compatible; use safe_default or an exact ID returned by list_pipeline_templates"
                                );
                            }
                            created.draft = self.create_workflow_draft_with_template(
                                project_id,
                                settings,
                                false,
                                Some(template_id),
                            )?;
                        } else {
                            created.draft.id = uuid::Uuid::new_v4().to_string();
                        }
                        if tool == PipelineBuilderTool::CreatePipelineDraft {
                            created.draft.nodes.clear();
                            created.draft.edges.clear();
                            created.draft.label_pipeline = None;
                            created.rationale = vec![
                                "Created an empty, constrained Pipeline Draft for incremental editing."
                                    .to_owned(),
                            ];
                        }
                        if let Some(name) = call.arguments.get("name").and_then(|value| value.as_str())
                            && !name.trim().is_empty()
                        {
                            name.clone_into(&mut created.draft.name);
                        }
                        created.draft.status = WorkflowDraftStatus::Editing;
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
                        let previous_draft = suggestion.draft.clone();
                        let removed = PipelineDraftTools.disconnect(
                            &mut suggestion.draft,
                            &from_node,
                            &to_node,
                        )?;
                        draft_history.record_before_change(&previous_draft)?;
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
                        let source = suggestion
                            .draft
                            .nodes
                            .iter()
                            .find(|node| node.id == edge.from_node)
                            .ok_or_else(|| anyhow!("connection source is not a Draft node"))?;
                        let target_node = suggestion
                            .draft
                            .nodes
                            .iter()
                            .find(|node| node.id == edge.to_node)
                            .ok_or_else(|| anyhow!("connection target is not a Draft node"))?;
                        let source_port = source
                            .outputs
                            .iter()
                            .find(|port| port.id == edge.from_port)
                            .ok_or_else(|| {
                                anyhow!(
                                    "connection source port {:?} is not registered; valid outputs on {:?}: {}",
                                    edge.from_port,
                                    edge.from_node,
                                    source
                                        .outputs
                                        .iter()
                                        .map(|port| format!("{}:{:?}", port.id, port.artifact_type))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            })?;
                        let target_port = target_node
                            .inputs
                            .iter()
                            .find(|port| port.id == edge.to_port)
                            .ok_or_else(|| {
                                anyhow!(
                                    "connection target port {:?} is not registered; valid inputs on {:?}: {}",
                                    edge.to_port,
                                    edge.to_node,
                                    target_node
                                        .inputs
                                        .iter()
                                        .map(|port| format!("{}:{:?}", port.id, port.artifact_type))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            })?;
                        if source_port.artifact_type != target_port.artifact_type {
                            bail!(
                                "connection Artifact types do not match: {:?}.{} is {:?}, but {:?}.{} requires {:?}",
                                edge.from_node,
                                edge.from_port,
                                source_port.artifact_type,
                                edge.to_node,
                                edge.to_port,
                                target_port.artifact_type
                            );
                        }
                        let previous_draft = suggestion.draft.clone();
                        PipelineDraftTools.connect(&mut suggestion.draft, edge.clone())?;
                        draft_history.record_before_change(&previous_draft)?;
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
                        if call
                            .arguments
                            .get("guided_template")
                            .and_then(serde_json::Value::as_str)
                            == Some("prompted_segmentation_refinement")
                        {
                            let (task_id, label) = target.ok_or_else(|| {
                                anyhow!("Geometry refinement requires a target Label session")
                            })?;
                            if !inspected_dry_run {
                                bail!(
                                    "inspect the latest Dry Run before considering geometry refinement"
                                );
                            }
                            let report = dry_run.as_ref().ok_or_else(|| {
                                anyhow!("run and inspect a Dry Run before geometry refinement")
                            })?;
                            let draft = current
                                .as_ref()
                                .ok_or_else(|| anyhow!("Dry Run has no current Draft"))?;
                            let observation = agent_dry_run_summary(report, &draft.draft);
                            let segmenter = input.expert_models.iter().find(|model| {
                                model.availability == ModelAvailability::Available
                                    && model
                                        .capabilities
                                        .contains(&ModelCapability::PromptedSegmentation)
                            });
                            let paths = annotagent_core::ArtifactConversionRegistry::default()
                                .find_conversion_path(
                                    ArtifactKind::DetectionSet,
                                    ArtifactKind::DetectionSet,
                                    &nodes,
                                );
                            let path_available = paths.iter().any(|path| {
                                path.steps
                                    .iter()
                                    .map(|step| step.node_id.as_str())
                                    .collect::<Vec<_>>()
                                    == vec![
                                        annotagent_runtime::CORE_DETECTIONS_TO_BOX_PROMPTS,
                                        "capability.segment",
                                        annotagent_runtime::CORE_MASK_TO_BBOX,
                                    ]
                            });
                            let assessment = assess_prompted_segmentation_revision(
                                &observation,
                                segmenter.is_some(),
                                path_available,
                            );
                            if !assessment.applicable {
                                return Ok(annotagent_core::AgentToolResult::summary(
                                    "Prompted segmentation was not applied",
                                    json!({
                                        "applied": false,
                                        "assessment": assessment,
                                        "setup_only_alternatives": input.expert_models.iter()
                                            .filter(|model| model.capabilities.contains(&ModelCapability::PromptedSegmentation))
                                            .filter(|model| model.availability != ModelAvailability::Available)
                                            .map(|model| json!({
                                                "model_id": model.model_id,
                                                "availability": model.availability,
                                                "requires_setup": true,
                                            }))
                                            .collect::<Vec<_>>(),
                                    }),
                                ));
                            }
                            let model_id = segmenter
                                .expect("applicable assessment requires an available model")
                                .model_id
                                .clone();
                            let suggestion = current
                                .as_mut()
                                .ok_or_else(|| anyhow!("create a Draft before revising it"))?;
                            let previous_draft = suggestion.draft.clone();
                            if !add_prompted_segmentation_revision(
                                suggestion,
                                task_id,
                                label,
                                &model_id,
                                &observation,
                            )? {
                                bail!(
                                    "Prompted segmentation refinement is not applicable or is already present"
                                );
                            }
                            draft_history.record_before_change(&previous_draft)?;
                            validation = None;
                            dry_run = None;
                            inspected_dry_run = false;
                            self.store.save_workflow_draft(&suggestion.draft)?;
                            return Ok(annotagent_core::AgentToolResult::summary(
                                "Added evidence-backed prompted segmentation refinement",
                                json!({
                                    "draft_id": suggestion.draft.id,
                                    "applied": true,
                                    "model_id": model_id,
                                    "assessment": assessment,
                                    "nodes": [
                                        annotagent_runtime::CORE_DETECTIONS_TO_BOX_PROMPTS,
                                        "capability.segment",
                                        annotagent_runtime::CORE_MASK_TO_BBOX,
                                    ],
                                }),
                            ));
                        }
                        if call
                            .arguments
                            .get("guided_template")
                            .and_then(serde_json::Value::as_str)
                            == Some("crop_verification")
                        {
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
                            let previous_draft = suggestion.draft.clone();
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
                            draft_history.record_before_change(&previous_draft)?;
                            validation = None;
                            dry_run = None;
                            inspected_dry_run = false;
                            self.store.save_workflow_draft(&suggestion.draft)?;
                            return Ok(annotagent_core::AgentToolResult::summary(
                                "Added Crop verification from Dry Run evidence",
                                json!({
                                    "draft_id": suggestion.draft.id,
                                    "guided_action": "crop_verification",
                                    "evidence": {
                                        "review_count": observation.review_count,
                                        "review_rate": observation.review_rate(),
                                    }
                                }),
                            ));
                        }
                        let node_type = required_string_argument(&call.arguments, "node_type")?;
                        if node_type == "capability.segment" {
                            if !inspected_dry_run {
                                bail!(
                                    "inspect a Dry Run before adding prompted segmentation"
                                );
                            }
                            let report = dry_run.as_ref().ok_or_else(|| {
                                anyhow!("run a Dry Run before adding prompted segmentation")
                            })?;
                            let draft = current
                                .as_ref()
                                .ok_or_else(|| anyhow!("Dry Run has no current Draft"))?;
                            let observation = agent_dry_run_summary(report, &draft.draft);
                            let assessment = assess_prompted_segmentation_revision(
                                &observation,
                                input.expert_models.iter().any(|model| {
                                    model.availability == ModelAvailability::Available
                                        && model
                                            .capabilities
                                            .contains(&ModelCapability::PromptedSegmentation)
                                }),
                                !annotagent_core::ArtifactConversionRegistry::default()
                                    .find_conversion_path(
                                        ArtifactKind::DetectionSet,
                                        ArtifactKind::DetectionSet,
                                        &nodes,
                                    )
                                    .is_empty(),
                            );
                            if !assessment.applicable {
                                bail!("{}", assessment.explanation);
                            }
                        }
                        let definition = input
                            .node_catalog
                            .iter()
                            .find(|node| node.id == node_type)
                            .ok_or_else(|| anyhow!("node definition {node_type:?} is not registered"))?;
                        let node_id = call
                            .arguments
                            .get("node_id")
                            .and_then(serde_json::Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .map_or_else(
                                || format!("agent-{}", uuid::Uuid::new_v4().simple()),
                                ToOwned::to_owned,
                            );
                        let configuration = call
                            .arguments
                            .get("configuration")
                            .cloned()
                            .unwrap_or_else(|| json!({}));
                        let node = workflow_node_from_definition(
                            definition,
                            node_id.clone(),
                            &configuration,
                        )?;
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before adding a node"))?;
                        let previous_draft = suggestion.draft.clone();
                        PipelineDraftTools.add_node(
                            &mut suggestion.draft,
                            node,
                            &nodes,
                            &models,
                            &enabled_skills,
                        )?;
                        draft_history.record_before_change(&previous_draft)?;
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Added {node_type} as {node_id}"),
                            json!({"draft_id": suggestion.draft.id, "node_id": node_id, "node_type": node_type}),
                        ))
                    }
                    Ok(PipelineBuilderTool::RemovePipelineNode) => {
                        let node_id = required_string_argument(&call.arguments, "node_id")?;
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before removing a node"))?;
                        let previous_draft = suggestion.draft.clone();
                        PipelineDraftTools.remove_node(&mut suggestion.draft, &node_id)?;
                        draft_history.record_before_change(&previous_draft)?;
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Removed {node_id}"),
                            json!({"draft_id": suggestion.draft.id, "node_id": node_id}),
                        ))
                    }
                    Ok(
                        tool @ (PipelineBuilderTool::SetNodeConfiguration
                        | PipelineBuilderTool::SetDecisionPolicy
                        | PipelineBuilderTool::SetLabelMapping),
                    ) => {
                        let node_id = required_string_argument(&call.arguments, "node_id")?;
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before editing it"))?;
                        let previous_draft = suggestion.draft.clone();
                        let summary = match tool {
                            PipelineBuilderTool::SetDecisionPolicy => {
                                let value = call
                                    .arguments
                                    .get("threshold")
                                    .cloned()
                                    .ok_or_else(|| anyhow!("threshold is required"))?;
                                PipelineDraftTools.set_parameter(
                                    &mut suggestion.draft,
                                    &node_id,
                                    "threshold",
                                    value.clone(),
                                )?;
                                sync_label_step_parameter(
                                    &mut suggestion.draft,
                                    &node_id,
                                    "threshold",
                                    value,
                                );
                                "decision policy"
                            }
                            PipelineBuilderTool::SetLabelMapping => {
                                let value = call
                                    .arguments
                                    .get("class_mapping")
                                    .cloned()
                                    .ok_or_else(|| anyhow!("class_mapping is required"))?;
                                PipelineDraftTools.set_parameter(
                                    &mut suggestion.draft,
                                    &node_id,
                                    "class_mapping",
                                    value.clone(),
                                )?;
                                sync_label_step_parameter(
                                    &mut suggestion.draft,
                                    &node_id,
                                    "class_mapping",
                                    value,
                                );
                                "label mapping"
                            }
                            PipelineBuilderTool::SetNodeConfiguration => {
                                let configuration = call
                                    .arguments
                                    .get("configuration")
                                    .and_then(serde_json::Value::as_object)
                                    .ok_or_else(|| anyhow!("configuration object is required"))?
                                    .iter()
                                    .map(|(key, value)| (key.clone(), value.clone()))
                                    .collect();
                                PipelineDraftTools.set_configuration(
                                    &mut suggestion.draft,
                                    &node_id,
                                    configuration,
                                    &nodes,
                                )?;
                                "node configuration"
                            }
                            _ => unreachable!(),
                        };
                        draft_history.record_before_change(&previous_draft)?;
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Updated {summary} on {node_id}"),
                            json!({"draft_id": suggestion.draft.id, "node_id": node_id, "updated": summary}),
                        ))
                    }
                    Ok(PipelineBuilderTool::BindModelProfile) => {
                        let node_id = required_string_argument(&call.arguments, "node_id")?;
                        let model_id =
                            required_string_argument(&call.arguments, "model_profile_id")?;
                        let model_profile = input
                            .model_profiles
                            .iter()
                            .find(|model| model.id.to_string() == model_id);
                        let expert_model = input
                            .expert_models
                            .iter()
                            .find(|model| model.model_id == model_id);
                        if model_profile.is_none() && expert_model.is_none() {
                            bail!("Model Profile {model_id:?} is not registered");
                        }
                        let locked = call
                            .arguments
                            .get("locked")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true);
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before binding a Model"))?;
                        let previous_draft = suggestion.draft.clone();
                        let revision = if let Some(model_profile) = model_profile {
                            PipelineDraftTools.bind_model_profile(
                                &mut suggestion.draft,
                                &node_id,
                                model_profile,
                                locked,
                                &nodes,
                            )?;
                            let runtime_binding = input
                                .model_registry
                                .iter()
                                .find(|model| {
                                    model.id == model_profile.remote_model_id
                                        || model.model == model_profile.remote_model_id
                                })
                                .map(|model| model.id.clone());
                            if let Some(node) = suggestion
                                .draft
                                .nodes
                                .iter_mut()
                                .find(|node| node.id == node_id)
                            {
                                node.model_binding.clone_from(&runtime_binding);
                            }
                            if let Some(runtime_binding) = runtime_binding {
                                sync_label_step_model(
                                    &mut suggestion.draft,
                                    &node_id,
                                    &runtime_binding,
                                );
                            }
                            Some(model_profile.revision)
                        } else {
                            let expert_model = expert_model.expect("checked above");
                            if expert_model.availability != ModelAvailability::Available {
                                bail!(
                                    "model_profile_unavailable: Expert Model {:?} is {:?}; setup-only alternatives cannot be applied to a Draft",
                                    expert_model.model_id,
                                    expert_model.availability
                                );
                            }
                            let definition = suggestion
                                .draft
                                .nodes
                                .iter()
                                .find(|node| node.id == node_id)
                                .and_then(|node| nodes.definition(&node.node_type))
                                .ok_or_else(|| anyhow!("node {node_id:?} is not registered"))?;
                            if let Some(capability) = definition.required_model_capability
                                && !expert_model.capabilities.contains(&capability)
                            {
                                bail!(
                                    "incompatible_model_capability: Expert Model {:?} does not support {:?}",
                                    expert_model.model_id,
                                    capability
                                );
                            }
                            let node = suggestion
                                .draft
                                .nodes
                                .iter_mut()
                                .find(|node| node.id == node_id)
                                .ok_or_else(|| anyhow!("node {node_id:?} is not in the Draft"))?;
                            node.model_binding = Some(expert_model.model_id.clone());
                            node.model_profile_binding = None;
                            sync_label_step_model(
                                &mut suggestion.draft,
                                &node_id,
                                &expert_model.model_id,
                            );
                            None
                        };
                        draft_history.record_before_change(&previous_draft)?;
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Bound Model Profile {model_id} to {node_id}"),
                            json!({"draft_id": suggestion.draft.id, "node_id": node_id, "model_profile_id": model_id, "revision": revision, "locked": locked}),
                        ))
                    }
                    Ok(PipelineBuilderTool::SetRuntimePolicy) => {
                        let policy_id = required_string_argument(&call.arguments, "policy_id")?;
                        let configuration = call
                            .arguments
                            .get("configuration")
                            .cloned()
                            .ok_or_else(|| anyhow!("configuration is required"))?;
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before setting Runtime Policy"))?;
                        let previous_draft = suggestion.draft.clone();
                        PipelineDraftTools.set_runtime_policy(
                            &mut suggestion.draft,
                            &policy_id,
                            configuration,
                            &nodes,
                        )?;
                        draft_history.record_before_change(&previous_draft)?;
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Configured Runtime Policy {policy_id}"),
                            json!({"draft_id": suggestion.draft.id, "policy_id": policy_id, "graph_node_added": false}),
                        ))
                    }
                    Ok(PipelineBuilderTool::ComparePipelineDrafts) => {
                        let other_id = required_string_argument(&call.arguments, "other_draft_id")?;
                        let suggestion = current
                            .as_ref()
                            .ok_or_else(|| anyhow!("create a Draft before comparing it"))?;
                        let other = self.store.get_workflow_draft(&other_id)?;
                        let diff = PipelineDraftDiff::between(&other, &suggestion.draft)
                            .map_err(|error| anyhow!(error))?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Compared current Draft with {other_id}"),
                            json!({"base_draft_id": other_id, "current_draft_id": suggestion.draft.id, "diff": diff}),
                        ))
                    }
                    Ok(PipelineBuilderTool::UndoLastDraftChange) => {
                        let suggestion = current
                            .as_mut()
                            .ok_or_else(|| anyhow!("create a Draft before undoing a change"))?;
                        draft_history.undo_last(&mut suggestion.draft)?;
                        validation = None;
                        dry_run = None;
                        inspected_dry_run = false;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Restored and persisted the previous Draft revision",
                            json!({"draft_id": suggestion.draft.id, "remaining_undo_changes": draft_history.len()}),
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
                            json!({
                                "valid": report.valid,
                                "issues": report.issues,
                                "execution_order": report.execution_order,
                                "next_required_tool": if report.valid { "dry_run_pipeline" } else { "repair_the_reported_issues_then_validate_pipeline" },
                                "add_or_rewire_before_initial_dry_run": false,
                            }),
                        );
                        validation = Some(report);
                        Ok(result)
                    }
                    Ok(PipelineBuilderTool::EstimatePipelineCost) => {
                        let suggestion = current
                            .as_ref()
                            .ok_or_else(|| anyhow!("create a Draft before estimating it"))?;
                        let image_count = call
                            .arguments
                            .get("image_count")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(1);
                        let estimates = suggestion
                            .draft
                            .nodes
                            .iter()
                            .filter_map(|node| {
                                node.model_profile_binding.as_ref().and_then(|binding| {
                                    input
                                        .model_profiles
                                        .iter()
                                        .find(|model| model.id == binding.model_profile_id)
                                        .map(|model| {
                                            json!({
                                                "node_id": node.id,
                                                "model_profile_id": model.id,
                                                "revision": model.revision,
                                                "estimate": estimate_model_profile_cost_for_counts(model, image_count, 0, 0),
                                            })
                                        })
                                })
                            })
                            .collect::<Vec<_>>();
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Estimated Pipeline cost from bound Model Profile pricing",
                            json!({"image_count": image_count, "node_estimates": estimates, "model_request_sent": false}),
                        ))
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
                            json!({
                                "summary": observation,
                                "review_rate": observation.review_rate(),
                                "next_required_tool": "submit_draft_for_human_approval",
                                "submission_guidance": if observation.failed_images > 0 {
                                    "Include the Dry Run provider/backend failures as unresolved warnings. Do not replace or rewire model nodes to work around infrastructure failures."
                                } else {
                                    "Submit the validated, Dry Run tested Draft for explicit human approval."
                                },
                                "published": false,
                                "formal_run_started": false,
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectFailureClasses) => {
                        let report = dry_run.as_ref().ok_or_else(|| {
                            anyhow!("run dry_run_pipeline before inspecting failure classes")
                        })?;
                        let limit = bounded_inspection_limit(&call.arguments)?;
                        let mut counts = BTreeMap::<AnnotationFailureClass, usize>::new();
                        for failure_class in report
                            .samples
                            .iter()
                            .flat_map(|sample| sample.failure_classes.iter().copied())
                        {
                            *counts.entry(failure_class).or_default() += 1;
                        }
                        let samples = report
                            .samples
                            .iter()
                            .filter(|sample| !sample.failure_classes.is_empty())
                            .take(limit)
                            .map(|sample| json!({
                                "image_index": sample.image_index,
                                "failure_classes": sample.failure_classes,
                                "candidate_produced": sample.result_count > 0,
                            }))
                            .collect::<Vec<_>>();
                        inspected_dry_run = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Inspected structured Dry Run failure classes",
                            json!({
                                "counts": counts,
                                "samples": samples,
                                "policy": {
                                    "provider_failure_is_geometry_evidence": false,
                                    "no_candidate_is_promptable": false,
                                    "semantic_error_is_geometry_error": false,
                                }
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectGeometryQuality) => {
                        let report = dry_run.as_ref().ok_or_else(|| {
                            anyhow!("run dry_run_pipeline before inspecting geometry quality")
                        })?;
                        let limit = bounded_inspection_limit(&call.arguments)?;
                        let reports = report
                            .samples
                            .iter()
                            .flat_map(|sample| {
                                sample.outcomes.iter().filter_map(move |outcome| {
                                    outcome.geometry_quality.as_ref().map(|quality| {
                                        json!({
                                            "image_index": sample.image_index,
                                            "outcome_id": outcome.id,
                                            "label": outcome.label,
                                            "score": outcome.confidence,
                                            "geometry": quality,
                                        })
                                    })
                                })
                            })
                            .take(limit)
                            .collect::<Vec<_>>();
                        inspected_dry_run = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Inspected Dry Run geometry evidence",
                            json!({
                                "summary": report.summary.geometry_quality,
                                "reports": reports,
                                "confidence_is_geometry_quality": false,
                            }),
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
                            json!({
                                "sample_count": samples.len(),
                                "limit": limit,
                                "samples": samples,
                                "next_required_tool": "inspect_dry_run_summary",
                            }),
                        ))
                    }
                    Ok(PipelineBuilderTool::InspectNodeStatistics) => {
                        let report = dry_run.as_ref().ok_or_else(|| {
                            anyhow!("run dry_run_pipeline before inspecting node statistics")
                        })?;
                        let node_id = required_string_argument(&call.arguments, "node_id")?;
                        let results = report
                            .samples
                            .iter()
                            .filter_map(|sample| {
                                sample.nodes.iter().find(|node| node.node_id == node_id)
                            })
                            .collect::<Vec<_>>();
                        if results.is_empty() {
                            let available_node_ids = report
                                .samples
                                .iter()
                                .flat_map(|sample| sample.nodes.iter().map(|node| node.node_id.clone()))
                                .collect::<BTreeSet<_>>();
                            bail!(
                                "node {node_id:?} has no result in the latest Dry Run; node IDs with results: {}",
                                available_node_ids.into_iter().collect::<Vec<_>>().join(", ")
                            );
                        }
                        let total_latency_ms = results.iter().map(|node| node.latency_ms).sum::<u64>();
                        let maximum_latency_ms = results
                            .iter()
                            .map(|node| node.latency_ms)
                            .max()
                            .unwrap_or_default();
                        let mut statuses = BTreeMap::<String, usize>::new();
                        for node in &results {
                            *statuses.entry(node.status.clone()).or_default() += 1;
                        }
                        inspected_dry_run = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected aggregate statistics for {node_id}"),
                            json!({
                                "node_id": node_id,
                                "sample_count": results.len(),
                                "total_latency_ms": total_latency_ms,
                                "average_latency_ms": total_latency_ms / u64::try_from(results.len()).unwrap_or(1),
                                "maximum_latency_ms": maximum_latency_ms,
                                "statuses": statuses,
                                "issue_count": results.iter().map(|node| node.issues.len()).sum::<usize>(),
                                "estimated_costs": results.iter().map(|node| node.estimated_cost.clone()).collect::<Vec<_>>(),
                            }),
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
                            let available_node_ids = report
                                .samples
                                .iter()
                                .flat_map(|sample| sample.nodes.iter().map(|node| node.node_id.clone()))
                                .collect::<BTreeSet<_>>();
                            bail!(
                                "node {node_id:?} has no result in the latest Dry Run; node IDs with results: {}",
                                available_node_ids.into_iter().collect::<Vec<_>>().join(", ")
                            );
                        }
                        inspected_dry_run = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Inspected {} bounded result(s) for {node_id}", node_results.len()),
                            json!({"node_id": node_id, "result_count": node_results.len(), "limit": limit, "results": node_results}),
                        ))
                    }
                    Ok(PipelineBuilderTool::CompareDryRuns) => {
                        let report = dry_run.as_ref().ok_or_else(|| {
                            anyhow!("run dry_run_pipeline before comparing Dry Runs")
                        })?;
                        let suggestion = current
                            .as_ref()
                            .ok_or_else(|| anyhow!("Dry Run has no current Draft"))?;
                        let other_id = required_string_argument(&call.arguments, "other_draft_id")?;
                        let other_draft = self.store.get_workflow_draft(&other_id)?;
                        if other_draft.project_id != project_id {
                            bail!("Dry Run comparison requires Drafts from the same Project");
                        }
                        let other = self
                            .store
                            .get_workflow_sample_test(&other_id)?
                            .ok_or_else(|| anyhow!("Draft {other_id:?} has no persisted Dry Run"))?;
                        let current_summary = agent_dry_run_summary(report, &suggestion.draft);
                        let other_summary =
                            agent_dry_run_summary(&other.report, &other_draft);
                        inspected_dry_run = true;
                        Ok(annotagent_core::AgentToolResult::summary(
                            format!("Compared latest Dry Run with {other_id}"),
                            json!({
                                "current_draft_id": suggestion.draft.id,
                                "other_draft_id": other_id,
                                "current": current_summary,
                                "other": other_summary,
                                "delta": {
                                    "review_count": i64::from(current_summary.review_count) - i64::from(other_summary.review_count),
                                    "geometry_review_count": i64::from(current_summary.geometry_review_count) - i64::from(other_summary.geometry_review_count),
                                    "provider_failure_count": i64::from(current_summary.provider_failure_count) - i64::from(other_summary.provider_failure_count),
                                    "refiner_success_count": i64::from(current_summary.refiner_success_count) - i64::from(other_summary.refiner_success_count),
                                    "duration_ms": i128::from(current_summary.duration_ms) - i128::from(other_summary.duration_ms),
                                }
                            }),
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
                        let previous_draft = suggestion.draft.clone();
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
                        draft_history.record_before_change(&previous_draft)?;
                        self.store.save_workflow_draft(&suggestion.draft)?;
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Draft is ready for explicit human approval",
                            json!({"draft_id": suggestion.draft.id, "published": false, "formal_run_started": false, "requires_human": true}),
                        ))
                    }
                    Ok(PipelineBuilderTool::FinishAgentSession) => {
                        let suggestion = current.as_ref().ok_or_else(|| {
                            anyhow!("create and submit a Draft before finishing the session")
                        })?;
                        if suggestion.draft.status != WorkflowDraftStatus::Suggested {
                            bail!("submit_draft_for_human_approval must succeed before finish_agent_session");
                        }
                        Ok(annotagent_core::AgentToolResult::summary(
                            "Pipeline Builder session is ready to finish at human approval",
                            json!({"draft_id": suggestion.draft.id, "requires_human": true}),
                        ))
                    }
                        Err(error) => Err(anyhow!(error)),
                    }
                }
                .await;

                let (result, success, failed_attempts) = match outcome {
                    Ok(result) => {
                        let prefix = format!("{}:", call.name);
                        failed_tool_attempts.retain(|key, _| !key.starts_with(&prefix));
                        (result, true, 0)
                    }
                    Err(error) => {
                        let message = error.to_string();
                        let code = pipeline_builder_tool_error_code(&message);
                        let failure_key = if code == "invalid_declared_tool_value" {
                            format!("{}:{code}", call.name)
                        } else {
                            format!("{}:{}", call.name, call.arguments)
                        };
                        let failed_attempts = failed_tool_attempts
                            .entry(failure_key)
                            .and_modify(|count| *count = count.saturating_add(1))
                            .or_insert(1);
                        (
                            annotagent_core::AgentToolResult::summary(
                                format!("{} failed", call.name),
                                json!({
                                    "code": code,
                                    "error": message,
                                    "retryable": *failed_attempts < 3,
                                    "failed_attempts": *failed_attempts,
                                }),
                            ),
                            false,
                            *failed_attempts,
                        )
                    }
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
                if !success && failed_attempts >= 3 {
                    session.fail(format!(
                        "Pipeline Builder stopped after {failed_attempts} failed {} attempts; inspect the declared tool values before retrying",
                        call.name
                    ));
                    break;
                }
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
        migrate_legacy_expert_workflow(&mut draft)?;
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
        self.save_workflow_draft(draft)
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
        let mut draft = self.store.get_workflow_draft(draft_id)?;
        if !matches!(
            draft.status,
            WorkflowDraftStatus::Published | WorkflowDraftStatus::Archived
        ) && migrate_legacy_expert_workflow(&mut draft)?
        {
            self.store.save_workflow_draft(&draft)?;
        }
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
        temporary_api_key: Option<&str>,
    ) -> Result<WorkflowDryRunReport> {
        self.dry_run_workflow_samples_with_provider(
            draft_id,
            settings,
            image_indices,
            &settings.default_provider,
            temporary_api_key,
        )
        .await
    }

    pub async fn dry_run_workflow_samples_with_provider(
        &self,
        draft_id: &str,
        settings: &Settings,
        image_indices: &[usize],
        provider_kind: &str,
        temporary_api_key: Option<&str>,
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
                    DryRunRuntimeProvider {
                        kind: provider_kind,
                        api_key: temporary_api_key,
                    },
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
                        failure_classes: node_issues
                            .iter()
                            .map(|issue| {
                                annotagent_core::classify_annotation_failure(
                                    &issue.code,
                                    &issue.message,
                                )
                            })
                            .collect(),
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
                    failure_classes: node_results
                        .iter()
                        .flat_map(|node| node.failure_classes.iter().copied())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
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
        for node in samples.iter().flat_map(|sample| sample.nodes.iter()) {
            for failure_class in &node.failure_classes {
                let worker_failure = node.issues.iter().any(|issue| {
                    format!("{} {}", issue.code, issue.message)
                        .to_ascii_lowercase()
                        .contains("worker")
                });
                record_dry_run_failure(&mut summary, *failure_class, worker_failure);
            }
        }
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
        mut draft: WorkflowDraft,
        settings: &Settings,
        images: &[PathBuf],
        selected: &[usize],
        started: std::time::Instant,
        runtime_provider: DryRunRuntimeProvider<'_>,
    ) -> Result<WorkflowDryRunReport> {
        let project_path = self.project_path(&draft.project_id)?;
        let (project, _) = load_project_schema_with_registry(&project_path, &self.skills)?;
        let (_, models) = workflow_catalog(settings)?;
        let model_profiles = self.freeze_registry_model_profiles(&mut draft)?;
        let snapshot =
            WorkflowSnapshot::frozen(&draft, &models, project.project.enabled_skill_versions())
                .with_model_profiles(model_profiles);
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
            runtime_provider.kind,
            settings,
            runtime_provider.api_key,
            self.store.clone(),
            validators,
            refiners,
        )?;
        let project = Arc::new(project);
        let project_root = project_path
            .parent()
            .unwrap_or(&self.workspace)
            .to_path_buf();
        let historical_corrections = self
            .store
            .list_project_corrections(stable_project_id(&project_root), 500)?;
        let geometry_correction_count = historical_corrections
            .iter()
            .filter(|record| {
                record
                    .image_features
                    .geometry
                    .contains_key("manual_center_shift")
            })
            .count();
        let historical_geometry_correction_rate = (!historical_corrections.is_empty())
            .then(|| geometry_correction_count as f32 / historical_corrections.len() as f32);
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
                                let status = sample_test_outcome_status(Some(set.validation_state));
                                let mut geometry_quality = GeometryQualityReport::from_detection(
                                    set.reference.artifact_id.clone(),
                                    detection,
                                );
                                geometry_quality.historical_correction_rate =
                                    historical_geometry_correction_rate;
                                let mut failure_classes = Vec::new();
                                if detection.score.value.is_none() {
                                    failure_classes.push(AnnotationFailureClass::MissingScore);
                                }
                                if status == SampleTestOutcomeStatus::NeedsReview
                                    && geometry_quality.has_geometry_issue()
                                {
                                    failure_classes.push(AnnotationFailureClass::GeometryError);
                                }
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
                                        status,
                                        value: Some(VisionArtifactValue::BoundingBox {
                                            rect: detection.bbox,
                                        }),
                                        failure_classes,
                                        geometry_quality: Some(geometry_quality),
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
                                        failure_classes: Vec::new(),
                                        geometry_quality: None,
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
                                        failure_classes: Vec::new(),
                                        geometry_quality: None,
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
                                        failure_classes: Vec::new(),
                                        geometry_quality: None,
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
            for outcome in &outcomes {
                for failure_class in &outcome.failure_classes {
                    record_dry_run_failure(&mut summary, *failure_class, false);
                }
                if let Some(report) = &outcome.geometry_quality {
                    summary.geometry_quality.add_report(
                        report,
                        outcome.status == SampleTestOutcomeStatus::NeedsReview,
                    );
                    if let Some(iou) = report.iou_with_refiner {
                        summary.refiner_usage_count += 1;
                        let count = summary.refiner_usage_count;
                        summary.geometry_quality.mean_refiner_iou = Some(
                            summary
                                .geometry_quality
                                .mean_refiner_iou
                                .map_or(iou, |mean| mean + (iou - mean) / count as f32),
                        );
                        if iou < 1.0 - f32::EPSILON {
                            summary.refiner_success_count += 1;
                        } else {
                            summary.refiner_fallback_count += 1;
                        }
                    }
                }
            }
            summary.auto_accepted_count += sample_auto_accepted;
            summary.needs_review_count += sample_review;
            summary.empty_count += usize::from(sample_empty);
            if sample_empty {
                summary.no_candidate_count += 1;
            }
            let nodes: Vec<WorkflowDryRunNodeResult> = result
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
                    let failure_classes = trace
                        .error
                        .iter()
                        .map(|error| {
                            annotagent_core::classify_annotation_failure(
                                &error.code,
                                &error.summary,
                            )
                        })
                        .collect::<Vec<_>>();
                    for failure_class in &failure_classes {
                        let worker_failure = trace.error.as_ref().is_some_and(|error| {
                            format!("{} {}", error.code, error.summary)
                                .to_ascii_lowercase()
                                .contains("worker")
                        });
                        record_dry_run_failure(&mut summary, *failure_class, worker_failure);
                    }
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
                        failure_classes,
                        issues,
                    }
                })
                .collect();
            let mut sample_failure_classes = nodes
                .iter()
                .flat_map(|node| node.failure_classes.iter().copied())
                .chain(
                    outcomes
                        .iter()
                        .flat_map(|outcome| outcome.failure_classes.iter().copied()),
                )
                .collect::<BTreeSet<_>>();
            if sample_empty {
                sample_failure_classes.insert(AnnotationFailureClass::NoCandidate);
            }
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
                failure_classes: sample_failure_classes.into_iter().collect(),
                nodes,
            });
        }
        let total_latency_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        for record in &historical_corrections {
            let geometry = &record.image_features.geometry;
            if let (Some(center_shift), Some(area_change)) = (
                geometry.get("manual_center_shift"),
                geometry.get("manual_area_change"),
            ) {
                summary
                    .geometry_quality
                    .add_manual_adjustment(*center_shift as f32, *area_change as f32);
            }
        }
        summary.manual_resize_count = summary.geometry_quality.human_adjustment_count as usize;
        summary.average_center_shift = summary.geometry_quality.mean_manual_center_shift;
        summary.average_area_adjustment = summary.geometry_quality.mean_manual_area_change;
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

    fn freeze_registry_model_profiles(
        &self,
        draft: &mut WorkflowDraft,
    ) -> Result<Vec<ModelProfileSnapshot>> {
        let project_path = self.project_path(&draft.project_id)?;
        let project_id = stable_project_id(project_path.parent().unwrap_or(&self.workspace));
        let project_bindings = self.store.list_project_model_bindings(project_id)?;
        let defaults = self.store.get_global_model_defaults()?;
        let registry_models = self.store.list_model_profiles(None, false)?;
        let providers = self.store.list_provider_profiles()?;
        let mut referenced = BTreeSet::new();
        for node in &mut draft.nodes {
            if node.model_profile_binding.is_none()
                && let Some((capability, role)) = registry_requirement_for_node(node)
            {
                let resolved =
                    resolve_model_binding(None, &project_bindings, &defaults, capability, role)
                        .ok()
                        .or_else(|| {
                            let legacy_binding = node.model_binding.as_deref()?;
                            if !legacy_binding.starts_with("mock") {
                                return None;
                            }
                            registry_models
                                .iter()
                                .find(|model| {
                                    model.enabled
                                        && model.status == ModelProfileStatus::Available
                                        && model.task_capabilities.contains(&capability)
                                        && providers.iter().any(|provider| {
                                            provider.id == model.provider_id
                                                && provider.enabled
                                                && provider.adapter == ProviderAdapterKind::Mock
                                                && provider.preset_id.as_deref() == Some("mock")
                                        })
                                })
                                .map(|model| annotagent_core::ResolvedModelBinding {
                                    model_profile_id: model.id,
                                    source: annotagent_core::ModelBindingSource::GlobalDefault,
                                    locked: false,
                                })
                        });
                if let Some(resolved) = resolved {
                    node.model_profile_binding = Some(annotagent_core::WorkflowModelBinding {
                        model_profile_id: resolved.model_profile_id,
                        locked: resolved.locked,
                    });
                }
            }
            if let Some(binding) = node.model_profile_binding.as_ref() {
                referenced.insert(binding.model_profile_id);
            }
        }
        referenced
            .into_iter()
            .map(|model_id| {
                let model = self.store.get_model_profile(model_id, None)?;
                let provider = self.store.get_provider_profile(model.provider_id)?;
                ModelProfileSnapshot::frozen(&model, &provider).map_err(|error| {
                    anyhow!(
                        "Model Profile {} cannot be frozen for publication: {error}",
                        model.display_name
                    )
                })
            })
            .collect()
    }

    pub fn workflow_draft_model_profile_snapshots(
        &self,
        draft_id: &str,
    ) -> Result<Vec<ModelProfileSnapshot>> {
        self.resolved_workflow_draft_model_profiles(draft_id)
            .map(|(_, profiles)| profiles)
    }

    pub fn resolved_workflow_draft_model_profiles(
        &self,
        draft_id: &str,
    ) -> Result<(WorkflowDraft, Vec<ModelProfileSnapshot>)> {
        let mut draft = self.store.get_workflow_draft(draft_id)?;
        let profiles = self.freeze_registry_model_profiles(&mut draft)?;
        Ok((draft, profiles))
    }

    fn validate_published_registry_models(
        &self,
        workflow: &PublishedWorkflowVersion,
    ) -> Result<()> {
        for frozen in &workflow.snapshot.model_profiles {
            let current = self
                .store
                .get_model_profile(frozen.model_profile_id, None)
                .with_context(|| {
                    format!(
                        "Model Profile {} used by the Published Workflow no longer exists",
                        frozen.model_profile_id
                    )
                })?;
            let provider = self
                .store
                .get_provider_profile(frozen.provider_id)
                .with_context(|| {
                    format!(
                        "Provider {} used by the Published Workflow no longer exists",
                        frozen.provider_id
                    )
                })?;
            if !current.enabled || current.status != ModelProfileStatus::Available {
                bail!(
                    "new Run blocked: Model Profile {} is disabled or unavailable",
                    current.display_name
                );
            }
            if !provider.enabled
                || !matches!(
                    provider.health.status,
                    ProviderHealthStatus::Available | ProviderHealthStatus::Configured
                )
            {
                bail!(
                    "new Run blocked: Provider {} is disabled or unavailable",
                    provider.display_name
                );
            }
            let published_revision = self
                .store
                .get_model_profile(frozen.model_profile_id, Some(frozen.revision))?;
            let published_semantics = ModelProfileSnapshot::frozen(&published_revision, &provider)
                .map_err(|error| {
                    anyhow!("published Model Profile snapshot is unusable: {error}")
                })?;
            if &published_semantics != frozen {
                bail!(
                    "new Run blocked: published Model Profile snapshot no longer matches its frozen revision"
                );
            }
        }
        Ok(())
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
        let model_profiles = self.freeze_registry_model_profiles(&mut draft)?;
        let (_, models) = workflow_catalog(settings)?;
        let snapshot = WorkflowSnapshot::frozen(&draft, &models, draft.enabled_skills.clone())
            .with_model_profiles(model_profiles);
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
                self.validate_published_registry_models(&published)?;
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
            temporary_api_key.as_deref(),
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

fn record_dry_run_failure(
    summary: &mut SampleTestSummary,
    failure_class: AnnotationFailureClass,
    worker_failure: bool,
) {
    match failure_class {
        AnnotationFailureClass::ProviderFailure => summary.provider_failure_count += 1,
        AnnotationFailureClass::NoCandidate => summary.no_candidate_count += 1,
        AnnotationFailureClass::SemanticError => summary.semantic_review_count += 1,
        AnnotationFailureClass::GeometryError => summary.geometry_review_count += 1,
        AnnotationFailureClass::MissingScore => summary.missing_score_count += 1,
        AnnotationFailureClass::DomainRisk => summary.domain_risk_count += 1,
        AnnotationFailureClass::InfrastructureFailure
        | AnnotationFailureClass::InvalidArtifact
        | AnnotationFailureClass::BudgetLimit => {}
    }
    if worker_failure {
        summary.worker_failure_count += 1;
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
        provider_failure_count: as_u32(report.summary.provider_failure_count),
        worker_failure_count: as_u32(report.summary.worker_failure_count),
        no_candidate_count: as_u32(report.summary.no_candidate_count),
        semantic_review_count: as_u32(report.summary.semantic_review_count),
        geometry_review_count: as_u32(report.summary.geometry_review_count),
        missing_score_count: as_u32(report.summary.missing_score_count),
        domain_risk_count: as_u32(report.summary.domain_risk_count),
        refiner_usage_count: as_u32(report.summary.refiner_usage_count),
        refiner_success_count: as_u32(report.summary.refiner_success_count),
        refiner_fallback_count: as_u32(report.summary.refiner_fallback_count),
        geometry_quality: report.summary.geometry_quality.clone(),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct PromptedSegmentationAssessment {
    applicable: bool,
    code: &'static str,
    explanation: String,
}

fn assess_prompted_segmentation_revision(
    evidence: &AgentDryRunSummary,
    model_available: bool,
    conversion_path_available: bool,
) -> PromptedSegmentationAssessment {
    if evidence.provider_failure_count > 0 || evidence.worker_failure_count > 0 {
        return PromptedSegmentationAssessment {
            applicable: false,
            code: "infrastructure_failure",
            explanation: format!(
                "Prompted segmentation was not added: the Dry Run recorded {} Provider and {} Worker failure(s), which did not produce geometry evidence.",
                evidence.provider_failure_count, evidence.worker_failure_count
            ),
        };
    }
    if evidence.detection_count == 0 || evidence.no_candidate_count == evidence.image_count {
        return PromptedSegmentationAssessment {
            applicable: false,
            code: "no_promptable_candidate",
            explanation: "Prompted segmentation was not added: no Detection candidate exists to convert into a box or point prompt.".to_owned(),
        };
    }
    let geometry_evidence = evidence.geometry_review_count > 0
        || evidence.geometry_quality.geometry_review_count > 0
        || evidence.geometry_quality.inaccurate_bbox_reason_count > 0
        || evidence.geometry_quality.human_adjustment_count > 0
        || evidence
            .geometry_quality
            .mean_manual_center_shift
            .is_some_and(|value| value > 0.02)
        || evidence
            .geometry_quality
            .mean_manual_area_change
            .is_some_and(|value| value.abs() > 0.10);
    if !geometry_evidence {
        let code = if evidence.semantic_review_count > 0 || evidence.domain_risk_count > 0 {
            "semantic_or_domain_error"
        } else {
            "no_geometry_evidence"
        };
        return PromptedSegmentationAssessment {
            applicable: false,
            code,
            explanation: if code == "semantic_or_domain_error" {
                "Prompted segmentation was not added: the observed issue is semantic/domain risk, so Crop Classification, validation, a second detector, or Review is the appropriate primary repair.".to_owned()
            } else {
                "Prompted segmentation was not added: the Dry Run has candidates but no structured evidence that their geometry is inaccurate.".to_owned()
            },
        };
    }
    if !conversion_path_available {
        return PromptedSegmentationAssessment {
            applicable: false,
            code: "conversion_path_unavailable",
            explanation: "Prompted segmentation was not added: the registered DetectionSet → BoxPromptSet → MaskSet → DetectionSet conversion path is incomplete.".to_owned(),
        };
    }
    if !model_available {
        return PromptedSegmentationAssessment {
            applicable: false,
            code: "model_requires_setup",
            explanation: "Prompted segmentation was not added: no Available prompted-segmentation Model Profile has completed setup, health, contract, and sample-conversion evidence.".to_owned(),
        };
    }
    PromptedSegmentationAssessment {
        applicable: true,
        code: "geometry_refinement_supported",
        explanation: format!(
            "Prompted segmentation is supported by evidence: {} geometry Review(s), {} inaccurate-bbox reason(s), and {} human geometry adjustment(s) were observed with promptable candidates.",
            evidence
                .geometry_review_count
                .max(evidence.geometry_quality.geometry_review_count),
            evidence.geometry_quality.inaccurate_bbox_reason_count,
            evidence.geometry_quality.human_adjustment_count
        ),
    }
}

fn add_prompted_segmentation_revision(
    suggestion: &mut WorkflowSuggestion,
    target_task_id: &str,
    target_label: &str,
    model_id: &str,
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
        .any(|step| step.node_type == "capability.segment")
    {
        return Ok(false);
    }
    let filter = pipeline
        .steps
        .iter()
        .find(|step| step.node_type == annotagent_runtime::CORE_FILTER)
        .cloned()
        .ok_or_else(|| anyhow!("Geometry refinement requires a Select detections step"))?;
    let gate = pipeline
        .steps
        .iter()
        .find(|step| step.kind == WorkflowNodeKind::Gate)
        .cloned()
        .ok_or_else(|| anyhow!("Geometry refinement requires a Decision step"))?;
    let prefix = format!("{target_task_id}.{target_label}.geometry_refine");
    let prompts = PipelineStep {
        id: format!("{prefix}.prompts"),
        node_type: annotagent_runtime::CORE_DETECTIONS_TO_BOX_PROMPTS.to_owned(),
        kind: WorkflowNodeKind::Transform,
        inputs: BTreeMap::from([(
            "detections".to_owned(),
            PipelineSource::Step {
                step_id: filter.id.clone(),
                port: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
            },
        )]),
        outputs: BTreeMap::from([("prompts".to_owned(), ArtifactKind::BoxPromptSet)]),
        model_binding: None,
        skill_binding: None,
        parameters: BTreeMap::from([("padding".to_owned(), json!(0.02))]),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        retry_policy: RetryPolicy::default(),
        review_gate: ReviewGate::default(),
        resources: ResourceRequirements::default(),
    };
    let segment = PipelineStep {
        id: format!("{prefix}.segment"),
        node_type: "capability.segment".to_owned(),
        kind: WorkflowNodeKind::VisionModel,
        inputs: BTreeMap::from([
            ("images".to_owned(), PipelineSource::Image),
            (
                "box_prompts".to_owned(),
                PipelineSource::Step {
                    step_id: prompts.id.clone(),
                    port: "prompts".to_owned(),
                    artifact_type: ArtifactKind::BoxPromptSet,
                },
            ),
        ]),
        outputs: BTreeMap::from([("masks".to_owned(), ArtifactKind::MaskSet)]),
        model_binding: Some(PipelineModelBinding {
            model_id: model_id.to_owned(),
            capability: VisionCapability::PromptedSegmentation,
            configuration: BTreeMap::new(),
        }),
        skill_binding: None,
        parameters: BTreeMap::new(),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        retry_policy: RetryPolicy::default(),
        review_gate: ReviewGate::default(),
        resources: ResourceRequirements::default(),
    };
    let mask_to_bbox = PipelineStep {
        id: format!("{prefix}.mask_to_bbox"),
        node_type: annotagent_runtime::CORE_MASK_TO_BBOX.to_owned(),
        kind: WorkflowNodeKind::Transform,
        inputs: BTreeMap::from([
            (
                "masks".to_owned(),
                PipelineSource::Step {
                    step_id: segment.id.clone(),
                    port: "masks".to_owned(),
                    artifact_type: ArtifactKind::MaskSet,
                },
            ),
            (
                "box_prompts".to_owned(),
                PipelineSource::Step {
                    step_id: prompts.id.clone(),
                    port: "prompts".to_owned(),
                    artifact_type: ArtifactKind::BoxPromptSet,
                },
            ),
        ]),
        outputs: BTreeMap::from([("detections".to_owned(), ArtifactKind::DetectionSet)]),
        model_binding: None,
        skill_binding: None,
        parameters: BTreeMap::new(),
        validators: Vec::new(),
        refiners: Vec::new(),
        fallback: None,
        retry_policy: RetryPolicy::default(),
        review_gate: ReviewGate::default(),
        resources: ResourceRequirements::default(),
    };
    let mut revised_gate = gate;
    for source in revised_gate.inputs.values_mut() {
        if matches!(source, PipelineSource::Step { step_id, .. } if step_id == &filter.id) {
            *source = PipelineSource::Step {
                step_id: mask_to_bbox.id.clone(),
                port: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
            };
        }
    }
    let revised_gate_id = revised_gate.id.clone();
    let pipeline = &mut composition.label_pipelines[pipeline_index];
    pipeline.steps = pipeline
        .steps
        .iter()
        .filter(|step| step.id != revised_gate_id)
        .cloned()
        .chain([prompts, segment, mask_to_bbox, revised_gate])
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
    compiled.runtime_policies = old.runtime_policies;
    compiled.allow_unvalidated_commit = old.allow_unvalidated_commit;
    compiled.updated_at = chrono::Utc::now();
    suggestion.draft = compiled;
    suggestion.rationale.push(format!(
        "Geometry evidence justified an explicit Detection → Box Prompt → Prompted Segmentation → Mask to BBox revision using available Model Profile {model_id}; {} geometry Review(s) and {} inaccurate-bbox reason(s) were observed.",
        evidence.geometry_review_count.max(evidence.geometry_quality.geometry_review_count),
        evidence.geometry_quality.inaccurate_bbox_reason_count,
    ));
    suggestion.warnings.push(
        "Prompted segmentation refines existing candidate geometry only; it does not repair missing candidates, Provider failures, or semantic false positives."
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
    use annotagent_provider::MockToolCall;

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

    fn register_pipeline_builder_model(
        application: &LocalApplication,
        remote_model_id: &str,
    ) -> PipelineBuilderModelRuntime {
        let now = chrono::Utc::now();
        let provider_id = annotagent_core::ProviderId::new();
        application
            .store
            .save_provider_profile(&ProviderProfile {
                id: provider_id,
                display_name: "Scripted Builder Provider".to_owned(),
                preset_id: Some("mock".to_owned()),
                adapter: ProviderAdapterKind::Mock,
                base_url: "https://mock.invalid/v1".parse().expect("URL"),
                organization: None,
                workspace: None,
                credential_ref: None,
                safe_headers: BTreeMap::new(),
                connection_policy: annotagent_core::ProviderConnectionPolicy::default(),
                enabled: true,
                health: annotagent_core::ProviderHealthSnapshot {
                    status: ProviderHealthStatus::Available,
                    safe_message: Some("Scripted test Provider".to_owned()),
                    checked_at: Some(now),
                },
                created_at: now,
                updated_at: now,
            })
            .expect("Provider Profile");
        let model = ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id,
            display_name: "Scripted Pipeline Builder".to_owned(),
            remote_model_id: remote_model_id.to_owned(),
            input_modalities: BTreeSet::from([InputModality::Text]),
            protocol_features: annotagent_core::ProtocolFeatures {
                tool_calls: true,
                structured_output: true,
                usage_reporting: true,
                ..annotagent_core::ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([ModelCapability::TextGeneration]),
            capability_source: annotagent_core::CapabilityDeclarationSource::UserDeclared,
            limits: annotagent_core::ModelLimits {
                context_tokens: Some(32_768),
                maximum_output_tokens: Some(2_048),
                ..annotagent_core::ModelLimits::default()
            },
            generation_defaults: annotagent_core::GenerationDefaults::default(),
            pricing: annotagent_core::ModelPricing {
                currency: "USD".to_owned(),
                input_per_million_tokens: Some(rust_decimal::Decimal::from(2)),
                output_per_million_tokens: Some(rust_decimal::Decimal::from(4)),
                per_request: Some(rust_decimal::Decimal::new(1, 3)),
                source: annotagent_core::PricingSource::UserConfigured,
                updated_at: Some(now),
                ..annotagent_core::ModelPricing::default()
            },
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        application
            .store
            .save_model_profile(&model)
            .expect("Model Profile");
        application
            .store
            .save_global_model_defaults(&annotagent_core::GlobalModelDefaults {
                pipeline_builder: Some(model.id),
                ..annotagent_core::GlobalModelDefaults::default()
            })
            .expect("Pipeline Builder default");
        PipelineBuilderModelRuntime {
            provider: application
                .store
                .get_provider_profile(provider_id)
                .expect("Provider Profile"),
            model,
            binding_source: ModelBindingSource::GlobalDefault,
            locked: false,
        }
    }

    #[test]
    fn pipeline_builder_model_selection_respects_registry_priority_and_requirements() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("builder-selection", GENERIC_BBOX_PROJECT)
            .expect("Project");
        let global = register_pipeline_builder_model(&application, "global-builder");
        let resolved = application
            .resolve_pipeline_builder_model("builder-selection", None)
            .expect("global default");
        assert_eq!(resolved.model.id, global.model.id);
        assert_eq!(resolved.binding_source, ModelBindingSource::GlobalDefault);

        let mut project_model = global.model.clone();
        project_model.id = ModelProfileId::new();
        project_model.remote_model_id = "project-builder".to_owned();
        application
            .store
            .save_model_profile(&project_model)
            .expect("Project Model Profile");
        let project_path = application
            .project_path("builder-selection")
            .expect("Project path");
        application
            .store
            .save_project_model_binding(
                &annotagent_core::ProjectModelBinding {
                    id: annotagent_core::ModelBindingId::new(),
                    project_id: stable_project_id(project_path.parent().expect("Project root")),
                    capability: ModelCapability::TextGeneration,
                    role: ModelBindingRole::PipelineBuilder,
                    match_kind: annotagent_core::ModelBindingMatch::Role,
                    model_profile_id: project_model.id,
                    locked: true,
                    created_at: chrono::Utc::now(),
                },
                annotagent_core::BindingMutationActor::User,
            )
            .expect("Project binding");
        let resolved = application
            .resolve_pipeline_builder_model("builder-selection", None)
            .expect("Project default");
        assert_eq!(resolved.model.id, project_model.id);
        assert_eq!(resolved.binding_source, ModelBindingSource::ProjectRole);
        assert!(resolved.locked);

        let mut live_provider = application
            .store
            .get_provider_profile(global.provider.id)
            .expect("Provider Profile");
        live_provider.adapter = ProviderAdapterKind::OpenAiCompatible;
        live_provider.base_url = "https://provider.example/v1".parse().expect("URL");
        live_provider.credential_ref = Some(annotagent_core::CredentialReference {
            provider_id: live_provider.id,
            source: annotagent_core::CredentialSource::EnvironmentVariable,
            locator: "PIPELINE_BUILDER_SECRET_FIXTURE".to_owned(),
        });
        live_provider.health.status = ProviderHealthStatus::Configured;
        application
            .store
            .save_provider_profile(&live_provider)
            .expect("OpenAI-compatible Provider Profile");

        let explicit = application
            .resolve_pipeline_builder_model("builder-selection", Some(global.model.id))
            .expect("explicit selection");
        assert_eq!(explicit.model.id, global.model.id);
        assert_eq!(explicit.binding_source, ModelBindingSource::WorkflowNode);
        let config = explicit
            .openai_compatible_config()
            .expect("OpenAI-compatible runtime config");
        assert_eq!(config.model, "global-builder");
        assert_eq!(config.endpoint, "https://provider.example/v1");
        assert_eq!(config.max_output_tokens, 2_048);
        assert!(
            !serde_json::to_string(&explicit.safe_selection())
                .expect("safe selection")
                .contains("PIPELINE_BUILDER_SECRET_FIXTURE")
        );

        let mut incompatible = global.model.clone();
        incompatible.id = ModelProfileId::new();
        incompatible.remote_model_id = "no-structured-output".to_owned();
        incompatible.protocol_features.structured_output = false;
        application
            .store
            .save_model_profile(&incompatible)
            .expect("incompatible profile remains valid Registry metadata");
        let error = match application
            .resolve_pipeline_builder_model("builder-selection", Some(incompatible.id))
        {
            Ok(_) => panic!("Builder protocol requirements must fail closed"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("ToolCalls and StructuredOutput"));
    }

    #[test]
    fn pipeline_builder_context_compaction_keeps_tool_call_groups_intact() {
        let mut messages = vec![
            ModelMessage {
                role: ModelRole::System,
                content: "policy".to_owned(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            ModelMessage {
                role: ModelRole::User,
                content: "project".to_owned(),
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
        ];
        for index in 0..10 {
            let call_id = annotagent_core::ToolCallId::new(format!("call-{index}"));
            messages.push(ModelMessage {
                role: ModelRole::Assistant,
                content: String::new(),
                tool_call_id: None,
                tool_calls: vec![annotagent_core::ModelToolCall {
                    id: call_id.clone(),
                    name: "inspect_project".to_owned(),
                    arguments: json!({}),
                }],
            });
            messages.push(ModelMessage {
                role: ModelRole::Tool,
                content: format!("result-{index}-{}", "x".repeat(5_000)),
                tool_call_id: Some(call_id),
                tool_calls: Vec::new(),
            });
        }

        assert!(compact_pipeline_builder_messages(
            &mut messages,
            Some(1_000)
        ));
        assert_eq!(messages[0].role, ModelRole::System);
        assert_eq!(messages[1].role, ModelRole::User);
        assert!(messages.last().is_some_and(|message| {
            message.role == ModelRole::Tool && message.content.starts_with("result-9-")
        }));
        for (index, message) in messages.iter().enumerate() {
            if message.role != ModelRole::Tool {
                continue;
            }
            let assistant = messages.get(index.saturating_sub(1)).expect("Assistant");
            assert_eq!(assistant.role, ModelRole::Assistant);
            assert_eq!(assistant.tool_calls.len(), 1);
            assert_eq!(
                message.tool_call_id,
                Some(assistant.tool_calls[0].id.clone())
            );
        }
    }

    #[test]
    fn pipeline_builder_catalog_is_exact_and_provider_context_is_credential_safe() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("builder-tools", GENERIC_BBOX_PROJECT)
            .expect("Project");
        let provider_id = annotagent_core::ProviderId::new();
        let now = chrono::Utc::now();
        application
            .store
            .save_provider_profile(&annotagent_core::ProviderProfile {
                id: provider_id,
                display_name: "Credential-safe fixture".to_owned(),
                preset_id: None,
                adapter: ProviderAdapterKind::OpenAiCompatible,
                base_url: "https://provider.example/v1".parse().expect("URL"),
                organization: None,
                workspace: None,
                credential_ref: Some(annotagent_core::CredentialReference {
                    provider_id,
                    source: annotagent_core::CredentialSource::EnvironmentVariable,
                    locator: "ANNOTAGENT_SUPER_SECRET_FIXTURE".to_owned(),
                }),
                safe_headers: BTreeMap::new(),
                connection_policy: annotagent_core::ProviderConnectionPolicy::default(),
                enabled: true,
                health: annotagent_core::ProviderHealthSnapshot {
                    status: ProviderHealthStatus::Configured,
                    safe_message: Some("Configured".to_owned()),
                    checked_at: None,
                },
                created_at: now,
                updated_at: now,
            })
            .expect("Provider Profile");
        let settings = load_settings(None).expect("settings");
        let input = application
            .workflow_advisor_input_for_label(
                "builder-tools",
                &settings,
                WorkflowConstraints::default(),
                Some("components"),
                Some("component"),
            )
            .expect("Builder input");
        let tools = pipeline_builder_live_tools(&input);
        let names = tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 51);
        assert_eq!(
            names,
            PipelineBuilderTool::ALL
                .iter()
                .map(|tool| tool.as_str())
                .collect::<BTreeSet<_>>()
        );
        for forbidden in [
            "publish_pipeline",
            "start_full_dataset_run",
            "set_api_key",
            "create_provider",
            "delete_provider",
            "execute_shell",
            "execute_python",
            "download_model",
            "open_arbitrary_url",
        ] {
            assert!(!names.contains(forbidden), "{forbidden}");
        }
        let encoded = serde_json::to_string(&input.provider_profiles).expect("safe Provider JSON");
        assert!(encoded.contains("Credential-safe fixture"));
        assert!(!encoded.contains("ANNOTAGENT_SUPER_SECRET_FIXTURE"));
        assert!(!encoded.contains("credential_ref"));
        assert!(!encoded.contains("locator"));
    }

    #[tokio::test]
    async fn pipeline_builder_profile_binding_runtime_policy_and_undo_persist_real_draft() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("builder-mutations", GENERIC_CLASSIFICATION_PROJECT)
            .expect("Project");
        let settings = load_settings(None).expect("settings");
        let provider_id = annotagent_core::ProviderId::new();
        let now = chrono::Utc::now();
        application
            .store
            .save_provider_profile(&annotagent_core::ProviderProfile {
                id: provider_id,
                display_name: "Offline Builder Provider".to_owned(),
                preset_id: Some("mock".to_owned()),
                adapter: ProviderAdapterKind::Mock,
                base_url: "https://mock.invalid/v1".parse().expect("URL"),
                organization: None,
                workspace: None,
                credential_ref: None,
                safe_headers: BTreeMap::new(),
                connection_policy: annotagent_core::ProviderConnectionPolicy::default(),
                enabled: true,
                health: annotagent_core::ProviderHealthSnapshot {
                    status: ProviderHealthStatus::Available,
                    safe_message: Some("Offline fixture".to_owned()),
                    checked_at: Some(now),
                },
                created_at: now,
                updated_at: now,
            })
            .expect("Provider Profile");
        let model_profile = ModelProfile {
            id: annotagent_core::ModelProfileId::new(),
            revision: 1,
            provider_id,
            display_name: "Offline classifier".to_owned(),
            remote_model_id: settings.provider.model.clone(),
            input_modalities: BTreeSet::from([
                annotagent_core::InputModality::Text,
                annotagent_core::InputModality::Image,
            ]),
            protocol_features: annotagent_core::ProtocolFeatures::default(),
            task_capabilities: BTreeSet::from([ModelCapability::ImageClassification]),
            capability_source: annotagent_core::CapabilityDeclarationSource::UserDeclared,
            limits: annotagent_core::ModelLimits::default(),
            generation_defaults: annotagent_core::GenerationDefaults::default(),
            pricing: annotagent_core::ModelPricing::default(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        application
            .store
            .save_model_profile(&model_profile)
            .expect("Model Profile");
        let constraints = WorkflowConstraints::default();
        let input = application
            .workflow_advisor_input_for_label(
                "builder-mutations",
                &settings,
                constraints.clone(),
                Some("scene"),
                Some("day"),
            )
            .expect("Builder input");
        let safe = application
            .suggest_label_pipeline_preview(
                "builder-mutations",
                &settings,
                "scene",
                "day",
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
                input_tokens: 0,
                output_tokens: 0,
            },
        };
        let provider = MockVisionProvider::new(MockScript {
            steps: vec![
                scripted_step("inspect_project", json!({})),
                scripted_step("inspect_label", json!({})),
                scripted_step("list_enabled_skills", json!({})),
                scripted_step("list_node_definitions", json!({})),
                scripted_step(
                    "list_compatible_models",
                    json!({"node_type": "capability.classify"}),
                ),
                scripted_step(
                    "create_pipeline_draft",
                    json!({"name": "Agent mutation fixture"}),
                ),
                scripted_step(
                    "add_pipeline_node",
                    json!({"node_type": "core.image_input", "node_id": "image", "configuration": {}}),
                ),
                scripted_step(
                    "add_pipeline_node",
                    json!({"node_type": "capability.classify", "node_id": "classifier", "configuration": {"labels": ["day", "night"]}}),
                ),
                scripted_step(
                    "bind_model_profile",
                    json!({"node_id": "classifier", "model_profile_id": model_profile.id, "locked": true}),
                ),
                scripted_step(
                    "set_runtime_policy",
                    json!({"policy_id": "retry", "configuration": {"maximum_attempts": 2}}),
                ),
                scripted_step("undo_last_draft_change", json!({})),
            ],
        });
        let report = application
            .run_workflow_advisor_with_provider(
                "builder-mutations",
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
            .expect("bounded Builder loop");
        let suggestion = report.suggestion.expect("persisted Draft suggestion");
        let persisted = application
            .store
            .get_workflow_draft(&suggestion.draft.id)
            .expect("persisted Draft");
        assert_eq!(persisted.name, "Agent mutation fixture");
        assert_eq!(persisted.nodes.len(), 2);
        assert!(
            persisted.runtime_policies.is_empty(),
            "undo restored Runtime Policy"
        );
        let classifier = persisted
            .nodes
            .iter()
            .find(|node| node.id == "classifier")
            .expect("classifier node");
        assert_eq!(
            classifier
                .model_profile_binding
                .as_ref()
                .map(|binding| (binding.model_profile_id, binding.locked)),
            Some((model_profile.id, true))
        );
        assert_eq!(classifier.model_binding.as_deref(), Some("default-vision"));
        assert!(
            application
                .store
                .list_published_workflow_versions(Some("builder-mutations"))
                .expect("published versions")
                .is_empty()
        );
        assert!(application.list_runs().expect("formal Runs").is_empty());
    }

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
        assert_eq!(settings.detection_workers.len(), 4);
        assert!(
            settings
                .detection_workers
                .iter()
                .all(|worker| !worker.enabled)
        );
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
                    && model.status == ModelAvailabilityStatus::MissingWeights)
        );
        assert!(models.resolve("locate-anything-local").is_err());
        let locate = models
            .expert_manifest("locate-anything-local")
            .expect("LocateAnything capability manifest");
        assert_eq!(locate.availability, ModelAvailability::MissingWeights);
        assert_eq!(locate.score_semantics, ScoreSemantics::NotProvided);
        assert!(
            locate
                .capabilities
                .contains(&ModelCapability::OpenVocabularyDetection)
        );
        assert!(
            locate
                .capabilities
                .contains(&ModelCapability::PhraseGrounding)
        );
        let sam = models
            .expert_manifest("sam2.1-hiera-tiny")
            .expect("SAM capability manifest");
        assert_eq!(sam.availability, ModelAvailability::MissingWeights);
        assert!(
            sam.capabilities
                .contains(&ModelCapability::PromptedSegmentation)
        );
        assert!(sam.input_contracts.iter().any(|contract| {
            contract.data_type == ContractDataType::Artifact(ArtifactKind::BoxPromptSet)
        }));
        assert!(sam.output_contracts.iter().any(|contract| {
            contract.data_type == ContractDataType::Artifact(ArtifactKind::MaskSet)
        }));
    }

    #[test]
    fn pipeline_builder_rules_separate_geometry_from_provider_and_semantic_failures() {
        assert!(PIPELINE_BUILDER_SYSTEM_PROMPT.contains("confidence is not geometry accuracy"));
        assert!(PIPELINE_BUILDER_SYSTEM_PROMPT.contains("NoCandidate has no box or point prompt"));
        assert!(PIPELINE_BUILDER_SYSTEM_PROMPT.contains("white footwear"));
        assert!(PIPELINE_BUILDER_SYSTEM_PROMPT.contains("Missing scores remain missing"));
        assert!(PIPELINE_BUILDER_SYSTEM_PROMPT.contains("submit for explicit human approval"));

        let geometry = AgentDryRunSummary {
            image_count: 4,
            successful_images: 4,
            detection_count: 8,
            geometry_review_count: 5,
            geometry_quality: annotagent_core::GeometryQualitySummary {
                total_candidates: 8,
                coarse_geometry_count: 8,
                geometry_review_count: 5,
                human_adjustment_count: 5,
                mean_manual_center_shift: Some(0.12),
                mean_manual_area_change: Some(-0.41),
                mean_refiner_iou: None,
                inaccurate_bbox_reason_count: 5,
            },
            ..AgentDryRunSummary::default()
        };
        let supported = assess_prompted_segmentation_revision(&geometry, true, true);
        assert!(supported.applicable);
        assert_eq!(supported.code, "geometry_refinement_supported");

        let provider_failure = assess_prompted_segmentation_revision(
            &AgentDryRunSummary {
                image_count: 4,
                failed_images: 4,
                provider_failure_count: 4,
                ..AgentDryRunSummary::default()
            },
            true,
            true,
        );
        assert!(!provider_failure.applicable);
        assert_eq!(provider_failure.code, "infrastructure_failure");

        let no_candidate = assess_prompted_segmentation_revision(
            &AgentDryRunSummary {
                image_count: 4,
                no_candidate_count: 4,
                ..AgentDryRunSummary::default()
            },
            true,
            true,
        );
        assert!(!no_candidate.applicable);
        assert_eq!(no_candidate.code, "no_promptable_candidate");

        let semantic = assess_prompted_segmentation_revision(
            &AgentDryRunSummary {
                image_count: 4,
                successful_images: 4,
                detection_count: 4,
                semantic_review_count: 3,
                domain_risk_count: 3,
                ..AgentDryRunSummary::default()
            },
            true,
            true,
        );
        assert!(!semantic.applicable);
        assert_eq!(semantic.code, "semantic_or_domain_error");

        let unavailable = assess_prompted_segmentation_revision(&geometry, false, true);
        assert!(!unavailable.applicable);
        assert_eq!(unavailable.code, "model_requires_setup");
    }

    #[test]
    fn geometry_evidence_builds_a_typed_prompt_mask_bbox_revision() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("geometry-builder", OBJECT_DETECTION_PROJECT)
            .expect("Project");
        let settings = load_settings(None).expect("Settings");
        let mut suggestion = application
            .suggest_label_pipeline_preview(
                "geometry-builder",
                &settings,
                "objects",
                "ball",
                &WorkflowConstraints::default(),
            )
            .expect("safe detection Draft");
        let evidence = AgentDryRunSummary {
            image_count: 3,
            successful_images: 3,
            detection_count: 6,
            geometry_review_count: 4,
            geometry_quality: annotagent_core::GeometryQualitySummary {
                total_candidates: 6,
                coarse_geometry_count: 6,
                geometry_review_count: 4,
                human_adjustment_count: 4,
                mean_manual_center_shift: Some(0.08),
                mean_manual_area_change: Some(-0.35),
                mean_refiner_iou: None,
                inaccurate_bbox_reason_count: 4,
            },
            ..AgentDryRunSummary::default()
        };
        assert!(
            add_prompted_segmentation_revision(
                &mut suggestion,
                "objects",
                "ball",
                "mock-prompted-segmenter",
                &evidence,
            )
            .expect("evidence-backed revision")
        );
        let node_types = suggestion
            .draft
            .nodes
            .iter()
            .map(|node| node.node_type.as_str())
            .collect::<BTreeSet<_>>();
        assert!(node_types.contains(annotagent_runtime::CORE_DETECTIONS_TO_BOX_PROMPTS));
        assert!(node_types.contains("capability.segment"));
        assert!(node_types.contains(annotagent_runtime::CORE_MASK_TO_BBOX));
        let segment = suggestion
            .draft
            .nodes
            .iter()
            .find(|node| node.node_type == "capability.segment")
            .expect("segment node");
        assert_eq!(
            segment.model_binding.as_deref(),
            Some("mock-prompted-segmenter")
        );
        let validation = application
            .validate_workflow_draft(&suggestion.draft, &settings, false)
            .expect("static validation");
        assert!(validation.valid, "{:#?}", validation.issues);
        assert!(
            !suggestion
                .draft
                .nodes
                .iter()
                .any(|node| node.node_type.contains("sam"))
        );
    }

    #[test]
    fn public_node_catalog_is_constrained_and_runtime_policies_are_separate() {
        let settings = load_settings(None).expect("default Settings");
        let (nodes, _) = workflow_catalog(&settings).expect("catalog");
        let ids = nodes
            .definitions()
            .into_iter()
            .map(|definition| definition.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            ids,
            BTreeSet::from([
                "capability.classify".to_owned(),
                "capability.detect".to_owned(),
                "capability.segment".to_owned(),
                "core.attach_result".to_owned(),
                "core.combine_evidence".to_owned(),
                "core.commit".to_owned(),
                "core.crop".to_owned(),
                "core.decision".to_owned(),
                "core.detections_to_box_prompts".to_owned(),
                "core.existing_annotations".to_owned(),
                "core.human_review".to_owned(),
                "core.image_input".to_owned(),
                "core.mask_to_bbox".to_owned(),
                "core.mask_to_polygon".to_owned(),
                "core.project_coordinates".to_owned(),
                "core.resize".to_owned(),
                "core.select_and_map".to_owned(),
                "core.tile".to_owned(),
                "core.validate".to_owned(),
            ])
        );
        for internal in [
            "core.artifact_cache",
            "core.filter",
            "core.map_label",
            "core.confidence_gate",
            "core.evidence_gate",
        ] {
            assert!(
                !ids.contains(internal),
                "{internal} leaked into public catalog"
            );
            assert!(
                nodes.get(internal).is_some(),
                "legacy operation must remain executable"
            );
        }
        let policy_ids = nodes
            .runtime_policies()
            .into_iter()
            .map(|policy| policy.id)
            .collect::<BTreeSet<_>>();
        assert!(policy_ids.is_superset(&BTreeSet::from([
            "cache".to_owned(),
            "replay".to_owned(),
            "retry".to_owned(),
            "budget".to_owned(),
        ])));
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
        assert!(
            migrated
                .detection_workers
                .iter()
                .any(|worker| worker.model_id == "sam2.1-hiera-tiny")
        );
        assert!(
            migrated
                .detection_workers
                .iter()
                .any(|worker| worker.model_id == "yolo-http-worker")
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
        let rfdetr = models
            .expert_manifest("rfdetr-specialist-local")
            .expect("RF-DETR manifest");
        let yolo = models
            .expert_manifest("yolo-http-worker")
            .expect("YOLO manifest");
        for manifest in [rfdetr, yolo] {
            assert!(
                manifest
                    .capabilities
                    .contains(&ModelCapability::ObjectDetection)
            );
            assert!(manifest.output_contracts.iter().any(|contract| {
                contract.data_type == ContractDataType::Artifact(ArtifactKind::DetectionSet)
            }));
            assert!(matches!(
                manifest.connection,
                ModelConnection::VisionWorkerModel { .. }
            ));
        }
    }

    #[test]
    fn expert_worker_registration_requires_persisted_live_evidence() {
        let mut settings = load_settings(None).expect("default Settings");
        let sam = settings
            .detection_workers
            .iter_mut()
            .find(|worker| worker.model_id == "sam2.1-hiera-tiny")
            .expect("SAM profile");
        sam.enabled = true;
        sam.version.model_version = "sam2.1-hiera-tiny-v1".to_owned();
        sam.version.checkpoint_sha256 = Some("b".repeat(64));
        sam.license.weight_license = Some("checkpoint-owner-supplied".to_owned());
        sam.availability = ModelAvailability::Available;
        sam.availability_evidence = ModelAvailabilityEvidence {
            health_passed: true,
            protocol_compatible: true,
            contracts_validated: true,
            sample_conversion_passed: true,
            weights_ready: true,
            checked_at: Some(chrono::Utc::now()),
            detail: Some("selected-image sample conversion passed".to_owned()),
        };
        assert_eq!(
            sam.expert_manifest()
                .expect("verified SAM manifest")
                .availability,
            ModelAvailability::Available
        );

        let serialized = toml::to_string_pretty(&settings).expect("serialized Settings");
        let restored: Settings = toml::from_str(&serialized).expect("restored Settings");
        let restored_sam = restored
            .detection_workers
            .iter()
            .find(|worker| worker.model_id == "sam2.1-hiera-tiny")
            .expect("restored SAM profile");
        assert!(restored_sam.availability_evidence.sample_conversion_passed);
        assert_eq!(
            restored_sam
                .expert_manifest()
                .expect("restored verified manifest")
                .availability,
            ModelAvailability::Available
        );

        let mut untested = restored_sam.clone();
        untested.availability_evidence.sample_conversion_passed = false;
        assert_eq!(
            untested
                .expert_manifest()
                .expect("untested manifest")
                .availability,
            ModelAvailability::Unknown
        );
        untested.authentication_reference = Some("workspace-secret".to_owned());
        assert!(untested.validate_authentication_reference().is_err());
    }

    #[test]
    fn legacy_sam_refiner_migrates_to_an_auditable_capability_chain() {
        let now = chrono::Utc::now();
        let mut draft = WorkflowDraft {
            schema_version: 1,
            id: "legacy-sam".to_owned(),
            project_id: "project".to_owned(),
            name: "Legacy SAM".to_owned(),
            status: WorkflowDraftStatus::Editing,
            nodes: vec![
                WorkflowDraftNode {
                    id: "image".to_owned(),
                    node_type: "core.image_input".to_owned(),
                    kind: WorkflowNodeKind::ImageInput,
                    outputs: vec![NodePort {
                        id: "image".to_owned(),
                        artifact_type: ArtifactKind::Image,
                        required: true,
                        multiple: false,
                    }],
                    ..WorkflowDraftNode::default()
                },
                WorkflowDraftNode {
                    id: "coarse".to_owned(),
                    node_type: "capability.detect".to_owned(),
                    kind: WorkflowNodeKind::VisionModel,
                    outputs: vec![NodePort {
                        id: "detections".to_owned(),
                        artifact_type: ArtifactKind::DetectionSet,
                        required: true,
                        multiple: true,
                    }],
                    ..WorkflowDraftNode::default()
                },
                WorkflowDraftNode {
                    id: "refine".to_owned(),
                    node_type: "sam_prompted_refiner".to_owned(),
                    kind: WorkflowNodeKind::Refiner,
                    depends_on: vec!["image".to_owned(), "coarse".to_owned()],
                    model_binding: Some("sam2.1-hiera-tiny".to_owned()),
                    outputs: vec![NodePort {
                        id: "detections".to_owned(),
                        artifact_type: ArtifactKind::DetectionSet,
                        required: true,
                        multiple: true,
                    }],
                    ..WorkflowDraftNode::default()
                },
                WorkflowDraftNode {
                    id: "next".to_owned(),
                    node_type: "core.select_and_map".to_owned(),
                    kind: WorkflowNodeKind::Transform,
                    depends_on: vec!["refine".to_owned()],
                    ..WorkflowDraftNode::default()
                },
            ],
            edges: vec![
                WorkflowEdge {
                    from_node: "image".to_owned(),
                    from_port: "image".to_owned(),
                    to_node: "refine".to_owned(),
                    to_port: "image".to_owned(),
                    route: None,
                },
                WorkflowEdge {
                    from_node: "coarse".to_owned(),
                    from_port: "detections".to_owned(),
                    to_node: "refine".to_owned(),
                    to_port: "detections".to_owned(),
                    route: None,
                },
                WorkflowEdge {
                    from_node: "refine".to_owned(),
                    from_port: "detections".to_owned(),
                    to_node: "next".to_owned(),
                    to_port: "detections".to_owned(),
                    route: None,
                },
            ],
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: false,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        };
        assert!(migrate_legacy_expert_workflow(&mut draft).expect("migration"));
        assert_eq!(draft.schema_version, WORKFLOW_SCHEMA_VERSION);
        assert_eq!(
            draft
                .nodes
                .iter()
                .find(|node| node.id == "refine")
                .expect("stable downstream node id")
                .node_type,
            annotagent_runtime::CORE_MASK_TO_BBOX
        );
        let chain = draft
            .nodes
            .iter()
            .map(|node| node.node_type.as_str())
            .collect::<BTreeSet<_>>();
        assert!(chain.contains(annotagent_runtime::CORE_DETECTIONS_TO_BOX_PROMPTS));
        assert!(chain.contains("capability.segment"));
        assert!(chain.contains(annotagent_runtime::CORE_MASK_TO_BBOX));
        assert!(
            draft
                .edges
                .iter()
                .any(|edge| { edge.from_node == "refine" && edge.to_node == "next" })
        );
        assert!(!migrate_legacy_expert_workflow(&mut draft).expect("idempotent migration"));
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
        let quality_draft = application
            .suggest_label_pipeline(
                "object-detection",
                &settings,
                "objects",
                "ball",
                &WorkflowConstraints::default(),
            )
            .expect("controlled detection Pipeline")
            .draft;
        application
            .save_workflow_draft(quality_draft.clone())
            .expect("save quality Draft");
        let quality_dry_run = application
            .dry_run_workflow_samples(&quality_draft.id, &settings, &[0])
            .await
            .expect("quality-aware detection Dry Run");
        assert_eq!(quality_dry_run.summary.provider_failure_count, 0);
        assert_eq!(quality_dry_run.summary.missing_score_count, 0);
        assert_eq!(quality_dry_run.summary.geometry_quality.total_candidates, 1);
        assert_eq!(
            quality_dry_run.samples[0].outcomes[0]
                .geometry_quality
                .as_ref()
                .map(|report| report.geometry_semantics),
            Some(annotagent_core::GeometrySemantics::PredictedGeometry)
        );
        let mut empty_draft = quality_draft.clone();
        empty_draft.id = "object-detection-empty-quality".to_owned();
        let composition = empty_draft.label_pipeline.as_mut().expect("Label Pipeline");
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
            .filter(|step| step.kind == WorkflowNodeKind::VisionModel)
        {
            step.node_type =
                annotagent_skill_object_detection::OBJECT_DETECTION_OPERATION.to_owned();
            step.model_binding
                .as_mut()
                .expect("detector binding")
                .model_id = "mock-object-detector".to_owned();
            step.parameters.insert("mock_empty".to_owned(), json!(true));
        }
        let empty_draft = application
            .save_workflow_draft(empty_draft)
            .expect("save empty-result Draft");
        let empty_dry_run = application
            .dry_run_workflow_samples(&empty_draft.id, &settings, &[0])
            .await
            .expect("empty-result Dry Run");
        assert_eq!(
            empty_dry_run.summary.no_candidate_count, 1,
            "{empty_dry_run:#?}"
        );
        assert_eq!(
            empty_dry_run.samples[0].failure_classes,
            vec![AnnotationFailureClass::NoCandidate]
        );
        let mut provider_failure_draft = quality_draft;
        provider_failure_draft.id = "object-detection-provider-failure".to_owned();
        for step in provider_failure_draft
            .label_pipeline
            .as_mut()
            .expect("Label Pipeline")
            .shared_stages
            .iter_mut()
            .flat_map(|stage| stage.steps.iter_mut())
            .filter(|step| step.kind == WorkflowNodeKind::VisionModel)
        {
            step.node_type =
                annotagent_skill_object_detection::OBJECT_DETECTION_OPERATION.to_owned();
            step.model_binding
                .as_mut()
                .expect("detector binding")
                .model_id = "mock-object-detector".to_owned();
            step.parameters
                .insert("mock_backend_error".to_owned(), json!(true));
        }
        let provider_failure_draft = application
            .save_workflow_draft(provider_failure_draft)
            .expect("save Provider-failure Draft");
        let provider_failure_dry_run = application
            .dry_run_workflow_samples(&provider_failure_draft.id, &settings, &[0])
            .await
            .expect("Provider-failure Dry Run remains inspectable");
        assert_eq!(
            provider_failure_dry_run.summary.provider_failure_count, 1,
            "{provider_failure_dry_run:#?}"
        );
        assert_eq!(provider_failure_dry_run.summary.no_candidate_count, 0);
        assert!(
            provider_failure_dry_run.samples[0]
                .failure_classes
                .contains(&AnnotationFailureClass::ProviderFailure)
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
            runtime_policies: BTreeMap::new(),
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
    async fn published_prompted_segmentation_pipeline_runs_end_to_end_offline() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("prompted-segmentation", OBJECT_DETECTION_PROJECT)
            .expect("detection Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary
                .path()
                .join("prompted-segmentation/images/sample.png"),
        )
        .expect("sample image");
        let port = |id: &str, artifact_type| NodePort {
            id: id.to_owned(),
            artifact_type,
            required: true,
            multiple: false,
        };
        let node = |id: &str,
                    node_type: &str,
                    kind: WorkflowNodeKind,
                    inputs: Vec<NodePort>,
                    outputs: Vec<NodePort>| WorkflowDraftNode {
            id: id.to_owned(),
            node_type: node_type.to_owned(),
            kind,
            inputs,
            outputs,
            ..WorkflowDraftNode::default()
        };
        let edge = |from_node: &str, from_port: &str, to_node: &str, to_port: &str| WorkflowEdge {
            from_node: from_node.to_owned(),
            from_port: from_port.to_owned(),
            to_node: to_node.to_owned(),
            to_port: to_port.to_owned(),
            route: None,
        };
        let now = chrono::Utc::now();
        let mut detector = node(
            "detector",
            "capability.detect",
            WorkflowNodeKind::VisionModel,
            vec![port("images", ArtifactKind::Image)],
            vec![port("detections", ArtifactKind::DetectionSet)],
        );
        detector.model_binding = Some("mock-detector".to_owned());
        detector.parameters = BTreeMap::from([
            ("target_labels".to_owned(), json!(["ball"])),
            ("mock_model_label".to_owned(), json!("ball")),
            ("mock_bbox".to_owned(), json!([0.10, 0.20, 0.40, 0.40])),
        ]);
        let mut segment = node(
            "segment",
            "capability.segment",
            WorkflowNodeKind::VisionModel,
            vec![
                port("images", ArtifactKind::Image),
                port("box_prompts", ArtifactKind::BoxPromptSet),
            ],
            vec![port("masks", ArtifactKind::MaskSet)],
        );
        segment.model_binding = Some("mock-prompted-segmenter".to_owned());
        segment.parameters = BTreeMap::from([("mock_inset".to_owned(), json!(0.10))]);
        let draft = WorkflowDraft {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            id: "prompted-segmentation-draft".to_owned(),
            project_id: "prompted-segmentation".to_owned(),
            name: "Prompted Segmentation Demo".to_owned(),
            status: WorkflowDraftStatus::Editing,
            nodes: vec![
                node(
                    "image",
                    annotagent_core::IMAGE_INPUT_OPERATION,
                    WorkflowNodeKind::ImageInput,
                    Vec::new(),
                    vec![port("image", ArtifactKind::Image)],
                ),
                detector,
                node(
                    "prompts",
                    annotagent_runtime::CORE_DETECTIONS_TO_BOX_PROMPTS,
                    WorkflowNodeKind::Transform,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    vec![port("prompts", ArtifactKind::BoxPromptSet)],
                ),
                segment,
                node(
                    "refine",
                    annotagent_runtime::CORE_MASK_TO_BBOX,
                    WorkflowNodeKind::Transform,
                    vec![
                        port("masks", ArtifactKind::MaskSet),
                        port("box_prompts", ArtifactKind::BoxPromptSet),
                    ],
                    vec![port("detections", ArtifactKind::DetectionSet)],
                ),
                node(
                    "review",
                    "core.human_review",
                    WorkflowNodeKind::HumanReview,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    vec![port("detections", ArtifactKind::DetectionSet)],
                ),
                {
                    let mut commit = node(
                        "commit",
                        "core.commit",
                        WorkflowNodeKind::Commit,
                        vec![port("detections", ArtifactKind::DetectionSet)],
                        Vec::new(),
                    );
                    commit.parameters = BTreeMap::from([("task_id".to_owned(), json!("objects"))]);
                    commit
                },
            ],
            edges: vec![
                edge("image", "image", "detector", "images"),
                edge("detector", "detections", "prompts", "detections"),
                edge("image", "image", "segment", "images"),
                edge("prompts", "prompts", "segment", "box_prompts"),
                edge("segment", "masks", "refine", "masks"),
                edge("prompts", "prompts", "refine", "box_prompts"),
                edge("refine", "detections", "review", "detections"),
                edge("review", "detections", "commit", "detections"),
            ],
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: false,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        };
        application
            .save_workflow_draft(draft)
            .expect("save prompted segmentation Draft");
        let settings = load_settings(None).expect("settings");
        let published = application
            .publish_workflow("prompted-segmentation-draft", &settings)
            .expect("publish prompted segmentation");
        let started = application
            .start_run_path_with_settings_idempotent_workflow(
                &temporary.path().join("prompted-segmentation/project.yaml"),
                "mock",
                settings,
                None,
                Some("prompted-segmentation-run"),
                Some((&published.workflow_id, published.version)),
            )
            .expect("start prompted segmentation Run");
        let result = application.wait_run(started.run_id).await.expect("Run");
        assert_eq!(
            result.status,
            RunStatus::CompletedWithReview,
            "{:#?}",
            result.issues
        );
        assert!(result.committed.is_empty());
        assert_eq!(result.review_queue.len(), 1);
        let annotagent_core::AnnotationValue::BoundingBox { rect } = &result.review_queue[0].value
        else {
            panic!("refined bounding box")
        };
        assert!(rect.width() < 0.40 && rect.height() < 0.40);
        let inspection = application
            .inspect_run_pipeline_artifacts(started.run_id)
            .expect("Pipeline Artifact inspection");
        let kinds = inspection
            .nodes
            .iter()
            .flat_map(|node| node.outputs.iter().map(PipelineArtifact::artifact_type))
            .collect::<BTreeSet<_>>();
        assert!(kinds.contains(&ArtifactKind::BoxPromptSet));
        assert!(kinds.contains(&ArtifactKind::MaskSet));
        assert!(kinds.contains(&ArtifactKind::DetectionSet));
    }

    #[tokio::test]
    async fn published_runtime_routes_same_operation_to_each_frozen_model_profile() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("multi-profile", GENERIC_CLASSIFICATION_PROJECT)
            .expect("Project");
        let image_path = temporary.path().join("multi-profile/images/sample.png");
        annotagent_image_tools::generate_synthetic_inspection(&image_path).expect("image");
        let now = chrono::Utc::now();
        let provider = ProviderProfile {
            id: ProviderId::new(),
            display_name: "Shared Mock Provider".to_owned(),
            preset_id: Some("mock".to_owned()),
            adapter: ProviderAdapterKind::Mock,
            base_url: "http://127.0.0.1".parse().expect("URL"),
            organization: None,
            workspace: None,
            credential_ref: None,
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
        let model = |name: &str| ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id: provider.id,
            display_name: name.to_owned(),
            remote_model_id: name.to_owned(),
            input_modalities: BTreeSet::from([InputModality::Image]),
            protocol_features: ProtocolFeatures::default(),
            task_capabilities: BTreeSet::from([ModelCapability::ImageClassification]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing::default(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        };
        let model_a = model("classifier-a");
        let model_b = model("classifier-b");
        let port = |id: &str, artifact_type| NodePort {
            id: id.to_owned(),
            artifact_type,
            required: true,
            multiple: false,
        };
        let classifier = |id: &str, profile: &ModelProfile, label: &str| WorkflowDraftNode {
            id: id.to_owned(),
            node_type: annotagent_skill_classification::CLASSIFICATION_OPERATION.to_owned(),
            kind: WorkflowNodeKind::VisionModel,
            inputs: vec![port("image", ArtifactKind::Image)],
            outputs: vec![port("classifications", ArtifactKind::ClassificationSet)],
            model_profile_binding: Some(annotagent_core::WorkflowModelBinding {
                model_profile_id: profile.id,
                locked: true,
            }),
            parameters: BTreeMap::from([
                ("labels".to_owned(), json!(["day", "night"])),
                ("mock_label".to_owned(), json!(label)),
            ]),
            ..WorkflowDraftNode::default()
        };
        let draft = WorkflowDraft {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            id: "multi-profile-runtime".to_owned(),
            project_id: "multi-profile".to_owned(),
            name: "Two profile execution".to_owned(),
            status: WorkflowDraftStatus::Published,
            nodes: vec![
                WorkflowDraftNode {
                    id: "image".to_owned(),
                    node_type: annotagent_core::IMAGE_INPUT_OPERATION.to_owned(),
                    kind: WorkflowNodeKind::ImageInput,
                    outputs: vec![port("image", ArtifactKind::Image)],
                    ..WorkflowDraftNode::default()
                },
                classifier("classifier-a", &model_a, "day"),
                classifier("classifier-b", &model_b, "night"),
            ],
            edges: vec![
                WorkflowEdge {
                    from_node: "image".to_owned(),
                    from_port: "image".to_owned(),
                    to_node: "classifier-a".to_owned(),
                    to_port: "image".to_owned(),
                    route: None,
                },
                WorkflowEdge {
                    from_node: "image".to_owned(),
                    from_port: "image".to_owned(),
                    to_node: "classifier-b".to_owned(),
                    to_port: "image".to_owned(),
                    route: None,
                },
            ],
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: false,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        };
        let snapshot = WorkflowSnapshot {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            draft: Some(draft.clone()),
            model_profiles: vec![
                ModelProfileSnapshot::frozen(&model_a, &provider).expect("snapshot A"),
                ModelProfileSnapshot::frozen(&model_b, &provider).expect("snapshot B"),
            ],
            ..WorkflowSnapshot::default()
        };
        let workflow = PublishedWorkflowVersion {
            workflow_id: draft.id.clone(),
            version: 1,
            project_id: draft.project_id.clone(),
            source_draft_id: draft.id.clone(),
            content_hash: annotagent_image_tools::sha256(
                &snapshot.content_hash_material().expect("hash material"),
            ),
            draft,
            snapshot,
            published_at: now,
        };
        let project_path = application.project_path("multi-profile").expect("path");
        let (project, _) =
            load_project_schema_with_registry(&project_path, &application.skills).expect("schema");
        let image = Arc::new(load_image(&image_path, 40_000_000).expect("frame"));
        let model_image = to_model_image("multi-profile-test", &image, 1280).expect("model image");
        let runtime = PublishedWorkflowRuntime::new(
            workflow,
            "mock",
            &load_settings(None).expect("settings"),
            None,
            application.store.clone(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .expect("Runtime");
        let result = runtime
            .execute_sandbox(&ImageRunRequest {
                run_id: RunId::new(),
                project_id: stable_project_id(project_path.parent().expect("root")),
                project_root: project_path.parent().expect("root").to_path_buf(),
                project: Arc::new(project),
                image_id: ImageId::new(),
                image,
                model_image: Some(model_image),
            })
            .await
            .expect("execution");
        assert_eq!(
            result.checkpoint.node_outputs["classifier-a"].metadata["model"],
            json!("classifier-a")
        );
        assert_eq!(
            result.checkpoint.node_outputs["classifier-b"].metadata["model"],
            json!("classifier-b")
        );
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
        assert_eq!(
            report.session.status,
            AgentSessionStatus::WaitingForHuman,
            "{report:#?}"
        );
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
            "inspect_existing_pipeline",
            "list_enabled_skills",
            "list_node_definitions",
            "list_provider_profiles",
            "list_available_capabilities",
            "list_compatible_models",
            "list_pipeline_templates",
            "create_draft_from_template",
            "disconnect_pipeline_nodes",
            "validate_pipeline",
            "connect_pipeline_nodes",
            "validate_pipeline",
            "dry_run_pipeline",
            "inspect_dry_run_summary",
            "inspect_failure_classes",
            "inspect_geometry_quality",
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
                "inspect_existing_pipeline",
                "list_enabled_skills",
                "list_node_definitions",
                "list_provider_profiles",
                "list_available_capabilities",
                "list_compatible_models",
                "list_pipeline_templates",
                "create_draft_from_template",
                "disconnect_pipeline_nodes",
                "validate_pipeline",
                "connect_pipeline_nodes",
                "validate_pipeline",
                "dry_run_pipeline",
                "inspect_dry_run_summary",
                "inspect_failure_classes",
                "inspect_geometry_quality",
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
        let selected_model = register_pipeline_builder_model(&application, "scripted-builder-v1");
        let settings = load_settings(None).expect("settings");
        let constraints = WorkflowConstraints::default();
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
                scripted_step("list_node_definitions", json!({}), Some("skill_ids")),
                scripted_step("list_compatible_models", json!({}), Some("nodes")),
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
            .run_workflow_advisor_with_selected_model(
                "live-builder",
                &settings,
                &selected_model,
                &provider,
                &constraints,
                Some(("scene", "day")),
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
        assert_eq!(report.session.model_calls.len(), 11);
        assert_eq!(
            report
                .session
                .model_selection
                .as_ref()
                .map(|selection| selection.model_profile_id),
            Some(selected_model.model.id)
        );
        assert!(report.session.model_calls.iter().all(|call| {
            call.model_profile_id == Some(selected_model.model.id)
                && call.request_id.is_some()
                && call.succeeded
        }));
        assert!(report.session.usage.cost > rust_decimal::Decimal::ZERO);
        let session_json = serde_json::to_string(&report.session).expect("Agent Session JSON");
        assert!(!session_json.contains("credential"));
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
                "list_node_definitions",
                "list_compatible_models",
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
    async fn pipeline_builder_baseline_reproduces_repeated_inspection_budget_exhaustion() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("inspection-loop", GENERIC_CLASSIFICATION_PROJECT)
            .expect("classification Project");
        let selected_model =
            register_pipeline_builder_model(&application, "scripted-inspection-loop");

        let repeated_calls = |count: usize, inspect_models: bool| {
            (0..count)
                .map(|index| {
                    if inspect_models && index % 2 == 1 {
                        MockToolCall {
                            name: "inspect_model_profile".to_owned(),
                            arguments: json!({"model_profile_id": selected_model.model.id}),
                        }
                    } else {
                        MockToolCall {
                            name: "inspect_node_definition".to_owned(),
                            arguments: json!({"node_type": "core.image_input"}),
                        }
                    }
                })
                .collect::<Vec<_>>()
        };
        // Six turns leave ten calls in the budget. The seventh response asks for eleven more,
        // reproducing the live GLM trace where the 49th read changes the generic Agent outcome to
        // BudgetExceeded after 48 successful, non-progressing observations.
        let provider = MockVisionProvider::new(MockScript {
            steps: vec![5_usize, 3, 4, 8, 8, 10, 11]
                .into_iter()
                .enumerate()
                .map(|(turn, count)| MockStep {
                    expect_task: Some("pipeline_builder".to_owned()),
                    expect_message_contains: None,
                    response: MockResponseSpec::ToolCalls {
                        calls: repeated_calls(count, turn % 2 == 1),
                        content: None,
                    },
                    usage: MockUsage {
                        input_tokens: 13_618,
                        output_tokens: 1_290,
                    },
                })
                .collect(),
        });

        let report = application
            .run_workflow_advisor_with_selected_model(
                "inspection-loop",
                &load_settings(None).expect("settings"),
                &selected_model,
                &provider,
                &WorkflowConstraints::default(),
                Some(("scene", "day")),
                PipelineBuilderConstraints::default(),
                CancellationToken::new(),
            )
            .await
            .expect("bounded Pipeline Builder loop");

        assert_eq!(provider.remaining_steps(), 0);
        assert_eq!(report.session.status, AgentSessionStatus::BudgetExceeded);
        assert_eq!(report.session.usage.tool_calls, 48);
        assert_eq!(report.session.model_calls.len(), 7);
        assert_eq!(report.session.usage.input_tokens, 95_326);
        assert!(report.suggestion.is_none());
        assert!(
            application
                .store
                .list_workflow_drafts(Some("inspection-loop"))
                .expect("Drafts")
                .is_empty()
        );
        assert_eq!(
            report.session.stop_reason.as_deref(),
            Some("step or tool-call budget exhausted")
        );
    }

    #[tokio::test]
    async fn live_pipeline_builder_exposes_and_loads_exact_domain_resource_ids() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "live-resource-builder",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("RoboCup Project");
        generate_synthetic_robocup(
            &temporary
                .path()
                .join("live-resource-builder/images/synthetic.png"),
        )
        .expect("synthetic image");
        let selected_model =
            register_pipeline_builder_model(&application, "scripted-resource-builder");
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
                MockStep {
                    expect_task: Some("pipeline_builder".to_owned()),
                    expect_message_contains: None,
                    response: MockResponseSpec::ToolCalls {
                        calls: vec![
                            MockToolCall {
                                name: "inspect_project".to_owned(),
                                arguments: json!({}),
                            },
                            MockToolCall {
                                name: "inspect_label".to_owned(),
                                arguments: json!({}),
                            },
                            MockToolCall {
                                name: "list_enabled_skills".to_owned(),
                                arguments: json!({}),
                            },
                        ],
                        content: None,
                    },
                    usage: MockUsage {
                        input_tokens: 10,
                        output_tokens: 5,
                    },
                },
                scripted_step(
                    "load_skill_resource",
                    json!({
                        "skill_id": "robocup",
                        "resource_name": "resources/advisor.md"
                    }),
                    Some("resources/advisor.md"),
                ),
                scripted_step(
                    "list_node_definitions",
                    json!({}),
                    Some("smallest Pipeline"),
                ),
                scripted_step("list_pipeline_templates", json!({}), Some("nodes")),
                scripted_step("list_compatible_models", json!({}), Some("nodes")),
                scripted_step(
                    "create_draft_from_template",
                    json!({"template_id": "safe_default"}),
                    Some("models"),
                ),
                scripted_step("validate_pipeline", json!({}), Some("draft_id")),
                MockStep {
                    expect_task: Some("pipeline_builder".to_owned()),
                    expect_message_contains: Some("valid".to_owned()),
                    response: MockResponseSpec::Content {
                        content: "The Draft is valid, so I will test it next.".to_owned(),
                    },
                    usage: MockUsage {
                        input_tokens: 10,
                        output_tokens: 5,
                    },
                },
                scripted_step(
                    "dry_run_pipeline",
                    json!({"image_indices": [0]}),
                    Some("did not include a registered Tool Call"),
                ),
                scripted_step("inspect_dry_run_summary", json!({}), Some("sandbox")),
                scripted_step(
                    "submit_draft_for_human_approval",
                    json!({
                        "name": "RoboCup Ball resource-aware proposal",
                        "rationale": ["Loaded the exact enabled Domain Advisor resource."],
                        "warnings": [],
                        "alternatives": []
                    }),
                    Some("review_rate"),
                ),
            ],
        });

        let report = application
            .run_workflow_advisor_with_selected_model(
                "live-resource-builder",
                &load_settings(None).expect("settings"),
                &selected_model,
                &provider,
                &WorkflowConstraints::default(),
                Some(("objects", "ball")),
                PipelineBuilderConstraints::default(),
                CancellationToken::new(),
            )
            .await
            .expect("resource-aware Pipeline Builder");

        assert_eq!(provider.remaining_steps(), 0);
        assert_eq!(report.session.status, AgentSessionStatus::WaitingForHuman);
        let skill_list = report
            .session
            .steps
            .iter()
            .find(|step| step.tool_name == "list_enabled_skills")
            .expect("enabled Skills result");
        assert!(
            skill_list.result["model_payload"]["declared_resource_ids"]
                .as_array()
                .is_some_and(|resources| resources.contains(&json!("resources/advisor.md")))
        );
        let resource = report
            .session
            .steps
            .iter()
            .find(|step| step.tool_name == "load_skill_resource")
            .expect("loaded resource");
        assert!(resource.success);
        assert_eq!(
            resource.result["model_payload"]["resource_id"],
            json!("resources/advisor.md")
        );
        let templates = report
            .session
            .steps
            .iter()
            .find(|step| step.tool_name == "list_pipeline_templates")
            .expect("compatible templates");
        assert_eq!(
            templates.result["model_payload"]["templates"]
                .as_array()
                .expect("template list")
                .iter()
                .map(|template| template["id"].as_str().expect("template id"))
                .collect::<Vec<_>>(),
            vec!["safe_default"]
        );
        assert!(
            templates.result["model_payload"]["incompatible_templates_hidden"]
                .as_u64()
                .is_some_and(|count| count > 0)
        );
        let dry_run_summary = report
            .session
            .steps
            .iter()
            .find(|step| step.tool_name == "inspect_dry_run_summary")
            .expect("Dry Run summary");
        assert_eq!(
            dry_run_summary.result["model_payload"]["next_required_tool"],
            json!("submit_draft_for_human_approval")
        );
    }

    #[tokio::test]
    async fn live_pipeline_builder_stops_repeated_invalid_resource_guesses() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "live-resource-stall",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("RoboCup Project");
        generate_synthetic_robocup(
            &temporary
                .path()
                .join("live-resource-stall/images/synthetic.png"),
        )
        .expect("synthetic image");
        let selected_model =
            register_pipeline_builder_model(&application, "scripted-resource-stall");
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
        let provider = MockVisionProvider::new(MockScript {
            steps: vec![
                scripted_step("inspect_project", json!({})),
                scripted_step("inspect_label", json!({})),
                scripted_step("list_enabled_skills", json!({})),
                scripted_step("list_node_definitions", json!({})),
                scripted_step("list_compatible_models", json!({})),
                scripted_step(
                    "load_skill_resource",
                    json!({"skill_id": "robocup", "resource_name": "advisor"}),
                ),
                scripted_step(
                    "load_skill_resource",
                    json!({"skill_id": "robocup", "resource_name": "domain_advisor"}),
                ),
                scripted_step(
                    "load_skill_resource",
                    json!({"skill_id": "robocup", "resource_name": "DomainAdvisor"}),
                ),
            ],
        });

        let report = application
            .run_workflow_advisor_with_selected_model(
                "live-resource-stall",
                &load_settings(None).expect("settings"),
                &selected_model,
                &provider,
                &WorkflowConstraints::default(),
                Some(("objects", "ball")),
                PipelineBuilderConstraints::default(),
                CancellationToken::new(),
            )
            .await
            .expect("bounded invalid resource handling");

        assert_eq!(provider.remaining_steps(), 0);
        assert_eq!(report.session.status, AgentSessionStatus::Failed);
        assert!(
            report
                .session
                .stop_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("3 failed load_skill_resource attempts"))
        );
        let last = report
            .session
            .steps
            .last()
            .expect("third failed resource load");
        assert!(!last.success);
        assert_eq!(last.result["model_payload"]["retryable"], json!(false));
        assert!(
            last.result["model_payload"]["error"]
                .as_str()
                .is_some_and(|error| error.contains("resources/advisor.md"))
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
        let selected_builder =
            register_pipeline_builder_model(&application, "scripted-revision-builder");
        let incompatible_model = selected_builder.model.clone();
        let mut compatible_model = incompatible_model.clone();
        compatible_model.id = ModelProfileId::new();
        compatible_model.remote_model_id = "mock-detector".to_owned();
        compatible_model
            .input_modalities
            .insert(InputModality::Image);
        compatible_model.task_capabilities.extend([
            ModelCapability::VisionLanguage,
            ModelCapability::ObjectDetection,
            ModelCapability::OpenVocabularyDetection,
            ModelCapability::PhraseGrounding,
        ]);
        application
            .store
            .save_model_profile(&compatible_model)
            .expect("compatible detection Model Profile");
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
        let mut safe = application
            .suggest_label_pipeline_preview(
                "live-revision",
                &settings,
                "components",
                "component",
                &constraints,
            )
            .expect("safe Draft");
        let detector = safe
            .draft
            .nodes
            .iter_mut()
            .find(|node| {
                matches!(
                    node.kind,
                    WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
                )
            })
            .expect("detection model node");
        detector.node_type = "capability.detect".to_owned();
        let detector_id = detector.id.clone();
        let detector_type = detector.node_type.clone();
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
                scripted_step("list_node_definitions", json!({})),
                scripted_step("list_compatible_models", json!({})),
                scripted_step("create_draft_from_template", json!({})),
                scripted_step(
                    "bind_model_profile",
                    json!({
                        "node_id": detector_id,
                        "model_profile_id": incompatible_model.id,
                        "locked": true
                    }),
                ),
                scripted_step(
                    "list_compatible_models",
                    json!({"node_type": detector_type}),
                ),
                scripted_step(
                    "bind_model_profile",
                    json!({
                        "node_id": detector_id,
                        "model_profile_id": compatible_model.id,
                        "locked": true
                    }),
                ),
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
            maximum_agent_turns: 24,
            maximum_tool_calls: 24,
            ..PipelineBuilderConstraints::default()
        };

        let report = application
            .run_workflow_advisor_loop(
                "live-revision",
                &settings,
                &constraints,
                Some(("components", "component")),
                input,
                safe,
                &provider,
                Some(&selected_builder),
                builder_constraints,
                CancellationToken::new(),
            )
            .await
            .expect("live revision loop");

        assert_eq!(provider.remaining_steps(), 0);
        assert_eq!(
            report.session.status,
            AgentSessionStatus::WaitingForHuman,
            "{report:#?}"
        );
        assert!(report.approval_required);
        assert_eq!(report.session.usage.tool_calls, 20);
        assert_eq!(report.session.usage.input_tokens, 200);
        assert_eq!(report.session.usage.output_tokens, 100);
        let incompatible_binding = report
            .session
            .steps
            .iter()
            .find(|step| {
                step.tool_name == "bind_model_profile"
                    && step.arguments["model_profile_id"] == json!(incompatible_model.id)
            })
            .expect("incompatible binding attempt");
        assert!(!incompatible_binding.success);
        assert_eq!(
            incompatible_binding.result["model_payload"]["code"],
            json!("incompatible_model_capability"),
            "{result:#?}",
            result = incompatible_binding.result
        );
        assert!(report.session.steps.iter().any(|step| {
            step.tool_name == "bind_model_profile"
                && step.success
                && step.arguments["model_profile_id"] == json!(compatible_model.id)
        }));
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

    /// Explicitly opt-in because this test sends billable requests to a real Provider. It never
    /// persists the API key and is excluded from normal CI.
    #[tokio::test]
    #[ignore = "set ANNOTAGENT_RUN_BILLABLE_PROVIDER_SMOKE=1 and the PIPELINE_BUILDER_SMOKE_* environment variables"]
    async fn real_openai_compatible_pipeline_builder_smoke_when_explicitly_enabled() {
        assert_eq!(
            std::env::var("ANNOTAGENT_RUN_BILLABLE_PROVIDER_SMOKE").as_deref(),
            Ok("1"),
            "billable smoke requires explicit opt-in"
        );
        let base_url = std::env::var("PIPELINE_BUILDER_SMOKE_BASE_URL")
            .expect("PIPELINE_BUILDER_SMOKE_BASE_URL");
        let remote_model_id =
            std::env::var("PIPELINE_BUILDER_SMOKE_MODEL").expect("PIPELINE_BUILDER_SMOKE_MODEL");
        let api_key = std::env::var("PIPELINE_BUILDER_SMOKE_API_KEY")
            .expect("PIPELINE_BUILDER_SMOKE_API_KEY");
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project("provider-smoke", GENERIC_CLASSIFICATION_PROJECT)
            .expect("Project");
        annotagent_image_tools::generate_synthetic_inspection(
            &temporary.path().join("provider-smoke/images/sample.png"),
        )
        .expect("sample image");

        let now = chrono::Utc::now();
        let provider_id = annotagent_core::ProviderId::new();
        let provider_profile = ProviderProfile {
            id: provider_id,
            display_name: "Explicit live smoke".to_owned(),
            preset_id: None,
            adapter: ProviderAdapterKind::OpenAiCompatible,
            base_url: base_url.parse().expect("Provider URL"),
            organization: None,
            workspace: None,
            credential_ref: Some(annotagent_core::CredentialReference {
                provider_id,
                source: annotagent_core::CredentialSource::EnvironmentVariable,
                locator: "PIPELINE_BUILDER_SMOKE_API_KEY".to_owned(),
            }),
            safe_headers: BTreeMap::new(),
            connection_policy: annotagent_core::ProviderConnectionPolicy {
                maximum_retries: 0,
                ..annotagent_core::ProviderConnectionPolicy::default()
            },
            enabled: true,
            health: annotagent_core::ProviderHealthSnapshot {
                status: ProviderHealthStatus::Configured,
                safe_message: Some("Explicit smoke configuration".to_owned()),
                checked_at: None,
            },
            created_at: now,
            updated_at: now,
        };
        application
            .store
            .save_provider_profile(&provider_profile)
            .expect("Provider Profile");
        let model_profile = ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id,
            display_name: "Explicit live smoke model".to_owned(),
            remote_model_id,
            input_modalities: BTreeSet::from([InputModality::Text]),
            protocol_features: annotagent_core::ProtocolFeatures {
                tool_calls: true,
                structured_output: true,
                json_schema: true,
                usage_reporting: true,
                ..annotagent_core::ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([ModelCapability::TextGeneration]),
            capability_source: annotagent_core::CapabilityDeclarationSource::UserDeclared,
            limits: annotagent_core::ModelLimits {
                context_tokens: Some(32_768),
                maximum_output_tokens: Some(2_048),
                ..annotagent_core::ModelLimits::default()
            },
            generation_defaults: annotagent_core::GenerationDefaults::default(),
            pricing: annotagent_core::ModelPricing::default(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: true,
            created_at: now,
            updated_at: now,
        };
        application
            .store
            .save_model_profile(&model_profile)
            .expect("Model Profile");
        let selected = PipelineBuilderModelRuntime {
            provider: provider_profile,
            model: model_profile,
            binding_source: ModelBindingSource::WorkflowNode,
            locked: true,
        };
        let provider = OpenAiCompatibleProvider::new_with_api_key(
            selected
                .openai_compatible_config()
                .expect("Provider config"),
            Some(api_key.clone()),
        )
        .expect("Provider");
        let report = application
            .run_workflow_advisor_with_selected_model(
                "provider-smoke",
                &load_settings(None).expect("settings"),
                &selected,
                &provider,
                &WorkflowConstraints::default(),
                Some(("scene", "day")),
                PipelineBuilderConstraints {
                    maximum_agent_turns: 20,
                    maximum_tool_calls: 20,
                    maximum_dry_runs: 2,
                    ..PipelineBuilderConstraints::default()
                },
                CancellationToken::new(),
            )
            .await
            .expect("live Pipeline Builder smoke");
        assert_eq!(report.session.status, AgentSessionStatus::WaitingForHuman);
        assert!(!report.session.model_calls.is_empty());
        assert!(
            !serde_json::to_string(&report.session)
                .expect("Agent Session")
                .contains(&api_key)
        );
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
        assert!(summary.default_workflow_version.is_none());
        assert!(
            summary
                .blocking_issues
                .iter()
                .any(|issue| issue.code == "no_published_pipeline" && issue.next_step == "pipeline")
        );
        assert_eq!(summary.enabled_skills.len(), 1);
        assert_eq!(summary.enabled_skills[0].id, "robocup");
        assert_eq!(summary.enabled_skills[0].display_name, "RoboCup Ball");
        assert_eq!(
            summary.active_workflow.name,
            "Unpublished Project task graph"
        );
        assert_eq!(summary.active_workflow.status, WorkflowStatus::Draft);
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

    #[tokio::test]
    async fn pipeline_builder_budget_stop_is_persisted_for_refresh_recovery() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "builder-budget",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("RoboCup Ball Project");
        generate_synthetic_robocup(&temporary.path().join("builder-budget/images/synthetic.png"))
            .expect("synthetic image");
        let report = application
            .run_workflow_advisor_agent(
                "builder-budget",
                &load_settings(None).expect("settings"),
                &WorkflowConstraints::default(),
                Some(("objects", "ball")),
                PipelineBuilderConstraints {
                    maximum_tool_calls: 1,
                    ..PipelineBuilderConstraints::default()
                },
                CancellationToken::new(),
            )
            .await
            .expect("bounded Pipeline Builder");

        assert_eq!(report.session.status, AgentSessionStatus::BudgetExceeded);
        let persisted = application
            .list_agent_sessions("builder-budget")
            .expect("persisted sessions");
        assert_eq!(persisted[0].id, report.session.id);
        assert_eq!(persisted[0].status, AgentSessionStatus::BudgetExceeded);
        assert_eq!(
            persisted[0].stop_reason.as_deref(),
            Some("step or tool-call budget exhausted")
        );
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
        let message = error.to_string();
        assert!(
            message.contains("not ready")
                || message.contains("unavailable")
                || message.contains("not executable"),
            "unexpected unready-model error: {message}"
        );

        let input = application
            .workflow_advisor_input_for_label(
                "robocup-model-choice",
                &settings,
                WorkflowConstraints::default(),
                Some("objects"),
                Some("ball"),
            )
            .expect("Advisor input");
        let tools = pipeline_builder_live_tools(&input);
        let resource_tool = tools
            .iter()
            .find(|tool| tool.name == "load_skill_resource")
            .expect("Skill resource tool");
        assert_eq!(
            resource_tool.parameters["properties"]["skill_id"]["enum"],
            json!(["robocup"])
        );
        assert!(
            resource_tool.parameters["properties"]["resource_name"]["enum"]
                .as_array()
                .is_some_and(|resources| resources.contains(&json!("resources/advisor.md")))
        );
        assert!(resource_tool.description.contains("resources/advisor.md"));
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
    async fn legacy_registry_import_freezes_new_publication_and_blocks_disabled_provider_runs() {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let application = LocalApplication::new(temporary.path()).expect("application");
        application
            .create_project(
                "registry-migration",
                include_str!("../../../examples/robocup/project.yaml"),
            )
            .expect("project");
        let project_path = application
            .project_path("registry-migration")
            .expect("project path");
        generate_synthetic_robocup(
            &project_path
                .parent()
                .expect("project root")
                .join("images/sample.png"),
        )
        .expect("sample image");
        let historical_run = RunId::new();
        application
            .store
            .create_run(&annotagent_runtime::RunRecord {
                id: historical_run,
                project_id: stable_project_id(project_path.parent().expect("project root")),
                project_name: "Historical compatibility Run".to_owned(),
                skill_id: "robocup".to_owned(),
                provider: "mock".to_owned(),
                model: "legacy-model".to_owned(),
                status: RunStatus::Completed,
                project_schema_json: std::fs::read_to_string(&project_path).expect("schema"),
                workflow_snapshot_json: Some("{\"legacy\":true}".to_owned()),
            })
            .await
            .expect("historical Run");
        let history_before =
            serde_json::to_value(application.list_runs().expect("history")).expect("history JSON");

        let settings = load_settings(None).expect("settings");
        let preview = application
            .preview_legacy_registry_import(&settings)
            .expect("preview");
        assert!(!preview.already_applied);
        assert_eq!(preview.project_binding_count, 1);
        assert!(!preview.moves_secret);
        assert!(!preview.modifies_historical_runs);
        let imported = application
            .apply_legacy_registry_import(&settings)
            .expect("import");
        assert_eq!(imported.historical_runs_modified, 0);
        assert_eq!(imported.bindings_created, 1);
        assert!(
            application
                .apply_legacy_registry_import(&settings)
                .expect("idempotent import")
                .already_applied
        );
        assert_eq!(
            serde_json::to_value(application.list_runs().expect("unchanged history"))
                .expect("history JSON"),
            history_before
        );

        let suggestion = application
            .suggest_workflow(
                "registry-migration",
                &settings,
                &WorkflowConstraints::default(),
            )
            .expect("Workflow suggestion");
        application
            .dry_run_workflow(&suggestion.draft.id, &settings)
            .expect("Dry Run validation");
        let published = application
            .publish_workflow(&suggestion.draft.id, &settings)
            .expect("published Workflow");
        assert_eq!(published.snapshot.model_profiles.len(), 1);
        let frozen = &published.snapshot.model_profiles[0];
        assert_eq!(frozen.model_profile_id, imported.model_profile_id);
        assert!(published.draft.nodes.iter().any(|node| {
            node.model_profile_binding
                .as_ref()
                .is_some_and(|binding| binding.model_profile_id == imported.model_profile_id)
        }));

        let mut provider = application
            .store
            .get_provider_profile(imported.provider_id)
            .expect("Provider");
        provider.enabled = false;
        provider.health = ProviderHealthSnapshot {
            status: ProviderHealthStatus::Disabled,
            safe_message: Some("Disabled by test.".to_owned()),
            checked_at: Some(chrono::Utc::now()),
        };
        provider.updated_at = chrono::Utc::now();
        application
            .store
            .save_provider_profile(&provider)
            .expect("disable Provider");
        let error = application
            .start_run_path_with_settings_idempotent_workflow(
                &project_path,
                "mock",
                settings,
                None,
                None,
                Some((&published.workflow_id, published.version)),
            )
            .expect_err("disabled Provider blocks a new Run");
        assert!(error.to_string().contains("new Run blocked: Provider"));
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
