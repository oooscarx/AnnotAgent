//! Object-safe extension contracts used by the runtime.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    Annotation, AnnotationId, AnnotationSnapshot, CoreResult, CorrectionKind, ImageFrame, ImageId,
    LabelId, ProjectId, ProjectSchema, RunId, SkillManifest, SkillResource, SkillResourceRequest,
    TaskGraph, TaskId, TaskTemplate, TokenUsage, ToolCallId, VisionArtifact,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub read_only: bool,
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub project_root: PathBuf,
    pub run_id: RunId,
    pub image_id: Option<ImageId>,
    pub image: Option<Arc<ImageFrame>>,
    pub task_id: Option<TaskId>,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    /// Complete auditable result. It may contain fields that are not appropriate for a model.
    #[serde(default)]
    pub persisted_result: serde_json::Value,
    /// Bounded structured result sent back to the model, or an artifact reference.
    #[serde(default, alias = "data")]
    pub model_result: serde_json::Value,
    /// Short display text for traces and operator-facing UI.
    #[serde(alias = "summary")]
    pub ui_summary: String,
    /// Typed outputs created by this node. Model-facing messages contain references to these.
    #[serde(default)]
    pub artifacts: Vec<VisionArtifact>,
}

impl ToolResult {
    #[must_use]
    pub fn structured(summary: impl Into<String>, result: serde_json::Value) -> Self {
        Self {
            persisted_result: result.clone(),
            model_result: result,
            ui_summary: summary.into(),
            artifacts: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_artifacts(
        summary: impl Into<String>,
        artifacts: Vec<VisionArtifact>,
        metadata: &serde_json::Value,
    ) -> Self {
        let references = artifacts
            .iter()
            .map(VisionArtifact::reference)
            .collect::<Vec<_>>();
        Self {
            persisted_result: serde_json::json!({
                "artifacts": artifacts,
                "metadata": metadata,
            }),
            model_result: serde_json::json!({
                "artifact_references": references,
                "metadata": metadata,
            }),
            ui_summary: summary.into(),
            artifacts,
        }
    }
}

#[async_trait]
pub trait AgentTool: Send + Sync {
    fn definition(&self) -> ToolDefinition;

    /// Empty means the tool is generic; otherwise it is only exposed for these task ids.
    fn applicable_tasks(&self) -> Vec<TaskId> {
        Vec::new()
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: serde_json::Value,
    ) -> CoreResult<ToolResult>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedAction {
    Accept,
    Retry,
    Refine,
    Remove,
    HumanReview,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValidationEvidence {
    Geometry {
        metric: String,
        value: f64,
        threshold: f64,
    },
    ImageStatistics {
        region: String,
        measurements: BTreeMap<String, f64>,
    },
    Rule {
        facts: BTreeMap<String, String>,
    },
    MissingDependency {
        task_id: TaskId,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub code: String,
    pub severity: IssueSeverity,
    pub annotation_ids: Vec<AnnotationId>,
    pub message: String,
    pub suggested_action: SuggestedAction,
    pub evidence: ValidationEvidence,
}

pub struct ValidationContext<'a> {
    pub project: &'a ProjectSchema,
    pub image: Option<&'a ImageFrame>,
    pub candidate: &'a Annotation,
    pub related_annotations: &'a [Annotation],
    pub correction_risk: f32,
}

pub trait AnnotationValidator: Send + Sync {
    fn id(&self) -> &str;
    fn validate(&self, context: &ValidationContext<'_>) -> CoreResult<Vec<ValidationIssue>>;
}

pub struct RefinementContext<'a> {
    pub project: &'a ProjectSchema,
    pub image: &'a ImageFrame,
    pub candidate: &'a Annotation,
    pub related_annotations: &'a [Annotation],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefinementResult {
    pub annotation: Annotation,
    pub confidence: f32,
    pub issues: Vec<ValidationIssue>,
    pub summary: String,
}

pub trait AnnotationRefiner: Send + Sync {
    fn id(&self) -> &str;
    fn refine(&self, context: &RefinementContext<'_>) -> CoreResult<RefinementResult>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ReviewDecision {
    AutoAccept { reasons: Vec<String> },
    Retry { reasons: Vec<String> },
    HumanReview { reasons: Vec<String> },
    Reject { reasons: Vec<String> },
}

pub struct ReviewContext<'a> {
    pub annotation: &'a Annotation,
    pub issues: &'a [ValidationIssue],
    pub refiner_confidence: Option<f32>,
    pub correction_risk: f32,
    pub evidence_conflict: bool,
    pub retry_count: u32,
    pub max_retries: u32,
}

pub trait ReviewPolicy: Send + Sync {
    fn decide(&self, context: &ReviewContext<'_>) -> ReviewDecision;
}

pub trait DomainSkill: Send + Sync {
    fn id(&self) -> &str;
    fn manifest(&self) -> &SkillManifest;
    fn task_templates(&self) -> Vec<TaskTemplate>;
    fn workflow(&self) -> TaskGraph;
    fn tool_factories(&self) -> Vec<Arc<dyn AgentTool>>;
    fn validators(&self) -> Vec<Arc<dyn AnnotationValidator>>;
    fn refiners(&self) -> Vec<Arc<dyn AnnotationRefiner>>;
    fn prompt_resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>>;
    fn correction_taxonomy(&self) -> Vec<CorrectionKind>;
    fn review_policy(&self) -> Arc<dyn ReviewPolicy>;

    /// Optional typed workflow starters owned by this domain extension.
    fn workflow_templates(&self) -> Vec<crate::WorkflowTemplate> {
        Vec::new()
    }

    /// Optional starter project supplied by the domain extension, never by Core or the GUI.
    fn project_template(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub vision: bool,
    pub tool_calls: bool,
    pub json_schema: bool,
    pub usage_reporting: bool,
    pub multi_image: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: ModelRole,
    pub content: String,
    pub tool_call_id: Option<ToolCallId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ModelToolCall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelImage {
    pub id: String,
    pub mime_type: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: String,
    pub task_id: TaskId,
    pub messages: Vec<ModelMessage>,
    pub images: Vec<ModelImage>,
    pub tools: Vec<ToolDefinition>,
    pub max_output_tokens: u32,
    pub temperature: f32,
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelToolCall {
    pub id: ToolCallId,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ModelToolCall>,
    pub usage: TokenUsage,
    pub request_id: Option<String>,
    pub provider_metadata: BTreeMap<String, String>,
}

#[async_trait]
pub trait VisionModelProvider: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> ModelCapabilities;
    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<ModelResponse>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectionFeatures {
    pub geometry: BTreeMap<String, f64>,
    pub colors: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectionRecord {
    pub id: Uuid,
    pub project_id: ProjectId,
    pub skill_id: String,
    pub task_id: TaskId,
    pub predicted_label: Option<LabelId>,
    pub corrected_label: Option<LabelId>,
    pub reason_code: String,
    pub original_annotation: Option<AnnotationSnapshot>,
    pub corrected_annotation: Option<AnnotationSnapshot>,
    pub note: Option<String>,
    pub image_features: CorrectionFeatures,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub schema: ProjectSchema,
    pub images: Vec<SnapshotImage>,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotImage {
    pub id: ImageId,
    pub relative_path: PathBuf,
    pub metadata: crate::ImageMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportCompatibility {
    pub supported: bool,
    pub unsupported_task_kinds: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub project: ProjectSnapshot,
    pub output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportReport {
    pub exported_count: u64,
    pub skipped_count: u64,
    pub warnings: Vec<String>,
    pub unsupported_task_kinds: Vec<String>,
    pub output_files: Vec<PathBuf>,
}

#[async_trait]
pub trait DatasetExporter: Send + Sync {
    fn format_id(&self) -> &str;
    fn compatibility(&self, project: &ProjectSnapshot) -> ExportCompatibility;
    async fn export(&self, request: ExportRequest) -> CoreResult<ExportReport>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCallTiming {
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration: Duration,
}
