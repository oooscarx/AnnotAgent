#![forbid(unsafe_code)]

//! Scripted Rust protocol fixture for the unsupported `LocateAnything` contract.
//! This binary is test-only evidence and is never copied into the production `.annotplugin`.

use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ArtifactValidationState, DetectionArtifactItem, DetectionScore,
    DetectionSetArtifact, DetectionSource, NormalizedRect, PipelineArtifact,
    PipelineInferenceRequest, PipelineInferenceResponse, ScoreSemantics, VisionCapability,
};
use annotagent_plugin_api::{
    ModelRuntimeDescriptor, PLUGIN_API_VERSION, PLUGIN_PROTOCOL_VERSION, PluginManifest,
    PluginRuntimeDescriptor,
};
use annotagent_plugin_sdk::{
    ExpertModelPlugin, InferenceContext, PluginSdkError, PluginServer, WarmupContext,
};
use async_trait::async_trait;

struct ScriptedLocateAnythingFixture {
    manifest: PluginManifest,
}

impl ScriptedLocateAnythingFixture {
    fn load() -> Result<Self, PluginSdkError> {
        let manifest = PluginManifest::from_toml(include_str!("../annotagent-plugin.toml"))
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        Ok(Self { manifest })
    }
}

#[async_trait]
impl ExpertModelPlugin for ScriptedLocateAnythingFixture {
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
            loaded: true,
            checkpoint_sha256: None,
            device: "protocol-fixture".to_owned(),
        }]
    }

    async fn warmup(&self, model_id: &str, _context: WarmupContext) -> Result<(), PluginSdkError> {
        if model_id == self.manifest.models[0].id {
            Ok(())
        } else {
            Err(PluginSdkError::Plugin("unknown model".to_owned()))
        }
    }

    async fn infer(
        &self,
        request: PipelineInferenceRequest,
        context: InferenceContext,
    ) -> Result<PipelineInferenceResponse, PluginSdkError> {
        if context.cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
        }
        if !matches!(
            request.operation,
            VisionCapability::OpenVocabularyDetection | VisionCapability::PhraseGrounding
        ) {
            return Err(PluginSdkError::Plugin(
                "fixture only accepts open-vocabulary detection or phrase grounding".to_owned(),
            ));
        }
        let queries = fixture_queries(&request)?;
        let artifact_id = format!("locate-anything-fixture:{}", request.request_id);
        let detections = queries
            .into_iter()
            .enumerate()
            .map(|(index, query)| {
                let inset = (index as f32 * 0.05).min(0.2);
                DetectionArtifactItem::from_source(
                    format!("fixture-{index}"),
                    Some(query.clone()),
                    Some(query),
                    None,
                    NormalizedRect::new(
                        0.1 + inset,
                        0.1 + inset,
                        0.8 - inset * 2.0,
                        0.8 - inset * 2.0,
                    )
                    .map_err(|error| PluginSdkError::Plugin(error.to_string()))?,
                    DetectionScore::new(None, ScoreSemantics::NotProvided)
                        .map_err(PluginSdkError::Plugin)?,
                    DetectionSource {
                        model_id: request.model_id.clone(),
                        capability: request.operation,
                        artifact_id: artifact_id.clone(),
                    },
                )
                .map_err(PluginSdkError::Plugin)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PipelineInferenceResponse {
            request_id: Some(request.request_id),
            model_identity: Some(request.model_id.clone()),
            artifacts: vec![PipelineArtifact::DetectionSet(DetectionSetArtifact {
                schema_version: annotagent_core::DETECTION_ARTIFACT_SCHEMA_VERSION,
                reference: ArtifactRef {
                    artifact_id,
                    source_node: request.node_id,
                    port: "detections".to_owned(),
                    artifact_type: ArtifactKind::DetectionSet,
                    item_id: None,
                },
                image_id: request.image_id,
                model_binding: request.model_id,
                validation_state: ArtifactValidationState::Unvalidated,
                detections,
                metadata: BTreeMap::from([
                    ("protocol_fixture".to_owned(), serde_json::json!(true)),
                    ("real_inference".to_owned(), serde_json::json!(false)),
                ]),
            })],
            metadata: BTreeMap::from([(
                "unsupported_production_model".to_owned(),
                serde_json::json!(true),
            )]),
            ..PipelineInferenceResponse::default()
        })
    }

    async fn cancel(&self, _request_id: &str) -> Result<(), PluginSdkError> {
        Ok(())
    }
}

fn fixture_queries(request: &PipelineInferenceRequest) -> Result<Vec<String>, PluginSdkError> {
    let values = if let Some(values) = request
        .parameters
        .get("queries")
        .and_then(serde_json::Value::as_array)
    {
        values
            .iter()
            .map(|value| value.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
    } else {
        request
            .parameters
            .get("query")
            .and_then(serde_json::Value::as_str)
            .map(|value| vec![value.to_owned()])
    }
    .ok_or_else(|| PluginSdkError::Plugin("fixture requires query or queries".to_owned()))?;
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return Err(PluginSdkError::Plugin(
            "fixture queries must be non-empty".to_owned(),
        ));
    }
    Ok(values)
}

#[tokio::main]
async fn main() -> Result<(), PluginSdkError> {
    PluginServer::run_from_stdin(Arc::new(ScriptedLocateAnythingFixture::load()?)).await
}
