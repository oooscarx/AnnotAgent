//! Formal Classification Skill for whole-image and crop subjects.

use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactKind, ArtifactRef, Classification, ClassificationSetArtifact, CoreError, CoreResult,
    LabelId, ModelImage, PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact,
    PipelineInferenceRequest, PipelineInferenceResponse, PipelineModelBackend, VisionCapability,
    VisionNodeDescriptor,
};
use annotagent_runtime::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};
use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio_util::sync::CancellationToken;

pub const CLASSIFICATION_SKILL_ID: &str = "classification";
pub const CLASSIFICATION_SKILL_VERSION: &str = "1";
pub const CLASSIFICATION_OPERATION: &str = "classification.classify";

#[must_use]
pub fn node_descriptor() -> VisionNodeDescriptor {
    VisionNodeDescriptor {
        id: CLASSIFICATION_OPERATION.to_owned(),
        display_name: "Classification".to_owned(),
        required_capabilities: vec![VisionCapability::Classification],
        accepts: vec![ArtifactKind::Image, ArtifactKind::CropSet],
        produces: vec![ArtifactKind::ClassificationSet],
        deterministic: false,
    }
}

pub struct ClassificationSkillRunner {
    backend: Arc<dyn PipelineModelBackend>,
    model_id: String,
    image: Option<ModelImage>,
}

impl ClassificationSkillRunner {
    pub fn new(
        backend: Arc<dyn PipelineModelBackend>,
        model_id: impl Into<String>,
        image: Option<ModelImage>,
    ) -> CoreResult<Self> {
        if backend.capability() != VisionCapability::Classification {
            return Err(CoreError::Validation(
                "Classification Skill requires a Classification backend".to_owned(),
            ));
        }
        Ok(Self {
            backend,
            model_id: model_id.into(),
            image,
        })
    }
}

#[async_trait]
impl DagNodeRunner for ClassificationSkillRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        if context.node.node_type != CLASSIFICATION_OPERATION {
            return Err(DagNodeFailure::terminal(
                "wrong_skill_operation",
                "Classification runner received another operation",
            ));
        }
        let model_id = context
            .node
            .model_binding
            .as_deref()
            .unwrap_or(&self.model_id);
        let response = self
            .backend
            .infer_pipeline(
                PipelineInferenceRequest {
                    protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
                    request_id: uuid::Uuid::new_v4().to_string(),
                    run_id: context.run_id,
                    image_id: context.image_id,
                    node_id: context.node.id.clone(),
                    model_id: model_id.to_owned(),
                    operation: VisionCapability::Classification,
                    image: self.image.clone(),
                    input_artifacts: context.input_pipeline_artifacts,
                    parameters: context.node.parameters.clone(),
                    timeout_ms: context
                        .node
                        .resources
                        .timeout_seconds
                        .map(|seconds| seconds.saturating_mul(1_000)),
                },
                context.cancellation,
            )
            .await
            .map_err(|error| {
                DagNodeFailure::retryable("classification_backend", error.to_string())
            })?;
        if let Some(error) = response.error {
            return Err(DagNodeFailure {
                code: error.code,
                summary: error.message,
                retryable: error.retryable,
            });
        }
        if response.artifacts.is_empty()
            || response.artifacts.iter().any(|artifact| {
                !matches!(artifact, PipelineArtifact::ClassificationSet(_))
                    || artifact.image_id() != context.image_id
                    || artifact.reference().source_node != context.node.id
            })
        {
            return Err(DagNodeFailure::terminal(
                "invalid_classification_output",
                "Classification backend must return scoped ClassificationSet Artifacts",
            ));
        }
        Ok(DagNodeOutput {
            pipeline_artifacts: response.artifacts,
            metadata: response.metadata,
            usage: annotagent_runtime::DagNodeUsage {
                input_tokens: 0,
                output_tokens: 0,
                cost: Decimal::ZERO,
            },
            ..DagNodeOutput::default()
        })
    }
}

#[derive(Debug, Clone)]
pub struct MockClassificationBackend {
    id: String,
}

impl MockClassificationBackend {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl PipelineModelBackend for MockClassificationBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn capability(&self) -> VisionCapability {
        VisionCapability::Classification
    }

    async fn infer_pipeline(
        &self,
        request: PipelineInferenceRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<PipelineInferenceResponse> {
        if cancellation.is_cancelled() {
            return Err(CoreError::Provider(
                "mock Classification inference cancelled".to_owned(),
            ));
        }
        let labels = request
            .parameters
            .get("labels")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(LabelId::from)
            .collect::<Vec<_>>();
        let label = request
            .parameters
            .get("mock_label")
            .and_then(serde_json::Value::as_str)
            .map(LabelId::from)
            .or_else(|| labels.first().cloned())
            .ok_or_else(|| {
                CoreError::Validation(
                    "mock Classification requires labels or mock_label".to_owned(),
                )
            })?;
        let confidence = request
            .parameters
            .get("mock_confidence")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.95) as f32;
        let subjects = subjects(&request.input_artifacts);
        if subjects.is_empty() {
            return Err(CoreError::Validation(
                "Classification requires Image or CropSet input".to_owned(),
            ));
        }
        let classifications = subjects
            .into_iter()
            .enumerate()
            .map(|(index, (subject, parent))| Classification {
                id: format!("classification-{index}"),
                subject,
                parent,
                label: label.clone(),
                confidence,
                scores: BTreeMap::from([(label.clone(), confidence)]),
            })
            .collect();
        let artifact = ClassificationSetArtifact {
            reference: ArtifactRef {
                artifact_id: format!("classification-set:{}", request.request_id),
                source_node: request.node_id,
                port: "classifications".to_owned(),
                artifact_type: ArtifactKind::ClassificationSet,
                item_id: None,
            },
            image_id: request.image_id,
            model_binding: request.model_id,
            validation_state: annotagent_core::ArtifactValidationState::Valid,
            classifications,
        };
        artifact.validate().map_err(CoreError::Validation)?;
        Ok(PipelineInferenceResponse {
            request_id: Some(request.request_id),
            model_identity: Some(self.id.clone()),
            artifacts: vec![PipelineArtifact::ClassificationSet(artifact)],
            metadata: BTreeMap::from([("mode".to_owned(), serde_json::json!("mock"))]),
            ..PipelineInferenceResponse::default()
        })
    }
}

fn subjects(inputs: &[PipelineArtifact]) -> Vec<(ArtifactRef, Option<ArtifactRef>)> {
    let mut subjects = Vec::new();
    for artifact in inputs {
        match artifact {
            PipelineArtifact::Image(image) => {
                subjects.push((image.reference.clone(), None));
            }
            PipelineArtifact::CropSet(crops) => {
                subjects.extend(
                    crops
                        .crops
                        .iter()
                        .map(|crop| (crops.reference.item(&crop.id), Some(crop.parent.clone()))),
                );
            }
            _ => {}
        }
    }
    subjects
}
