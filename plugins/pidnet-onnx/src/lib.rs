#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ArtifactValidationState, ImageArtifact, LabelId,
    PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact, PipelineInferenceRequest,
    PipelineInferenceResponse, SemanticMaskArtifact, VisionCapability,
};
use annotagent_model_runtime_common::{TensorLayout, image_to_tensor, resize};
use annotagent_model_runtime_onnx::{
    InferenceCancellation, NamedTensor, OnnxSession, SessionOptions, TensorData, TensorDescriptor,
};
use annotagent_plugin_api::{
    ModelRuntimeDescriptor, PLUGIN_API_VERSION, PLUGIN_PROTOCOL_VERSION, PluginManifest,
    PluginRuntimeDescriptor, Sha256Digest,
};
use annotagent_plugin_sdk::{
    ExpertModelPlugin, InferenceContext, PluginRuntimeContext, PluginSdkError, WarmupContext,
    decode_image,
};
use async_trait::async_trait;
use image::GenericImageView as _;

const MODEL_ID: &str = "pidnet-semantic-onnx";
const MODEL_FILENAME: &str = "pidnet.onnx";
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MIN_INPUT_SIZE: u32 = 32;
const MAX_INPUT_SIZE: u32 = 4096;
const DEFAULT_DYNAMIC_WIDTH: u32 = 512;
const DEFAULT_DYNAMIC_HEIGHT: u32 = 256;

pub struct PidNetOnnxPlugin {
    manifest: PluginManifest,
    loaded: RwLock<Option<LoadedModel>>,
}

struct LoadedModel {
    session: Arc<OnnxSession>,
    checkpoint_sha256: String,
    input_name: String,
}

impl PidNetOnnxPlugin {
    pub fn load() -> Result<Self, PluginSdkError> {
        let manifest = PluginManifest::from_toml(include_str!("../annotagent-plugin.toml"))
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        Ok(Self {
            manifest,
            loaded: RwLock::new(None),
        })
    }

    fn descriptor_model(&self) -> &annotagent_plugin_api::PluginModelManifest {
        &self.manifest.models[0]
    }

    fn loaded_model(&self) -> Result<(Arc<OnnxSession>, String, String), PluginSdkError> {
        let loaded = self
            .loaded
            .read()
            .map_err(|_| PluginSdkError::Plugin("model state lock was poisoned".to_owned()))?;
        let model = loaded
            .as_ref()
            .ok_or_else(|| PluginSdkError::Plugin("model weights are not loaded".to_owned()))?;
        Ok((
            Arc::clone(&model.session),
            model.checkpoint_sha256.clone(),
            model.input_name.clone(),
        ))
    }
}

#[async_trait]
impl ExpertModelPlugin for PidNetOnnxPlugin {
    async fn setup(&self, context: PluginRuntimeContext) -> Result<(), PluginSdkError> {
        let model_path = context
            .model_files
            .get("model")
            .cloned()
            .map_or_else(|| find_component(&context.weights_dir, MODEL_FILENAME), Ok)?;
        let session = tokio::task::spawn_blocking(move || {
            OnnxSession::load(model_path, &SessionOptions::default())
        })
        .await
        .map_err(|error| PluginSdkError::Plugin(format!("model setup task failed: {error}")))?
        .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        validate_contract(&session)?;
        let checkpoint_sha256 = session.descriptor().sha256.clone();
        let input_name = session.descriptor().inputs[0].name.clone();
        *self
            .loaded
            .write()
            .map_err(|_| PluginSdkError::Plugin("model state lock was poisoned".to_owned()))? =
            Some(LoadedModel {
                session: Arc::new(session),
                checkpoint_sha256,
                input_name,
            });
        Ok(())
    }

    fn descriptor(&self) -> PluginRuntimeDescriptor {
        PluginRuntimeDescriptor {
            plugin_id: self.manifest.id.clone(),
            plugin_version: self.manifest.version.clone(),
            plugin_api: PLUGIN_API_VERSION.to_owned(),
            protocol_version: PLUGIN_PROTOCOL_VERSION.to_owned(),
            capabilities: self.descriptor_model().capabilities.clone(),
        }
    }

    fn models(&self) -> Vec<ModelRuntimeDescriptor> {
        let checkpoint = self
            .loaded
            .read()
            .ok()
            .and_then(|loaded| loaded.as_ref().map(|model| model.checkpoint_sha256.clone()));
        vec![ModelRuntimeDescriptor {
            model: self.descriptor_model().clone(),
            loaded: checkpoint.is_some(),
            checkpoint_sha256: checkpoint.and_then(|value| Sha256Digest::parse(value).ok()),
            device: "cpu".to_owned(),
        }]
    }

    async fn warmup(&self, model_id: &str, context: WarmupContext) -> Result<(), PluginSdkError> {
        if model_id != MODEL_ID {
            return Err(PluginSdkError::Plugin("unknown model".to_owned()));
        }
        if context.cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("warmup cancelled".to_owned()));
        }
        let (session, _, input_name) = self.loaded_model()?;
        let (width, height) =
            input_dimensions(&session.descriptor().inputs[0], &BTreeMap::new(), true)?;
        let input = NamedTensor {
            name: input_name,
            shape: vec![1, 3, height as usize, width as usize],
            data: TensorData::Float32(vec![0.0; 3 * width as usize * height as usize]),
        };
        tokio::task::spawn_blocking(move || {
            session.warmup(&[input], 1, &InferenceCancellation::default())
        })
        .await
        .map_err(|error| PluginSdkError::Plugin(format!("warmup task failed: {error}")))?
        .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        if context.cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("warmup cancelled".to_owned()));
        }
        Ok(())
    }

    async fn infer(
        &self,
        request: PipelineInferenceRequest,
        context: InferenceContext,
    ) -> Result<PipelineInferenceResponse, PluginSdkError> {
        if request.model_id != MODEL_ID
            || request.operation != VisionCapability::SemanticSegmentation
        {
            return Err(PluginSdkError::Plugin(
                "request does not match the PIDNet model capability".to_owned(),
            ));
        }
        if context.cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
        }
        let model_image = request
            .image
            .as_ref()
            .ok_or_else(|| PluginSdkError::InvalidImage("image is required".to_owned()))?;
        let image = decode_image(model_image, MAX_IMAGE_BYTES)?;
        let image_artifact = one_image_artifact(&request)?;
        if image_artifact.image_id != request.image_id
            || image.dimensions() != (image_artifact.width, image_artifact.height)
        {
            return Err(PluginSdkError::Plugin(
                "request image does not match its Image Artifact lineage".to_owned(),
            ));
        }
        let (session, checkpoint_sha256, input_name) = self.loaded_model()?;
        let (input_width, input_height) =
            input_dimensions(&session.descriptor().inputs[0], &request.parameters, false)?;
        let resized = resize(&image, input_width, input_height)
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?
            .to_rgb8();
        let tensor = image_to_tensor(
            &resized,
            TensorLayout::Nchw,
            1.0 / 255.0,
            [0.485, 0.456, 0.406],
            [0.229, 0.224, 0.225],
        )
        .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        let input = NamedTensor::float32(input_name, tensor);
        let outputs = tokio::task::spawn_blocking(move || {
            session.infer(&[input], &InferenceCancellation::default())
        })
        .await
        .map_err(|error| PluginSdkError::Plugin(format!("inference task failed: {error}")))?
        .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        if context.cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
        }
        let logits = outputs
            .tensors
            .iter()
            .find(|tensor| matches!(tensor.data, TensorData::Float32(_)))
            .ok_or_else(|| {
                PluginSdkError::Plugin("model returned no float32 logit tensor".to_owned())
            })?;
        let (class_ids, class_count, logit_width, logit_height) =
            decode_logits(logits, image_artifact.width, image_artifact.height)?;
        let class_mapping = parse_class_mapping(&request.parameters)?;
        let reference = ArtifactRef {
            artifact_id: format!(
                "semantic-mask:{}:{}:{}",
                request.run_id, request.image_id, request.node_id
            ),
            source_node: request.node_id.clone(),
            port: "semantic_mask".to_owned(),
            artifact_type: ArtifactKind::SemanticMask,
            item_id: None,
        };
        let artifact = SemanticMaskArtifact {
            schema_version: annotagent_core::SEMANTIC_MASK_ARTIFACT_SCHEMA_VERSION,
            reference,
            image_id: request.image_id,
            source_image: image_artifact.reference.clone(),
            model_binding: request.model_id.clone(),
            width: image_artifact.width,
            height: image_artifact.height,
            class_ids,
            class_mapping,
            validation_state: ArtifactValidationState::Unvalidated,
            metadata: BTreeMap::from([
                (
                    "checkpoint_sha256".to_owned(),
                    serde_json::json!(checkpoint_sha256),
                ),
                ("class_count".to_owned(), serde_json::json!(class_count)),
                (
                    "input_size".to_owned(),
                    serde_json::json!([input_width, input_height]),
                ),
                (
                    "logit_size".to_owned(),
                    serde_json::json!([logit_width, logit_height]),
                ),
                ("runtime".to_owned(), serde_json::json!("rust-onnx-cpu")),
            ]),
        };
        artifact.validate().map_err(PluginSdkError::Plugin)?;
        Ok(PipelineInferenceResponse {
            protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: Some(request.request_id),
            model_identity: Some(request.model_id),
            artifacts: vec![PipelineArtifact::SemanticMask(artifact)],
            metadata: BTreeMap::from([
                (
                    "plugin_id".to_owned(),
                    serde_json::json!("org.annotagent.pidnet-onnx"),
                ),
                (
                    "contract".to_owned(),
                    serde_json::json!("pidnet-nchw-logits-v1"),
                ),
            ]),
            ..PipelineInferenceResponse::default()
        })
    }

    async fn cancel(&self, _request_id: &str) -> Result<(), PluginSdkError> {
        Ok(())
    }
}

fn validate_contract(session: &OnnxSession) -> Result<(), PluginSdkError> {
    let descriptor = session.descriptor();
    let input_valid = descriptor.inputs.len() == 1
        && descriptor.inputs[0].element_type == "f32"
        && descriptor.inputs[0].shape.len() == 4
        && matches!(descriptor.inputs[0].shape[0], 1 | -1)
        && descriptor.inputs[0].shape[1] == 3;
    let output_valid = descriptor.outputs.len() == 1
        && descriptor.outputs[0].element_type == "f32"
        && descriptor.outputs[0].shape.len() == 4
        && matches!(descriptor.outputs[0].shape[0], 1 | -1)
        && descriptor.outputs[0].shape[1] != 0;
    if !input_valid || !output_valid {
        return Err(PluginSdkError::Plugin(format!(
            "checkpoint does not match the PIDNet NCHW logit contract: inputs={:?}, outputs={:?}",
            descriptor.inputs, descriptor.outputs
        )));
    }
    Ok(())
}

fn input_dimensions(
    descriptor: &TensorDescriptor,
    parameters: &BTreeMap<String, serde_json::Value>,
    warmup: bool,
) -> Result<(u32, u32), PluginSdkError> {
    let resolve = |index: usize, parameter: &str, fallback: u32| {
        if descriptor.shape[index] > 0 {
            u32::try_from(descriptor.shape[index])
                .map_err(|_| PluginSdkError::Plugin(format!("fixed {parameter} exceeds u32")))
        } else if warmup {
            Ok(fallback)
        } else {
            parameters
                .get(parameter)
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    PluginSdkError::Plugin(format!("dynamic checkpoint requires {parameter}"))
                })
        }
    };
    let height = resolve(2, "input_height", DEFAULT_DYNAMIC_HEIGHT)?;
    let width = resolve(3, "input_width", DEFAULT_DYNAMIC_WIDTH)?;
    if !(MIN_INPUT_SIZE..=MAX_INPUT_SIZE).contains(&width)
        || !(MIN_INPUT_SIZE..=MAX_INPUT_SIZE).contains(&height)
    {
        return Err(PluginSdkError::Plugin(format!(
            "input dimensions must be within [{MIN_INPUT_SIZE},{MAX_INPUT_SIZE}]"
        )));
    }
    Ok((width, height))
}

fn decode_logits(
    tensor: &NamedTensor,
    target_width: u32,
    target_height: u32,
) -> Result<(Vec<u32>, usize, usize, usize), PluginSdkError> {
    let [batch, classes, height, width] = tensor.shape.as_slice() else {
        return Err(PluginSdkError::Plugin(format!(
            "logits must use NCHW shape, received {:?}",
            tensor.shape
        )));
    };
    if *batch != 1 || *classes == 0 || *height == 0 || *width == 0 {
        return Err(PluginSdkError::Plugin(
            "logit dimensions must be non-zero with batch one".to_owned(),
        ));
    }
    let TensorData::Float32(values) = &tensor.data else {
        return Err(PluginSdkError::Plugin("logits must be float32".to_owned()));
    };
    let plane = height
        .checked_mul(*width)
        .ok_or_else(|| PluginSdkError::Plugin("logit dimensions overflow".to_owned()))?;
    if values.len() != classes.saturating_mul(plane)
        || values.iter().any(|value| !value.is_finite())
    {
        return Err(PluginSdkError::Plugin(
            "logit values do not match the tensor shape or contain non-finite data".to_owned(),
        ));
    }
    let mut low_resolution = vec![0_u32; plane];
    for pixel in 0..plane {
        let mut best_class = 0_usize;
        let mut best_value = values[pixel];
        for class in 1..*classes {
            let value = values[class * plane + pixel];
            if value > best_value {
                best_class = class;
                best_value = value;
            }
        }
        low_resolution[pixel] = u32::try_from(best_class)
            .map_err(|_| PluginSdkError::Plugin("class id exceeds u32".to_owned()))?;
    }
    let mut restored = vec![0_u32; target_width as usize * target_height as usize];
    for y in 0..target_height {
        let source_y = (u64::from(y) * *height as u64 / u64::from(target_height)) as usize;
        for x in 0..target_width {
            let source_x = (u64::from(x) * *width as u64 / u64::from(target_width)) as usize;
            restored[y as usize * target_width as usize + x as usize] =
                low_resolution[source_y.min(height - 1) * width + source_x.min(width - 1)];
        }
    }
    Ok((restored, *classes, *width, *height))
}

fn parse_class_mapping(
    parameters: &BTreeMap<String, serde_json::Value>,
) -> Result<BTreeMap<u32, LabelId>, PluginSdkError> {
    let Some(mapping) = parameters.get("class_mapping") else {
        return Ok(BTreeMap::new());
    };
    let mapping = mapping
        .as_object()
        .ok_or_else(|| PluginSdkError::Plugin("class_mapping must be a JSON object".to_owned()))?;
    mapping
        .iter()
        .map(|(class_id, label)| {
            let class_id = class_id.parse::<u32>().map_err(|_| {
                PluginSdkError::Plugin("class_mapping keys must be u32 class ids".to_owned())
            })?;
            let label = label
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    PluginSdkError::Plugin(
                        "class_mapping values must be non-empty label ids".to_owned(),
                    )
                })?;
            Ok((class_id, LabelId::from(label)))
        })
        .collect()
}

fn one_image_artifact(
    request: &PipelineInferenceRequest,
) -> Result<&ImageArtifact, PluginSdkError> {
    let images = request
        .input_artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PipelineArtifact::Image(image) => Some(image),
            _ => None,
        })
        .collect::<Vec<_>>();
    match images.as_slice() {
        [image] => Ok(image),
        _ => Err(PluginSdkError::Plugin(
            "PIDNet requires exactly one Image Artifact".to_owned(),
        )),
    }
}

fn find_component(root: &Path, filename: &str) -> Result<PathBuf, PluginSdkError> {
    if !root.is_dir() {
        return Err(PluginSdkError::Plugin(
            "weights directory is unavailable".to_owned(),
        ));
    }
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut matches = Vec::new();
    while let Some((directory, depth)) = pending.pop() {
        if depth > 5 {
            continue;
        }
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                return Err(PluginSdkError::Plugin(
                    "weight directory cannot contain links".to_owned(),
                ));
            }
            let path = entry.path();
            if file_type.is_dir() {
                pending.push((path, depth + 1));
            } else if file_type.is_file()
                && path.file_name().and_then(std::ffi::OsStr::to_str) == Some(filename)
            {
                matches.push(path);
            }
        }
    }
    if matches.len() != 1 {
        return Err(PluginSdkError::Plugin(format!(
            "expected exactly one {filename} component, found {}",
            matches.len()
        )));
    }
    Ok(matches.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_declares_semantic_segmentation_without_commit_capability() {
        let plugin = PidNetOnnxPlugin::load().expect("plugin");
        assert_eq!(plugin.manifest.weights.components.len(), 1);
        assert_eq!(
            plugin.descriptor_model().capabilities,
            [annotagent_core::ModelCapability::SemanticSegmentation]
        );
        assert_eq!(
            plugin.descriptor_model().output_contracts[0].data_type,
            annotagent_core::ContractDataType::Artifact(ArtifactKind::SemanticMask)
        );
    }

    #[test]
    fn logit_decoder_argmaxes_channels_and_restores_original_dimensions() {
        let tensor = NamedTensor {
            name: "logits".to_owned(),
            shape: vec![1, 3, 2, 2],
            data: TensorData::Float32(vec![
                3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 5.0, 6.0,
            ]),
        };
        let (classes, class_count, width, height) = decode_logits(&tensor, 4, 2).expect("decode");
        assert_eq!(class_count, 3);
        assert_eq!((width, height), (2, 2));
        assert_eq!(classes, [0, 0, 1, 1, 2, 2, 2, 2]);
    }

    #[test]
    fn dynamic_input_requires_bounded_explicit_dimensions() {
        let descriptor = TensorDescriptor {
            name: "image".to_owned(),
            element_type: "f32".to_owned(),
            shape: vec![1, 3, -1, -1],
        };
        assert!(input_dimensions(&descriptor, &BTreeMap::new(), false).is_err());
        let parameters = BTreeMap::from([
            ("input_width".to_owned(), serde_json::json!(640)),
            ("input_height".to_owned(), serde_json::json!(384)),
        ]);
        assert_eq!(
            input_dimensions(&descriptor, &parameters, false).expect("dimensions"),
            (640, 384)
        );
    }
}
