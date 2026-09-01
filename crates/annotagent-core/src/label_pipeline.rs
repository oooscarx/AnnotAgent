//! Domain-neutral label pipeline authoring and intermediate Artifact contracts.
//!
//! A label workflow is authored as shared stages plus one pipeline per target label, then
//! compiled into the ordinary flat Workflow DAG. The flat graph is the only runtime graph: a
//! shared model step therefore has one node identity and executes once per image even when many
//! label pipelines consume its output.

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{
    ArtifactKind, ArtifactValidationState, CoreResult, FallbackPolicy, ImageId, LabelId,
    MaskEncoding, ModelId, ModelImage, ModelRegistry, NodePort, NodeRegistry, NormalizedPoint,
    NormalizedRect, ProjectSchema, ResourceRequirements, RetryPolicy, ReviewGate, RunId,
    ScoreSemantics, TaskId, ValidationIssue, VisionArtifactValue, VisionBackendError,
    VisionBackendTimings, VisionBackendUsage, VisionCapability, WORKFLOW_SCHEMA_VERSION,
    WorkflowDraft, WorkflowDraftNode, WorkflowDraftStatus, WorkflowEdge, WorkflowNodeKind,
};

pub const LABEL_PIPELINE_SCHEMA_VERSION: u32 = 1;
pub const PIPELINE_VISION_PROTOCOL_VERSION: u32 = crate::VISION_WORKER_PROTOCOL_VERSION;
pub const DETECTION_ARTIFACT_SCHEMA_VERSION: u32 = 2;
pub const IMAGE_INPUT_NODE_ID: &str = "core.image_input";
pub const IMAGE_INPUT_OPERATION: &str = "core.image_input";

/// A concrete Artifact or item within a set Artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub artifact_id: String,
    pub source_node: String,
    pub port: String,
    pub artifact_type: ArtifactKind,
    /// Set item identity. Crop and classification results must retain this subject identity.
    #[serde(default)]
    pub item_id: Option<String>,
}

impl ArtifactRef {
    #[must_use]
    pub fn item(&self, item_id: impl Into<String>) -> Self {
        let mut reference = self.clone();
        reference.item_id = Some(item_id.into());
        reference
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageArtifact {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub width: u32,
    pub height: u32,
    pub mime_type: String,
    /// Workspace/cache reference; raw image bytes are not serialized into the node trace.
    pub blob_ref: String,
    /// Parent image/tile when this is a virtual resize or tile. The item identity prevents
    /// fan-out results from being correlated by array position.
    #[serde(default)]
    pub parent: Option<ArtifactRef>,
    /// Region occupied by this image in normalized root-image coordinates. `None` is identity.
    #[serde(default)]
    pub root_region: Option<NormalizedRect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DetectionScore {
    pub value: Option<f32>,
    #[serde(default)]
    pub semantics: ScoreSemantics,
}

impl Default for DetectionScore {
    fn default() -> Self {
        Self::not_provided()
    }
}

impl DetectionScore {
    pub fn new(value: Option<f32>, semantics: ScoreSemantics) -> Result<Self, String> {
        let score = Self { value, semantics };
        score.validate()?;
        Ok(score)
    }

    pub fn relative(value: f32) -> Result<Self, String> {
        Self::new(Some(value), ScoreSemantics::RelativeConfidence)
    }

    #[must_use]
    pub const fn not_provided() -> Self {
        Self {
            value: None,
            semantics: ScoreSemantics::NotProvided,
        }
    }

    /// Returns a value only when the score can be used by a conventional confidence gate.
    /// Ranking-only, unknown, and absent scores require evidence-aware handling instead.
    #[must_use]
    pub const fn comparable_confidence(self) -> Option<f32> {
        match self.semantics {
            ScoreSemantics::CalibratedProbability | ScoreSemantics::RelativeConfidence => {
                self.value
            }
            ScoreSemantics::RankingScore
            | ScoreSemantics::NotProvided
            | ScoreSemantics::Unknown => None,
        }
    }

    pub fn validate(self) -> Result<(), String> {
        if let Some(value) = self.value {
            validate_confidence(value)?;
            if self.semantics == ScoreSemantics::NotProvided {
                return Err("score marked not_provided cannot contain a value".to_owned());
            }
        } else if !matches!(
            self.semantics,
            ScoreSemantics::NotProvided | ScoreSemantics::Unknown
        ) {
            return Err("a scored semantics requires a numeric value".to_owned());
        }
        Ok(())
    }
}

fn deserialize_detection_score<'de, D>(deserializer: D) -> Result<DetectionScore, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum WireScore {
        Structured(DetectionScore),
        Legacy(Option<f32>),
    }
    let score = match WireScore::deserialize(deserializer)? {
        WireScore::Structured(score) => score,
        WireScore::Legacy(Some(value)) => DetectionScore {
            value: Some(value),
            semantics: ScoreSemantics::Unknown,
        },
        WireScore::Legacy(None) => DetectionScore::not_provided(),
    };
    score.validate().map_err(serde::de::Error::custom)?;
    Ok(score)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPayloadRef {
    pub id: String,
    pub media_type: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionEvidence {
    pub source_model_id: ModelId,
    pub source_artifact_id: String,
    pub bbox: NormalizedRect,
    pub score: DetectionScore,
    pub query_id: Option<String>,
    pub model_label: Option<String>,
    #[serde(default)]
    pub project_label: Option<LabelId>,
    #[serde(default = "default_object_detection_capability")]
    pub source_capability: VisionCapability,
    pub raw_output_ref: Option<StoredPayloadRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionSource {
    pub model_id: ModelId,
    pub capability: VisionCapability,
    pub artifact_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionArtifactItem {
    #[serde(alias = "id")]
    pub detection_id: DetectionId,
    #[serde(default)]
    pub query_id: Option<String>,
    #[serde(default, alias = "class_id")]
    pub model_label: Option<String>,
    #[serde(default, alias = "label")]
    pub project_label: Option<LabelId>,
    #[serde(alias = "rect")]
    pub bbox: NormalizedRect,
    #[serde(
        default,
        alias = "confidence",
        deserialize_with = "deserialize_detection_score"
    )]
    pub score: DetectionScore,
    #[serde(default)]
    pub source_model_id: ModelId,
    #[serde(default = "default_object_detection_capability")]
    pub source_capability: VisionCapability,
    #[serde(default)]
    pub geometry_semantics: crate::GeometrySemantics,
    #[serde(default)]
    pub evidence: Vec<DetectionEvidence>,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

/// Detection IDs are stable within one `DetectionSet` and intentionally opaque to Core.
pub type DetectionId = String;

impl DetectionArtifactItem {
    pub fn from_source(
        detection_id: impl Into<String>,
        query_id: Option<String>,
        model_label: Option<String>,
        project_label: Option<LabelId>,
        bbox: NormalizedRect,
        score: DetectionScore,
        source: DetectionSource,
    ) -> Result<Self, String> {
        score.validate()?;
        let evidence = DetectionEvidence {
            source_model_id: source.model_id.clone(),
            source_artifact_id: source.artifact_id,
            bbox,
            score,
            query_id: query_id.clone(),
            model_label: model_label.clone(),
            project_label: project_label.clone(),
            source_capability: source.capability,
            raw_output_ref: None,
        };
        let geometry_semantics = crate::default_geometry_semantics(&[source.capability]);
        Ok(Self {
            detection_id: detection_id.into(),
            query_id,
            model_label,
            project_label,
            bbox,
            score,
            source_model_id: source.model_id,
            source_capability: source.capability,
            geometry_semantics,
            evidence: vec![evidence],
            attributes: BTreeMap::new(),
        })
    }
}

/// Compatibility name retained while callers migrate to `DetectionArtifactItem`.
pub type Detection = DetectionArtifactItem;

const fn default_object_detection_capability() -> VisionCapability {
    VisionCapability::ObjectDetection
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetectionSetArtifact {
    pub schema_version: u32,
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub model_binding: String,
    #[serde(default = "default_unvalidated")]
    pub validation_state: ArtifactValidationState,
    pub detections: Vec<Detection>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// A detector result converted into an explicit prompt for a prompted-segmentation model.
/// `subject` always identifies the exact source Detection item, so conversion never relies on
/// array order or label matching.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxPrompt {
    pub id: String,
    pub subject: ArtifactRef,
    pub bbox: NormalizedRect,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxPromptSetArtifact {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub source_detections: ArtifactRef,
    pub prompts: Vec<BoxPrompt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PromptPoint {
    pub point: NormalizedPoint,
    pub positive: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointPrompt {
    pub id: String,
    /// The annotation, keypoint, detection, or image region from which this prompt was derived.
    pub subject: ArtifactRef,
    pub points: Vec<PromptPoint>,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointPromptSetArtifact {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub source_artifact: ArtifactRef,
    pub prompts: Vec<PointPrompt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaskArtifactItem {
    pub mask_id: String,
    /// Exact `BoxPromptSet` or `PointPromptSet` item consumed by the segmenter.
    pub prompt: ArtifactRef,
    pub mask: MaskEncoding,
    #[serde(default)]
    pub score: DetectionScore,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaskSetArtifact {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub model_binding: String,
    pub source_prompts: ArtifactRef,
    #[serde(default = "default_unvalidated")]
    pub validation_state: ArtifactValidationState,
    pub masks: Vec<MaskArtifactItem>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolygonArtifactItem {
    pub polygon_id: String,
    pub parent: ArtifactRef,
    pub rings: Vec<Vec<NormalizedPoint>>,
    #[serde(default)]
    pub score: DetectionScore,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolygonSetArtifact {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub source_masks: ArtifactRef,
    pub polygons: Vec<PolygonArtifactItem>,
}

impl<'de> Deserialize<'de> for DetectionSetArtifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireDetectionSet {
            #[serde(default = "legacy_detection_artifact_schema_version")]
            schema_version: u32,
            reference: ArtifactRef,
            image_id: ImageId,
            model_binding: String,
            #[serde(default = "default_unvalidated")]
            validation_state: ArtifactValidationState,
            detections: Vec<DetectionArtifactItem>,
            #[serde(default)]
            metadata: BTreeMap<String, serde_json::Value>,
        }

        let wire = WireDetectionSet::deserialize(deserializer)?;
        if wire.schema_version == 0 || wire.schema_version > DETECTION_ARTIFACT_SCHEMA_VERSION {
            return Err(serde::de::Error::custom(format!(
                "unsupported Detection Artifact schema version {}",
                wire.schema_version
            )));
        }
        let mut detections = wire.detections;
        for detection in &mut detections {
            if detection.source_model_id.trim().is_empty() {
                detection.source_model_id.clone_from(&wire.model_binding);
            }
            if detection.evidence.is_empty() {
                detection.evidence.push(DetectionEvidence {
                    source_model_id: detection.source_model_id.clone(),
                    source_artifact_id: wire.reference.artifact_id.clone(),
                    bbox: detection.bbox,
                    score: detection.score,
                    query_id: detection.query_id.clone(),
                    model_label: detection.model_label.clone(),
                    project_label: detection.project_label.clone(),
                    source_capability: detection.source_capability,
                    raw_output_ref: None,
                });
            }
            if detection.geometry_semantics == crate::GeometrySemantics::NotApplicable {
                detection.geometry_semantics =
                    crate::default_geometry_semantics(&[detection.source_capability]);
            }
        }
        Ok(Self {
            schema_version: DETECTION_ARTIFACT_SCHEMA_VERSION,
            reference: wire.reference,
            image_id: wire.image_id,
            model_binding: wire.model_binding,
            validation_state: wire.validation_state,
            detections,
            metadata: wire.metadata,
        })
    }
}

const fn legacy_detection_artifact_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateAgreement {
    SingleSource,
    MultiSourceAgreement { minimum_iou: f32, mean_iou: f32 },
    GeometryConflict,
    LabelConflict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateCluster {
    pub id: CandidateClusterId,
    pub target_label: LabelId,
    pub representative_bbox: NormalizedRect,
    pub members: Vec<DetectionEvidence>,
    pub agreement: CandidateAgreement,
}

/// Candidate cluster IDs are stable within one `CandidateClusterSet`.
pub type CandidateClusterId = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateClusterSetArtifact {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub source_detection_sets: Vec<ArtifactRef>,
    #[serde(default = "default_unvalidated")]
    pub validation_state: ArtifactValidationState,
    pub candidates: Vec<CandidateCluster>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectionRisk {
    pub score: f32,
    #[serde(default)]
    pub reasons: Vec<String>,
}

impl CorrectionRisk {
    pub fn validate(&self) -> Result<(), String> {
        validate_confidence(self.score)?;
        if self.reasons.iter().any(|reason| reason.trim().is_empty()) {
            return Err("CorrectionRisk reasons cannot be empty".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceGateInput {
    pub candidates: Vec<CandidateCluster>,
    #[serde(default)]
    pub validation_issues: Vec<ValidationIssue>,
    pub correction_risk: Option<CorrectionRisk>,
}

impl EvidenceGateInput {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(risk) = &self.correction_risk {
            risk.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGateDecision {
    Accept,
    Fallback,
    Review,
    Reject,
}

impl EvidenceGateDecision {
    #[must_use]
    pub const fn route(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Fallback => "fallback",
            Self::Review => "review",
            Self::Reject => "reject",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceGateReason {
    pub code: String,
    pub message: String,
    pub candidate_id: Option<CandidateClusterId>,
    #[serde(default)]
    pub source_model_ids: Vec<ModelId>,
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceGateReport {
    pub decision: EvidenceGateDecision,
    pub reasons: Vec<EvidenceGateReason>,
    pub candidate_count: usize,
    pub validation_issue_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EvidenceGateConfig {
    pub accept_when: Vec<EvidenceAcceptRule>,
    pub fallback_when: Vec<EvidenceFallbackRule>,
    pub review_when: Vec<EvidenceReviewRule>,
    pub reject_when: Vec<EvidenceRejectRule>,
}

impl EvidenceGateConfig {
    pub fn validate(&self) -> Result<(), String> {
        for threshold in self
            .accept_when
            .iter()
            .flat_map(|rule| [rule.minimum_iou, rule.minimum_score])
            .chain(
                self.fallback_when
                    .iter()
                    .flat_map(|rule| [rule.specialist_score_below, rule.correction_risk_above]),
            )
            .chain(
                self.review_when
                    .iter()
                    .map(|rule| rule.correction_risk_above),
            )
            .flatten()
        {
            validate_confidence(threshold)?;
        }
        if self
            .accept_when
            .iter()
            .any(|rule| rule.minimum_sources == Some(0))
        {
            return Err("minimum_sources must be greater than zero".to_owned());
        }
        for source in self
            .accept_when
            .iter()
            .filter_map(|rule| rule.source.as_deref())
            .chain(
                self.fallback_when
                    .iter()
                    .filter_map(|rule| rule.source.as_deref()),
            )
        {
            if source.trim().is_empty() {
                return Err("evidence source cannot be empty".to_owned());
            }
        }
        if self
            .reject_when
            .iter()
            .flat_map(|rule| &rule.domain_issue_codes)
            .any(|code| code.trim().is_empty())
        {
            return Err("domain issue code cannot be empty".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EvidenceAcceptRule {
    pub minimum_sources: Option<usize>,
    pub minimum_iou: Option<f32>,
    /// Exact Model Registry id. Core does not assign semantic meaning to model brands.
    pub source: Option<ModelId>,
    pub minimum_score: Option<f32>,
    pub no_domain_issue: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EvidenceFallbackRule {
    /// Exact source Model Registry id; omitted means the first declared source.
    pub source: Option<ModelId>,
    pub empty_specialist_result: bool,
    pub specialist_score_below: Option<f32>,
    pub domain_issue: bool,
    pub correction_risk_above: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EvidenceReviewRule {
    pub geometry_conflict: bool,
    pub label_conflict: bool,
    pub open_vocab_only: bool,
    pub score_missing: bool,
    pub empty_result: bool,
    pub correction_risk_above: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EvidenceRejectRule {
    pub empty_result: bool,
    pub domain_issue_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    pub id: String,
    /// The exact `DetectionSet` item that produced this crop.
    pub parent: ArtifactRef,
    pub rect: NormalizedRect,
    #[serde(default)]
    pub source_width: u32,
    #[serde(default)]
    pub source_height: u32,
    #[serde(default)]
    pub crop_width: u32,
    #[serde(default)]
    pub crop_height: u32,
    #[serde(default)]
    pub padding: f32,
    pub mime_type: Option<String>,
    /// A cache/blob reference; image bytes are deliberately not embedded in traces.
    pub blob_ref: Option<String>,
    #[serde(default)]
    pub cache_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropSetArtifact {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub source_detections: ArtifactRef,
    pub crops: Vec<Crop>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classification {
    pub id: String,
    /// Whole-image classification references the Image Artifact; crop classification references
    /// a `CropSet` item. The parent Detection is reachable through that Crop item.
    pub subject: ArtifactRef,
    /// Crop classification keeps the originating Detection item here so downstream fan-in never
    /// relies on result ordering or label coincidence.
    #[serde(default)]
    pub parent: Option<ArtifactRef>,
    pub label: LabelId,
    pub confidence: f32,
    #[serde(default)]
    pub scores: BTreeMap<LabelId, f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationSetArtifact {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub model_binding: String,
    #[serde(default = "default_unvalidated")]
    pub validation_state: ArtifactValidationState,
    pub classifications: Vec<Classification>,
}

const fn default_unvalidated() -> ArtifactValidationState {
    ArtifactValidationState::Unvalidated
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationCandidate {
    pub id: String,
    pub task_id: TaskId,
    pub label: LabelId,
    /// Detection, crop, or whole-image subject that this candidate annotates.
    pub subject: ArtifactRef,
    /// Optional geometric value, normally copied from the parent Detection.
    pub value: Option<VisionArtifactValue>,
    pub confidence: Option<f32>,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub evidence: Vec<ArtifactRef>,
    #[serde(default)]
    pub validation_state: Option<ArtifactValidationState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationCandidateSet {
    pub reference: ArtifactRef,
    pub image_id: ImageId,
    pub candidates: Vec<AnnotationCandidate>,
}

/// Runtime intermediate values carried beside annotation-shaped `VisionArtifact`s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "artifact", rename_all = "snake_case")]
pub enum PipelineArtifact {
    Image(ImageArtifact),
    DetectionSet(DetectionSetArtifact),
    BoxPromptSet(BoxPromptSetArtifact),
    PointPromptSet(PointPromptSetArtifact),
    MaskSet(MaskSetArtifact),
    PolygonSet(PolygonSetArtifact),
    CandidateClusterSet(CandidateClusterSetArtifact),
    CropSet(CropSetArtifact),
    ClassificationSet(ClassificationSetArtifact),
    AnnotationCandidateSet(AnnotationCandidateSet),
}

impl PipelineArtifact {
    #[must_use]
    pub const fn artifact_type(&self) -> ArtifactKind {
        match self {
            Self::Image(_) => ArtifactKind::Image,
            Self::DetectionSet(_) => ArtifactKind::DetectionSet,
            Self::BoxPromptSet(_) => ArtifactKind::BoxPromptSet,
            Self::PointPromptSet(_) => ArtifactKind::PointPromptSet,
            Self::MaskSet(_) => ArtifactKind::MaskSet,
            Self::PolygonSet(_) => ArtifactKind::PolygonSet,
            Self::CandidateClusterSet(_) => ArtifactKind::CandidateClusterSet,
            Self::CropSet(_) => ArtifactKind::CropSet,
            Self::ClassificationSet(_) => ArtifactKind::ClassificationSet,
            Self::AnnotationCandidateSet(_) => ArtifactKind::AnnotationCandidateSet,
        }
    }

    #[must_use]
    pub const fn reference(&self) -> &ArtifactRef {
        match self {
            Self::Image(artifact) => &artifact.reference,
            Self::DetectionSet(artifact) => &artifact.reference,
            Self::BoxPromptSet(artifact) => &artifact.reference,
            Self::PointPromptSet(artifact) => &artifact.reference,
            Self::MaskSet(artifact) => &artifact.reference,
            Self::PolygonSet(artifact) => &artifact.reference,
            Self::CandidateClusterSet(artifact) => &artifact.reference,
            Self::CropSet(artifact) => &artifact.reference,
            Self::ClassificationSet(artifact) => &artifact.reference,
            Self::AnnotationCandidateSet(artifact) => &artifact.reference,
        }
    }

    #[must_use]
    pub const fn image_id(&self) -> ImageId {
        match self {
            Self::Image(artifact) => artifact.image_id,
            Self::DetectionSet(artifact) => artifact.image_id,
            Self::BoxPromptSet(artifact) => artifact.image_id,
            Self::PointPromptSet(artifact) => artifact.image_id,
            Self::MaskSet(artifact) => artifact.image_id,
            Self::PolygonSet(artifact) => artifact.image_id,
            Self::CandidateClusterSet(artifact) => artifact.image_id,
            Self::CropSet(artifact) => artifact.image_id,
            Self::ClassificationSet(artifact) => artifact.image_id,
            Self::AnnotationCandidateSet(artifact) => artifact.image_id,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Image(artifact) => artifact.validate(),
            Self::DetectionSet(artifact) => artifact.validate(),
            Self::BoxPromptSet(artifact) => artifact.validate(),
            Self::PointPromptSet(artifact) => artifact.validate(),
            Self::MaskSet(artifact) => artifact.validate(),
            Self::PolygonSet(artifact) => artifact.validate(),
            Self::CandidateClusterSet(artifact) => artifact.validate(),
            Self::CropSet(artifact) => artifact.validate(),
            Self::ClassificationSet(artifact) => artifact.validate(),
            Self::AnnotationCandidateSet(artifact) => artifact.validate(),
        }
    }
}

/// Versioned wire contract used by generic HTTP JSON classifiers and detectors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineInferenceRequest {
    #[serde(default = "default_pipeline_vision_protocol_version")]
    pub protocol_version: u32,
    pub request_id: String,
    pub run_id: RunId,
    pub image_id: ImageId,
    pub node_id: String,
    pub model_id: String,
    pub operation: VisionCapability,
    pub image: Option<ModelImage>,
    #[serde(default)]
    pub input_artifacts: Vec<PipelineArtifact>,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    pub timeout_ms: Option<u64>,
}

const fn default_pipeline_vision_protocol_version() -> u32 {
    PIPELINE_VISION_PROTOCOL_VERSION
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineInferenceResponse {
    #[serde(default = "default_pipeline_vision_protocol_version")]
    pub protocol_version: u32,
    pub request_id: Option<String>,
    pub model_identity: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<PipelineArtifact>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub usage: VisionBackendUsage,
    #[serde(default)]
    pub timings: VisionBackendTimings,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub error: Option<VisionBackendError>,
}

impl Default for PipelineInferenceResponse {
    fn default() -> Self {
        Self {
            protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: None,
            model_identity: None,
            artifacts: Vec::new(),
            metadata: BTreeMap::new(),
            usage: VisionBackendUsage::default(),
            timings: VisionBackendTimings::default(),
            warnings: Vec::new(),
            error: None,
        }
    }
}

#[async_trait]
pub trait PipelineModelBackend: Send + Sync {
    fn id(&self) -> &str;
    fn capability(&self) -> VisionCapability;

    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse>;
}

/// Registry-bound model selection. Backend kind and endpoint remain owned by the Model Registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelBinding {
    pub model_id: String,
    pub capability: VisionCapability,
    #[serde(default)]
    pub configuration: BTreeMap<String, serde_json::Value>,
}

/// Versioned Skill operation selection. Core treats the ids as opaque registry identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillBinding {
    pub skill_id: String,
    pub version: String,
    pub operation: String,
}

/// An input to a pipeline node. Step ids are globally unique inside the composition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PipelineSource {
    Image,
    SharedStage {
        stage_id: String,
        step_id: String,
        port: String,
        artifact_type: ArtifactKind,
    },
    Step {
        step_id: String,
        port: String,
        artifact_type: ArtifactKind,
    },
}

impl PipelineSource {
    #[must_use]
    pub const fn artifact_type(&self) -> ArtifactKind {
        match self {
            Self::Image => ArtifactKind::Image,
            Self::SharedStage { artifact_type, .. } | Self::Step { artifact_type, .. } => {
                *artifact_type
            }
        }
    }

    fn producer(&self) -> (&str, &str) {
        match self {
            Self::Image => (IMAGE_INPUT_NODE_ID, "image"),
            Self::SharedStage { step_id, port, .. } | Self::Step { step_id, port, .. } => {
                (step_id, port)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStep {
    pub id: String,
    pub node_type: String,
    pub kind: WorkflowNodeKind,
    #[serde(default)]
    pub inputs: BTreeMap<String, PipelineSource>,
    #[serde(default)]
    pub outputs: BTreeMap<String, ArtifactKind>,
    pub model_binding: Option<ModelBinding>,
    pub skill_binding: Option<SkillBinding>,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub validators: Vec<String>,
    #[serde(default)]
    pub refiners: Vec<String>,
    pub fallback: Option<String>,
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    #[serde(default)]
    pub review_gate: ReviewGate,
    #[serde(default)]
    pub resources: ResourceRequirements,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedWorkflowStage {
    pub id: String,
    pub name: String,
    pub steps: Vec<PipelineStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelPipeline {
    pub id: String,
    pub target_task_id: TaskId,
    pub target_label: LabelId,
    pub steps: Vec<PipelineStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelWorkflowComposition {
    #[serde(default = "default_label_pipeline_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub shared_stages: Vec<SharedWorkflowStage>,
    #[serde(default)]
    pub label_pipelines: Vec<LabelPipeline>,
}

const fn default_label_pipeline_schema_version() -> u32 {
    LABEL_PIPELINE_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelPipelineValidationIssue {
    pub code: String,
    pub path: String,
    pub message: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelPipelineValidationReport {
    pub valid: bool,
    pub issues: Vec<LabelPipelineValidationIssue>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LabelPipelineStaticValidator;

impl LabelPipelineStaticValidator {
    #[must_use]
    pub fn validate(
        &self,
        composition: &LabelWorkflowComposition,
        project: &ProjectSchema,
        nodes: &NodeRegistry,
        models: &ModelRegistry,
    ) -> LabelPipelineValidationReport {
        let mut issues = Vec::new();
        if composition.schema_version != LABEL_PIPELINE_SCHEMA_VERSION {
            push_issue(
                &mut issues,
                "unsupported_label_pipeline_schema",
                "schema_version",
                format!("Label Pipeline schema version must be {LABEL_PIPELINE_SCHEMA_VERSION}"),
            );
        }

        let enabled_skills = project.project.enabled_skill_versions();
        let shared_stage_ids = composition
            .shared_stages
            .iter()
            .map(|stage| stage.id.as_str())
            .collect::<BTreeSet<_>>();
        if shared_stage_ids.len() != composition.shared_stages.len() {
            push_issue(
                &mut issues,
                "duplicate_shared_stage_id",
                "shared_stages",
                "shared stage ids must be unique",
            );
        }

        let mut step_paths = BTreeMap::new();
        let mut step_outputs = BTreeMap::<String, BTreeMap<String, ArtifactKind>>::new();
        let mut step_stage = BTreeMap::<String, String>::new();
        for (stage_index, stage) in composition.shared_stages.iter().enumerate() {
            for (step_index, step) in stage.steps.iter().enumerate() {
                let path = format!("shared_stages[{stage_index}].steps[{step_index}]");
                if step_paths.insert(step.id.clone(), path.clone()).is_some() {
                    push_issue(
                        &mut issues,
                        "duplicate_pipeline_step_id",
                        format!("{path}.id"),
                        format!("step id {:?} is not globally unique", step.id),
                    );
                }
                step_outputs.insert(step.id.clone(), step.outputs.clone());
                step_stage.insert(step.id.clone(), stage.id.clone());
            }
        }
        for (pipeline_index, pipeline) in composition.label_pipelines.iter().enumerate() {
            for (step_index, step) in pipeline.steps.iter().enumerate() {
                let path = format!("label_pipelines[{pipeline_index}].steps[{step_index}]");
                if step_paths.insert(step.id.clone(), path.clone()).is_some() {
                    push_issue(
                        &mut issues,
                        "duplicate_pipeline_step_id",
                        format!("{path}.id"),
                        format!("step id {:?} is not globally unique", step.id),
                    );
                }
                step_outputs.insert(step.id.clone(), step.outputs.clone());
            }
        }

        for (stage_index, stage) in composition.shared_stages.iter().enumerate() {
            for (step_index, step) in stage.steps.iter().enumerate() {
                validate_step(
                    step,
                    &format!("shared_stages[{stage_index}].steps[{step_index}]"),
                    Some(&stage.id),
                    &shared_stage_ids,
                    &step_outputs,
                    &step_stage,
                    &enabled_skills,
                    nodes,
                    models,
                    &mut issues,
                );
            }
        }

        let mut pipeline_ids = BTreeSet::new();
        for (pipeline_index, pipeline) in composition.label_pipelines.iter().enumerate() {
            let path = format!("label_pipelines[{pipeline_index}]");
            if !pipeline_ids.insert(pipeline.id.as_str()) {
                push_issue(
                    &mut issues,
                    "duplicate_label_pipeline_id",
                    format!("{path}.id"),
                    format!("duplicate Label Pipeline id {:?}", pipeline.id),
                );
            }
            match project
                .tasks
                .iter()
                .find(|task| task.id == pipeline.target_task_id)
            {
                None => push_issue(
                    &mut issues,
                    "unknown_target_task",
                    format!("{path}.target_task_id"),
                    format!(
                        "task {:?} is not in Project Schema",
                        pipeline.target_task_id
                    ),
                ),
                Some(task)
                    if !task
                        .labels
                        .iter()
                        .any(|label| label == pipeline.target_label.as_str()) =>
                {
                    push_issue(
                        &mut issues,
                        "unknown_target_label",
                        format!("{path}.target_label"),
                        format!(
                            "label {:?} is not declared by task {:?}",
                            pipeline.target_label, pipeline.target_task_id
                        ),
                    );
                }
                Some(_) => {}
            }
            if pipeline.steps.is_empty() {
                push_issue(
                    &mut issues,
                    "empty_label_pipeline",
                    format!("{path}.steps"),
                    "a Label Pipeline must contain at least one step",
                );
            }
            for (step_index, step) in pipeline.steps.iter().enumerate() {
                validate_step(
                    step,
                    &format!("{path}.steps[{step_index}]"),
                    None,
                    &shared_stage_ids,
                    &step_outputs,
                    &step_stage,
                    &enabled_skills,
                    nodes,
                    models,
                    &mut issues,
                );
            }
            if !pipeline
                .steps
                .iter()
                .any(|step| step.kind == WorkflowNodeKind::Commit)
            {
                push_issue(
                    &mut issues,
                    "label_pipeline_has_no_commit",
                    format!("{path}.steps"),
                    "each Label Pipeline requires a Commit terminal",
                );
            }
        }

        LabelPipelineValidationReport {
            valid: issues.iter().all(|issue| !issue.blocking),
            issues,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_step(
    step: &PipelineStep,
    path: &str,
    current_stage: Option<&str>,
    shared_stage_ids: &BTreeSet<&str>,
    outputs: &BTreeMap<String, BTreeMap<String, ArtifactKind>>,
    step_stage: &BTreeMap<String, String>,
    enabled_skills: &BTreeMap<String, String>,
    nodes: &NodeRegistry,
    models: &ModelRegistry,
    issues: &mut Vec<LabelPipelineValidationIssue>,
) {
    let Some(descriptor) = nodes.get(&step.node_type) else {
        push_issue(
            issues,
            "unknown_node",
            format!("{path}.node_type"),
            format!("node operation {:?} is not registered", step.node_type),
        );
        return;
    };

    for (port, source) in &step.inputs {
        let source_type = source.artifact_type();
        if !descriptor.accepts.is_empty() && !descriptor.accepts.contains(&source_type) {
            push_issue(
                issues,
                "node_input_type_unsupported",
                format!("{path}.inputs.{port}"),
                format!(
                    "operation {:?} does not accept {source_type:?}",
                    step.node_type
                ),
            );
        }
        if let PipelineSource::SharedStage {
            stage_id,
            step_id,
            port: source_port,
            artifact_type,
        } = source
        {
            if !shared_stage_ids.contains(stage_id.as_str()) {
                push_issue(
                    issues,
                    "unknown_shared_stage",
                    format!("{path}.inputs.{port}.stage_id"),
                    format!("shared stage {stage_id:?} does not exist"),
                );
            }
            if step_stage.get(step_id) != Some(stage_id) {
                push_issue(
                    issues,
                    "shared_stage_source_mismatch",
                    format!("{path}.inputs.{port}"),
                    format!("step {step_id:?} is not owned by shared stage {stage_id:?}"),
                );
            }
            validate_source_output(
                step_id,
                source_port,
                *artifact_type,
                outputs,
                &format!("{path}.inputs.{port}"),
                issues,
            );
        } else if let PipelineSource::Step {
            step_id,
            port: source_port,
            artifact_type,
        } = source
        {
            if current_stage.is_none() && step_stage.contains_key(step_id) {
                push_issue(
                    issues,
                    "shared_step_requires_shared_source",
                    format!("{path}.inputs.{port}"),
                    "Label Pipeline references to shared steps must name their Shared Stage",
                );
            }
            validate_source_output(
                step_id,
                source_port,
                *artifact_type,
                outputs,
                &format!("{path}.inputs.{port}"),
                issues,
            );
        }
    }
    for (port, artifact_type) in &step.outputs {
        if !descriptor.produces.contains(artifact_type) {
            push_issue(
                issues,
                "node_output_type_unsupported",
                format!("{path}.outputs.{port}"),
                format!(
                    "operation {:?} does not produce {artifact_type:?}",
                    step.node_type
                ),
            );
        }
    }

    if !descriptor.required_capabilities.is_empty() {
        let Some(binding) = &step.model_binding else {
            push_issue(
                issues,
                "unresolved_model_binding",
                format!("{path}.model_binding"),
                "this node requires a Model Registry binding",
            );
            return;
        };
        if !descriptor
            .required_capabilities
            .contains(&binding.capability)
        {
            push_issue(
                issues,
                "model_binding_capability_mismatch",
                format!("{path}.model_binding.capability"),
                format!(
                    "binding capability {:?} is not required by operation {:?}",
                    binding.capability, step.node_type
                ),
            );
        }
        match models.resolve(&binding.model_id) {
            Ok((model, _)) if !model.capabilities.contains(&binding.capability) => push_issue(
                issues,
                "model_capability_mismatch",
                format!("{path}.model_binding.model_id"),
                format!(
                    "model {:?} lacks {:?}",
                    binding.model_id, binding.capability
                ),
            ),
            Ok((model, _)) => {
                if requests_visual_prompt(&step.parameters)
                    && !model.input_contract.supports_visual_prompt
                {
                    push_issue(
                        issues,
                        "visual_prompt_unsupported",
                        format!("{path}.parameters"),
                        format!(
                            "model {:?} does not advertise visual prompt support",
                            binding.model_id
                        ),
                    );
                }
            }
            Err(_) => push_issue(
                issues,
                "unknown_model",
                format!("{path}.model_binding.model_id"),
                format!("model {:?} is not registered", binding.model_id),
            ),
        }
    }

    if let Some(binding) = &step.skill_binding {
        match enabled_skills.get(&binding.skill_id) {
            Some(version) if version == &binding.version => {}
            Some(version) => push_issue(
                issues,
                "skill_version_mismatch",
                format!("{path}.skill_binding.version"),
                format!(
                    "Project enables Skill {:?} at version {version:?}, not {:?}",
                    binding.skill_id, binding.version
                ),
            ),
            None => push_issue(
                issues,
                "required_skill_not_enabled",
                format!("{path}.skill_binding.skill_id"),
                format!("Skill {:?} is not enabled", binding.skill_id),
            ),
        }
    }
}

fn requests_visual_prompt(parameters: &BTreeMap<String, serde_json::Value>) -> bool {
    parameters.iter().any(|(name, value)| {
        matches!(
            name.as_str(),
            "visual_prompt" | "visual_prompt_box" | "visual_exemplar" | "exemplar_image"
        ) && !value.is_null()
    })
}

fn validate_source_output(
    step_id: &str,
    port: &str,
    artifact_type: ArtifactKind,
    outputs: &BTreeMap<String, BTreeMap<String, ArtifactKind>>,
    path: &str,
    issues: &mut Vec<LabelPipelineValidationIssue>,
) {
    match outputs.get(step_id).and_then(|ports| ports.get(port)) {
        Some(actual) if *actual != artifact_type => push_issue(
            issues,
            "pipeline_artifact_type_mismatch",
            path,
            format!(
                "source {step_id}.{port} produces {actual:?}, but the input declares {artifact_type:?}"
            ),
        ),
        Some(_) => {}
        None => push_issue(
            issues,
            "unknown_pipeline_source",
            path,
            format!("source output {step_id}.{port} does not exist"),
        ),
    }
}

fn push_issue(
    issues: &mut Vec<LabelPipelineValidationIssue>,
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(LabelPipelineValidationIssue {
        code: code.into(),
        path: path.into(),
        message: message.into(),
        blocking: true,
    });
}

impl LabelWorkflowComposition {
    /// Compile the authoring projection into one ordinary flat Workflow DAG.
    ///
    /// The generated Image Input node is shared. Shared-stage steps are emitted once and all
    /// Label Pipeline references become edges to those same node ids.
    #[must_use]
    pub fn compile_draft(
        &self,
        project_id: impl Into<String>,
        name: impl Into<String>,
        enabled_skills: BTreeMap<String, String>,
        now: DateTime<Utc>,
    ) -> WorkflowDraft {
        let mut nodes = vec![WorkflowDraftNode {
            id: IMAGE_INPUT_NODE_ID.to_owned(),
            node_type: IMAGE_INPUT_OPERATION.to_owned(),
            kind: WorkflowNodeKind::ImageInput,
            depends_on: Vec::new(),
            inputs: Vec::new(),
            outputs: vec![NodePort {
                id: "image".to_owned(),
                artifact_type: ArtifactKind::Image,
                required: true,
                multiple: false,
            }],
            model_binding: None,
            model_profile_binding: None,
            unresolved_model_requirement: None,
            required_skills: Vec::new(),
            validators: Vec::new(),
            refiners: Vec::new(),
            fallback: None,
            max_retries: 0,
            review_gate: false,
            parameters: BTreeMap::new(),
            retry_policy: RetryPolicy::default(),
            fallback_policy: FallbackPolicy::default(),
            gate: ReviewGate::default(),
            resources: ResourceRequirements::default(),
        }];
        let mut edges = Vec::new();
        for step in self
            .shared_stages
            .iter()
            .flat_map(|stage| stage.steps.iter())
            .chain(
                self.label_pipelines
                    .iter()
                    .flat_map(|pipeline| pipeline.steps.iter()),
            )
        {
            let mut dependencies = BTreeSet::new();
            let inputs = step
                .inputs
                .iter()
                .map(|(port, source)| {
                    let (source_node, source_port) = source.producer();
                    dependencies.insert(source_node.to_owned());
                    edges.push(WorkflowEdge {
                        from_node: source_node.to_owned(),
                        from_port: source_port.to_owned(),
                        to_node: step.id.clone(),
                        to_port: port.clone(),
                        route: None,
                    });
                    NodePort {
                        id: port.clone(),
                        artifact_type: source.artifact_type(),
                        required: true,
                        multiple: false,
                    }
                })
                .collect();
            nodes.push(WorkflowDraftNode {
                id: step.id.clone(),
                node_type: step.node_type.clone(),
                kind: step.kind,
                depends_on: dependencies.into_iter().collect(),
                inputs,
                outputs: step
                    .outputs
                    .iter()
                    .map(|(port, artifact_type)| NodePort {
                        id: port.clone(),
                        artifact_type: *artifact_type,
                        required: true,
                        multiple: true,
                    })
                    .collect(),
                model_binding: step
                    .model_binding
                    .as_ref()
                    .map(|binding| binding.model_id.clone()),
                model_profile_binding: None,
                unresolved_model_requirement: None,
                required_skills: step
                    .skill_binding
                    .as_ref()
                    .map(|binding| vec![binding.skill_id.clone()])
                    .unwrap_or_default(),
                validators: step.validators.clone(),
                refiners: step.refiners.clone(),
                fallback: step.fallback.clone(),
                max_retries: step.retry_policy.max_attempts.saturating_sub(1),
                review_gate: step.review_gate.required,
                parameters: step.parameters.clone(),
                retry_policy: step.retry_policy,
                fallback_policy: FallbackPolicy {
                    target_node: step.fallback.clone(),
                    on_timeout: true,
                    on_error: true,
                },
                gate: step.review_gate,
                resources: step.resources.clone(),
            });
        }
        WorkflowDraft {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            name: name.into(),
            status: WorkflowDraftStatus::Editing,
            nodes,
            edges,
            enabled_skills,
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: false,
            label_pipeline: Some(self.clone()),
            created_at: now,
            updated_at: now,
        }
    }
}

impl DetectionSetArtifact {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != DETECTION_ARTIFACT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported Detection Artifact schema version {}",
                self.schema_version
            ));
        }
        validate_set_reference(&self.reference, ArtifactKind::DetectionSet)?;
        if self.model_binding.trim().is_empty() {
            return Err("DetectionSet model_binding cannot be empty".to_owned());
        }
        let mut ids = BTreeSet::new();
        for detection in &self.detections {
            if detection.detection_id.trim().is_empty()
                || !ids.insert(detection.detection_id.as_str())
            {
                return Err("Detection ids must be non-empty and unique".to_owned());
            }
            detection.score.validate()?;
            if detection.source_model_id.trim().is_empty() {
                return Err("Detection source_model_id cannot be empty".to_owned());
            }
            if !is_detection_capability(detection.source_capability) {
                return Err("Detection source_capability must be a detection capability".to_owned());
            }
            if detection.query_id.as_deref().is_none_or(str::is_empty)
                && detection.model_label.as_deref().is_none_or(str::is_empty)
            {
                return Err("Detection requires a query_id or model_label".to_owned());
            }
            if detection.evidence.is_empty() {
                return Err("Detection must preserve at least one source evidence item".to_owned());
            }
            for evidence in &detection.evidence {
                validate_detection_evidence(evidence)?;
            }
            if !detection
                .evidence
                .iter()
                .any(|evidence| evidence.source_model_id == detection.source_model_id)
            {
                return Err("Detection evidence must include its source model".to_owned());
            }
        }
        Ok(())
    }
}

impl BoxPromptSetArtifact {
    pub fn from_detections(
        reference: ArtifactRef,
        detections: &DetectionSetArtifact,
        padding: f32,
    ) -> Result<Self, String> {
        detections.validate()?;
        if !padding.is_finite() || !(0.0..=0.5).contains(&padding) {
            return Err("Box prompt padding must be finite and within [0,0.5]".to_owned());
        }
        let prompts = detections
            .detections
            .iter()
            .map(|detection| {
                let left = (detection.bbox.x() - padding).max(0.0);
                let top = (detection.bbox.y() - padding).max(0.0);
                let right = (detection.bbox.x() + detection.bbox.width() + padding).min(1.0);
                let bottom = (detection.bbox.y() + detection.bbox.height() + padding).min(1.0);
                Ok(BoxPrompt {
                    id: format!("box-prompt:{}", detection.detection_id),
                    subject: detections.reference.item(&detection.detection_id),
                    bbox: NormalizedRect::new(left, top, right - left, bottom - top)
                        .map_err(|error| error.to_string())?,
                    attributes: BTreeMap::from([
                        (
                            "source_detection".to_owned(),
                            serde_json::to_value(detection).unwrap_or(serde_json::Value::Null),
                        ),
                        (
                            "source_model_id".to_owned(),
                            serde_json::json!(detection.source_model_id),
                        ),
                        (
                            "source_detection_score".to_owned(),
                            serde_json::to_value(detection.score)
                                .unwrap_or(serde_json::Value::Null),
                        ),
                    ]),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let artifact = Self {
            reference,
            image_id: detections.image_id,
            source_detections: detections.reference.clone(),
            prompts,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_set_reference(&self.reference, ArtifactKind::BoxPromptSet)?;
        validate_set_reference(&self.source_detections, ArtifactKind::DetectionSet)?;
        let mut ids = BTreeSet::new();
        for prompt in &self.prompts {
            if prompt.id.trim().is_empty() || !ids.insert(prompt.id.as_str()) {
                return Err("Box prompt ids must be non-empty and unique".to_owned());
            }
            validate_item_reference(&prompt.subject, ArtifactKind::DetectionSet)?;
            if prompt.subject.artifact_id != self.source_detections.artifact_id {
                return Err("Box prompt subject must belong to source_detections".to_owned());
            }
        }
        Ok(())
    }
}

impl PointPromptSetArtifact {
    pub fn validate(&self) -> Result<(), String> {
        validate_set_reference(&self.reference, ArtifactKind::PointPromptSet)?;
        if self.source_artifact.artifact_id.trim().is_empty() {
            return Err("PointPromptSet source_artifact cannot be empty".to_owned());
        }
        let mut ids = BTreeSet::new();
        for prompt in &self.prompts {
            if prompt.id.trim().is_empty() || !ids.insert(prompt.id.as_str()) {
                return Err("Point prompt ids must be non-empty and unique".to_owned());
            }
            if prompt.points.is_empty() {
                return Err("Point prompt must contain at least one point".to_owned());
            }
            if prompt.subject.artifact_id != self.source_artifact.artifact_id {
                return Err("Point prompt subject must belong to source_artifact".to_owned());
            }
        }
        Ok(())
    }
}

impl MaskSetArtifact {
    pub fn validate(&self) -> Result<(), String> {
        validate_set_reference(&self.reference, ArtifactKind::MaskSet)?;
        if self.model_binding.trim().is_empty() {
            return Err("MaskSet model_binding cannot be empty".to_owned());
        }
        if !matches!(
            self.source_prompts.artifact_type,
            ArtifactKind::BoxPromptSet | ArtifactKind::PointPromptSet
        ) || self.source_prompts.item_id.is_some()
        {
            return Err("MaskSet source_prompts must reference a prompt set".to_owned());
        }
        let mut ids = BTreeSet::new();
        for item in &self.masks {
            if item.mask_id.trim().is_empty() || !ids.insert(item.mask_id.as_str()) {
                return Err("Mask ids must be non-empty and unique".to_owned());
            }
            if item.prompt.artifact_type != self.source_prompts.artifact_type
                || item.prompt.artifact_id != self.source_prompts.artifact_id
            {
                return Err("Mask prompt must belong to source_prompts".to_owned());
            }
            validate_item_reference(&item.prompt, self.source_prompts.artifact_type)?;
            item.score.validate()?;
            crate::AnnotationValue::InstanceMask {
                mask: item.mask.clone(),
            }
            .validate()
            .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

impl PolygonSetArtifact {
    pub fn validate(&self) -> Result<(), String> {
        validate_set_reference(&self.reference, ArtifactKind::PolygonSet)?;
        validate_set_reference(&self.source_masks, ArtifactKind::MaskSet)?;
        let mut ids = BTreeSet::new();
        for polygon in &self.polygons {
            if polygon.polygon_id.trim().is_empty() || !ids.insert(polygon.polygon_id.as_str()) {
                return Err("Polygon ids must be non-empty and unique".to_owned());
            }
            validate_item_reference(&polygon.parent, ArtifactKind::MaskSet)?;
            if polygon.parent.artifact_id != self.source_masks.artifact_id {
                return Err("Polygon parent must belong to source_masks".to_owned());
            }
            polygon.score.validate()?;
            crate::AnnotationValue::Polygon {
                rings: polygon.rings.clone(),
            }
            .validate()
            .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

impl CandidateClusterSetArtifact {
    pub fn validate(&self) -> Result<(), String> {
        validate_set_reference(&self.reference, ArtifactKind::CandidateClusterSet)?;
        if self.source_detection_sets.is_empty() {
            return Err("CandidateClusterSet requires source DetectionSets".to_owned());
        }
        let mut source_ids = BTreeSet::new();
        for source in &self.source_detection_sets {
            validate_set_reference(source, ArtifactKind::DetectionSet)?;
            if !source_ids.insert(source.artifact_id.as_str()) {
                return Err("CandidateClusterSet sources must be unique".to_owned());
            }
        }
        let mut candidate_ids = BTreeSet::new();
        for candidate in &self.candidates {
            if candidate.id.trim().is_empty() || !candidate_ids.insert(candidate.id.as_str()) {
                return Err("CandidateCluster ids must be non-empty and unique".to_owned());
            }
            if candidate.members.is_empty() {
                return Err("CandidateCluster must retain source evidence".to_owned());
            }
            if candidate.target_label.as_str().trim().is_empty() {
                return Err("CandidateCluster target_label cannot be empty".to_owned());
            }
            for member in &candidate.members {
                validate_detection_evidence(member)?;
                if !source_ids.contains(member.source_artifact_id.as_str()) {
                    return Err(
                        "CandidateCluster evidence must belong to a declared source DetectionSet"
                            .to_owned(),
                    );
                }
            }
            match candidate.agreement {
                CandidateAgreement::MultiSourceAgreement {
                    minimum_iou,
                    mean_iou,
                } => {
                    validate_confidence(minimum_iou)?;
                    validate_confidence(mean_iou)?;
                    if minimum_iou > mean_iou || candidate.members.len() < 2 {
                        return Err(
                            "multi-source agreement requires at least two members and minimum_iou <= mean_iou"
                                .to_owned(),
                        );
                    }
                }
                CandidateAgreement::SingleSource if candidate.members.len() != 1 => {
                    return Err("single-source agreement requires exactly one member".to_owned());
                }
                CandidateAgreement::SingleSource
                | CandidateAgreement::GeometryConflict
                | CandidateAgreement::LabelConflict => {}
            }
        }
        Ok(())
    }
}

fn validate_detection_evidence(evidence: &DetectionEvidence) -> Result<(), String> {
    if evidence.source_model_id.trim().is_empty() || evidence.source_artifact_id.trim().is_empty() {
        return Err("Detection evidence requires source model and Artifact ids".to_owned());
    }
    if !is_detection_capability(evidence.source_capability) {
        return Err(
            "Detection evidence source_capability must be a detection capability".to_owned(),
        );
    }
    if evidence
        .project_label
        .as_ref()
        .is_some_and(|label| label.as_str().trim().is_empty())
    {
        return Err("Detection evidence project_label cannot be empty".to_owned());
    }
    if evidence.query_id.as_deref().is_none_or(str::is_empty)
        && evidence.model_label.as_deref().is_none_or(str::is_empty)
    {
        return Err("Detection evidence requires a query_id or model_label".to_owned());
    }
    evidence.score.validate()?;
    if let Some(payload) = &evidence.raw_output_ref {
        if payload.id.trim().is_empty()
            || payload.media_type.trim().is_empty()
            || payload.sha256.len() != 64
            || !payload.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("raw output reference is invalid".to_owned());
        }
    }
    Ok(())
}

const fn is_detection_capability(capability: VisionCapability) -> bool {
    matches!(
        capability,
        VisionCapability::OpenVocabularyDetection
            | VisionCapability::PhraseGrounding
            | VisionCapability::ObjectDetection
            | VisionCapability::PromptedSegmentation
    )
}

impl ImageArtifact {
    pub fn validate(&self) -> Result<(), String> {
        validate_set_reference(&self.reference, ArtifactKind::Image)?;
        if self.width == 0
            || self.height == 0
            || self.mime_type.trim().is_empty()
            || self.blob_ref.trim().is_empty()
        {
            return Err(
                "Image Artifact requires dimensions, MIME type, and a blob reference".to_owned(),
            );
        }
        Ok(())
    }
}

impl CropSetArtifact {
    /// Materialize the Image + `DetectionSet` -> `CropSet` fan-out contract. Pixel extraction is
    /// performed by the runtime Crop node; this Core method owns geometry and parent lineage.
    pub fn fan_out(
        reference: ArtifactRef,
        detections: &DetectionSetArtifact,
        padding: f32,
        blob_ref: impl Fn(&Detection) -> Option<String>,
    ) -> Result<Self, String> {
        detections.validate()?;
        if !padding.is_finite() || padding < 0.0 {
            return Err("Crop padding must be finite and non-negative".to_owned());
        }
        let crops = detections
            .detections
            .iter()
            .map(|detection| {
                let left = (detection.bbox.x() - padding).max(0.0);
                let top = (detection.bbox.y() - padding).max(0.0);
                let right = (detection.bbox.x() + detection.bbox.width() + padding).min(1.0);
                let bottom = (detection.bbox.y() + detection.bbox.height() + padding).min(1.0);
                let rect = NormalizedRect::new(left, top, right - left, bottom - top)
                    .map_err(|error| error.to_string())?;
                Ok(Crop {
                    id: format!("crop:{}", detection.detection_id),
                    parent: detections.reference.item(&detection.detection_id),
                    rect,
                    source_width: 0,
                    source_height: 0,
                    crop_width: 0,
                    crop_height: 0,
                    padding,
                    mime_type: None,
                    blob_ref: blob_ref(detection),
                    cache_key: None,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let artifact = Self {
            reference,
            image_id: detections.image_id,
            source_detections: detections.reference.clone(),
            crops,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_set_reference(&self.reference, ArtifactKind::CropSet)?;
        validate_set_reference(&self.source_detections, ArtifactKind::DetectionSet)?;
        let mut ids = BTreeSet::new();
        for crop in &self.crops {
            if crop.id.trim().is_empty() || !ids.insert(crop.id.as_str()) {
                return Err("Crop ids must be non-empty and unique".to_owned());
            }
            validate_item_reference(&crop.parent, ArtifactKind::DetectionSet)?;
            if crop.parent.artifact_id != self.source_detections.artifact_id {
                return Err("Crop parent must belong to source_detections".to_owned());
            }
            if !crop.padding.is_finite() || crop.padding < 0.0 {
                return Err("Crop padding must be finite and non-negative".to_owned());
            }
            let dimensions_absent = crop.source_width == 0
                && crop.source_height == 0
                && crop.crop_width == 0
                && crop.crop_height == 0;
            let dimensions_complete = crop.source_width > 0
                && crop.source_height > 0
                && crop.crop_width > 0
                && crop.crop_height > 0;
            if !dimensions_absent && !dimensions_complete {
                return Err("Crop dimensions must be all present or all absent".to_owned());
            }
        }
        Ok(())
    }
}

impl ClassificationSetArtifact {
    pub fn validate(&self) -> Result<(), String> {
        validate_set_reference(&self.reference, ArtifactKind::ClassificationSet)?;
        let mut ids = BTreeSet::new();
        for classification in &self.classifications {
            if classification.id.trim().is_empty() || !ids.insert(classification.id.as_str()) {
                return Err("Classification ids must be non-empty and unique".to_owned());
            }
            match classification.subject.artifact_type {
                ArtifactKind::Image => {
                    if classification.subject.item_id.is_some() {
                        return Err(
                            "Image classification subject cannot name a set item".to_owned()
                        );
                    }
                }
                ArtifactKind::CropSet => {
                    validate_item_reference(&classification.subject, ArtifactKind::CropSet)?;
                    let parent = classification.parent.as_ref().ok_or_else(|| {
                        "Crop classification must retain its parent Detection reference".to_owned()
                    })?;
                    validate_item_reference(parent, ArtifactKind::DetectionSet)?;
                }
                ArtifactKind::DetectionSet => {
                    validate_item_reference(&classification.subject, ArtifactKind::DetectionSet)?;
                    if let Some(parent) = &classification.parent {
                        validate_item_reference(parent, ArtifactKind::DetectionSet)?;
                    }
                }
                _ => {
                    return Err(
                        "Classification subject must be Image, DetectionSet, or CropSet".to_owned(),
                    );
                }
            }
            validate_confidence(classification.confidence)?;
            for score in classification.scores.values().copied() {
                validate_confidence(score)?;
            }
        }
        Ok(())
    }
}

impl AnnotationCandidateSet {
    /// Materialize `DetectionSet` + `ClassificationSet` -> `AnnotationCandidateSet` fan-in. Every
    /// classification is joined by its exact Detection item reference, never by array position.
    pub fn fan_in(
        reference: ArtifactRef,
        detections: &DetectionSetArtifact,
        classifications: &ClassificationSetArtifact,
        task_id: &TaskId,
        label_mapping: &BTreeMap<LabelId, LabelId>,
    ) -> Result<Self, String> {
        detections.validate()?;
        classifications.validate()?;
        if detections.image_id != classifications.image_id {
            return Err("fan-in Artifacts belong to different images".to_owned());
        }
        let detections_by_id = detections
            .detections
            .iter()
            .map(|detection| (detection.detection_id.as_str(), detection))
            .collect::<BTreeMap<_, _>>();
        let candidates = classifications
            .classifications
            .iter()
            .map(|classification| {
                let detection_ref = match classification.subject.artifact_type {
                    ArtifactKind::DetectionSet => &classification.subject,
                    ArtifactKind::CropSet => classification.parent.as_ref().ok_or_else(|| {
                        "Crop classification is missing its parent Detection reference".to_owned()
                    })?,
                    _ => {
                        return Err(
                            "detection fan-in cannot consume whole-image classification".to_owned()
                        );
                    }
                };
                if detection_ref.artifact_id != detections.reference.artifact_id {
                    return Err(
                        "classification parent belongs to a different DetectionSet".to_owned()
                    );
                }
                let detection_id = detection_ref
                    .item_id
                    .as_deref()
                    .ok_or_else(|| "classification parent does not name a Detection".to_owned())?;
                let detection = detections_by_id.get(detection_id).ok_or_else(|| {
                    format!("classification references unknown Detection {detection_id:?}")
                })?;
                let label = label_mapping
                    .get(&classification.label)
                    .cloned()
                    .unwrap_or_else(|| classification.label.clone());
                Ok(AnnotationCandidate {
                    id: format!("candidate:{}:{}", detection.detection_id, classification.id),
                    task_id: task_id.clone(),
                    label,
                    subject: detection_ref.clone(),
                    value: Some(VisionArtifactValue::BoundingBox {
                        rect: detection.bbox,
                    }),
                    confidence: detection
                        .score
                        .comparable_confidence()
                        .map(|score| score.min(classification.confidence)),
                    attributes: BTreeMap::new(),
                    evidence: vec![
                        detection_ref.clone(),
                        classifications.reference.item(&classification.id),
                    ],
                    validation_state: Some(
                        if detections.validation_state == ArtifactValidationState::Invalid
                            || classifications.validation_state == ArtifactValidationState::Invalid
                        {
                            ArtifactValidationState::Invalid
                        } else if detections.validation_state
                            == ArtifactValidationState::NeedsReview
                            || classifications.validation_state
                                == ArtifactValidationState::NeedsReview
                        {
                            ArtifactValidationState::NeedsReview
                        } else {
                            ArtifactValidationState::Unvalidated
                        },
                    ),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let artifact = Self {
            reference,
            image_id: detections.image_id,
            candidates,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_set_reference(&self.reference, ArtifactKind::AnnotationCandidateSet)?;
        let mut ids = BTreeSet::new();
        for candidate in &self.candidates {
            if candidate.id.trim().is_empty() || !ids.insert(candidate.id.as_str()) {
                return Err("Annotation candidate ids must be non-empty and unique".to_owned());
            }
            if candidate.subject.item_id.is_none()
                && candidate.subject.artifact_type != ArtifactKind::Image
            {
                return Err("Annotation candidate subject must reference a set item".to_owned());
            }
            if let Some(confidence) = candidate.confidence {
                validate_confidence(confidence)?;
            }
            if let Some(value) = &candidate.value {
                value.validate().map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }
}

fn validate_set_reference(reference: &ArtifactRef, kind: ArtifactKind) -> Result<(), String> {
    if reference.artifact_id.trim().is_empty()
        || reference.source_node.trim().is_empty()
        || reference.port.trim().is_empty()
        || reference.artifact_type != kind
        || reference.item_id.is_some()
    {
        return Err(format!("invalid {kind:?} Artifact reference"));
    }
    Ok(())
}

fn validate_item_reference(reference: &ArtifactRef, kind: ArtifactKind) -> Result<(), String> {
    if reference.artifact_type != kind
        || reference
            .item_id
            .as_deref()
            .is_none_or(|item| item.trim().is_empty())
    {
        return Err(format!("reference must identify a {kind:?} item"));
    }
    Ok(())
}

fn validate_confidence(confidence: f32) -> Result<(), String> {
    if confidence.is_finite() && (0.0..=1.0).contains(&confidence) {
        Ok(())
    } else {
        Err("confidence must be finite and within [0,1]".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::{
        CoreResult, DatasetConfig, EnabledSkillConfig, ExportConfig, ProjectDescriptor,
        ReviewConfig, RuntimeConfig, TaskConfig, TaskKind, VisionBackendKind,
        VisionInferenceRequest, VisionInferenceResponse, VisionModelBackend, VisionModelDescriptor,
        VisionNodeDescriptor,
    };

    struct Backend;

    #[async_trait]
    impl VisionModelBackend for Backend {
        fn id(&self) -> &str {
            "backend"
        }

        fn kind(&self) -> VisionBackendKind {
            VisionBackendKind::Mock
        }

        fn capabilities(&self) -> Vec<VisionCapability> {
            vec![
                VisionCapability::ObjectDetection,
                VisionCapability::Classification,
            ]
        }

        async fn infer(
            &self,
            _request: VisionInferenceRequest,
            _cancellation: CancellationToken,
        ) -> CoreResult<VisionInferenceResponse> {
            Ok(VisionInferenceResponse::default())
        }
    }

    fn project() -> ProjectSchema {
        ProjectSchema {
            version: 1,
            project: ProjectDescriptor {
                name: "generic".to_owned(),
                skill: String::new(),
                skill_version: String::new(),
                enabled_skills: vec![EnabledSkillConfig {
                    id: "classification".to_owned(),
                    version: "1".to_owned(),
                    configuration: BTreeMap::new(),
                }],
                language: "en".to_owned(),
            },
            dataset: DatasetConfig {
                root: "images".into(),
                include: Vec::new(),
                recursive: true,
            },
            runtime: RuntimeConfig {
                max_parallel_images: 1,
                max_model_turns_per_task: 1,
                max_tool_calls_per_task: 1,
                max_recovery_turns_per_task: 1,
                task_timeout_seconds: 30,
                provider_request_timeout_seconds: 30,
                max_retries: 1,
                auto_resume: true,
            },
            tasks: vec![TaskConfig {
                id: TaskId::from("objects"),
                display_name: None,
                kind: TaskKind::BoundingBox,
                labels: vec![
                    "person".to_owned(),
                    "vehicle".to_owned(),
                    "animal".to_owned(),
                ],
                required: false,
                multi_label: true,
                depends_on: Vec::new(),
                validators: Vec::new(),
                refiners: Vec::new(),
                target_task: None,
                target_labels: Vec::new(),
                attributes: BTreeMap::new(),
            }],
            review: ReviewConfig {
                auto_accept_confidence: 0.9,
                force_review_below: 0.5,
                force_review_on_warning_codes: Vec::new(),
            },
            export: ExportConfig {
                formats: Vec::new(),
            },
        }
    }

    fn registries() -> (NodeRegistry, ModelRegistry) {
        let mut nodes = NodeRegistry::new();
        for descriptor in [
            VisionNodeDescriptor {
                id: IMAGE_INPUT_OPERATION.to_owned(),
                display_name: "Image Input".to_owned(),
                required_capabilities: Vec::new(),
                accepts: Vec::new(),
                produces: vec![ArtifactKind::Image],
                deterministic: true,
            },
            VisionNodeDescriptor {
                id: "skill.detection".to_owned(),
                display_name: "Detection".to_owned(),
                required_capabilities: vec![VisionCapability::ObjectDetection],
                accepts: vec![ArtifactKind::Image],
                produces: vec![ArtifactKind::DetectionSet],
                deterministic: false,
            },
            VisionNodeDescriptor {
                id: "core.filter".to_owned(),
                display_name: "Filter".to_owned(),
                required_capabilities: Vec::new(),
                accepts: vec![ArtifactKind::DetectionSet],
                produces: vec![ArtifactKind::DetectionSet],
                deterministic: true,
            },
            VisionNodeDescriptor {
                id: "core.commit".to_owned(),
                display_name: "Commit".to_owned(),
                required_capabilities: Vec::new(),
                accepts: vec![ArtifactKind::DetectionSet],
                // Registry descriptors require a non-empty output catalog even though Commit is
                // terminal and this particular PipelineStep declares no output port.
                produces: vec![ArtifactKind::DetectionSet],
                deterministic: true,
            },
        ] {
            nodes.register(descriptor).expect("node");
        }
        let mut models = ModelRegistry::new();
        models.register_backend(Arc::new(Backend)).expect("backend");
        models
            .register_model(VisionModelDescriptor {
                id: "detector".to_owned(),
                display_name: "Shared detector".to_owned(),
                backend_id: "backend".to_owned(),
                capabilities: vec![VisionCapability::ObjectDetection],
                input_types: Vec::new(),
                output_types: vec![ArtifactKind::DetectionSet],
                model: "fixture".to_owned(),
                model_version: "1".to_owned(),
                ..VisionModelDescriptor::default()
            })
            .expect("model");
        (nodes, models)
    }

    fn shared_composition() -> LabelWorkflowComposition {
        let detector = PipelineStep {
            id: "shared.detector".to_owned(),
            node_type: "skill.detection".to_owned(),
            kind: WorkflowNodeKind::VisionModel,
            inputs: BTreeMap::from([("image".to_owned(), PipelineSource::Image)]),
            outputs: BTreeMap::from([("detections".to_owned(), ArtifactKind::DetectionSet)]),
            model_binding: Some(ModelBinding {
                model_id: "detector".to_owned(),
                capability: VisionCapability::ObjectDetection,
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
        let pipelines = ["person", "vehicle", "animal"]
            .into_iter()
            .map(|label| {
                let filter_id = format!("{label}.filter");
                LabelPipeline {
                    id: format!("{label}-pipeline"),
                    target_task_id: TaskId::from("objects"),
                    target_label: LabelId::from(label),
                    steps: vec![
                        PipelineStep {
                            id: filter_id.clone(),
                            node_type: "core.filter".to_owned(),
                            kind: WorkflowNodeKind::Transform,
                            inputs: BTreeMap::from([(
                                "detections".to_owned(),
                                PipelineSource::SharedStage {
                                    stage_id: "shared-vision".to_owned(),
                                    step_id: "shared.detector".to_owned(),
                                    port: "detections".to_owned(),
                                    artifact_type: ArtifactKind::DetectionSet,
                                },
                            )]),
                            outputs: BTreeMap::from([(
                                "detections".to_owned(),
                                ArtifactKind::DetectionSet,
                            )]),
                            model_binding: None,
                            skill_binding: None,
                            parameters: BTreeMap::from([(
                                "label".to_owned(),
                                serde_json::json!(label),
                            )]),
                            validators: Vec::new(),
                            refiners: Vec::new(),
                            fallback: None,
                            retry_policy: RetryPolicy::default(),
                            review_gate: ReviewGate::default(),
                            resources: ResourceRequirements::default(),
                        },
                        PipelineStep {
                            id: format!("{label}.commit"),
                            node_type: "core.commit".to_owned(),
                            kind: WorkflowNodeKind::Commit,
                            inputs: BTreeMap::from([(
                                "candidates".to_owned(),
                                PipelineSource::Step {
                                    step_id: filter_id,
                                    port: "detections".to_owned(),
                                    artifact_type: ArtifactKind::DetectionSet,
                                },
                            )]),
                            outputs: BTreeMap::new(),
                            model_binding: None,
                            skill_binding: None,
                            parameters: BTreeMap::new(),
                            validators: Vec::new(),
                            refiners: Vec::new(),
                            fallback: None,
                            retry_policy: RetryPolicy::default(),
                            review_gate: ReviewGate::default(),
                            resources: ResourceRequirements::default(),
                        },
                    ],
                }
            })
            .collect();
        LabelWorkflowComposition {
            schema_version: LABEL_PIPELINE_SCHEMA_VERSION,
            shared_stages: vec![SharedWorkflowStage {
                id: "shared-vision".to_owned(),
                name: "Shared vision".to_owned(),
                steps: vec![detector],
            }],
            label_pipelines: pipelines,
        }
    }

    #[test]
    fn one_shared_detector_compiles_once_for_three_label_pipelines() {
        let composition = shared_composition();
        let (nodes, models) = registries();
        let report =
            LabelPipelineStaticValidator.validate(&composition, &project(), &nodes, &models);
        assert!(report.valid, "{:#?}", report.issues);

        let draft =
            composition.compile_draft("generic", "shared detector", BTreeMap::new(), Utc::now());
        assert_eq!(
            draft
                .nodes
                .iter()
                .filter(|node| node.id == "shared.detector")
                .count(),
            1
        );
        assert_eq!(
            draft
                .edges
                .iter()
                .filter(|edge| edge.from_node == "shared.detector")
                .count(),
            3
        );
        assert_eq!(draft.label_pipeline, Some(composition));
    }

    #[test]
    fn pipeline_type_error_and_unknown_label_are_blocking() {
        let mut composition = shared_composition();
        composition.label_pipelines[0].target_label = LabelId::from("not-in-schema");
        let source = composition.label_pipelines[0].steps[0]
            .inputs
            .get_mut("detections")
            .expect("input");
        if let PipelineSource::SharedStage { artifact_type, .. } = source {
            *artifact_type = ArtifactKind::CropSet;
        }
        let (nodes, models) = registries();
        let report =
            LabelPipelineStaticValidator.validate(&composition, &project(), &nodes, &models);
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| {
            issue.code == "unknown_target_label" && issue.path.ends_with("target_label")
        }));
        assert!(
            report
                .issues
                .iter()
                .any(|issue| { issue.code == "pipeline_artifact_type_mismatch" && issue.blocking })
        );
    }

    #[test]
    fn crop_and_classification_preserve_parent_subject_references() {
        let detections = ArtifactRef {
            artifact_id: "detections-1".to_owned(),
            source_node: "shared.detector".to_owned(),
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        };
        let crops = ArtifactRef {
            artifact_id: "crops-1".to_owned(),
            source_node: "core.crop".to_owned(),
            port: "crops".to_owned(),
            artifact_type: ArtifactKind::CropSet,
            item_id: None,
        };
        let crop_set = CropSetArtifact {
            reference: crops.clone(),
            image_id: ImageId::new(),
            source_detections: detections.clone(),
            crops: vec![Crop {
                id: "crop-1".to_owned(),
                parent: detections.item("detection-1"),
                rect: NormalizedRect::new(0.1, 0.2, 0.3, 0.4).expect("rect"),
                source_width: 0,
                source_height: 0,
                crop_width: 0,
                crop_height: 0,
                padding: 0.05,
                mime_type: None,
                blob_ref: Some("cache://crop-1".to_owned()),
                cache_key: None,
            }],
        };
        crop_set.validate().expect("valid CropSet");

        let classifications = ClassificationSetArtifact {
            reference: ArtifactRef {
                artifact_id: "classifications-1".to_owned(),
                source_node: "classifier".to_owned(),
                port: "classifications".to_owned(),
                artifact_type: ArtifactKind::ClassificationSet,
                item_id: None,
            },
            image_id: crop_set.image_id,
            model_binding: "classifier".to_owned(),
            validation_state: ArtifactValidationState::Unvalidated,
            classifications: vec![Classification {
                id: "classification-1".to_owned(),
                subject: crops.item("crop-1"),
                parent: Some(detections.item("detection-1")),
                label: LabelId::from("person"),
                confidence: 0.91,
                scores: BTreeMap::new(),
            }],
        };
        classifications.validate().expect("valid ClassificationSet");
        assert_eq!(
            classifications.classifications[0]
                .subject
                .item_id
                .as_deref(),
            Some("crop-1")
        );
        assert_eq!(
            crop_set.crops[0].parent.item_id.as_deref(),
            Some("detection-1")
        );
    }

    #[test]
    fn fan_out_and_fan_in_join_by_parent_reference() {
        let image_id = ImageId::new();
        let detection_ref = ArtifactRef {
            artifact_id: "detections".to_owned(),
            source_node: "shared.detector".to_owned(),
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        };
        let detections = DetectionSetArtifact {
            schema_version: DETECTION_ARTIFACT_SCHEMA_VERSION,
            reference: detection_ref.clone(),
            image_id,
            model_binding: "detector".to_owned(),
            validation_state: ArtifactValidationState::Unvalidated,
            detections: vec![
                Detection::from_source(
                    "d1",
                    None,
                    Some("0".to_owned()),
                    None,
                    NormalizedRect::new(0.1, 0.2, 0.3, 0.4).expect("rect"),
                    DetectionScore::relative(0.9).expect("score"),
                    DetectionSource {
                        model_id: "detector".to_owned(),
                        capability: VisionCapability::ObjectDetection,
                        artifact_id: "detections".to_owned(),
                    },
                )
                .expect("detection"),
            ],
            metadata: BTreeMap::new(),
        };
        let crop_ref = ArtifactRef {
            artifact_id: "crops".to_owned(),
            source_node: "core.crop".to_owned(),
            port: "crops".to_owned(),
            artifact_type: ArtifactKind::CropSet,
            item_id: None,
        };
        let crops = CropSetArtifact::fan_out(crop_ref.clone(), &detections, 0.05, |detection| {
            Some(format!("cache://{}", detection.detection_id))
        })
        .expect("fan-out");
        assert_eq!(crops.crops[0].parent, detection_ref.item("d1"));

        let classifications = ClassificationSetArtifact {
            reference: ArtifactRef {
                artifact_id: "classifications".to_owned(),
                source_node: "classifier".to_owned(),
                port: "classifications".to_owned(),
                artifact_type: ArtifactKind::ClassificationSet,
                item_id: None,
            },
            image_id,
            model_binding: "classifier".to_owned(),
            validation_state: ArtifactValidationState::Unvalidated,
            classifications: vec![Classification {
                id: "c1".to_owned(),
                subject: crop_ref.item("crop:d1"),
                parent: Some(detection_ref.item("d1")),
                label: LabelId::from("upright"),
                confidence: 0.8,
                scores: BTreeMap::new(),
            }],
        };
        let candidates = AnnotationCandidateSet::fan_in(
            ArtifactRef {
                artifact_id: "candidates".to_owned(),
                source_node: "attach".to_owned(),
                port: "candidates".to_owned(),
                artifact_type: ArtifactKind::AnnotationCandidateSet,
                item_id: None,
            },
            &detections,
            &classifications,
            &TaskId::from("objects"),
            &BTreeMap::from([(LabelId::from("upright"), LabelId::from("person"))]),
        )
        .expect("fan-in");
        assert_eq!(candidates.candidates.len(), 1);
        assert_eq!(candidates.candidates[0].label, LabelId::from("person"));
        assert_eq!(candidates.candidates[0].subject, detection_ref.item("d1"));
        assert!(matches!(
            candidates.candidates[0].value,
            Some(VisionArtifactValue::BoundingBox { .. })
        ));
    }

    #[test]
    fn detection_scores_preserve_unknown_and_reject_invalid_numbers() {
        assert_eq!(DetectionScore::not_provided().comparable_confidence(), None);
        assert_eq!(
            DetectionScore::new(Some(0.8), ScoreSemantics::RankingScore)
                .expect("ranking score")
                .comparable_confidence(),
            None
        );
        assert_eq!(
            DetectionScore::relative(0.8)
                .expect("relative confidence")
                .comparable_confidence(),
            Some(0.8)
        );
        assert!(DetectionScore::relative(f32::NAN).is_err());
        assert!(DetectionScore::relative(f32::INFINITY).is_err());
        assert!(DetectionScore::relative(-0.01).is_err());
        assert!(DetectionScore::relative(1.01).is_err());
        assert!(DetectionScore::new(Some(0.8), ScoreSemantics::NotProvided).is_err());
    }

    #[test]
    fn legacy_detection_json_migrates_without_losing_lineage() {
        let image_id = ImageId::new();
        let legacy = serde_json::json!({
            "reference": {
                "artifact_id": "legacy-set",
                "source_node": "legacy-detector",
                "port": "detections",
                "artifact_type": "detection_set",
                "item_id": null
            },
            "image_id": image_id,
            "model_binding": "legacy-model",
            "validation_state": "unvalidated",
            "detections": [{
                "id": "legacy-1",
                "class_id": "target_object",
                "label": "ball",
                "rect": [0.1, 0.2, 0.3, 0.4],
                "confidence": 0.91,
                "attributes": {"fixture": true}
            }],
            "metadata": {}
        });
        let migrated: DetectionSetArtifact =
            serde_json::from_value(legacy).expect("legacy DetectionSet migration");
        migrated.validate().expect("migrated artifact validates");
        assert_eq!(migrated.schema_version, DETECTION_ARTIFACT_SCHEMA_VERSION);
        let detection = &migrated.detections[0];
        assert_eq!(detection.detection_id, "legacy-1");
        assert_eq!(detection.model_label.as_deref(), Some("target_object"));
        assert_eq!(detection.project_label, Some(LabelId::from("ball")));
        assert_eq!(detection.score.value, Some(0.91));
        assert_eq!(detection.score.semantics, ScoreSemantics::Unknown);
        assert_eq!(detection.source_model_id, "legacy-model");
        assert_eq!(detection.evidence.len(), 1);
        assert_eq!(detection.evidence[0].source_artifact_id, "legacy-set");

        let current = serde_json::to_value(&migrated).expect("current serialization");
        assert_eq!(current["schema_version"], DETECTION_ARTIFACT_SCHEMA_VERSION);
        assert!(current["detections"][0].get("confidence").is_none());
        assert!(current["detections"][0].get("rect").is_none());
        assert_eq!(current["detections"][0]["detection_id"], "legacy-1");

        let mut future = current;
        future["schema_version"] = serde_json::json!(DETECTION_ARTIFACT_SCHEMA_VERSION + 1);
        assert!(serde_json::from_value::<DetectionSetArtifact>(future).is_err());
    }

    #[test]
    fn missing_detector_score_stays_not_provided() {
        let value = serde_json::json!({
            "schema_version": DETECTION_ARTIFACT_SCHEMA_VERSION,
            "reference": {
                "artifact_id": "open-vocabulary-set",
                "source_node": "open-vocabulary-detector",
                "port": "detections",
                "artifact_type": "detection_set",
                "item_id": null
            },
            "image_id": ImageId::new(),
            "model_binding": "open-vocabulary-model",
            "detections": [{
                "detection_id": "phrase-1",
                "query_id": "query-target-object",
                "project_label": "ball",
                "bbox": [0.2, 0.3, 0.1, 0.1],
                "source_model_id": "open-vocabulary-model",
                "source_capability": "open_vocabulary_detection"
            }]
        });
        let artifact: DetectionSetArtifact =
            serde_json::from_value(value).expect("score-less DetectionSet");
        artifact.validate().expect("valid score-less artifact");
        assert_eq!(artifact.detections[0].score, DetectionScore::not_provided());
        assert_eq!(
            artifact.detections[0].evidence[0].score,
            DetectionScore::not_provided()
        );
    }

    #[test]
    fn candidate_cluster_round_trip_retains_independent_model_evidence() {
        let bbox_a = NormalizedRect::new(0.1, 0.2, 0.3, 0.3).expect("bbox A");
        let bbox_b = NormalizedRect::new(0.11, 0.19, 0.29, 0.31).expect("bbox B");
        let evidence = vec![
            DetectionEvidence {
                source_model_id: "specialist-model".to_owned(),
                source_artifact_id: "specialist-set".to_owned(),
                bbox: bbox_a,
                score: DetectionScore::relative(0.87).expect("score"),
                query_id: None,
                model_label: Some("sports ball".to_owned()),
                project_label: Some(LabelId::from("ball")),
                source_capability: VisionCapability::ObjectDetection,
                raw_output_ref: Some(StoredPayloadRef {
                    id: "payload-1".to_owned(),
                    media_type: "application/json".to_owned(),
                    sha256: "a".repeat(64),
                    size_bytes: 128,
                }),
            },
            DetectionEvidence {
                source_model_id: "open-vocabulary-model".to_owned(),
                source_artifact_id: "open-vocabulary-set".to_owned(),
                bbox: bbox_b,
                score: DetectionScore::not_provided(),
                query_id: Some("query-target-object".to_owned()),
                model_label: None,
                project_label: Some(LabelId::from("ball")),
                source_capability: VisionCapability::OpenVocabularyDetection,
                raw_output_ref: None,
            },
        ];
        let source = |artifact_id: &str, source_node: &str| ArtifactRef {
            artifact_id: artifact_id.to_owned(),
            source_node: source_node.to_owned(),
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        };
        let artifact = CandidateClusterSetArtifact {
            reference: ArtifactRef {
                artifact_id: "cluster-set".to_owned(),
                source_node: "core.match_detection_sets".to_owned(),
                port: "candidates".to_owned(),
                artifact_type: ArtifactKind::CandidateClusterSet,
                item_id: None,
            },
            image_id: ImageId::new(),
            source_detection_sets: vec![
                source("specialist-set", "specialist"),
                source("open-vocabulary-set", "open-vocabulary"),
            ],
            validation_state: ArtifactValidationState::Unvalidated,
            candidates: vec![CandidateCluster {
                id: "cluster-1".to_owned(),
                target_label: LabelId::from("ball"),
                representative_bbox: bbox_a,
                members: evidence,
                agreement: CandidateAgreement::MultiSourceAgreement {
                    minimum_iou: 0.8,
                    mean_iou: 0.84,
                },
            }],
        };
        artifact.validate().expect("valid CandidateClusterSet");
        let json = serde_json::to_value(&artifact).expect("serialize cluster");
        let stored: CandidateClusterSetArtifact =
            serde_json::from_value(json).expect("deserialize cluster");
        assert_eq!(stored, artifact);
        assert_eq!(stored.candidates[0].members[0].score.value, Some(0.87));
        assert_eq!(
            stored.candidates[0].members[1].score,
            DetectionScore::not_provided()
        );
    }

    #[test]
    fn invalid_raw_payload_reference_is_rejected() {
        let mut detection = Detection::from_source(
            "d1",
            None,
            Some("ball".to_owned()),
            Some(LabelId::from("ball")),
            NormalizedRect::new(0.1, 0.1, 0.2, 0.2).expect("bbox"),
            DetectionScore::relative(0.7).expect("score"),
            DetectionSource {
                model_id: "model".to_owned(),
                capability: VisionCapability::ObjectDetection,
                artifact_id: "set".to_owned(),
            },
        )
        .expect("detection");
        detection.evidence[0].raw_output_ref = Some(StoredPayloadRef {
            id: "payload".to_owned(),
            media_type: "application/json".to_owned(),
            sha256: "not-a-sha256".to_owned(),
            size_bytes: 1,
        });
        let artifact = DetectionSetArtifact {
            schema_version: DETECTION_ARTIFACT_SCHEMA_VERSION,
            reference: ArtifactRef {
                artifact_id: "set".to_owned(),
                source_node: "detector".to_owned(),
                port: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                item_id: None,
            },
            image_id: ImageId::new(),
            model_binding: "model".to_owned(),
            validation_state: ArtifactValidationState::Unvalidated,
            detections: vec![detection],
            metadata: BTreeMap::new(),
        };
        assert!(artifact.validate().is_err());
    }
}
