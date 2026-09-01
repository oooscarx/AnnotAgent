//! Capability-driven manifests shared by provider-backed and Worker-backed vision models.
//!
//! A manifest describes facts that may be inspected and frozen into a Workflow. It never stores
//! credentials and it does not make a Worker executable merely because an adapter exists.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    ArtifactKind, CoreError, CoreResult, LicenseMetadata, ModelAvailabilityStatus, ModelCapability,
    ModelProfile, ProviderId, RuntimeRequirements, ScoreSemantics, VisionBackendKind,
    VisionCapability, VisionInputType, VisionModelDescriptor, VisionModelHealthStatus,
};

pub const EXPERT_MODEL_MANIFEST_SCHEMA_VERSION: u32 = 1;

pub type VisionWorkerId = String;

/// Infrastructure identity supporting a selectable Model Profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelConnection {
    ProviderModel {
        provider_id: ProviderId,
        remote_model_id: String,
    },
    VisionWorkerModel {
        worker_id: VisionWorkerId,
        worker_model_id: String,
    },
    Mock {
        fixture_id: String,
    },
}

impl ModelConnection {
    fn validate(&self) -> CoreResult<()> {
        let values = match self {
            Self::ProviderModel {
                remote_model_id, ..
            } => vec![("remote_model_id", remote_model_id.as_str())],
            Self::VisionWorkerModel {
                worker_id,
                worker_model_id,
            } => vec![
                ("worker_id", worker_id.as_str()),
                ("worker_model_id", worker_model_id.as_str()),
            ],
            Self::Mock { fixture_id } => vec![("fixture_id", fixture_id.as_str())],
        };
        for (name, value) in values {
            if value.trim().is_empty() || value.len() > 512 || value.contains(['\r', '\n']) {
                return Err(CoreError::Validation(format!(
                    "Expert Model connection {name} must be non-empty, single-line and bounded"
                )));
            }
        }
        Ok(())
    }
}

impl ModelProfile {
    /// Provider-backed Model Profiles expose the same connection abstraction used by expert
    /// Worker profiles. Credentials remain owned by the Provider Registry.
    #[must_use]
    pub fn connection(&self) -> ModelConnection {
        ModelConnection::ProviderModel {
            provider_id: self.provider_id,
            remote_model_id: self.remote_model_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GeometrySemantics {
    #[default]
    NotApplicable,
    CoarseHypothesis,
    PredictedGeometry,
    RefinedGeometry,
    MaskRefinedGeometry,
    CalibratedGeometry,
    HumanVerified,
}

impl GeometrySemantics {
    #[must_use]
    pub const fn is_refined(self) -> bool {
        matches!(self, Self::RefinedGeometry | Self::MaskRefinedGeometry)
    }

    #[must_use]
    pub const fn requires_external_calibration(self) -> bool {
        matches!(
            self,
            Self::CoarseHypothesis
                | Self::PredictedGeometry
                | Self::RefinedGeometry
                | Self::MaskRefinedGeometry
                | Self::CalibratedGeometry
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractDataType {
    Text,
    Artifact(ArtifactKind),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactContract {
    pub name: String,
    pub data_type: ContractDataType,
    pub required: bool,
    pub multiple: bool,
}

impl ArtifactContract {
    #[must_use]
    pub fn artifact(
        name: impl Into<String>,
        artifact_type: ArtifactKind,
        required: bool,
        multiple: bool,
    ) -> Self {
        Self {
            name: name.into(),
            data_type: ContractDataType::Artifact(artifact_type),
            required,
            multiple,
        }
    }

    fn validate(&self) -> CoreResult<()> {
        validate_identity("Artifact Contract name", &self.name, 120)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    Text,
    Box,
    Point,
    ExistingAnnotation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptContract {
    pub kind: PromptKind,
    pub required: bool,
    pub multiple: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointIdentity {
    pub sha256: String,
    pub source: Option<String>,
    pub training_dataset_version: Option<String>,
}

impl CheckpointIdentity {
    fn validate(&self) -> CoreResult<()> {
        if self.sha256.len() != 64 || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(CoreError::Validation(
                "Expert Model checkpoint SHA-256 must contain 64 hexadecimal characters".to_owned(),
            ));
        }
        for (name, value) in [
            ("checkpoint source", self.source.as_deref()),
            (
                "training dataset version",
                self.training_dataset_version.as_deref(),
            ),
        ] {
            if let Some(value) = value {
                validate_identity(name, value, 512)?;
            }
        }
        Ok(())
    }
}

/// Complete lifecycle state. Only `Available` may be bound into a publishable Draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelAvailability {
    Unconfigured,
    MissingWeights,
    Disabled,
    #[default]
    Unknown,
    Available,
    Unreachable,
    IncompatibleProtocol,
    InvalidContract,
    FailedSmokeTest,
}

impl ModelAvailability {
    #[must_use]
    pub const fn publishable(self) -> bool {
        matches!(self, Self::Available)
    }

    #[must_use]
    pub const fn legacy_status(self) -> ModelAvailabilityStatus {
        match self {
            Self::Available => ModelAvailabilityStatus::Available,
            Self::MissingWeights => ModelAvailabilityStatus::MissingWeights,
            Self::Disabled => ModelAvailabilityStatus::Disabled,
            Self::Unreachable => ModelAvailabilityStatus::Unreachable,
            Self::IncompatibleProtocol => ModelAvailabilityStatus::IncompatibleProtocol,
            Self::Unconfigured | Self::InvalidContract | Self::FailedSmokeTest => {
                ModelAvailabilityStatus::Misconfigured
            }
            Self::Unknown => ModelAvailabilityStatus::Unknown,
        }
    }
}

/// Evidence required before a configured expert Worker may claim `Available`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ModelAvailabilityEvidence {
    pub health_passed: bool,
    pub protocol_compatible: bool,
    pub contracts_validated: bool,
    pub sample_conversion_passed: bool,
    pub weights_ready: bool,
    pub checked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub detail: Option<String>,
}

impl ModelAvailabilityEvidence {
    #[must_use]
    pub const fn available(&self) -> bool {
        self.health_passed
            && self.protocol_compatible
            && self.contracts_validated
            && self.sample_conversion_passed
            && self.weights_ready
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpertModelManifest {
    pub schema_version: String,
    pub model_id: String,
    pub display_name: String,
    pub architecture: Option<String>,
    pub model_version: String,
    pub connection: ModelConnection,
    pub capabilities: BTreeSet<ModelCapability>,
    pub input_contracts: Vec<ArtifactContract>,
    pub output_contracts: Vec<ArtifactContract>,
    pub prompt_contracts: Vec<PromptContract>,
    pub score_semantics: ScoreSemantics,
    pub geometry_semantics: GeometrySemantics,
    pub label_space: Option<Vec<String>>,
    pub checkpoint: Option<CheckpointIdentity>,
    pub runtime_requirements: RuntimeRequirements,
    pub license: LicenseMetadata,
    pub availability: ModelAvailability,
    #[serde(default)]
    pub availability_evidence: ModelAvailabilityEvidence,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl ExpertModelManifest {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != EXPERT_MODEL_MANIFEST_SCHEMA_VERSION.to_string() {
            return Err(CoreError::Validation(format!(
                "unsupported Expert Model Manifest schema version {:?}",
                self.schema_version
            )));
        }
        validate_identity("Expert Model id", &self.model_id, 512)?;
        validate_identity("Expert Model display name", &self.display_name, 120)?;
        validate_identity("Expert Model version", &self.model_version, 120)?;
        if let Some(architecture) = &self.architecture {
            validate_identity("Expert Model architecture", architecture, 120)?;
        }
        self.connection.validate()?;
        if self.capabilities.is_empty() {
            return Err(CoreError::Validation(
                "Expert Model Manifest requires at least one capability".to_owned(),
            ));
        }
        if self.capabilities.contains(&ModelCapability::TextGeneration) {
            return Err(CoreError::Validation(
                "Text-generation models belong to Provider Model Profiles, not Expert Vision Manifests"
                    .to_owned(),
            ));
        }
        if self.input_contracts.is_empty() || self.output_contracts.is_empty() {
            return Err(CoreError::Validation(
                "Expert Model Manifest requires explicit input and output contracts".to_owned(),
            ));
        }
        validate_contracts(&self.input_contracts, "input")?;
        validate_contracts(&self.output_contracts, "output")?;
        if let Some(labels) = &self.label_space {
            validate_unique_values(labels, "Expert Model label space")?;
        }
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint.validate()?;
        }
        if self
            .capabilities
            .contains(&ModelCapability::PromptedSegmentation)
        {
            let input_kinds = self
                .input_contracts
                .iter()
                .filter_map(|contract| match contract.data_type {
                    ContractDataType::Artifact(kind) => Some(kind),
                    ContractDataType::Text => None,
                })
                .collect::<BTreeSet<_>>();
            let output_kinds = self
                .output_contracts
                .iter()
                .filter_map(|contract| match contract.data_type {
                    ContractDataType::Artifact(kind) => Some(kind),
                    ContractDataType::Text => None,
                })
                .collect::<BTreeSet<_>>();
            let prompt_kinds = self
                .prompt_contracts
                .iter()
                .map(|contract| contract.kind)
                .collect::<BTreeSet<_>>();
            if !prompt_kinds.contains(&PromptKind::Box)
                && !prompt_kinds.contains(&PromptKind::Point)
            {
                return Err(CoreError::Validation(
                    "Prompted Segmentation requires a Box or Point Prompt Contract".to_owned(),
                ));
            }
            if !input_kinds.contains(&ArtifactKind::Image)
                || (!input_kinds.contains(&ArtifactKind::BoxPromptSet)
                    && !input_kinds.contains(&ArtifactKind::PointPromptSet))
                || !output_kinds.contains(&ArtifactKind::MaskSet)
            {
                return Err(CoreError::Validation(
                    "Prompted Segmentation requires Image plus BoxPromptSet or PointPromptSet input and MaskSet output"
                        .to_owned(),
                ));
            }
            if !self.geometry_semantics.is_refined() {
                return Err(CoreError::Validation(
                    "Prompted Segmentation geometry must be refined_geometry".to_owned(),
                ));
            }
        }
        if self.availability == ModelAvailability::Available
            && !self.availability_evidence.available()
        {
            return Err(CoreError::Validation(
                "Available Expert Models require health, protocol, contract, weights and sample-conversion evidence"
                    .to_owned(),
            ));
        }
        if self.availability == ModelAvailability::MissingWeights
            && self.availability_evidence.weights_ready
        {
            return Err(CoreError::Validation(
                "MissingWeights conflicts with weights_ready evidence".to_owned(),
            ));
        }
        Ok(())
    }

    /// Migrates the existing generic descriptor without promoting an untested HTTP Worker to
    /// `Available`. This is a compatibility projection; M2 discovery can replace its evidence.
    pub fn from_vision_descriptor(descriptor: &VisionModelDescriptor) -> CoreResult<Self> {
        let backend_kind = descriptor.backend.kind.ok_or_else(|| {
            CoreError::Validation("Vision descriptor backend kind is unresolved".to_owned())
        })?;
        let connection = match backend_kind {
            VisionBackendKind::Mock => ModelConnection::Mock {
                fixture_id: descriptor.backend_id.clone(),
            },
            VisionBackendKind::HttpVision
            | VisionBackendKind::Onnx
            | VisionBackendKind::DeterministicCv => ModelConnection::VisionWorkerModel {
                worker_id: descriptor.backend_id.clone(),
                worker_model_id: descriptor.id.clone(),
            },
            VisionBackendKind::OpenAiCompatible => {
                return Err(CoreError::Validation(
                    "provider-backed models must derive their connection from a Provider Model Profile"
                        .to_owned(),
                ));
            }
        };
        let capabilities = descriptor
            .capabilities
            .iter()
            .filter_map(|capability| model_capability(*capability))
            .collect::<BTreeSet<_>>();
        let mut input_contracts = descriptor
            .input_contract
            .input_types
            .iter()
            .enumerate()
            .map(|(index, input)| ArtifactContract {
                name: match input {
                    VisionInputType::Image => "image".to_owned(),
                    VisionInputType::Text => "text".to_owned(),
                    VisionInputType::Artifact(kind) => format!("artifact_{index}_{kind:?}"),
                },
                data_type: match input {
                    VisionInputType::Image => ContractDataType::Artifact(ArtifactKind::Image),
                    VisionInputType::Text => ContractDataType::Text,
                    VisionInputType::Artifact(kind) => ContractDataType::Artifact(*kind),
                },
                required: true,
                multiple: false,
            })
            .collect::<Vec<_>>();
        if capabilities.contains(&ModelCapability::PromptedSegmentation)
            && !input_contracts.iter().any(|contract| {
                matches!(
                    contract.data_type,
                    ContractDataType::Artifact(
                        ArtifactKind::BoxPromptSet | ArtifactKind::PointPromptSet
                    )
                )
            })
        {
            input_contracts.extend([
                ArtifactContract::artifact("box_prompts", ArtifactKind::BoxPromptSet, false, true),
                ArtifactContract::artifact(
                    "point_prompts",
                    ArtifactKind::PointPromptSet,
                    false,
                    true,
                ),
            ]);
        }
        let output_contracts = descriptor
            .output_contract
            .output_types
            .iter()
            .enumerate()
            .map(|(index, kind)| {
                let kind = if capabilities.contains(&ModelCapability::PromptedSegmentation) {
                    ArtifactKind::MaskSet
                } else {
                    *kind
                };
                ArtifactContract::artifact(format!("output_{index}_{kind:?}"), kind, true, true)
            })
            .collect::<Vec<_>>();
        let prompt_contracts = if capabilities.contains(&ModelCapability::PromptedSegmentation) {
            vec![
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
            ]
        } else {
            Vec::new()
        };
        let in_process = matches!(
            backend_kind,
            VisionBackendKind::Mock | VisionBackendKind::DeterministicCv
        );
        let health_passed = descriptor.health.status == VisionModelHealthStatus::Healthy;
        let evidence = ModelAvailabilityEvidence {
            health_passed,
            protocol_compatible: in_process,
            contracts_validated: true,
            sample_conversion_passed: in_process && health_passed,
            weights_ready: in_process,
            checked_at: descriptor.health.checked_at,
            detail: descriptor.health.detail.clone(),
        };
        let availability = match descriptor.status {
            ModelAvailabilityStatus::Disabled => ModelAvailability::Disabled,
            ModelAvailabilityStatus::MissingWeights => ModelAvailability::MissingWeights,
            ModelAvailabilityStatus::IncompatibleProtocol => {
                ModelAvailability::IncompatibleProtocol
            }
            ModelAvailabilityStatus::Misconfigured => ModelAvailability::Unconfigured,
            ModelAvailabilityStatus::Unreachable => ModelAvailability::Unreachable,
            ModelAvailabilityStatus::Available if evidence.available() => {
                ModelAvailability::Available
            }
            ModelAvailabilityStatus::Unknown | ModelAvailabilityStatus::Available => {
                ModelAvailability::Unknown
            }
        };
        let checkpoint =
            descriptor
                .version
                .checkpoint_sha256
                .as_ref()
                .map(|sha256| CheckpointIdentity {
                    sha256: sha256.clone(),
                    source: None,
                    training_dataset_version: descriptor.version.training_dataset_version.clone(),
                });
        let manifest = Self {
            schema_version: EXPERT_MODEL_MANIFEST_SCHEMA_VERSION.to_string(),
            model_id: descriptor.id.clone(),
            display_name: descriptor.display_name.clone(),
            architecture: descriptor.version.architecture.clone(),
            model_version: descriptor.version.model_version.clone(),
            connection,
            capabilities,
            input_contracts,
            output_contracts,
            prompt_contracts,
            score_semantics: descriptor.score_semantics,
            geometry_semantics: default_geometry_semantics(&descriptor.capabilities),
            label_space: (!descriptor.output_contract.label_space.is_empty())
                .then(|| descriptor.output_contract.label_space.clone()),
            checkpoint,
            runtime_requirements: descriptor.runtime_requirements.clone(),
            license: descriptor.license.clone(),
            availability,
            availability_evidence: evidence,
            metadata: BTreeMap::from([(
                "migrated_from_vision_descriptor".to_owned(),
                serde_json::Value::Bool(true),
            )]),
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

fn validate_contracts(contracts: &[ArtifactContract], direction: &str) -> CoreResult<()> {
    let mut names = BTreeSet::new();
    for contract in contracts {
        contract.validate()?;
        if !names.insert(contract.name.as_str()) {
            return Err(CoreError::Validation(format!(
                "Expert Model {direction} contract names must be unique"
            )));
        }
    }
    Ok(())
}

fn validate_unique_values(values: &[String], name: &str) -> CoreResult<()> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_identity(name, value, 256)?;
        if !unique.insert(value.as_str()) {
            return Err(CoreError::Validation(format!(
                "{name} values must be unique"
            )));
        }
    }
    Ok(())
}

fn validate_identity(name: &str, value: &str, max_len: usize) -> CoreResult<()> {
    if value.trim().is_empty() || value.len() > max_len || value.contains(['\r', '\n']) {
        return Err(CoreError::Validation(format!(
            "{name} must be non-empty, single-line and at most {max_len} bytes"
        )));
    }
    Ok(())
}

#[must_use]
pub const fn model_capability(capability: VisionCapability) -> Option<ModelCapability> {
    match capability {
        VisionCapability::VisionLanguage => Some(ModelCapability::VisionLanguage),
        VisionCapability::OpenVocabularyDetection => Some(ModelCapability::OpenVocabularyDetection),
        VisionCapability::PhraseGrounding => Some(ModelCapability::PhraseGrounding),
        VisionCapability::ObjectDetection => Some(ModelCapability::ObjectDetection),
        VisionCapability::SemanticSegmentation => Some(ModelCapability::SemanticSegmentation),
        VisionCapability::InstanceSegmentation => Some(ModelCapability::InstanceSegmentation),
        VisionCapability::PromptedSegmentation => Some(ModelCapability::PromptedSegmentation),
        VisionCapability::Classification => Some(ModelCapability::ImageClassification),
        VisionCapability::KeypointDetection => Some(ModelCapability::KeypointDetection),
        VisionCapability::Embedding => None,
    }
}

#[must_use]
pub const fn vision_capability(capability: ModelCapability) -> VisionCapability {
    match capability {
        ModelCapability::TextGeneration | ModelCapability::VisionLanguage => {
            VisionCapability::VisionLanguage
        }
        ModelCapability::ImageClassification => VisionCapability::Classification,
        ModelCapability::ObjectDetection => VisionCapability::ObjectDetection,
        ModelCapability::OpenVocabularyDetection => VisionCapability::OpenVocabularyDetection,
        ModelCapability::PhraseGrounding => VisionCapability::PhraseGrounding,
        ModelCapability::SemanticSegmentation => VisionCapability::SemanticSegmentation,
        ModelCapability::PromptedSegmentation => VisionCapability::PromptedSegmentation,
        ModelCapability::InstanceSegmentation => VisionCapability::InstanceSegmentation,
        ModelCapability::KeypointDetection => VisionCapability::KeypointDetection,
    }
}

#[must_use]
pub fn default_geometry_semantics(capabilities: &[VisionCapability]) -> GeometrySemantics {
    if capabilities.contains(&VisionCapability::PromptedSegmentation) {
        GeometrySemantics::RefinedGeometry
    } else if capabilities.contains(&VisionCapability::VisionLanguage) {
        GeometrySemantics::CoarseHypothesis
    } else if capabilities.iter().any(|capability| {
        matches!(
            capability,
            VisionCapability::ObjectDetection
                | VisionCapability::OpenVocabularyDetection
                | VisionCapability::PhraseGrounding
                | VisionCapability::InstanceSegmentation
                | VisionCapability::SemanticSegmentation
                | VisionCapability::KeypointDetection
        )
    }) {
        GeometrySemantics::PredictedGeometry
    } else {
        GeometrySemantics::NotApplicable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BackendDescriptor, ModelInputContract, ModelOutputContract, ModelVersionMetadata,
        VisionModelHealth,
    };

    fn prompted_manifest() -> ExpertModelManifest {
        ExpertModelManifest {
            schema_version: "1".to_owned(),
            model_id: "prompted-segmenter-v1".to_owned(),
            display_name: "Prompted segmenter".to_owned(),
            architecture: Some("example-architecture".to_owned()),
            model_version: "1".to_owned(),
            connection: ModelConnection::VisionWorkerModel {
                worker_id: "worker-local".to_owned(),
                worker_model_id: "prompted-segmenter-v1".to_owned(),
            },
            capabilities: BTreeSet::from([ModelCapability::PromptedSegmentation]),
            input_contracts: vec![
                ArtifactContract::artifact("image", ArtifactKind::Image, true, false),
                ArtifactContract::artifact("box_prompts", ArtifactKind::BoxPromptSet, true, true),
            ],
            output_contracts: vec![ArtifactContract::artifact(
                "masks",
                ArtifactKind::MaskSet,
                true,
                true,
            )],
            prompt_contracts: vec![PromptContract {
                kind: PromptKind::Box,
                required: true,
                multiple: true,
            }],
            score_semantics: ScoreSemantics::RelativeConfidence,
            geometry_semantics: GeometrySemantics::RefinedGeometry,
            label_space: None,
            checkpoint: Some(CheckpointIdentity {
                sha256: "a".repeat(64),
                source: None,
                training_dataset_version: None,
            }),
            runtime_requirements: RuntimeRequirements::default(),
            license: LicenseMetadata::default(),
            availability: ModelAvailability::Available,
            availability_evidence: ModelAvailabilityEvidence {
                health_passed: true,
                protocol_compatible: true,
                contracts_validated: true,
                sample_conversion_passed: true,
                weights_ready: true,
                checked_at: Some(chrono::Utc::now()),
                detail: Some("sample conversion passed".to_owned()),
            },
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn prompted_segmentation_requires_prompts_mask_geometry_and_availability_evidence() {
        let manifest = prompted_manifest();
        manifest.validate().expect("valid prompted segmenter");

        let mut missing_prompt = manifest.clone();
        missing_prompt.prompt_contracts.clear();
        assert!(missing_prompt.validate().is_err());

        let mut unavailable_evidence = manifest;
        unavailable_evidence
            .availability_evidence
            .sample_conversion_passed = false;
        assert!(unavailable_evidence.validate().is_err());
    }

    #[test]
    fn legacy_http_descriptor_migrates_without_claiming_available() {
        let descriptor = VisionModelDescriptor {
            id: "worker-detector".to_owned(),
            display_name: "Worker detector".to_owned(),
            backend_id: "worker".to_owned(),
            backend: BackendDescriptor {
                kind: Some(VisionBackendKind::HttpVision),
                protocol_version: Some("1".to_owned()),
                endpoint: Some("http://127.0.0.1:9000".to_owned()),
            },
            capabilities: vec![VisionCapability::ObjectDetection],
            version: ModelVersionMetadata {
                model_version: "1".to_owned(),
                ..ModelVersionMetadata::default()
            },
            input_contract: ModelInputContract {
                input_types: vec![VisionInputType::Image],
                ..ModelInputContract::default()
            },
            output_contract: ModelOutputContract {
                output_types: vec![ArtifactKind::DetectionSet],
                ..ModelOutputContract::default()
            },
            status: ModelAvailabilityStatus::Available,
            health: VisionModelHealth {
                status: VisionModelHealthStatus::Healthy,
                ..VisionModelHealth::default()
            },
            ..VisionModelDescriptor::default()
        };
        let manifest =
            ExpertModelManifest::from_vision_descriptor(&descriptor).expect("migrated manifest");
        assert_eq!(manifest.availability, ModelAvailability::Unknown);
        assert_eq!(
            manifest.geometry_semantics,
            GeometrySemantics::PredictedGeometry
        );
        assert!(matches!(
            manifest.connection,
            ModelConnection::VisionWorkerModel { .. }
        ));
    }

    #[test]
    fn provider_model_profile_exposes_credential_free_connection() {
        let provider_id = ProviderId::new();
        let profile = ModelProfile {
            id: crate::ModelProfileId::new(),
            revision: 1,
            provider_id,
            display_name: "Vision model".to_owned(),
            remote_model_id: "remote-model".to_owned(),
            input_modalities: BTreeSet::from([crate::InputModality::Image]),
            protocol_features: crate::ProtocolFeatures::default(),
            task_capabilities: BTreeSet::from([ModelCapability::VisionLanguage]),
            capability_source: crate::CapabilityDeclarationSource::UserDeclared,
            limits: crate::ModelLimits::default(),
            generation_defaults: crate::GenerationDefaults::default(),
            pricing: crate::ModelPricing::default(),
            quality_contracts: Vec::new(),
            status: crate::ModelProfileStatus::Unverified,
            enabled: true,
            locked: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(
            profile.connection(),
            ModelConnection::ProviderModel {
                provider_id,
                remote_model_id: "remote-model".to_owned(),
            }
        );
    }
}
