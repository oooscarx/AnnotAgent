//! Operation-scoped score and geometry quality contracts.
//!
//! A model-provided score, measured geometry evidence, calibration and human validation are
//! deliberately separate. No constructor in this module fabricates localization confidence.

use serde::{Deserialize, Serialize};

use crate::{
    DetectionScore, GeometryQualityReportId, GeometrySemantics, ModelCapability, ModelProfile,
    ModelProfileId, ProjectId, ScoreSemantics, TaskKind, VisionCapability,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredGeometryQuality {
    CoarseLocalization,
    TrainingBoundingBox,
    TightBoundingBox,
    PixelAccurateMask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryAutoAcceptPolicy {
    HumanReviewRequired,
    RefinerOrReview,
    CalibrationRequired,
    ExplicitRiskAcceptance,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryCalibrationThresholds {
    pub minimum_iou: f32,
    pub maximum_normalized_center_shift: f32,
    pub minimum_area_ratio: f32,
    pub maximum_area_ratio: f32,
    pub minimum_sample_count: u32,
}

impl Default for GeometryCalibrationThresholds {
    fn default() -> Self {
        Self {
            minimum_iou: 0.70,
            maximum_normalized_center_shift: 0.05,
            minimum_area_ratio: 0.75,
            maximum_area_ratio: 1.35,
            minimum_sample_count: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectGeometryPolicy {
    pub project_id: ProjectId,
    pub task_kind: TaskKind,
    pub required_quality: RequiredGeometryQuality,
    pub auto_accept_policy: GeometryAutoAcceptPolicy,
    pub calibration_thresholds: GeometryCalibrationThresholds,
}

impl ProjectGeometryPolicy {
    #[must_use]
    pub fn conservative_default(project_id: ProjectId, task_kind: TaskKind) -> Self {
        let (required_quality, auto_accept_policy) = match task_kind {
            TaskKind::BoundingBox => (
                RequiredGeometryQuality::TrainingBoundingBox,
                GeometryAutoAcceptPolicy::RefinerOrReview,
            ),
            TaskKind::SemanticMask | TaskKind::InstanceMask | TaskKind::Polygon => (
                RequiredGeometryQuality::PixelAccurateMask,
                GeometryAutoAcceptPolicy::RefinerOrReview,
            ),
            TaskKind::Keypoints | TaskKind::Polyline => (
                RequiredGeometryQuality::TightBoundingBox,
                GeometryAutoAcceptPolicy::HumanReviewRequired,
            ),
            TaskKind::Classification | TaskKind::Attributes | TaskKind::Relations => (
                RequiredGeometryQuality::CoarseLocalization,
                GeometryAutoAcceptPolicy::CalibrationRequired,
            ),
        };
        Self {
            project_id,
            task_kind,
            required_quality,
            auto_accept_policy,
            calibration_thresholds: GeometryCalibrationThresholds::default(),
        }
    }

    #[must_use]
    pub const fn protects_bounding_boxes(&self) -> bool {
        matches!(self.task_kind, TaskKind::BoundingBox)
            && !matches!(
                self.required_quality,
                RequiredGeometryQuality::CoarseLocalization
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSafetyCompatibility {
    Safe,
    #[default]
    RequiresMigration,
    LegacyRiskAccepted,
    UnsafeForNewRuns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GeometryCalibrationStatus {
    #[default]
    Uncalibrated,
    CollectingEvidence,
    Provisional,
    Passed,
    Failed,
    Stale,
}

impl GeometryCalibrationStatus {
    #[must_use]
    pub const fn permits_calibrated_acceptance(self) -> bool {
        matches!(self, Self::Passed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoAcceptEligibility {
    NeverFromScoreAlone,
    RequiresProjectCalibration,
    EligibleWithCalibration,
}

impl AutoAcceptEligibility {
    #[must_use]
    pub const fn permits_score_path(self, calibration: GeometryCalibrationStatus) -> bool {
        match self {
            Self::NeverFromScoreAlone => false,
            Self::RequiresProjectCalibration | Self::EligibleWithCalibration => {
                calibration.permits_calibrated_acceptance()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractEvidenceSource {
    SystemDefault,
    UserDeclared,
    ActiveProbe,
    CalibrationReport,
    MigratedLegacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SmallObjectLocalizationSupport {
    Unsupported,
    Declared,
    Verified,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCapabilityQualityContract {
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,
    pub capability: ModelCapability,
    pub operation: String,
    pub output_geometry: GeometrySemantics,
    pub score_semantics: ScoreSemantics,
    pub auto_accept_eligibility: AutoAcceptEligibility,
    pub evidence_source: ContractEvidenceSource,
    #[serde(default)]
    pub small_object_localization: SmallObjectLocalizationSupport,
    #[serde(default)]
    pub requires_geometry_verification: bool,
}

impl ModelCapabilityQualityContract {
    #[must_use]
    pub fn vlm_detection(model_profile_id: ModelProfileId, revision: u64) -> Self {
        Self {
            model_profile_id,
            model_profile_revision: revision,
            capability: ModelCapability::VisionLanguage,
            operation: "vlm_detection.detect".to_owned(),
            output_geometry: GeometrySemantics::CoarseHypothesis,
            score_semantics: ScoreSemantics::SemanticConfidence,
            auto_accept_eligibility: AutoAcceptEligibility::NeverFromScoreAlone,
            evidence_source: ContractEvidenceSource::SystemDefault,
            small_object_localization: SmallObjectLocalizationSupport::Unknown,
            requires_geometry_verification: true,
        }
    }

    #[must_use]
    pub fn specialist_detection(
        model_profile_id: ModelProfileId,
        revision: u64,
        capability: ModelCapability,
        operation: impl Into<String>,
    ) -> Self {
        Self {
            model_profile_id,
            model_profile_revision: revision,
            capability,
            operation: operation.into(),
            output_geometry: GeometrySemantics::PredictedGeometry,
            score_semantics: ScoreSemantics::DetectionConfidence,
            auto_accept_eligibility: AutoAcceptEligibility::RequiresProjectCalibration,
            evidence_source: ContractEvidenceSource::SystemDefault,
            small_object_localization: SmallObjectLocalizationSupport::Unknown,
            requires_geometry_verification: true,
        }
    }

    #[must_use]
    pub fn prompted_segmentation(model_profile_id: ModelProfileId, revision: u64) -> Self {
        Self {
            model_profile_id,
            model_profile_revision: revision,
            capability: ModelCapability::PromptedSegmentation,
            operation: "capability.segment".to_owned(),
            output_geometry: GeometrySemantics::RefinedGeometry,
            score_semantics: ScoreSemantics::NotProvided,
            auto_accept_eligibility: AutoAcceptEligibility::RequiresProjectCalibration,
            evidence_source: ContractEvidenceSource::SystemDefault,
            small_object_localization: SmallObjectLocalizationSupport::Unknown,
            requires_geometry_verification: true,
        }
    }

    pub fn validate_for(&self, model: &ModelProfile) -> Result<(), String> {
        if self.model_profile_id != model.id || self.model_profile_revision != model.revision {
            return Err("quality contract must match its Model Profile id and revision".to_owned());
        }
        if self.operation.trim().is_empty()
            || self.operation.len() > 160
            || self.operation.contains(['\r', '\n'])
        {
            return Err(
                "quality contract operation must be non-empty, single-line and bounded".to_owned(),
            );
        }
        if !model.task_capabilities.contains(&self.capability) {
            return Err(
                "quality contract capability is not declared by the Model Profile".to_owned(),
            );
        }
        if self.output_geometry == GeometrySemantics::NotApplicable
            && self.requires_geometry_verification
        {
            return Err("non-geometric output cannot require geometry verification".to_owned());
        }
        if self.evidence_source == ContractEvidenceSource::UserDeclared
            && self.output_geometry == GeometrySemantics::HumanVerified
        {
            return Err(
                "a user-declared model contract cannot claim human-verified geometry".to_owned(),
            );
        }
        Ok(())
    }
}

#[must_use]
pub fn default_model_quality_contracts(
    model_profile_id: ModelProfileId,
    revision: u64,
    capabilities: impl IntoIterator<Item = ModelCapability>,
) -> Vec<ModelCapabilityQualityContract> {
    let mut contracts = Vec::new();
    for capability in capabilities {
        let contract = match capability {
            ModelCapability::VisionLanguage => Some(ModelCapabilityQualityContract::vlm_detection(
                model_profile_id,
                revision,
            )),
            ModelCapability::ObjectDetection => {
                Some(ModelCapabilityQualityContract::specialist_detection(
                    model_profile_id,
                    revision,
                    capability,
                    "capability.detect",
                ))
            }
            ModelCapability::OpenVocabularyDetection => {
                Some(ModelCapabilityQualityContract::specialist_detection(
                    model_profile_id,
                    revision,
                    capability,
                    "capability.detect_open_vocabulary",
                ))
            }
            ModelCapability::PhraseGrounding => {
                Some(ModelCapabilityQualityContract::specialist_detection(
                    model_profile_id,
                    revision,
                    capability,
                    "capability.ground",
                ))
            }
            ModelCapability::PromptedSegmentation => Some(
                ModelCapabilityQualityContract::prompted_segmentation(model_profile_id, revision),
            ),
            ModelCapability::TextGeneration
            | ModelCapability::ImageClassification
            | ModelCapability::SemanticSegmentation
            | ModelCapability::InstanceSegmentation
            | ModelCapability::KeypointDetection => None,
        };
        contracts.extend(contract);
    }
    contracts
}

#[must_use]
pub fn effective_model_quality_contracts(
    model: &ModelProfile,
) -> Vec<ModelCapabilityQualityContract> {
    if model.quality_contracts.is_empty() {
        default_model_quality_contracts(
            model.id,
            model.revision,
            model.task_capabilities.iter().copied(),
        )
    } else {
        model.quality_contracts.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityScoreSource {
    ModelOutput,
    ProviderAdapter,
    RuntimeMeasurement,
    CalibrationReport,
    HumanReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityScore {
    pub value: f32,
    pub semantics: ScoreSemantics,
    pub source: QualityScoreSource,
}

impl QualityScore {
    pub fn new(
        value: f32,
        semantics: ScoreSemantics,
        source: QualityScoreSource,
    ) -> Result<Self, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("quality score must be finite and within [0,1]".to_owned());
        }
        if semantics == ScoreSemantics::NotProvided {
            return Err("not_provided cannot carry a quality score".to_owned());
        }
        Ok(Self {
            value,
            semantics,
            source,
        })
    }

    #[must_use]
    pub fn from_detection_score(score: DetectionScore, source: QualityScoreSource) -> Option<Self> {
        score
            .value
            .and_then(|value| Self::new(value, score.semantics, source).ok())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValidationState {
    #[default]
    NotEvaluated,
    Passed,
    PassedWithWarnings,
    NeedsReview,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryQualityReference {
    pub semantics: GeometrySemantics,
    pub calibration_status: GeometryCalibrationStatus,
    pub report_id: Option<GeometryQualityReportId>,
}

impl GeometryQualityReference {
    #[must_use]
    pub const fn uncalibrated(semantics: GeometrySemantics) -> Self {
        Self {
            semantics,
            calibration_status: GeometryCalibrationStatus::Uncalibrated,
            report_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionQuality {
    pub semantic_score: Option<QualityScore>,
    pub detector_score: Option<QualityScore>,
    pub geometry: GeometryQualityReference,
    pub validation_state: ValidationState,
}

impl DetectionQuality {
    #[must_use]
    pub fn from_model_output(
        score: DetectionScore,
        geometry: GeometrySemantics,
        capability: VisionCapability,
    ) -> Self {
        let reported = QualityScore::from_detection_score(score, QualityScoreSource::ModelOutput);
        let (semantic_score, detector_score) = if capability == VisionCapability::VisionLanguage {
            (reported, None)
        } else {
            (None, reported)
        };
        Self {
            semantic_score,
            detector_score,
            geometry: GeometryQualityReference::uncalibrated(geometry),
            validation_state: ValidationState::NotEvaluated,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use chrono::Utc;

    use super::*;
    use crate::{
        CapabilityDeclarationSource, GenerationDefaults, InputModality, ModelLimits, ModelPricing,
        ModelProfileStatus, ProtocolFeatures, ProviderId,
    };

    fn profile(capabilities: BTreeSet<ModelCapability>) -> ModelProfile {
        ModelProfile {
            id: ModelProfileId::new(),
            revision: 3,
            provider_id: ProviderId::new(),
            display_name: "quality fixture".to_owned(),
            remote_model_id: "quality-fixture".to_owned(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures::default(),
            task_capabilities: capabilities,
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits::default(),
            generation_defaults: GenerationDefaults::default(),
            pricing: ModelPricing::default(),
            quality_contracts: Vec::new(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn vlm_detection_defaults_to_semantic_score_and_coarse_uncalibrated_geometry() {
        let model = profile(BTreeSet::from([ModelCapability::VisionLanguage]));
        let contracts = effective_model_quality_contracts(&model);
        assert_eq!(contracts.len(), 1);
        let contract = &contracts[0];
        assert_eq!(contract.operation, "vlm_detection.detect");
        assert_eq!(
            contract.output_geometry,
            GeometrySemantics::CoarseHypothesis
        );
        assert_eq!(contract.score_semantics, ScoreSemantics::SemanticConfidence);
        assert_eq!(
            contract.auto_accept_eligibility,
            AutoAcceptEligibility::NeverFromScoreAlone
        );
        assert!(
            !contract
                .auto_accept_eligibility
                .permits_score_path(GeometryCalibrationStatus::Passed)
        );
    }

    #[test]
    fn specialist_and_prompted_segmentation_remain_uncalibrated_by_default() {
        let model = profile(BTreeSet::from([
            ModelCapability::ObjectDetection,
            ModelCapability::PromptedSegmentation,
        ]));
        let contracts = effective_model_quality_contracts(&model);
        let detector = contracts
            .iter()
            .find(|contract| contract.capability == ModelCapability::ObjectDetection)
            .expect("detector contract");
        assert_eq!(
            detector.output_geometry,
            GeometrySemantics::PredictedGeometry
        );
        assert_eq!(
            detector.score_semantics,
            ScoreSemantics::DetectionConfidence
        );
        assert!(
            !detector
                .auto_accept_eligibility
                .permits_score_path(GeometryCalibrationStatus::Uncalibrated)
        );
        let refiner = contracts
            .iter()
            .find(|contract| contract.capability == ModelCapability::PromptedSegmentation)
            .expect("refiner contract");
        assert_eq!(refiner.output_geometry, GeometrySemantics::RefinedGeometry);
        assert_eq!(refiner.score_semantics, ScoreSemantics::NotProvided);
        assert!(
            !refiner
                .auto_accept_eligibility
                .permits_score_path(GeometryCalibrationStatus::Uncalibrated)
        );
    }

    #[test]
    fn detection_quality_does_not_fabricate_geometry_scores() {
        let score = DetectionScore::new(Some(0.99), ScoreSemantics::SemanticConfidence)
            .expect("semantic score");
        let quality = DetectionQuality::from_model_output(
            score,
            GeometrySemantics::CoarseHypothesis,
            VisionCapability::VisionLanguage,
        );
        assert!((quality.semantic_score.expect("score").value - 0.99).abs() < f32::EPSILON);
        assert!(quality.detector_score.is_none());
        assert_eq!(
            quality.geometry.calibration_status,
            GeometryCalibrationStatus::Uncalibrated
        );
        assert!(quality.geometry.report_id.is_none());
    }

    #[test]
    fn user_declaration_cannot_claim_human_verified_geometry() {
        let mut model = profile(BTreeSet::from([ModelCapability::ObjectDetection]));
        let mut contract = ModelCapabilityQualityContract::specialist_detection(
            model.id,
            model.revision,
            ModelCapability::ObjectDetection,
            "capability.detect",
        );
        contract.evidence_source = ContractEvidenceSource::UserDeclared;
        contract.output_geometry = GeometrySemantics::HumanVerified;
        assert!(contract.validate_for(&model).is_err());
        contract.output_geometry = GeometrySemantics::PredictedGeometry;
        model.quality_contracts.push(contract);
        model.validate().expect("valid user-declared contract");
    }

    #[test]
    fn bounding_box_projects_default_to_training_quality_and_refiner_or_review() {
        let policy =
            ProjectGeometryPolicy::conservative_default(ProjectId::new(), TaskKind::BoundingBox);
        assert_eq!(
            policy.required_quality,
            RequiredGeometryQuality::TrainingBoundingBox
        );
        assert_eq!(
            policy.auto_accept_policy,
            GeometryAutoAcceptPolicy::RefinerOrReview
        );
        assert!(policy.protects_bounding_boxes());
    }
}
