//! Durable, revisioned model profiles and explicit binding resolution.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::{
    ModelBindingId, ModelProfileId, ProjectId, ProviderAdapterKind, ProviderHealthStatus,
    ProviderId, ProviderProfile,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputModality {
    Text,
    Image,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    TextGeneration,
    VisionLanguage,
    ImageClassification,
    ObjectDetection,
    OpenVocabularyDetection,
    PhraseGrounding,
    SemanticSegmentation,
    PromptedSegmentation,
    InstanceSegmentation,
    KeypointDetection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ProtocolFeatures {
    pub tool_calls: bool,
    pub parallel_tool_calls: bool,
    pub structured_output: bool,
    pub json_schema: bool,
    pub usage_reporting: bool,
    pub streaming: bool,
    pub reasoning_controls: bool,
}

impl ProtocolFeatures {
    #[must_use]
    pub const fn supports(&self, required: &Self) -> bool {
        (!required.tool_calls || self.tool_calls)
            && (!required.parallel_tool_calls || self.parallel_tool_calls)
            && (!required.structured_output || self.structured_output)
            && (!required.json_schema || self.json_schema)
            && (!required.usage_reporting || self.usage_reporting)
            && (!required.streaming || self.streaming)
            && (!required.reasoning_controls || self.reasoning_controls)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ModelLimits {
    pub context_tokens: Option<u64>,
    pub maximum_output_tokens: Option<u64>,
    pub maximum_images_per_request: Option<u32>,
    pub maximum_image_pixels: Option<u64>,
}

impl ModelLimits {
    fn validate(&self) -> Result<(), ModelProfileValidationError> {
        for (name, value) in [
            ("context_tokens", self.context_tokens),
            ("maximum_output_tokens", self.maximum_output_tokens),
            ("maximum_image_pixels", self.maximum_image_pixels),
        ] {
            if value == Some(0) {
                return Err(ModelProfileValidationError::InvalidLimits(format!(
                    "{name} must be greater than zero when configured"
                )));
            }
        }
        if self.maximum_images_per_request == Some(0) {
            return Err(ModelProfileValidationError::InvalidLimits(
                "maximum_images_per_request must be greater than zero when configured".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct GenerationDefaults {
    pub temperature: Option<Decimal>,
    pub top_p: Option<Decimal>,
    pub maximum_output_tokens: Option<u64>,
    pub structured_output_mode: Option<String>,
    pub reasoning_mode: Option<String>,
    pub image_detail: Option<String>,
    pub system_prompt_version: Option<String>,
}

impl GenerationDefaults {
    fn validate(&self) -> Result<(), ModelProfileValidationError> {
        if self
            .temperature
            .is_some_and(|value| value < Decimal::ZERO || value > Decimal::from(2))
        {
            return Err(ModelProfileValidationError::InvalidGenerationDefaults(
                "temperature must be within 0..=2".to_owned(),
            ));
        }
        if self
            .top_p
            .is_some_and(|value| value <= Decimal::ZERO || value > Decimal::ONE)
        {
            return Err(ModelProfileValidationError::InvalidGenerationDefaults(
                "top_p must be within (0, 1]".to_owned(),
            ));
        }
        if self.maximum_output_tokens == Some(0) {
            return Err(ModelProfileValidationError::InvalidGenerationDefaults(
                "maximum_output_tokens must be greater than zero".to_owned(),
            ));
        }
        for (name, value) in [
            ("structured_output_mode", &self.structured_output_mode),
            ("reasoning_mode", &self.reasoning_mode),
            ("image_detail", &self.image_detail),
            ("system_prompt_version", &self.system_prompt_version),
        ] {
            if value.as_ref().is_some_and(|value| {
                value.trim().is_empty() || value.len() > 120 || value.contains(['\r', '\n'])
            }) {
                return Err(ModelProfileValidationError::InvalidGenerationDefaults(
                    format!("{name} must be non-empty, single-line and at most 120 bytes"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingSource {
    UserConfigured,
    ProviderDiscovered,
    Preset,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPricing {
    pub currency: String,
    pub input_per_million_tokens: Option<Decimal>,
    pub output_per_million_tokens: Option<Decimal>,
    pub cached_input_per_million_tokens: Option<Decimal>,
    pub per_image: Option<Decimal>,
    pub per_request: Option<Decimal>,
    pub source: PricingSource,
    pub updated_at: Option<DateTime<Utc>>,
}

impl Default for ModelPricing {
    fn default() -> Self {
        Self {
            currency: "USD".to_owned(),
            input_per_million_tokens: None,
            output_per_million_tokens: None,
            cached_input_per_million_tokens: None,
            per_image: None,
            per_request: None,
            source: PricingSource::Unknown,
            updated_at: None,
        }
    }
}

impl ModelPricing {
    fn validate(&self) -> Result<(), ModelProfileValidationError> {
        if self.currency.len() != 3
            || !self
                .currency
                .chars()
                .all(|character| character.is_ascii_uppercase())
        {
            return Err(ModelProfileValidationError::InvalidPricing(
                "currency must be a three-letter uppercase code".to_owned(),
            ));
        }
        if [
            self.input_per_million_tokens,
            self.output_per_million_tokens,
            self.cached_input_per_million_tokens,
            self.per_image,
            self.per_request,
        ]
        .into_iter()
        .flatten()
        .any(|value| value < Decimal::ZERO)
        {
            return Err(ModelProfileValidationError::InvalidPricing(
                "pricing values cannot be negative".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPricingSnapshot {
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,
    pub pricing: ModelPricing,
    pub captured_at: DateTime<Utc>,
}

impl ModelPricingSnapshot {
    #[must_use]
    pub fn capture(model: &ModelProfile, captured_at: DateTime<Utc>) -> Self {
        Self {
            model_profile_id: model.id,
            model_profile_revision: model.revision,
            pricing: model.pricing.clone(),
            captured_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDeclarationSource {
    UserDeclared,
    ProviderDiscovered,
    Preset,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProfileStatus {
    Unknown,
    Unverified,
    Available,
    Unavailable,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelProfile {
    pub id: ModelProfileId,
    pub revision: u64,
    pub provider_id: ProviderId,
    pub display_name: String,
    pub remote_model_id: String,
    pub input_modalities: BTreeSet<InputModality>,
    pub protocol_features: ProtocolFeatures,
    pub task_capabilities: BTreeSet<ModelCapability>,
    pub capability_source: CapabilityDeclarationSource,
    pub limits: ModelLimits,
    pub generation_defaults: GenerationDefaults,
    pub pricing: ModelPricing,
    #[serde(default)]
    pub quality_contracts: Vec<crate::ModelCapabilityQualityContract>,
    pub status: ModelProfileStatus,
    pub enabled: bool,
    pub locked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ModelProfile {
    pub fn validate(&self) -> Result<(), ModelProfileValidationError> {
        if self.revision == 0 {
            return Err(ModelProfileValidationError::InvalidRevision);
        }
        if self.display_name.trim().is_empty() || self.display_name.len() > 120 {
            return Err(ModelProfileValidationError::InvalidDisplayName);
        }
        if self.remote_model_id.trim().is_empty()
            || self.remote_model_id.len() > 512
            || self.remote_model_id.contains(['\r', '\n'])
        {
            return Err(ModelProfileValidationError::InvalidRemoteModelId);
        }
        if self.input_modalities.is_empty() {
            return Err(ModelProfileValidationError::MissingInputModality);
        }
        if self.task_capabilities.is_empty() {
            return Err(ModelProfileValidationError::MissingCapability);
        }
        if self
            .task_capabilities
            .contains(&ModelCapability::VisionLanguage)
            && !self.input_modalities.contains(&InputModality::Image)
        {
            return Err(ModelProfileValidationError::CapabilityModalityMismatch);
        }
        self.limits.validate()?;
        self.generation_defaults.validate()?;
        self.pricing.validate()?;
        let mut contract_keys = BTreeSet::new();
        for contract in &self.quality_contracts {
            contract
                .validate_for(self)
                .map_err(ModelProfileValidationError::InvalidQualityContract)?;
            if !contract_keys.insert((contract.capability, contract.operation.as_str())) {
                return Err(ModelProfileValidationError::DuplicateQualityContract);
            }
        }
        if self.enabled == (self.status == ModelProfileStatus::Disabled) {
            return Err(ModelProfileValidationError::DisabledStatusMismatch);
        }
        Ok(())
    }

    #[must_use]
    pub fn has_same_semantics(&self, other: &Self) -> bool {
        self.provider_id == other.provider_id
            && self.remote_model_id == other.remote_model_id
            && self.input_modalities == other.input_modalities
            && self.protocol_features == other.protocol_features
            && self.task_capabilities == other.task_capabilities
            && self.limits == other.limits
            && self.generation_defaults == other.generation_defaults
            && self.quality_contracts == other.quality_contracts
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModelProfileValidationError {
    #[error("Model Profile revision must be greater than zero")]
    InvalidRevision,
    #[error("Model Profile display name must be non-empty and at most 120 bytes")]
    InvalidDisplayName,
    #[error("remote model ID must be non-empty, single-line and at most 512 bytes")]
    InvalidRemoteModelId,
    #[error("Model Profile must declare at least one input modality")]
    MissingInputModality,
    #[error("Model Profile must declare at least one task capability")]
    MissingCapability,
    #[error("declared capability is incompatible with the input modalities")]
    CapabilityModalityMismatch,
    #[error("invalid Model limits: {0}")]
    InvalidLimits(String),
    #[error("invalid generation defaults: {0}")]
    InvalidGenerationDefaults(String),
    #[error("invalid Model pricing: {0}")]
    InvalidPricing(String),
    #[error("invalid Model capability quality contract: {0}")]
    InvalidQualityContract(String),
    #[error("Model capability quality contracts must be unique by capability and operation")]
    DuplicateQualityContract,
    #[error("enabled state and Model Profile status disagree")]
    DisabledStatusMismatch,
}

/// Frozen semantic identity embedded in a published Workflow. Price is snapshotted per call and
/// credentials resolve at runtime, so neither belongs here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelProfileSnapshot {
    pub model_profile_id: ModelProfileId,
    pub revision: u64,
    pub provider_id: ProviderId,
    pub provider_adapter: ProviderAdapterKind,
    pub provider_base_url: Url,
    pub remote_model_id: String,
    pub input_modalities: BTreeSet<InputModality>,
    pub protocol_features: ProtocolFeatures,
    pub task_capabilities: BTreeSet<ModelCapability>,
    pub limits: ModelLimits,
    pub generation_defaults: GenerationDefaults,
    #[serde(default)]
    pub quality_contracts: Vec<crate::ModelCapabilityQualityContract>,
}

impl ModelProfileSnapshot {
    pub fn frozen(
        model: &ModelProfile,
        provider: &ProviderProfile,
    ) -> Result<Self, ModelProfileSnapshotError> {
        model.validate()?;
        provider.validate()?;
        if model.provider_id != provider.id {
            return Err(ModelProfileSnapshotError::ProviderMismatch);
        }
        if !model.enabled || model.status != ModelProfileStatus::Available {
            return Err(ModelProfileSnapshotError::ModelUnavailable);
        }
        if !provider.enabled
            || !matches!(
                provider.health.status,
                ProviderHealthStatus::Available | ProviderHealthStatus::Configured
            )
        {
            return Err(ModelProfileSnapshotError::ProviderUnavailable);
        }
        Ok(Self {
            model_profile_id: model.id,
            revision: model.revision,
            provider_id: provider.id,
            provider_adapter: provider.adapter,
            provider_base_url: provider.base_url.clone(),
            remote_model_id: model.remote_model_id.clone(),
            input_modalities: model.input_modalities.clone(),
            protocol_features: model.protocol_features.clone(),
            task_capabilities: model.task_capabilities.clone(),
            limits: model.limits.clone(),
            generation_defaults: model.generation_defaults.clone(),
            quality_contracts: crate::effective_model_quality_contracts(model),
        })
    }
}

#[derive(Debug, Error)]
pub enum ModelProfileSnapshotError {
    #[error(transparent)]
    Model(#[from] ModelProfileValidationError),
    #[error(transparent)]
    Provider(#[from] crate::ProviderProfileValidationError),
    #[error("Model Profile belongs to a different Provider")]
    ProviderMismatch,
    #[error("Model Profile is not enabled and available")]
    ModelUnavailable,
    #[error("Provider Profile is not enabled and available")]
    ProviderUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelBindingRole {
    PipelineBuilder,
    PrimaryInference,
    Detection,
    Classification,
    Segmentation,
    Verification,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelBindingMatch {
    Capability,
    Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectModelBinding {
    pub id: ModelBindingId,
    pub project_id: ProjectId,
    pub capability: ModelCapability,
    pub role: ModelBindingRole,
    pub match_kind: ModelBindingMatch,
    pub model_profile_id: ModelProfileId,
    pub locked: bool,
    pub created_at: DateTime<Utc>,
}

impl ProjectModelBinding {
    pub fn validate_for_model(&self, model: &ModelProfile) -> Result<(), ModelBindingError> {
        if self.model_profile_id != model.id {
            return Err(ModelBindingError::ModelIdentityMismatch);
        }
        if !model.enabled || model.status == ModelProfileStatus::Disabled {
            return Err(ModelBindingError::ModelUnavailable);
        }
        if !model.task_capabilities.contains(&self.capability) {
            return Err(ModelBindingError::CapabilityMismatch);
        }
        Ok(())
    }

    pub fn authorize_replacement(
        &self,
        actor: BindingMutationActor,
    ) -> Result<(), ModelBindingError> {
        if self.locked && actor == BindingMutationActor::Agent {
            Err(ModelBindingError::LockedForAgent)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingMutationActor {
    User,
    Agent,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModelBindingError {
    #[error("binding references a Model Profile that does not exist")]
    MissingModelProfile,
    #[error("binding references a different Model Profile")]
    ModelIdentityMismatch,
    #[error("binding capability is not declared by the Model Profile")]
    CapabilityMismatch,
    #[error("Pipeline Builder binding requires model tool-call support")]
    ProtocolMismatch,
    #[error("binding references a disabled or unavailable Model Profile")]
    ModelUnavailable,
    #[error("Agent cannot replace a locked model binding")]
    LockedForAgent,
    #[error("unresolved model binding")]
    Unresolved,
    #[error("multiple bindings have the same priority")]
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct GlobalModelDefaults {
    pub pipeline_builder: Option<ModelProfileId>,
    pub vision_language: Option<ModelProfileId>,
    pub text_generation: Option<ModelProfileId>,
}

impl GlobalModelDefaults {
    #[must_use]
    pub const fn select(
        &self,
        role: ModelBindingRole,
        capability: ModelCapability,
    ) -> Option<ModelProfileId> {
        if matches!(role, ModelBindingRole::PipelineBuilder) {
            self.pipeline_builder
        } else if matches!(capability, ModelCapability::VisionLanguage) {
            self.vision_language
        } else if matches!(capability, ModelCapability::TextGeneration) {
            self.text_generation
        } else {
            None
        }
    }

    pub fn validate(
        &self,
        models: &BTreeMap<ModelProfileId, ModelProfile>,
    ) -> Result<(), ModelBindingError> {
        if let Some(id) = self.pipeline_builder {
            let model = models
                .get(&id)
                .ok_or(ModelBindingError::MissingModelProfile)?;
            if !model.enabled || model.status != ModelProfileStatus::Available {
                return Err(ModelBindingError::ModelUnavailable);
            }
            if !model
                .task_capabilities
                .contains(&ModelCapability::TextGeneration)
            {
                return Err(ModelBindingError::CapabilityMismatch);
            }
            if !model.protocol_features.tool_calls || !model.protocol_features.structured_output {
                return Err(ModelBindingError::ProtocolMismatch);
            }
        }
        for (id, capability) in [
            (self.vision_language, ModelCapability::VisionLanguage),
            (self.text_generation, ModelCapability::TextGeneration),
        ] {
            if let Some(id) = id {
                let model = models
                    .get(&id)
                    .ok_or(ModelBindingError::MissingModelProfile)?;
                if !model.enabled || model.status != ModelProfileStatus::Available {
                    return Err(ModelBindingError::ModelUnavailable);
                }
                if !model.task_capabilities.contains(&capability) {
                    return Err(ModelBindingError::CapabilityMismatch);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelBindingSource {
    WorkflowNode,
    ProjectCapability,
    ProjectRole,
    GlobalDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedModelBinding {
    pub model_profile_id: ModelProfileId,
    pub source: ModelBindingSource,
    pub locked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowModelBinding {
    pub model_profile_id: ModelProfileId,
    pub locked: bool,
}

pub fn resolve_model_binding(
    explicit: Option<ModelProfileId>,
    project_bindings: &[ProjectModelBinding],
    defaults: &GlobalModelDefaults,
    capability: ModelCapability,
    role: ModelBindingRole,
) -> Result<ResolvedModelBinding, ModelBindingError> {
    if let Some(model_profile_id) = explicit {
        return Ok(ResolvedModelBinding {
            model_profile_id,
            source: ModelBindingSource::WorkflowNode,
            locked: true,
        });
    }
    for (match_kind, source) in [
        (
            ModelBindingMatch::Capability,
            ModelBindingSource::ProjectCapability,
        ),
        (ModelBindingMatch::Role, ModelBindingSource::ProjectRole),
    ] {
        let matches = project_bindings
            .iter()
            .filter(|binding| {
                binding.match_kind == match_kind
                    && match match_kind {
                        ModelBindingMatch::Capability => binding.capability == capability,
                        ModelBindingMatch::Role => binding.role == role,
                    }
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {}
            [binding] => {
                return Ok(ResolvedModelBinding {
                    model_profile_id: binding.model_profile_id,
                    source,
                    locked: binding.locked,
                });
            }
            _ => return Err(ModelBindingError::Ambiguous),
        }
    }
    defaults
        .select(role, capability)
        .map(|model_profile_id| ResolvedModelBinding {
            model_profile_id,
            source: ModelBindingSource::GlobalDefault,
            locked: false,
        })
        .ok_or(ModelBindingError::Unresolved)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ModelRequirements {
    pub input_modalities: BTreeSet<InputModality>,
    pub protocol_features: ProtocolFeatures,
    pub task_capabilities: BTreeSet<ModelCapability>,
    pub allow_unverified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCompatibilityIssue {
    ProviderMissing,
    ProviderDisabled,
    ProviderUnavailable,
    MissingCredential,
    ModelDisabled,
    ModelUnavailable,
    MissingInputModality,
    MissingProtocolFeature,
    MissingTaskCapability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCompatibility {
    pub model_profile_id: ModelProfileId,
    pub revision: u64,
    pub compatible: bool,
    pub issues: BTreeSet<ModelCompatibilityIssue>,
}

#[must_use]
pub fn check_model_compatibility(
    model: &ModelProfile,
    provider: Option<&ProviderProfile>,
    credential_configured: bool,
    requirements: &ModelRequirements,
) -> ModelCompatibility {
    let mut issues = BTreeSet::new();
    if !model.enabled {
        issues.insert(ModelCompatibilityIssue::ModelDisabled);
    }
    if model.status != ModelProfileStatus::Available
        && !(requirements.allow_unverified && model.status == ModelProfileStatus::Unverified)
    {
        issues.insert(ModelCompatibilityIssue::ModelUnavailable);
    }
    if !requirements
        .input_modalities
        .is_subset(&model.input_modalities)
    {
        issues.insert(ModelCompatibilityIssue::MissingInputModality);
    }
    if !model
        .protocol_features
        .supports(&requirements.protocol_features)
    {
        issues.insert(ModelCompatibilityIssue::MissingProtocolFeature);
    }
    if !requirements
        .task_capabilities
        .is_subset(&model.task_capabilities)
    {
        issues.insert(ModelCompatibilityIssue::MissingTaskCapability);
    }
    if let Some(provider) = provider {
        if provider.id != model.provider_id {
            issues.insert(ModelCompatibilityIssue::ProviderMissing);
        }
        if !provider.enabled || provider.health.status == ProviderHealthStatus::Disabled {
            issues.insert(ModelCompatibilityIssue::ProviderDisabled);
        }
        if !matches!(
            provider.health.status,
            ProviderHealthStatus::Available | ProviderHealthStatus::Configured
        ) {
            issues.insert(ModelCompatibilityIssue::ProviderUnavailable);
        }
        if provider.adapter != ProviderAdapterKind::Mock && !credential_configured {
            issues.insert(ModelCompatibilityIssue::MissingCredential);
        }
    } else {
        issues.insert(ModelCompatibilityIssue::ProviderMissing);
    }
    ModelCompatibility {
        model_profile_id: model.id,
        revision: model.revision,
        compatible: issues.is_empty(),
        issues,
    }
}

#[must_use]
pub fn list_compatible_models<'a>(
    models: &'a [ModelProfile],
    providers: &BTreeMap<ProviderId, ProviderProfile>,
    configured_credentials: &BTreeSet<ProviderId>,
    requirements: &ModelRequirements,
) -> Vec<&'a ModelProfile> {
    let mut compatible = models
        .iter()
        .filter(|model| {
            check_model_compatibility(
                model,
                providers.get(&model.provider_id),
                configured_credentials.contains(&model.provider_id),
                requirements,
            )
            .compatible
        })
        .collect::<Vec<_>>();
    compatible.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| right.revision.cmp(&left.revision))
    });
    compatible
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfrastructureFailureKind {
    Timeout,
    RateLimited,
    ProviderUnavailable,
    TemporaryServerFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRoute {
    pub primary: ModelProfileId,
    pub fallbacks: Vec<ModelProfileId>,
    pub fallback_on: BTreeSet<InfrastructureFailureKind>,
    pub maximum_fallbacks: u32,
}

impl ProviderRoute {
    pub fn validate(&self) -> Result<(), ProviderRouteError> {
        if self.maximum_fallbacks > 8
            || usize::try_from(self.maximum_fallbacks).unwrap_or(usize::MAX) > self.fallbacks.len()
        {
            return Err(ProviderRouteError::InvalidMaximumFallbacks);
        }
        if self.fallback_on.is_empty() && self.maximum_fallbacks > 0 {
            return Err(ProviderRouteError::MissingInfrastructureFailure);
        }
        let unique = self.fallbacks.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != self.fallbacks.len() || unique.contains(&self.primary) {
            return Err(ProviderRouteError::DuplicateModel);
        }
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderRouteError {
    #[error("maximum_fallbacks must fit the configured route and cannot exceed 8")]
    InvalidMaximumFallbacks,
    #[error("Provider fallback requires an infrastructure failure condition")]
    MissingInfrastructureFailure,
    #[error("Provider route contains duplicate Model Profiles")]
    DuplicateModel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CredentialReference, CredentialSource, ProviderConnectionPolicy, ProviderHealthSnapshot,
    };

    fn provider(adapter: ProviderAdapterKind) -> ProviderProfile {
        let id = ProviderId::new();
        let now = Utc::now();
        ProviderProfile {
            id,
            display_name: "Lab Provider".to_owned(),
            preset_id: None,
            adapter,
            base_url: Url::parse("https://provider.example/v1").expect("URL"),
            organization: None,
            workspace: None,
            credential_ref: (adapter != ProviderAdapterKind::Mock).then(|| CredentialReference {
                provider_id: id,
                source: CredentialSource::SystemKeyring,
                locator: "lab-provider".to_owned(),
            }),
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
        }
    }

    fn model(provider_id: ProviderId) -> ModelProfile {
        let now = Utc::now();
        ModelProfile {
            id: ModelProfileId::new(),
            revision: 1,
            provider_id,
            display_name: "Qwen Vision".to_owned(),
            remote_model_id: "qwen-vision".to_owned(),
            input_modalities: BTreeSet::from([InputModality::Text, InputModality::Image]),
            protocol_features: ProtocolFeatures {
                tool_calls: true,
                structured_output: true,
                json_schema: true,
                usage_reporting: true,
                ..ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([
                ModelCapability::VisionLanguage,
                ModelCapability::ImageClassification,
            ]),
            capability_source: CapabilityDeclarationSource::UserDeclared,
            limits: ModelLimits {
                context_tokens: Some(32_768),
                maximum_output_tokens: Some(4_096),
                maximum_images_per_request: Some(8),
                maximum_image_pixels: Some(16_000_000),
            },
            generation_defaults: GenerationDefaults {
                temperature: Some(Decimal::new(1, 1)),
                structured_output_mode: Some("json_schema".to_owned()),
                ..GenerationDefaults::default()
            },
            pricing: ModelPricing::default(),
            quality_contracts: Vec::new(),
            status: ModelProfileStatus::Available,
            enabled: true,
            locked: false,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn semantic_changes_require_a_distinct_revision_basis_but_pricing_does_not() {
        let provider = provider(ProviderAdapterKind::OpenAiCompatible);
        let first = model(provider.id);
        let mut price_update = first.clone();
        price_update.pricing.per_request = Some(Decimal::new(1, 3));
        assert!(first.has_same_semantics(&price_update));
        let mut semantic_update = price_update;
        semantic_update.remote_model_id = "qwen-vision-new".to_owned();
        assert!(!first.has_same_semantics(&semantic_update));

        let mut quality_update = first.clone();
        quality_update.quality_contracts = crate::effective_model_quality_contracts(&first);
        assert!(!first.has_same_semantics(&quality_update));
    }

    #[test]
    fn compatibility_fails_closed_on_provider_credential_protocol_and_capability() {
        let provider = provider(ProviderAdapterKind::OpenAiCompatible);
        let model = model(provider.id);
        let requirements = ModelRequirements {
            input_modalities: BTreeSet::from([InputModality::Image]),
            protocol_features: ProtocolFeatures {
                tool_calls: true,
                json_schema: true,
                ..ProtocolFeatures::default()
            },
            task_capabilities: BTreeSet::from([ModelCapability::VisionLanguage]),
            allow_unverified: false,
        };
        assert!(check_model_compatibility(&model, Some(&provider), true, &requirements).compatible);
        let missing = check_model_compatibility(&model, Some(&provider), false, &requirements);
        assert!(!missing.compatible);
        assert!(
            missing
                .issues
                .contains(&ModelCompatibilityIssue::MissingCredential)
        );
        let unsupported = ModelRequirements {
            task_capabilities: BTreeSet::from([ModelCapability::ObjectDetection]),
            ..requirements.clone()
        };
        assert!(
            check_model_compatibility(&model, Some(&provider), true, &unsupported)
                .issues
                .contains(&ModelCompatibilityIssue::MissingTaskCapability)
        );
        let mut incompatible = model.clone();
        incompatible.id = ModelProfileId::new();
        incompatible.task_capabilities = BTreeSet::from([ModelCapability::TextGeneration]);
        let candidates = vec![incompatible, model.clone()];
        let providers = BTreeMap::from([(provider.id, provider.clone())]);
        let credentials = BTreeSet::from([provider.id]);
        let listed = list_compatible_models(&candidates, &providers, &credentials, &requirements);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, model.id);
    }

    #[test]
    fn binding_priority_and_agent_lock_are_explicit() {
        let provider = provider(ProviderAdapterKind::Mock);
        let capability_model = model(provider.id);
        let mut role_model = model(provider.id);
        role_model.id = ModelProfileId::new();
        let project_id = ProjectId::new();
        let capability = ProjectModelBinding {
            id: ModelBindingId::new(),
            project_id,
            capability: ModelCapability::VisionLanguage,
            role: ModelBindingRole::PrimaryInference,
            match_kind: ModelBindingMatch::Capability,
            model_profile_id: capability_model.id,
            locked: true,
            created_at: Utc::now(),
        };
        let role = ProjectModelBinding {
            id: ModelBindingId::new(),
            model_profile_id: role_model.id,
            match_kind: ModelBindingMatch::Role,
            locked: false,
            ..capability.clone()
        };
        let resolved = resolve_model_binding(
            None,
            &[role, capability.clone()],
            &GlobalModelDefaults::default(),
            ModelCapability::VisionLanguage,
            ModelBindingRole::PrimaryInference,
        )
        .expect("binding");
        assert_eq!(resolved.model_profile_id, capability_model.id);
        assert_eq!(resolved.source, ModelBindingSource::ProjectCapability);
        assert!(matches!(
            capability.authorize_replacement(BindingMutationActor::Agent),
            Err(ModelBindingError::LockedForAgent)
        ));
        capability
            .authorize_replacement(BindingMutationActor::User)
            .expect("user may replace");
    }

    #[test]
    fn frozen_snapshot_excludes_price_and_credential_and_pins_provider_endpoint() {
        let provider = provider(ProviderAdapterKind::OpenAiCompatible);
        let model = model(provider.id);
        let snapshot = ModelProfileSnapshot::frozen(&model, &provider).expect("snapshot");
        let json = serde_json::to_string(&snapshot).expect("JSON");
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.provider_base_url, provider.base_url);
        assert_eq!(snapshot.quality_contracts.len(), 1);
        assert_eq!(
            snapshot.quality_contracts[0].output_geometry,
            crate::GeometrySemantics::CoarseHypothesis
        );
        assert!(!json.contains("credential"));
        assert!(!json.contains("pricing"));
        assert!(!json.contains("lab-provider"));

        let mut rotated_provider = provider.clone();
        rotated_provider
            .credential_ref
            .as_mut()
            .expect("credential")
            .locator = "rotated-account".to_owned();
        let mut repriced_model = model.clone();
        repriced_model.pricing.per_request = Some(Decimal::new(2, 3));
        assert_eq!(
            snapshot,
            ModelProfileSnapshot::frozen(&repriced_model, &rotated_provider)
                .expect("non-semantic snapshot")
        );
    }

    #[test]
    fn legacy_model_json_without_quality_contracts_migrates_conservatively() {
        let provider = provider(ProviderAdapterKind::OpenAiCompatible);
        let original = model(provider.id);
        let mut legacy = serde_json::to_value(&original).expect("legacy JSON");
        legacy
            .as_object_mut()
            .expect("Model Profile object")
            .remove("quality_contracts");
        let migrated: ModelProfile = serde_json::from_value(legacy).expect("migrated profile");
        assert!(migrated.quality_contracts.is_empty());
        let effective = crate::effective_model_quality_contracts(&migrated);
        assert_eq!(effective.len(), 1);
        assert_eq!(
            effective[0].score_semantics,
            crate::ScoreSemantics::SemanticConfidence
        );
        assert_eq!(
            effective[0].auto_accept_eligibility,
            crate::AutoAcceptEligibility::NeverFromScoreAlone
        );
    }

    #[test]
    fn provider_route_accepts_only_bounded_infrastructure_failures() {
        let route = ProviderRoute {
            primary: ModelProfileId::new(),
            fallbacks: vec![ModelProfileId::new()],
            fallback_on: BTreeSet::from([InfrastructureFailureKind::Timeout]),
            maximum_fallbacks: 1,
        };
        route.validate().expect("valid route");
        let invalid = ProviderRoute {
            maximum_fallbacks: 2,
            ..route
        };
        assert!(invalid.validate().is_err());
    }
}
