#![forbid(unsafe_code)]

use annotagent_core::{PipelineInferenceRequest, PipelineInferenceResponse};
use annotagent_plugin_api::{
    ModelRuntimeDescriptor, PLUGIN_API_VERSION, PLUGIN_PROTOCOL_VERSION, PluginManifest,
    PluginRuntimeDescriptor,
};
use annotagent_plugin_sdk::{
    ExpertModelPlugin, InferenceContext, PluginRuntimeContext, PluginSdkError, WarmupContext,
};
use async_trait::async_trait;

pub const UNSUPPORTED_REASON: &str =
    "unsupported until a verified Rust-callable LocateAnything model runtime is available";

pub struct LocateAnythingUnsupportedPlugin {
    manifest: PluginManifest,
}

impl LocateAnythingUnsupportedPlugin {
    pub fn load() -> Result<Self, PluginSdkError> {
        let manifest = PluginManifest::from_toml(include_str!("../annotagent-plugin.toml"))
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        Ok(Self { manifest })
    }
}

#[async_trait]
impl ExpertModelPlugin for LocateAnythingUnsupportedPlugin {
    async fn setup(&self, _context: PluginRuntimeContext) -> Result<(), PluginSdkError> {
        Err(PluginSdkError::Plugin(UNSUPPORTED_REASON.to_owned()))
    }

    fn descriptor(&self) -> PluginRuntimeDescriptor {
        PluginRuntimeDescriptor {
            plugin_id: self.manifest.id.clone(),
            plugin_version: self.manifest.version.clone(),
            plugin_api: PLUGIN_API_VERSION.to_owned(),
            protocol_version: PLUGIN_PROTOCOL_VERSION.to_owned(),
            capabilities: self.manifest.models[0].capabilities.clone(),
        }
    }

    fn models(&self) -> Vec<ModelRuntimeDescriptor> {
        vec![ModelRuntimeDescriptor {
            model: self.manifest.models[0].clone(),
            loaded: false,
            checkpoint_sha256: None,
            device: "unsupported".to_owned(),
        }]
    }

    async fn warmup(&self, _model_id: &str, _context: WarmupContext) -> Result<(), PluginSdkError> {
        Err(PluginSdkError::Plugin(UNSUPPORTED_REASON.to_owned()))
    }

    async fn infer(
        &self,
        _request: PipelineInferenceRequest,
        _context: InferenceContext,
    ) -> Result<PipelineInferenceResponse, PluginSdkError> {
        Err(PluginSdkError::Plugin(UNSUPPORTED_REASON.to_owned()))
    }

    async fn cancel(&self, _request_id: &str) -> Result<(), PluginSdkError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::{ArtifactKind, ContractDataType, ModelCapability};
    use annotagent_plugin_api::PluginImplementationStatus;

    #[test]
    fn manifest_is_non_selectable_and_keeps_both_capability_contracts() {
        let manifest =
            PluginManifest::from_toml(include_str!("../annotagent-plugin.toml")).expect("manifest");
        assert_eq!(
            manifest.implementation_status,
            PluginImplementationStatus::Unsupported
        );
        assert!(
            manifest.models[0]
                .capabilities
                .contains(&ModelCapability::OpenVocabularyDetection)
        );
        assert!(
            manifest.models[0]
                .capabilities
                .contains(&ModelCapability::PhraseGrounding)
        );
        assert_eq!(
            manifest.models[0].output_contracts[0].data_type,
            ContractDataType::Artifact(ArtifactKind::DetectionSet)
        );
    }
}
