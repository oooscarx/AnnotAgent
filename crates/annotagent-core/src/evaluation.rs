//! Structured annotation failure and geometry-quality evidence.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    AnnotationId, AnnotationSnapshot, AnnotationValue, ArtifactId, ArtifactKind, ArtifactRef,
    DetectionArtifactItem, DetectionScore, GeometryCalibrationId, GeometryCalibrationStatus,
    GeometryCalibrationThresholds, GeometryQualityReportId, GeometrySemantics, ImageId, LabelId,
    ModelProfileId, NodeDefinitionId, NodeId, NormalizedRect, ProjectId, RunId, TaskId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationFailureClass {
    InfrastructureFailure,
    ProviderFailure,
    NoCandidate,
    SemanticError,
    GeometryError,
    MissingScore,
    DomainRisk,
    InvalidArtifact,
    BudgetLimit,
    InsufficientEvidence,
}

/// Classifies an already-structured Runtime/validation code. This is intentionally conservative:
/// unknown errors remain invalid-artifact failures instead of being guessed as geometry errors.
#[must_use]
pub fn classify_annotation_failure(code: &str, message: &str) -> AnnotationFailureClass {
    let evidence = format!("{code} {message}").to_ascii_lowercase();
    if contains_any(
        &evidence,
        &[
            "insufficient_evidence",
            "insufficient evidence",
            "missing_reference",
        ],
    ) {
        AnnotationFailureClass::InsufficientEvidence
    } else if contains_any(&evidence, &["budget", "cost_limit", "token_limit"]) {
        AnnotationFailureClass::BudgetLimit
    } else if contains_any(
        &evidence,
        &[
            "provider",
            "api key",
            "api_key",
            "credential",
            "rate_limit",
            "rate limit",
            "model request",
            "model timeout",
            "qwen",
            "openai",
        ],
    ) {
        AnnotationFailureClass::ProviderFailure
    } else if contains_any(
        &evidence,
        &[
            "worker",
            "unreachable",
            "connection",
            "connect",
            "service unavailable",
            "backend unavailable",
            "not started",
        ],
    ) {
        AnnotationFailureClass::InfrastructureFailure
    } else if contains_any(
        &evidence,
        &[
            "no_candidate",
            "no candidate",
            "empty_detection",
            "empty result",
        ],
    ) {
        AnnotationFailureClass::NoCandidate
    } else if contains_any(
        &evidence,
        &["missing_score", "missing score", "score_not_provided"],
    ) {
        AnnotationFailureClass::MissingScore
    } else if contains_any(
        &evidence,
        &[
            "semantic",
            "wrong_label",
            "label_conflict",
            "classification",
            "false_positive",
        ],
    ) {
        AnnotationFailureClass::SemanticError
    } else if contains_any(
        &evidence,
        &[
            "geometry",
            "bounding_box",
            "bbox",
            "mask",
            "iou",
            "aspect_ratio",
            "center_shift",
        ],
    ) {
        AnnotationFailureClass::GeometryError
    } else if contains_any(
        &evidence,
        &["domain", "correction_risk", "validator", "hard_negative"],
    ) {
        AnnotationFailureClass::DomainRisk
    } else {
        AnnotationFailureClass::InvalidArtifact
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

/// Lightweight quality observations emitted while a candidate is still inside a Dry Run.
///
/// This intentionally has no Project/Run identity. Durable human-reference evidence uses
/// [`GeometryQualityReport`] instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateGeometryQualityReport {
    /// Pipeline Artifact IDs are stable strings and may not be UUIDs.
    pub artifact_id: String,
    pub geometry_semantics: GeometrySemantics,
    pub clipped_to_image: bool,
    pub aspect_ratio_outlier: bool,
    pub area_ratio: Option<f32>,
    pub foreground_occupancy: Option<f32>,
    pub edge_support: Option<f32>,
    pub mask_support: Option<f32>,
    pub center_shift_from_refiner: Option<f32>,
    pub area_change_from_refiner: Option<f32>,
    pub iou_with_refiner: Option<f32>,
    pub manual_center_shift: Option<f32>,
    pub manual_area_change: Option<f32>,
    pub historical_correction_rate: Option<f32>,
    pub issue_codes: Vec<String>,
}

impl CandidateGeometryQualityReport {
    #[must_use]
    pub fn from_detection(
        artifact_id: impl Into<String>,
        detection: &DetectionArtifactItem,
    ) -> Self {
        let bbox = detection.bbox;
        let aspect_ratio = bbox.width() / bbox.height();
        let aspect_ratio_outlier = !(0.2..=5.0).contains(&aspect_ratio);
        let clipped_to_image = detection
            .attributes
            .get("clipped_to_image")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let mut report = Self {
            artifact_id: artifact_id.into(),
            geometry_semantics: detection.geometry_semantics,
            clipped_to_image,
            aspect_ratio_outlier,
            area_ratio: Some(bbox.area()),
            foreground_occupancy: None,
            edge_support: None,
            mask_support: None,
            center_shift_from_refiner: None,
            area_change_from_refiner: None,
            iou_with_refiner: None,
            manual_center_shift: None,
            manual_area_change: None,
            historical_correction_rate: None,
            issue_codes: Vec::new(),
        };
        if report.clipped_to_image {
            report
                .issue_codes
                .push("geometry_clipped_to_image".to_owned());
        }
        if touches_image_boundary(bbox) {
            report
                .issue_codes
                .push("geometry_touches_image_boundary".to_owned());
        }
        if aspect_ratio_outlier {
            report
                .issue_codes
                .push("geometry_aspect_ratio_outlier".to_owned());
        }
        if bbox.area() < 0.000_1 {
            report.issue_codes.push("geometry_very_small".to_owned());
        } else if bbox.area() > 0.8 {
            report.issue_codes.push("geometry_very_large".to_owned());
        }
        if let Some(refinement) = detection.attributes.get("geometry_refinement") {
            let original = refinement
                .get("original_bbox")
                .cloned()
                .and_then(|value| serde_json::from_value::<NormalizedRect>(value).ok());
            let refined = refinement
                .get("refined_bbox")
                .cloned()
                .and_then(|value| serde_json::from_value::<NormalizedRect>(value).ok())
                .unwrap_or(bbox);
            if let Some(original) = original {
                report.center_shift_from_refiner = Some(center_shift(original, refined));
                report.area_change_from_refiner = Some(relative_area_change(original, refined));
                report.iou_with_refiner = Some(rect_iou(original, refined));
            } else {
                report
                    .issue_codes
                    .push("refiner_lineage_invalid".to_owned());
            }
        }
        report
    }

    #[must_use]
    pub fn has_geometry_issue(&self) -> bool {
        self.aspect_ratio_outlier
            || self
                .issue_codes
                .iter()
                .any(|code| code.starts_with("geometry_"))
    }
}

pub const GEOMETRY_REFINEMENT_TRACE_SCHEMA_VERSION: u32 = 1;

/// Exact item lineage retained by a prompted geometry-refinement operation. This trace is an
/// observable Artifact contract, not a claim that the refined geometry is correct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryRefinementTrace {
    pub schema_version: u32,
    pub method: String,
    pub source_detection: ArtifactRef,
    pub box_prompt: ArtifactRef,
    pub mask: ArtifactRef,
    pub refined_detection: ArtifactRef,
    pub original_bbox: NormalizedRect,
    pub refined_bbox: NormalizedRect,
    pub mask_score: DetectionScore,
}

impl GeometryRefinementTrace {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != GEOMETRY_REFINEMENT_TRACE_SCHEMA_VERSION {
            return Err("unsupported geometry refinement trace schema version".to_owned());
        }
        if self.method != "mask_to_bbox" {
            return Err("geometry refinement trace method must be mask_to_bbox".to_owned());
        }
        for (name, reference, kind) in [
            (
                "source_detection",
                &self.source_detection,
                ArtifactKind::DetectionSet,
            ),
            ("box_prompt", &self.box_prompt, ArtifactKind::BoxPromptSet),
            ("mask", &self.mask, ArtifactKind::MaskSet),
            (
                "refined_detection",
                &self.refined_detection,
                ArtifactKind::DetectionSet,
            ),
        ] {
            if reference.artifact_type != kind
                || reference.artifact_id.trim().is_empty()
                || reference.source_node.trim().is_empty()
                || reference.port.trim().is_empty()
                || reference
                    .item_id
                    .as_deref()
                    .is_none_or(|item| item.trim().is_empty())
            {
                return Err(format!("{name} must identify one {kind:?} item"));
            }
        }
        self.mask_score.validate()?;
        Ok(())
    }
}

/// Thresholds for comparing a coarse prompt box with a prompted-segmentation result. Defaults are
/// deliberately broad enough to permit useful tightening while routing large changes to Review.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GeometryRefinementThresholds {
    pub minimum_coarse_refined_iou: f32,
    pub maximum_center_shift: f32,
    pub minimum_area_ratio: f32,
    pub maximum_area_ratio: f32,
    pub minimum_mask_score: Option<f32>,
}

impl Default for GeometryRefinementThresholds {
    fn default() -> Self {
        Self {
            minimum_coarse_refined_iou: 0.20,
            maximum_center_shift: 0.15,
            minimum_area_ratio: 0.20,
            maximum_area_ratio: 1.25,
            minimum_mask_score: None,
        }
    }
}

impl GeometryRefinementThresholds {
    pub fn validate(self) -> Result<(), String> {
        if !self.minimum_coarse_refined_iou.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_coarse_refined_iou)
            || !self.maximum_center_shift.is_finite()
            || !(0.0..=2.0_f32.sqrt()).contains(&self.maximum_center_shift)
            || !self.minimum_area_ratio.is_finite()
            || self.minimum_area_ratio <= 0.0
            || !self.maximum_area_ratio.is_finite()
            || self.maximum_area_ratio < self.minimum_area_ratio
            || self
                .minimum_mask_score
                .is_some_and(|score| !score.is_finite() || !(0.0..=1.0).contains(&score))
        {
            return Err("invalid geometry refinement thresholds".to_owned());
        }
        Ok(())
    }
}

/// Structured comparison consumed by `core.geometry_decision`. It measures a refiner's change but
/// never upgrades the model's semantic score into geometry evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryRefinementEvaluation {
    pub trace: GeometryRefinementTrace,
    pub thresholds: GeometryRefinementThresholds,
    pub coarse_refined_iou: f32,
    pub normalized_center_shift: f32,
    pub area_ratio: f32,
    pub width_ratio: f32,
    pub height_ratio: f32,
    pub issue_codes: Vec<GeometryIssueCode>,
    pub stable: bool,
}

impl GeometryRefinementEvaluation {
    pub fn validate(&self) -> Result<(), String> {
        let expected = evaluate_geometry_refinement(self.trace.clone(), self.thresholds)?;
        if &expected != self {
            return Err(
                "geometry refinement evaluation does not match its trace and thresholds".to_owned(),
            );
        }
        Ok(())
    }
}

pub fn evaluate_geometry_refinement(
    trace: GeometryRefinementTrace,
    thresholds: GeometryRefinementThresholds,
) -> Result<GeometryRefinementEvaluation, String> {
    trace.validate()?;
    thresholds.validate()?;
    let original_area = trace.original_bbox.area();
    if original_area <= f32::EPSILON {
        return Err("geometry refinement original bbox has zero area".to_owned());
    }
    let coarse_refined_iou = rect_iou(trace.original_bbox, trace.refined_bbox);
    let normalized_center_shift = center_shift(trace.original_bbox, trace.refined_bbox);
    let area_ratio = trace.refined_bbox.area() / original_area;
    let width_ratio = trace.refined_bbox.width() / trace.original_bbox.width();
    let height_ratio = trace.refined_bbox.height() / trace.original_bbox.height();
    let mut issue_codes = Vec::new();
    if coarse_refined_iou < thresholds.minimum_coarse_refined_iou {
        issue_codes.push(GeometryIssueCode::RefinerConflict);
    }
    if normalized_center_shift > thresholds.maximum_center_shift {
        issue_codes.push(GeometryIssueCode::CenterShift);
    }
    if area_ratio < thresholds.minimum_area_ratio {
        issue_codes.push(GeometryIssueCode::TooTight);
        issue_codes.push(GeometryIssueCode::PartialObject);
    }
    if area_ratio > thresholds.maximum_area_ratio {
        issue_codes.push(GeometryIssueCode::TooLoose);
        issue_codes.push(GeometryIssueCode::IncludesBackground);
    }
    if let Some(minimum_mask_score) = thresholds.minimum_mask_score
        && (trace.mask_score.semantics.is_semantic()
            || trace
                .mask_score
                .value
                .is_none_or(|score| score < minimum_mask_score))
    {
        issue_codes.push(GeometryIssueCode::InsufficientEvidence);
    }
    issue_codes.sort_unstable();
    issue_codes.dedup();
    Ok(GeometryRefinementEvaluation {
        trace,
        thresholds,
        coarse_refined_iou,
        normalized_center_shift,
        area_ratio,
        width_ratio,
        height_ratio,
        stable: issue_codes.is_empty(),
        issue_codes,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryEvidenceSource {
    HumanCorrection,
    PromptedSegmentation,
    SpecialistDetector,
    ImportedReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryIssueCode {
    TooLoose,
    TooTight,
    CenterShift,
    WidthError,
    HeightError,
    AspectRatioError,
    PartialObject,
    IncludesBackground,
    RefinerConflict,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeometryCorrectionReason {
    TooLoose,
    TooTight,
    Shifted,
    WrongObject,
    MissedObject,
    Duplicate,
    WrongLabel,
    DomainRisk(String),
    Other,
}

impl GeometryCorrectionReason {
    #[must_use]
    pub fn from_code(code: &str) -> Self {
        match code {
            "too_loose" => Self::TooLoose,
            "too_tight" => Self::TooTight,
            "shifted" => Self::Shifted,
            "wrong_object" | "not_target" => Self::WrongObject,
            "missed_object" => Self::MissedObject,
            "duplicate" => Self::Duplicate,
            "wrong_label" => Self::WrongLabel,
            "other" | "" => Self::Other,
            domain_code => Self::DomainRisk(domain_code.to_owned()),
        }
    }

    #[must_use]
    pub fn as_code(&self) -> &str {
        match self {
            Self::TooLoose => "too_loose",
            Self::TooTight => "too_tight",
            Self::Shifted => "shifted",
            Self::WrongObject => "wrong_object",
            Self::MissedObject => "missed_object",
            Self::Duplicate => "duplicate",
            Self::WrongLabel => "wrong_label",
            Self::DomainRisk(code) => code,
            Self::Other => "other",
        }
    }
}

impl Serialize for GeometryCorrectionReason {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_code())
    }
}

impl<'de> Deserialize<'de> for GeometryCorrectionReason {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let code = String::deserialize(deserializer)?;
        Ok(Self::from_code(&code))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectSizeBucket {
    Small,
    Medium,
    Large,
}

impl ObjectSizeBucket {
    /// COCO-compatible pixel-area buckets keep small objects visible in aggregate reports.
    #[must_use]
    pub const fn from_pixel_area(area: f32) -> Self {
        if area < 32.0 * 32.0 {
            Self::Small
        } else if area < 96.0 * 96.0 {
            Self::Medium
        } else {
            Self::Large
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeometrySnapshot {
    pub rect: NormalizedRect,
    pub image_width: u32,
    pub image_height: u32,
}

impl GeometrySnapshot {
    #[must_use]
    pub fn pixel_area(self) -> f32 {
        self.rect.area() * self.image_width as f32 * self.image_height as f32
    }

    #[must_use]
    pub fn size_bucket(self) -> ObjectSizeBucket {
        ObjectSizeBucket::from_pixel_area(self.pixel_area())
    }
}

/// Durable, scoped geometry evidence. Unlike semantic confidence, every numeric field here is
/// derived from geometry or an explicit reference source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryQualityReport {
    pub id: GeometryQualityReportId,
    pub project_id: ProjectId,
    pub image_id: ImageId,
    pub candidate_artifact_id: ArtifactId,
    pub reference_artifact_id: Option<ArtifactId>,
    pub source: GeometryEvidenceSource,
    pub iou: Option<f32>,
    pub normalized_center_shift: Option<f32>,
    pub pixel_center_shift: Option<f32>,
    pub predicted_area: Option<f32>,
    pub reference_area: Option<f32>,
    pub area_ratio: Option<f32>,
    pub width_ratio: Option<f32>,
    pub height_ratio: Option<f32>,
    pub foreground_occupancy: Option<f32>,
    pub mask_support: Option<f32>,
    pub edge_support: Option<f32>,
    pub size_bucket: Option<ObjectSizeBucket>,
    pub issue_codes: Vec<GeometryIssueCode>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryCorrectionEvidence {
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub image_id: ImageId,
    pub annotation_id: AnnotationId,
    pub source_node_id: NodeId,
    /// Legacy Runs may not contain a revisioned Model Profile. Such evidence is still retained,
    /// but is ineligible for calibration and carries `InsufficientEvidence` in its report.
    pub source_model_profile_id: Option<ModelProfileId>,
    pub source_model_revision: Option<u64>,
    pub original_geometry: GeometrySnapshot,
    pub corrected_geometry: GeometrySnapshot,
    pub reason: GeometryCorrectionReason,
    pub quality_report_id: GeometryQualityReportId,
    pub created_at: DateTime<Utc>,
}

impl GeometryCorrectionEvidence {
    #[must_use]
    pub const fn calibration_eligible(&self) -> bool {
        self.source_model_profile_id.is_some() && self.source_model_revision.is_some()
    }
}

pub struct GeometryCorrectionInput {
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub image_id: ImageId,
    pub annotation_id: AnnotationId,
    pub source_node_id: NodeId,
    pub source_model_profile_id: Option<ModelProfileId>,
    pub source_model_revision: Option<u64>,
    pub candidate_artifact_id: ArtifactId,
    pub reference_artifact_id: ArtifactId,
    pub original_geometry: GeometrySnapshot,
    pub corrected_geometry: GeometrySnapshot,
    pub reason: GeometryCorrectionReason,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryCalibrationKey {
    pub project_id: ProjectId,
    pub task_id: TaskId,
    pub label_id: Option<LabelId>,
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,
    pub node_definition_id: NodeDefinitionId,
    pub node_config_hash: String,
    pub prompt_version: Option<String>,
    pub preprocessing_hash: String,
    pub dataset_profile_revision: String,
    /// Label semantics are independent from image distribution and invalidate calibration.
    pub label_schema_hash: String,
    /// Downstream refiners and geometry conversion are part of the calibrated method.
    pub refinement_hash: String,
}

impl GeometryCalibrationKey {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("node_definition_id", self.node_definition_id.as_str()),
            ("node_config_hash", self.node_config_hash.as_str()),
            ("preprocessing_hash", self.preprocessing_hash.as_str()),
            (
                "dataset_profile_revision",
                self.dataset_profile_revision.as_str(),
            ),
            ("label_schema_hash", self.label_schema_hash.as_str()),
            ("refinement_hash", self.refinement_hash.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("geometry calibration {name} cannot be empty"));
            }
        }
        if self.model_profile_revision == 0 {
            return Err(
                "geometry calibration requires a positive Model Profile revision".to_owned(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryCalibrationStaleness {
    Project,
    Task,
    Label,
    ModelProfile,
    ModelRevision,
    NodeDefinition,
    NodeConfiguration,
    Prompt,
    Preprocessing,
    DatasetProfile,
    LabelSchema,
    Refinement,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryCalibrationReport {
    pub id: GeometryCalibrationId,
    pub key: GeometryCalibrationKey,
    pub status: GeometryCalibrationStatus,
    pub sample_count: u32,
    pub small_object_sample_count: u32,
    pub median_iou: Option<f32>,
    pub p10_iou: Option<f32>,
    pub median_center_shift: Option<f32>,
    pub p90_center_shift: Option<f32>,
    pub median_area_ratio_error: Option<f32>,
    pub manual_adjustment_rate: Option<f32>,
    pub too_loose_rate: Option<f32>,
    pub too_tight_rate: Option<f32>,
    pub thresholds: GeometryCalibrationThresholds,
    pub evidence_run_ids: Vec<RunId>,
    pub evidence_quality_report_ids: Vec<GeometryQualityReportId>,
    pub created_at: DateTime<Utc>,
}

impl GeometryCalibrationReport {
    #[must_use]
    pub fn staleness_reasons(
        &self,
        current: &GeometryCalibrationKey,
    ) -> Vec<GeometryCalibrationStaleness> {
        let mut reasons = Vec::new();
        let checks = [
            (
                self.key.project_id != current.project_id,
                GeometryCalibrationStaleness::Project,
            ),
            (
                self.key.task_id != current.task_id,
                GeometryCalibrationStaleness::Task,
            ),
            (
                self.key.label_id != current.label_id,
                GeometryCalibrationStaleness::Label,
            ),
            (
                self.key.model_profile_id != current.model_profile_id,
                GeometryCalibrationStaleness::ModelProfile,
            ),
            (
                self.key.model_profile_revision != current.model_profile_revision,
                GeometryCalibrationStaleness::ModelRevision,
            ),
            (
                self.key.node_definition_id != current.node_definition_id,
                GeometryCalibrationStaleness::NodeDefinition,
            ),
            (
                self.key.node_config_hash != current.node_config_hash,
                GeometryCalibrationStaleness::NodeConfiguration,
            ),
            (
                self.key.prompt_version != current.prompt_version,
                GeometryCalibrationStaleness::Prompt,
            ),
            (
                self.key.preprocessing_hash != current.preprocessing_hash,
                GeometryCalibrationStaleness::Preprocessing,
            ),
            (
                self.key.dataset_profile_revision != current.dataset_profile_revision,
                GeometryCalibrationStaleness::DatasetProfile,
            ),
            (
                self.key.label_schema_hash != current.label_schema_hash,
                GeometryCalibrationStaleness::LabelSchema,
            ),
            (
                self.key.refinement_hash != current.refinement_hash,
                GeometryCalibrationStaleness::Refinement,
            ),
        ];
        reasons.extend(
            checks
                .into_iter()
                .filter_map(|(changed, reason)| changed.then_some(reason)),
        );
        reasons
    }

    #[must_use]
    pub fn effective_status(&self, current: &GeometryCalibrationKey) -> GeometryCalibrationStatus {
        if self.staleness_reasons(current).is_empty() {
            self.status
        } else {
            GeometryCalibrationStatus::Stale
        }
    }

    #[must_use]
    pub fn exact_key_match(&self, current: &GeometryCalibrationKey) -> bool {
        self.key == *current
    }
}

#[must_use]
pub fn evaluate_geometry_calibration(
    key: GeometryCalibrationKey,
    thresholds: GeometryCalibrationThresholds,
    evidence: &[(GeometryQualityReport, GeometryCorrectionEvidence)],
    evaluated_sample_count: u32,
    created_at: DateTime<Utc>,
) -> GeometryCalibrationReport {
    let sample_count = u32::try_from(evidence.len()).unwrap_or(u32::MAX);
    let small_object_sample_count = evidence
        .iter()
        .filter(|(report, _)| report.size_bucket == Some(ObjectSizeBucket::Small))
        .count()
        .try_into()
        .unwrap_or(u32::MAX);
    let ious = evidence
        .iter()
        .filter_map(|(report, _)| report.iou)
        .collect::<Vec<_>>();
    let center_shifts = evidence
        .iter()
        .filter_map(|(report, _)| report.normalized_center_shift)
        .collect::<Vec<_>>();
    let area_ratios = evidence
        .iter()
        .filter_map(|(report, _)| report.area_ratio)
        .collect::<Vec<_>>();
    let median_iou = percentile(&ious, 0.5);
    let p10_iou = percentile(&ious, 0.1);
    let median_center_shift = percentile(&center_shifts, 0.5);
    let p90_center_shift = percentile(&center_shifts, 0.9);
    let median_area_ratio = percentile(&area_ratios, 0.5);
    let area_ratio_errors = area_ratios
        .iter()
        .map(|ratio| (ratio - 1.0).abs())
        .collect::<Vec<_>>();
    let median_area_ratio_error = percentile(&area_ratio_errors, 0.5);
    let denominator = evaluated_sample_count.max(sample_count);
    let manual_adjustment_rate =
        (denominator > 0).then(|| sample_count as f32 / denominator as f32);
    let too_loose_count = evidence
        .iter()
        .filter(|(_, item)| item.reason == GeometryCorrectionReason::TooLoose)
        .count() as f32;
    let too_tight_count = evidence
        .iter()
        .filter(|(_, item)| item.reason == GeometryCorrectionReason::TooTight)
        .count() as f32;
    let too_loose_rate = (sample_count > 0).then(|| too_loose_count / sample_count as f32);
    let too_tight_rate = (sample_count > 0).then(|| too_tight_count / sample_count as f32);
    let metrics_pass = p10_iou.is_some_and(|value| value >= thresholds.minimum_iou)
        && p90_center_shift
            .is_some_and(|value| value <= thresholds.maximum_normalized_center_shift)
        && median_area_ratio.is_some_and(|value| {
            value >= thresholds.minimum_area_ratio && value <= thresholds.maximum_area_ratio
        });
    let status = if sample_count == 0 {
        GeometryCalibrationStatus::Uncalibrated
    } else if sample_count < thresholds.minimum_sample_count {
        if metrics_pass && sample_count >= thresholds.minimum_sample_count.div_ceil(3).max(3) {
            GeometryCalibrationStatus::Provisional
        } else {
            GeometryCalibrationStatus::CollectingEvidence
        }
    } else if metrics_pass {
        GeometryCalibrationStatus::Passed
    } else {
        GeometryCalibrationStatus::Failed
    };
    let mut evidence_run_ids = evidence
        .iter()
        .map(|(_, item)| item.run_id)
        .collect::<Vec<_>>();
    evidence_run_ids.sort();
    evidence_run_ids.dedup();
    let mut evidence_quality_report_ids = evidence
        .iter()
        .map(|(report, _)| report.id)
        .collect::<Vec<_>>();
    evidence_quality_report_ids.sort();
    evidence_quality_report_ids.dedup();
    GeometryCalibrationReport {
        id: GeometryCalibrationId::new(),
        key,
        status,
        sample_count,
        small_object_sample_count,
        median_iou,
        p10_iou,
        median_center_shift,
        p90_center_shift,
        median_area_ratio_error,
        manual_adjustment_rate,
        too_loose_rate,
        too_tight_rate,
        thresholds,
        evidence_run_ids,
        evidence_quality_report_ids,
        created_at,
    }
}

fn percentile(values: &[f32], percentile: f32) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    let mut values = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    values.sort_by(f32::total_cmp);
    let rank = percentile.clamp(0.0, 1.0) * (values.len().saturating_sub(1)) as f32;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        Some(values[lower])
    } else {
        let weight = rank - lower as f32;
        Some(values[lower] + (values[upper] - values[lower]) * weight)
    }
}

#[must_use]
pub fn build_geometry_correction_evidence(
    input: GeometryCorrectionInput,
) -> (GeometryQualityReport, GeometryCorrectionEvidence) {
    let predicted = input.original_geometry.rect;
    let reference = input.corrected_geometry.rect;
    let predicted_area = input.original_geometry.pixel_area();
    let reference_area = input.corrected_geometry.pixel_area();
    let area_ratio = predicted_area / reference_area.max(f32::EPSILON);
    let width_ratio = predicted.width() / reference.width().max(f32::EPSILON);
    let height_ratio = predicted.height() / reference.height().max(f32::EPSILON);
    let normalized_center_shift = center_shift(predicted, reference);
    let pixel_center_shift = pixel_center_shift(input.original_geometry, input.corrected_geometry);
    let iou = rect_iou(predicted, reference);
    let mut issue_codes = geometry_issue_codes(
        area_ratio,
        width_ratio,
        height_ratio,
        normalized_center_shift,
        &input.reason,
    );
    if input.source_model_profile_id.is_none()
        || input.source_model_revision.is_none()
        || input.original_geometry.image_width == 0
        || input.original_geometry.image_height == 0
        || input.corrected_geometry.image_width == 0
        || input.corrected_geometry.image_height == 0
    {
        issue_codes.push(GeometryIssueCode::InsufficientEvidence);
    }
    issue_codes.sort();
    issue_codes.dedup();
    let report_id = GeometryQualityReportId::new();
    let report = GeometryQualityReport {
        id: report_id,
        project_id: input.project_id,
        image_id: input.image_id,
        candidate_artifact_id: input.candidate_artifact_id,
        reference_artifact_id: Some(input.reference_artifact_id),
        source: GeometryEvidenceSource::HumanCorrection,
        iou: Some(iou),
        normalized_center_shift: Some(normalized_center_shift),
        pixel_center_shift: Some(pixel_center_shift),
        predicted_area: Some(predicted_area),
        reference_area: Some(reference_area),
        area_ratio: Some(area_ratio),
        width_ratio: Some(width_ratio),
        height_ratio: Some(height_ratio),
        foreground_occupancy: None,
        mask_support: None,
        edge_support: None,
        size_bucket: Some(input.corrected_geometry.size_bucket()),
        issue_codes,
        created_at: input.created_at,
    };
    let evidence = GeometryCorrectionEvidence {
        project_id: input.project_id,
        run_id: input.run_id,
        image_id: input.image_id,
        annotation_id: input.annotation_id,
        source_node_id: input.source_node_id,
        source_model_profile_id: input.source_model_profile_id,
        source_model_revision: input.source_model_revision,
        original_geometry: input.original_geometry,
        corrected_geometry: input.corrected_geometry,
        reason: input.reason,
        quality_report_id: report_id,
        created_at: input.created_at,
    };
    (report, evidence)
}

fn geometry_issue_codes(
    area_ratio: f32,
    width_ratio: f32,
    height_ratio: f32,
    center_shift: f32,
    reason: &GeometryCorrectionReason,
) -> Vec<GeometryIssueCode> {
    let mut issues = Vec::new();
    if area_ratio > 1.2 || reason == &GeometryCorrectionReason::TooLoose {
        issues.extend([
            GeometryIssueCode::TooLoose,
            GeometryIssueCode::IncludesBackground,
        ]);
    }
    if area_ratio < 0.8 || reason == &GeometryCorrectionReason::TooTight {
        issues.extend([
            GeometryIssueCode::TooTight,
            GeometryIssueCode::PartialObject,
        ]);
    }
    if center_shift > 0.05 || reason == &GeometryCorrectionReason::Shifted {
        issues.push(GeometryIssueCode::CenterShift);
    }
    if !(0.85..=1.15).contains(&width_ratio) {
        issues.push(GeometryIssueCode::WidthError);
    }
    if !(0.85..=1.15).contains(&height_ratio) {
        issues.push(GeometryIssueCode::HeightError);
    }
    let aspect_ratio_change = width_ratio / height_ratio.max(f32::EPSILON);
    if !(0.8..=1.25).contains(&aspect_ratio_change) {
        issues.push(GeometryIssueCode::AspectRatioError);
    }
    issues
}

fn pixel_center_shift(left: GeometrySnapshot, right: GeometrySnapshot) -> f32 {
    let left_center = left.rect.center();
    let right_center = right.rect.center();
    let width = right.image_width.max(left.image_width) as f32;
    let height = right.image_height.max(left.image_height) as f32;
    ((left_center.x() - right_center.x()) * width)
        .hypot((left_center.y() - right_center.y()) * height)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GeometryQualitySummary {
    pub total_candidates: u32,
    pub coarse_geometry_count: u32,
    pub geometry_review_count: u32,
    pub human_adjustment_count: u32,
    pub mean_manual_center_shift: Option<f32>,
    pub mean_manual_area_change: Option<f32>,
    pub mean_refiner_iou: Option<f32>,
    pub inaccurate_bbox_reason_count: u32,
    #[serde(default)]
    pub size_buckets: BTreeMap<ObjectSizeBucket, GeometrySizeBucketSummary>,
    #[serde(default)]
    pub correction_reasons: BTreeMap<GeometryCorrectionReason, u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GeometrySizeBucketSummary {
    pub sample_count: u32,
    pub human_adjustment_count: u32,
    pub mean_iou: Option<f32>,
    pub mean_center_shift: Option<f32>,
}

impl GeometryQualitySummary {
    pub fn add_report(&mut self, report: &CandidateGeometryQualityReport, needs_review: bool) {
        self.total_candidates = self.total_candidates.saturating_add(1);
        if report.geometry_semantics == GeometrySemantics::CoarseHypothesis {
            self.coarse_geometry_count = self.coarse_geometry_count.saturating_add(1);
        }
        if needs_review && report.has_geometry_issue() {
            self.geometry_review_count = self.geometry_review_count.saturating_add(1);
        }
        if report.has_geometry_issue() {
            self.inaccurate_bbox_reason_count = self.inaccurate_bbox_reason_count.saturating_add(1);
        }
    }

    pub fn add_correction(
        &mut self,
        report: &GeometryQualityReport,
        evidence: &GeometryCorrectionEvidence,
    ) {
        self.human_adjustment_count = self.human_adjustment_count.saturating_add(1);
        self.inaccurate_bbox_reason_count = self.inaccurate_bbox_reason_count.saturating_add(1);
        *self
            .correction_reasons
            .entry(evidence.reason.clone())
            .or_default() += 1;
        if let Some(bucket) = report.size_bucket {
            let summary = self.size_buckets.entry(bucket).or_default();
            summary.sample_count = summary.sample_count.saturating_add(1);
            summary.human_adjustment_count = summary.human_adjustment_count.saturating_add(1);
            push_mean(&mut summary.mean_iou, report.iou, summary.sample_count);
            push_mean(
                &mut summary.mean_center_shift,
                report.normalized_center_shift,
                summary.sample_count,
            );
        }
        push_mean(
            &mut self.mean_manual_center_shift,
            report.normalized_center_shift,
            self.human_adjustment_count,
        );
        push_mean(
            &mut self.mean_manual_area_change,
            report.area_ratio.map(|ratio| ratio - 1.0),
            self.human_adjustment_count,
        );
    }

    pub fn add_manual_adjustment(&mut self, center_shift: f32, area_change: f32) {
        self.human_adjustment_count = self.human_adjustment_count.saturating_add(1);
        let count = self.human_adjustment_count;
        push_mean(
            &mut self.mean_manual_center_shift,
            Some(center_shift),
            count,
        );
        push_mean(&mut self.mean_manual_area_change, Some(area_change), count);
    }
}

fn push_mean(mean: &mut Option<f32>, value: Option<f32>, count: u32) {
    let Some(value) = value else { return };
    *mean = Some(match *mean {
        Some(current) if count > 1 => current + (value - current) / count as f32,
        _ => value,
    });
}

#[must_use]
pub fn manual_geometry_metrics(
    before: &AnnotationSnapshot,
    after: &AnnotationSnapshot,
) -> Option<(f32, f32, f32)> {
    let (
        AnnotationValue::BoundingBox { rect: before },
        AnnotationValue::BoundingBox { rect: after },
    ) = (&before.value, &after.value)
    else {
        return None;
    };
    (*before != *after).then(|| {
        (
            center_shift(*before, *after),
            relative_area_change(*before, *after),
            rect_iou(*before, *after),
        )
    })
}

#[must_use]
pub fn manual_geometry_feature_map(
    before: &AnnotationSnapshot,
    after: &AnnotationSnapshot,
) -> BTreeMap<String, f64> {
    manual_geometry_metrics(before, after).map_or_else(BTreeMap::new, |(shift, area, iou)| {
        BTreeMap::from([
            ("manual_center_shift".to_owned(), f64::from(shift)),
            ("manual_area_change".to_owned(), f64::from(area)),
            ("manual_iou".to_owned(), f64::from(iou)),
        ])
    })
}

#[must_use]
pub fn center_shift(left: NormalizedRect, right: NormalizedRect) -> f32 {
    let left = left.center();
    let right = right.center();
    (left.x() - right.x()).hypot(left.y() - right.y())
}

#[must_use]
pub fn relative_area_change(left: NormalizedRect, right: NormalizedRect) -> f32 {
    ((right.area() - left.area()) / left.area()).abs()
}

#[must_use]
pub fn rect_iou(left: NormalizedRect, right: NormalizedRect) -> f32 {
    let intersection = left.intersection_area(right);
    intersection / (left.area() + right.area() - intersection).max(f32::EPSILON)
}

fn touches_image_boundary(rect: NormalizedRect) -> bool {
    rect.x() <= f32::EPSILON
        || rect.y() <= f32::EPSILON
        || rect.x() + rect.width() >= 1.0 - f32::EPSILON
        || rect.y() + rect.height() >= 1.0 - f32::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DetectionScore, DetectionSource, LabelId, ReviewStatus, VisionCapability};

    fn refinement_reference(kind: ArtifactKind, artifact_id: &str, item_id: &str) -> ArtifactRef {
        ArtifactRef {
            artifact_id: artifact_id.to_owned(),
            source_node: format!("{artifact_id}-node"),
            port: "output".to_owned(),
            artifact_type: kind,
            item_id: Some(item_id.to_owned()),
        }
    }

    fn refinement_trace(
        original_bbox: NormalizedRect,
        refined_bbox: NormalizedRect,
    ) -> GeometryRefinementTrace {
        GeometryRefinementTrace {
            schema_version: GEOMETRY_REFINEMENT_TRACE_SCHEMA_VERSION,
            method: "mask_to_bbox".to_owned(),
            source_detection: refinement_reference(ArtifactKind::DetectionSet, "coarse", "ball-1"),
            box_prompt: refinement_reference(ArtifactKind::BoxPromptSet, "prompts", "prompt-1"),
            mask: refinement_reference(ArtifactKind::MaskSet, "masks", "mask-1"),
            refined_detection: refinement_reference(
                ArtifactKind::DetectionSet,
                "refined",
                "refined:ball-1",
            ),
            original_bbox,
            refined_bbox,
            mask_score: DetectionScore::not_provided(),
        }
    }

    #[test]
    fn prompted_refinement_evaluation_accepts_a_stable_tightening() {
        let evaluation = evaluate_geometry_refinement(
            refinement_trace(
                NormalizedRect::new(0.1, 0.2, 0.4, 0.4).expect("coarse"),
                NormalizedRect::new(0.16, 0.26, 0.26, 0.26).expect("refined"),
            ),
            GeometryRefinementThresholds::default(),
        )
        .expect("evaluation");

        assert!(evaluation.stable);
        assert!(evaluation.issue_codes.is_empty());
        assert!(evaluation.coarse_refined_iou > 0.4);
        assert!((evaluation.area_ratio - 0.4225).abs() < 0.000_1);
    }

    #[test]
    fn prompted_refinement_evaluation_routes_large_changes_to_review() {
        let evaluation = evaluate_geometry_refinement(
            refinement_trace(
                NormalizedRect::new(0.1, 0.1, 0.3, 0.3).expect("coarse"),
                NormalizedRect::new(0.75, 0.75, 0.05, 0.05).expect("refined"),
            ),
            GeometryRefinementThresholds::default(),
        )
        .expect("evaluation");

        assert!(!evaluation.stable);
        assert!(
            evaluation
                .issue_codes
                .contains(&GeometryIssueCode::RefinerConflict)
        );
        assert!(
            evaluation
                .issue_codes
                .contains(&GeometryIssueCode::CenterShift)
        );
        assert!(
            evaluation
                .issue_codes
                .contains(&GeometryIssueCode::PartialObject)
        );
    }

    #[test]
    fn provider_failure_is_not_geometry_evidence() {
        assert_eq!(
            classify_annotation_failure("provider_timeout", "Qwen did not respond"),
            AnnotationFailureClass::ProviderFailure
        );
        assert_eq!(
            classify_annotation_failure("no_candidate", "detector returned no boxes"),
            AnnotationFailureClass::NoCandidate
        );
        assert_eq!(
            classify_annotation_failure("bbox_too_large", "candidate needs tightening"),
            AnnotationFailureClass::GeometryError
        );
        assert_eq!(
            classify_annotation_failure("worker_unreachable", "service not started"),
            AnnotationFailureClass::InfrastructureFailure
        );
        assert_eq!(
            classify_annotation_failure("wrong_label", "shoe is not a ball"),
            AnnotationFailureClass::SemanticError
        );
        assert_eq!(
            classify_annotation_failure("missing_score", "score not provided"),
            AnnotationFailureClass::MissingScore
        );
        assert_eq!(
            classify_annotation_failure("domain_risk", "hard negative"),
            AnnotationFailureClass::DomainRisk
        );
        assert_eq!(
            classify_annotation_failure("artifact_schema", "malformed payload"),
            AnnotationFailureClass::InvalidArtifact
        );
        assert_eq!(
            classify_annotation_failure("cost_budget", "budget exhausted"),
            AnnotationFailureClass::BudgetLimit
        );
    }

    #[test]
    fn vlm_bbox_is_coarse_and_refiner_metrics_are_separate_from_score() {
        let original = NormalizedRect::new(0.1, 0.1, 0.6, 0.6).expect("original");
        let refined = NormalizedRect::new(0.2, 0.2, 0.3, 0.3).expect("refined");
        let mut detection = DetectionArtifactItem::from_source(
            "ball-1",
            None,
            Some("ball".to_owned()),
            Some(LabelId::from("ball")),
            refined,
            DetectionScore::relative(0.99).expect("score"),
            DetectionSource {
                model_id: "vlm".to_owned(),
                capability: VisionCapability::VisionLanguage,
                artifact_id: "detections".to_owned(),
            },
        )
        .expect("detection");
        detection.attributes.insert(
            "geometry_refinement".to_owned(),
            serde_json::json!({"original_bbox": original, "refined_bbox": refined}),
        );
        let report = CandidateGeometryQualityReport::from_detection("refined", &detection);
        assert_eq!(
            report.geometry_semantics,
            GeometrySemantics::CoarseHypothesis
        );
        assert!(report.iou_with_refiner.expect("IoU") < 1.0);
        assert_eq!(detection.score.comparable_confidence(), Some(0.99));
    }

    #[test]
    fn manual_bbox_adjustment_produces_geometry_metrics() {
        let snapshot = |rect| AnnotationSnapshot {
            label: Some(LabelId::from("ball")),
            value: AnnotationValue::BoundingBox { rect },
            attributes: BTreeMap::new(),
            confidence: None,
            review_status: ReviewStatus::NeedsReview,
        };
        let before = snapshot(NormalizedRect::new(0.1, 0.1, 0.6, 0.6).expect("before"));
        let after = snapshot(NormalizedRect::new(0.2, 0.2, 0.3, 0.3).expect("after"));
        let features = manual_geometry_feature_map(&before, &after);
        assert!(features["manual_center_shift"] > 0.0);
        assert!(features["manual_area_change"] > 0.0);
        assert!(features["manual_iou"] < 1.0);
    }

    #[test]
    fn bbox_correction_builds_scoped_typed_geometry_evidence() {
        let project_id = ProjectId::new();
        let run_id = RunId::new();
        let image_id = ImageId::new();
        let annotation_id = AnnotationId::new();
        let model_id = ModelProfileId::new();
        let original = GeometrySnapshot {
            rect: NormalizedRect::new(0.1, 0.1, 0.2, 0.2).expect("original"),
            image_width: 640,
            image_height: 480,
        };
        let corrected = GeometrySnapshot {
            rect: NormalizedRect::new(0.12, 0.12, 0.1, 0.1).expect("corrected"),
            image_width: 640,
            image_height: 480,
        };
        let (report, evidence) = build_geometry_correction_evidence(GeometryCorrectionInput {
            project_id,
            run_id,
            image_id,
            annotation_id,
            source_node_id: NodeId::from("vlm-detect"),
            source_model_profile_id: Some(model_id),
            source_model_revision: Some(3),
            candidate_artifact_id: ArtifactId::new(),
            reference_artifact_id: ArtifactId::new(),
            original_geometry: original,
            corrected_geometry: corrected,
            reason: GeometryCorrectionReason::TooLoose,
            created_at: Utc::now(),
        });
        assert_eq!(report.project_id, project_id);
        assert_eq!(report.iou, Some(0.25));
        assert_eq!(report.area_ratio, Some(4.0));
        assert_eq!(report.width_ratio, Some(2.0));
        assert_eq!(report.height_ratio, Some(2.0));
        assert!(report.pixel_center_shift.is_some_and(|shift| shift > 0.0));
        assert!(report.issue_codes.contains(&GeometryIssueCode::TooLoose));
        assert!(
            report
                .issue_codes
                .contains(&GeometryIssueCode::IncludesBackground)
        );
        assert_eq!(report.size_bucket, Some(ObjectSizeBucket::Medium));
        assert!(evidence.calibration_eligible());
        assert_eq!(evidence.quality_report_id, report.id);
    }

    #[test]
    fn small_object_bucket_and_missing_lineage_remain_visible() {
        let (report, evidence) = build_geometry_correction_evidence(GeometryCorrectionInput {
            project_id: ProjectId::new(),
            run_id: RunId::new(),
            image_id: ImageId::new(),
            annotation_id: AnnotationId::new(),
            source_node_id: NodeId::from("legacy.unresolved"),
            source_model_profile_id: None,
            source_model_revision: None,
            candidate_artifact_id: ArtifactId::new(),
            reference_artifact_id: ArtifactId::new(),
            original_geometry: GeometrySnapshot {
                rect: NormalizedRect::new(0.1, 0.1, 0.02, 0.02).expect("original"),
                image_width: 640,
                image_height: 480,
            },
            corrected_geometry: GeometrySnapshot {
                rect: NormalizedRect::new(0.1, 0.1, 0.025, 0.025).expect("corrected"),
                image_width: 640,
                image_height: 480,
            },
            reason: GeometryCorrectionReason::TooTight,
            created_at: Utc::now(),
        });
        assert_eq!(report.size_bucket, Some(ObjectSizeBucket::Small));
        assert!(
            report
                .issue_codes
                .contains(&GeometryIssueCode::InsufficientEvidence)
        );
        assert!(!evidence.calibration_eligible());
    }

    fn calibration_key(project_id: ProjectId, model_id: ModelProfileId) -> GeometryCalibrationKey {
        GeometryCalibrationKey {
            project_id,
            task_id: TaskId::from("objects"),
            label_id: Some(LabelId::from("ball")),
            model_profile_id: model_id,
            model_profile_revision: 2,
            node_definition_id: "vlm_detection.detect".to_owned(),
            node_config_hash: "node-v1".to_owned(),
            prompt_version: Some("prompt-v1".to_owned()),
            preprocessing_hash: "preprocess-v1".to_owned(),
            dataset_profile_revision: "dataset-v1".to_owned(),
            label_schema_hash: "labels-v1".to_owned(),
            refinement_hash: "refiners-v1".to_owned(),
        }
    }

    #[test]
    fn calibration_is_project_model_revision_and_pipeline_exact() {
        let project_id = ProjectId::new();
        let model_id = ModelProfileId::new();
        let key = calibration_key(project_id, model_id);
        let evidence = (0..30)
            .map(|index| {
                build_geometry_correction_evidence(GeometryCorrectionInput {
                    project_id,
                    run_id: RunId::new(),
                    image_id: ImageId::new(),
                    annotation_id: AnnotationId::new(),
                    source_node_id: NodeId::from("detector"),
                    source_model_profile_id: Some(model_id),
                    source_model_revision: Some(2),
                    candidate_artifact_id: ArtifactId::new(),
                    reference_artifact_id: ArtifactId::new(),
                    original_geometry: GeometrySnapshot {
                        rect: NormalizedRect::new(0.1, 0.1, 0.05, 0.05).expect("prediction"),
                        image_width: 640,
                        image_height: 480,
                    },
                    corrected_geometry: GeometrySnapshot {
                        rect: NormalizedRect::new(0.1 + index as f32 * 0.000_001, 0.1, 0.05, 0.05)
                            .expect("reference"),
                        image_width: 640,
                        image_height: 480,
                    },
                    reason: GeometryCorrectionReason::Other,
                    created_at: Utc::now(),
                })
            })
            .collect::<Vec<_>>();
        let report = evaluate_geometry_calibration(
            key.clone(),
            GeometryCalibrationThresholds::default(),
            &evidence,
            40,
            Utc::now(),
        );
        assert_eq!(report.status, GeometryCalibrationStatus::Passed);
        assert_eq!(report.sample_count, 30);
        assert_eq!(report.small_object_sample_count, 30);
        assert!(report.p10_iou.is_some_and(|value| value > 0.99));
        assert_eq!(report.manual_adjustment_rate, Some(0.75));
        assert_eq!(
            report.effective_status(&key),
            GeometryCalibrationStatus::Passed
        );

        let mut changed = key.clone();
        changed.project_id = ProjectId::new();
        changed.task_id = TaskId::from("other-task");
        changed.label_id = Some(LabelId::from("other-label"));
        changed.model_profile_id = ModelProfileId::new();
        changed.model_profile_revision += 1;
        changed.node_definition_id = "other.detect".to_owned();
        changed.prompt_version = Some("prompt-v2".to_owned());
        changed.preprocessing_hash = "preprocess-v2".to_owned();
        changed.node_config_hash = "node-v2".to_owned();
        changed.label_schema_hash = "labels-v2".to_owned();
        changed.refinement_hash = "refiners-v2".to_owned();
        changed.dataset_profile_revision = "dataset-v2".to_owned();
        let stale = report.staleness_reasons(&changed);
        assert!(stale.contains(&GeometryCalibrationStaleness::Project));
        assert!(stale.contains(&GeometryCalibrationStaleness::Task));
        assert!(stale.contains(&GeometryCalibrationStaleness::Label));
        assert!(stale.contains(&GeometryCalibrationStaleness::ModelProfile));
        assert!(stale.contains(&GeometryCalibrationStaleness::ModelRevision));
        assert!(stale.contains(&GeometryCalibrationStaleness::NodeDefinition));
        assert!(stale.contains(&GeometryCalibrationStaleness::Prompt));
        assert!(stale.contains(&GeometryCalibrationStaleness::Preprocessing));
        assert!(stale.contains(&GeometryCalibrationStaleness::NodeConfiguration));
        assert!(stale.contains(&GeometryCalibrationStaleness::LabelSchema));
        assert!(stale.contains(&GeometryCalibrationStaleness::Refinement));
        assert!(stale.contains(&GeometryCalibrationStaleness::DatasetProfile));
        assert_eq!(
            report.effective_status(&changed),
            GeometryCalibrationStatus::Stale
        );
        assert!(
            !serde_json::to_string(&report)
                .expect("calibration JSON")
                .contains("credential")
        );
    }

    #[test]
    fn insufficient_calibration_never_passes_as_production_evidence() {
        let project_id = ProjectId::new();
        let report = evaluate_geometry_calibration(
            calibration_key(project_id, ModelProfileId::new()),
            GeometryCalibrationThresholds::default(),
            &[],
            0,
            Utc::now(),
        );
        assert_eq!(report.status, GeometryCalibrationStatus::Uncalibrated);
        assert!(!report.status.permits_calibrated_acceptance());
    }

    #[test]
    fn domain_correction_reason_round_trips_without_core_taxonomy() {
        let reason = GeometryCorrectionReason::from_code("custom_domain_risk");
        assert_eq!(reason.as_code(), "custom_domain_risk");
        let encoded = serde_json::to_string(&reason).expect("serialize reason");
        assert_eq!(encoded, "\"custom_domain_risk\"");
        assert_eq!(
            serde_json::from_str::<GeometryCorrectionReason>(&encoded).expect("restore reason"),
            reason,
        );
    }
}
