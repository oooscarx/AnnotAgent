#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ArtifactValidationState, DetectionArtifactItem, DetectionScore,
    DetectionSetArtifact, DetectionSource, ImageArtifact, LabelId, NormalizedRect,
    PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact, PipelineInferenceRequest,
    PipelineInferenceResponse, ScoreSemantics, VisionCapability,
};
use annotagent_model_runtime_common::TensorF32;
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
use image::{DynamicImage, GenericImageView as _};

const MODEL_ID: &str = "rfdetr-detection-onnx-v1";
const MODEL_FILENAME: &str = "rfdetr.onnx";
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_MODEL_DIMENSION: i64 = 4096;

pub struct RfDetrOnnxPlugin {
    manifest: PluginManifest,
    loaded: RwLock<Option<LoadedModel>>,
}

struct LoadedModel {
    session: Arc<OnnxSession>,
    checkpoint_sha256: String,
    input: TensorDescriptor,
}

#[derive(Clone, Debug)]
struct DecodeOptions {
    confidence_threshold: f32,
    maximum_detections: usize,
    background_class_id: Option<i64>,
    class_labels: BTreeMap<usize, String>,
    class_mapping: BTreeMap<String, LabelId>,
    training_dataset_version: String,
}

#[derive(Clone, Debug, PartialEq)]
struct DecodedDetection {
    query_index: usize,
    class_id: usize,
    score: f32,
    bbox: NormalizedRect,
}

impl RfDetrOnnxPlugin {
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

    fn loaded_model(&self) -> Result<(Arc<OnnxSession>, String, TensorDescriptor), PluginSdkError> {
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
            model.input.clone(),
        ))
    }
}

#[async_trait]
impl ExpertModelPlugin for RfDetrOnnxPlugin {
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
        let input = session.descriptor().inputs[0].clone();
        *self
            .loaded
            .write()
            .map_err(|_| PluginSdkError::Plugin("model state lock was poisoned".to_owned()))? =
            Some(LoadedModel {
                session: Arc::new(session),
                checkpoint_sha256,
                input,
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
        let (session, _, input) = self.loaded_model()?;
        let width = usize::try_from(input.shape[3])
            .map_err(|_| PluginSdkError::Plugin("invalid input width".to_owned()))?;
        let height = usize::try_from(input.shape[2])
            .map_err(|_| PluginSdkError::Plugin("invalid input height".to_owned()))?;
        let tensor = NamedTensor {
            name: input.name,
            shape: vec![1, 3, height, width],
            data: TensorData::Float32(vec![0.0; 3 * width * height]),
        };
        tokio::task::spawn_blocking(move || {
            session.warmup(&[tensor], 1, &InferenceCancellation::default())
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
        if request.model_id != MODEL_ID || request.operation != VisionCapability::ObjectDetection {
            return Err(PluginSdkError::Plugin(
                "request does not match the RF-DETR detection contract".to_owned(),
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
        let image_artifact = one_image_artifact(&request)?.clone();
        if image_artifact.image_id != request.image_id
            || image.dimensions() != (image_artifact.width, image_artifact.height)
        {
            return Err(PluginSdkError::Plugin(
                "request image does not match its Image Artifact lineage".to_owned(),
            ));
        }
        let options = decode_options(&request.parameters)?;
        let (session, checkpoint_sha256, input_descriptor) = self.loaded_model()?;
        let input_width = u32::try_from(input_descriptor.shape[3])
            .map_err(|_| PluginSdkError::Plugin("invalid input width".to_owned()))?;
        let input_height = u32::try_from(input_descriptor.shape[2])
            .map_err(|_| PluginSdkError::Plugin("invalid input height".to_owned()))?;
        let tensor = preprocess(&image, input_width, input_height)?;
        let input = NamedTensor::float32(input_descriptor.name, tensor);
        let outputs = tokio::task::spawn_blocking(move || {
            session.infer(&[input], &InferenceCancellation::default())
        })
        .await
        .map_err(|error| PluginSdkError::Plugin(format!("inference task failed: {error}")))?
        .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        if context.cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
        }
        let boxes = outputs
            .tensors
            .iter()
            .find(|output| output.name == "dets")
            .ok_or_else(|| PluginSdkError::Plugin("RF-DETR output dets is missing".to_owned()))?;
        let logits = outputs
            .tensors
            .iter()
            .find(|output| output.name == "labels")
            .ok_or_else(|| PluginSdkError::Plugin("RF-DETR output labels is missing".to_owned()))?;
        let detections = decode_outputs(boxes, logits, &options)?;
        build_response(
            request,
            &image_artifact,
            &checkpoint_sha256,
            &options,
            detections,
        )
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
        && descriptor.inputs[0].shape[0] == 1
        && descriptor.inputs[0].shape[1] == 3
        && descriptor.inputs[0].shape[2] > 0
        && descriptor.inputs[0].shape[2] <= MAX_MODEL_DIMENSION
        && descriptor.inputs[0].shape[3] > 0
        && descriptor.inputs[0].shape[3] <= MAX_MODEL_DIMENSION;
    let boxes = descriptor
        .outputs
        .iter()
        .find(|output| output.name == "dets");
    let labels = descriptor
        .outputs
        .iter()
        .find(|output| output.name == "labels");
    let outputs_valid = matches!((boxes, labels), (Some(boxes), Some(labels))
        if boxes.element_type == "f32"
            && labels.element_type == "f32"
            && boxes.shape.len() == 3
            && labels.shape.len() == 3
            && boxes.shape[0] == 1
            && labels.shape[0] == 1
            && boxes.shape[1] > 0
            && boxes.shape[1] == labels.shape[1]
            && boxes.shape[2] == 4
            && labels.shape[2] > 1);
    if !input_valid || !outputs_valid {
        return Err(PluginSdkError::Plugin(format!(
            "checkpoint does not match official RF-DETR detection ONNX v1: inputs={:?}, outputs={:?}",
            descriptor.inputs, descriptor.outputs
        )));
    }
    Ok(())
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
        if depth > 4 {
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
    match matches.as_slice() {
        [path] => Ok(path.clone()),
        _ => Err(PluginSdkError::Plugin(format!(
            "expected exactly one {filename} component, found {}",
            matches.len()
        ))),
    }
}

fn preprocess(
    image: &DynamicImage,
    target_width: u32,
    target_height: u32,
) -> Result<TensorF32, PluginSdkError> {
    let source = image.to_rgb8();
    let (source_width, source_height) = source.dimensions();
    if source_width == 0
        || source_height == 0
        || target_width == 0
        || target_height == 0
        || target_width > MAX_MODEL_DIMENSION as u32
        || target_height > MAX_MODEL_DIMENSION as u32
    {
        return Err(PluginSdkError::InvalidImage(
            "RF-DETR resize dimensions are invalid".to_owned(),
        ));
    }
    let plane = target_width as usize * target_height as usize;
    let mut values = vec![0.0_f32; plane * 3];
    let mean = [0.485_f32, 0.456, 0.406];
    let std = [0.229_f32, 0.224, 0.225];
    for y in 0..target_height {
        let source_y = half_pixel_coordinate(y, source_height, target_height);
        let y0 = source_y.floor() as u32;
        let y1 = (y0 + 1).min(source_height - 1);
        let weight_y = source_y - y0 as f32;
        for x in 0..target_width {
            let source_x = half_pixel_coordinate(x, source_width, target_width);
            let x0 = source_x.floor() as u32;
            let x1 = (x0 + 1).min(source_width - 1);
            let weight_x = source_x - x0 as f32;
            let pixels = [
                source.get_pixel(x0, y0).0,
                source.get_pixel(x1, y0).0,
                source.get_pixel(x0, y1).0,
                source.get_pixel(x1, y1).0,
            ];
            let index = y as usize * target_width as usize + x as usize;
            for channel in 0..3 {
                let top = f32::from(pixels[0][channel]) * (1.0 - weight_x)
                    + f32::from(pixels[1][channel]) * weight_x;
                let bottom = f32::from(pixels[2][channel]) * (1.0 - weight_x)
                    + f32::from(pixels[3][channel]) * weight_x;
                let value = (top * (1.0 - weight_y) + bottom * weight_y) / 255.0;
                values[channel * plane + index] = (value - mean[channel]) / std[channel];
            }
        }
    }
    TensorF32::new(
        vec![1, 3, target_height as usize, target_width as usize],
        values,
    )
    .map_err(|error| PluginSdkError::Plugin(error.to_string()))
}

fn half_pixel_coordinate(destination: u32, source_size: u32, target_size: u32) -> f32 {
    (((destination as f32 + 0.5) * source_size as f32 / target_size as f32) - 0.5)
        .clamp(0.0, source_size.saturating_sub(1) as f32)
}

fn decode_options(
    parameters: &BTreeMap<String, serde_json::Value>,
) -> Result<DecodeOptions, PluginSdkError> {
    let confidence_threshold = numeric_parameter(parameters, "confidence_threshold", 0.3)?;
    let maximum_detections = parameters
        .get("max_detections")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(300);
    let maximum_detections = usize::try_from(maximum_detections)
        .ok()
        .filter(|value| (1..=10_000).contains(value))
        .ok_or_else(|| {
            PluginSdkError::Plugin("max_detections must be within 1..=10000".to_owned())
        })?;
    let background_class_id = match parameters.get("background_class_id") {
        None => Some(-1),
        Some(serde_json::Value::Null) => None,
        Some(value) => value
            .as_i64()
            .filter(|value| *value >= -1)
            .map(Some)
            .ok_or_else(|| {
                PluginSdkError::Plugin(
                    "background_class_id must be null, -1, or a non-negative integer".to_owned(),
                )
            })?,
    };
    let class_labels = string_map(parameters, "class_labels")?
        .into_iter()
        .map(|(class_id, label)| {
            class_id
                .parse::<usize>()
                .map(|class_id| (class_id, label))
                .map_err(|_| {
                    PluginSdkError::Plugin(
                        "class_labels keys must be non-negative integer strings".to_owned(),
                    )
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let class_mapping = string_map(parameters, "class_mapping")?
        .into_iter()
        .map(|(model_label, project_label)| (model_label, LabelId::new(project_label)))
        .collect();
    let training_dataset_version = parameters
        .get("training_dataset_version")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 512)
        .ok_or_else(|| {
            PluginSdkError::Plugin(
                "training_dataset_version is required and must be at most 512 characters"
                    .to_owned(),
            )
        })?
        .to_owned();
    Ok(DecodeOptions {
        confidence_threshold,
        maximum_detections,
        background_class_id,
        class_labels,
        class_mapping,
        training_dataset_version,
    })
}

fn numeric_parameter(
    parameters: &BTreeMap<String, serde_json::Value>,
    name: &str,
    default: f32,
) -> Result<f32, PluginSdkError> {
    let value = parameters
        .get(name)
        .map(|value| {
            value.as_f64().ok_or_else(|| {
                PluginSdkError::Plugin(format!("{name} must be a finite number within [0,1]"))
            })
        })
        .transpose()?
        .unwrap_or(f64::from(default)) as f32;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(PluginSdkError::Plugin(format!(
            "{name} must be a finite number within [0,1]"
        )));
    }
    Ok(value)
}

fn string_map(
    parameters: &BTreeMap<String, serde_json::Value>,
    name: &str,
) -> Result<BTreeMap<String, String>, PluginSdkError> {
    parameters
        .get(name)
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| PluginSdkError::Plugin(format!("{name} must be a string map")))?;
            object
                .iter()
                .map(|(key, value)| {
                    let value = value
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            PluginSdkError::Plugin(format!(
                                "{name} values must be non-empty strings"
                            ))
                        })?;
                    Ok((key.clone(), value.to_owned()))
                })
                .collect()
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn decode_outputs(
    boxes: &NamedTensor,
    logits: &NamedTensor,
    options: &DecodeOptions,
) -> Result<Vec<DecodedDetection>, PluginSdkError> {
    let TensorData::Float32(box_values) = &boxes.data else {
        return Err(PluginSdkError::Plugin(
            "RF-DETR dets must be float32".to_owned(),
        ));
    };
    let TensorData::Float32(logit_values) = &logits.data else {
        return Err(PluginSdkError::Plugin(
            "RF-DETR labels must be float32".to_owned(),
        ));
    };
    let query_count = match boxes.shape.as_slice() {
        [1, queries, 4] => *queries,
        shape => {
            return Err(PluginSdkError::Plugin(format!(
                "unexpected RF-DETR dets shape {shape:?}"
            )));
        }
    };
    let class_count = match logits.shape.as_slice() {
        [1, queries, classes] if *queries == query_count && *classes > 1 => *classes,
        shape => {
            return Err(PluginSdkError::Plugin(format!(
                "unexpected RF-DETR labels shape {shape:?}"
            )));
        }
    };
    if box_values.len() != query_count * 4 || logit_values.len() != query_count * class_count {
        return Err(PluginSdkError::Plugin(
            "RF-DETR output tensor length does not match its shape".to_owned(),
        ));
    }
    let background = options
        .background_class_id
        .map(|class_id| {
            if class_id == -1 {
                Ok(class_count - 1)
            } else {
                usize::try_from(class_id).map_err(|_| {
                    PluginSdkError::Plugin("background class id exceeds usize".to_owned())
                })
            }
        })
        .transpose()?;
    if background.is_some_and(|class_id| class_id >= class_count) {
        return Err(PluginSdkError::Plugin(format!(
            "background_class_id is outside the {class_count}-class output"
        )));
    }
    let mut candidates = Vec::with_capacity(query_count.saturating_mul(class_count));
    for query_index in 0..query_count {
        let query_box = &box_values[query_index * 4..query_index * 4 + 4];
        if query_box.iter().any(|value| !value.is_finite()) {
            continue;
        }
        let center_x = query_box[0];
        let center_y = query_box[1];
        let width = query_box[2];
        let height = query_box[3];
        let x_min = (center_x - width / 2.0).clamp(0.0, 1.0);
        let y_min = (center_y - height / 2.0).clamp(0.0, 1.0);
        let x_max = (center_x + width / 2.0).clamp(0.0, 1.0);
        let y_max = (center_y + height / 2.0).clamp(0.0, 1.0);
        if x_max <= x_min || y_max <= y_min {
            continue;
        }
        let bbox = NormalizedRect::new(x_min, y_min, x_max - x_min, y_max - y_min)
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        for class_id in 0..class_count {
            if background == Some(class_id) {
                continue;
            }
            let logit = logit_values[query_index * class_count + class_id];
            if !logit.is_finite() {
                continue;
            }
            let score = 1.0 / (1.0 + (-logit.clamp(-88.0, 88.0)).exp());
            candidates.push(DecodedDetection {
                query_index,
                class_id,
                score,
                bbox,
            });
        }
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.query_index.cmp(&right.query_index))
            .then_with(|| left.class_id.cmp(&right.class_id))
    });
    candidates.truncate(options.maximum_detections.min(query_count));
    candidates.retain(|detection| detection.score >= options.confidence_threshold);
    Ok(candidates)
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
        [image] if request.input_artifacts.len() == 1 => Ok(image),
        _ => Err(PluginSdkError::Plugin(
            "RF-DETR requires exactly one Image Artifact".to_owned(),
        )),
    }
}

fn build_response(
    request: PipelineInferenceRequest,
    image: &ImageArtifact,
    checkpoint_sha256: &str,
    options: &DecodeOptions,
    detections: Vec<DecodedDetection>,
) -> Result<PipelineInferenceResponse, PluginSdkError> {
    let artifact_id = format!("rfdetr-detections:{}", request.request_id);
    let reference = ArtifactRef {
        artifact_id: artifact_id.clone(),
        source_node: request.node_id.clone(),
        port: "detections".to_owned(),
        artifact_type: ArtifactKind::DetectionSet,
        item_id: None,
    };
    let items = detections
        .into_iter()
        .enumerate()
        .map(|(index, detection)| {
            let label = options
                .class_labels
                .get(&detection.class_id)
                .cloned()
                .unwrap_or_else(|| format!("class-{}", detection.class_id));
            let mut item = DetectionArtifactItem::from_source(
                format!("rfdetr-{index}"),
                None,
                Some(label.clone()),
                options.class_mapping.get(&label).cloned(),
                detection.bbox,
                DetectionScore::new(Some(detection.score), ScoreSemantics::DetectionConfidence)
                    .map_err(PluginSdkError::Plugin)?,
                DetectionSource {
                    model_id: request.model_id.clone(),
                    capability: VisionCapability::ObjectDetection,
                    artifact_id: artifact_id.clone(),
                },
            )
            .map_err(PluginSdkError::Plugin)?;
            item.attributes.insert(
                "model_class_id".to_owned(),
                serde_json::json!(detection.class_id),
            );
            item.attributes.insert(
                "query_index".to_owned(),
                serde_json::json!(detection.query_index),
            );
            Ok(item)
        })
        .collect::<Result<Vec<_>, PluginSdkError>>()?;
    let artifact = DetectionSetArtifact {
        schema_version: annotagent_core::DETECTION_ARTIFACT_SCHEMA_VERSION,
        reference,
        image_id: request.image_id,
        model_binding: request.model_id.clone(),
        validation_state: ArtifactValidationState::Unvalidated,
        detections: items,
        metadata: BTreeMap::from([
            (
                "checkpoint_sha256".to_owned(),
                serde_json::json!(checkpoint_sha256),
            ),
            (
                "training_dataset_version".to_owned(),
                serde_json::json!(options.training_dataset_version),
            ),
            (
                "source_image".to_owned(),
                serde_json::json!(image.reference.artifact_id),
            ),
            (
                "postprocess".to_owned(),
                serde_json::json!("rfdetr-flattened-topk-no-nms"),
            ),
            ("runtime".to_owned(), serde_json::json!("rust-onnx-cpu")),
        ]),
    };
    artifact.validate().map_err(PluginSdkError::Plugin)?;
    Ok(PipelineInferenceResponse {
        protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
        request_id: Some(request.request_id),
        model_identity: Some(request.model_id),
        artifacts: vec![PipelineArtifact::DetectionSet(artifact)],
        metadata: BTreeMap::from([
            (
                "plugin_id".to_owned(),
                serde_json::json!("org.annotagent.rfdetr-onnx"),
            ),
            (
                "contract".to_owned(),
                serde_json::json!("official-rfdetr-detection-onnx-v1"),
            ),
        ]),
        ..PipelineInferenceResponse::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::{ContractDataType, ModelCapability};
    use annotagent_plugin_api::PluginImplementationStatus;
    use image::{Rgb, RgbImage};

    fn options() -> DecodeOptions {
        DecodeOptions {
            confidence_threshold: 0.5,
            maximum_detections: 10,
            background_class_id: Some(2),
            class_labels: BTreeMap::from([(0, "ball".to_owned()), (1, "robot".to_owned())]),
            class_mapping: BTreeMap::new(),
            training_dataset_version: "fixture-v1".to_owned(),
        }
    }

    #[test]
    fn manifest_is_live_conditional_detection_without_hidden_postprocess() {
        let manifest =
            PluginManifest::from_toml(include_str!("../annotagent-plugin.toml")).expect("manifest");
        assert_eq!(
            manifest.implementation_status,
            PluginImplementationStatus::LiveConditional
        );
        assert_eq!(
            manifest.models[0].capabilities,
            [ModelCapability::ObjectDetection]
        );
        assert_eq!(manifest.models[0].output_contracts.len(), 1);
        assert_eq!(
            manifest.models[0].output_contracts[0].data_type,
            ContractDataType::Artifact(ArtifactKind::DetectionSet)
        );
    }

    #[test]
    fn preprocessing_is_rgb_nchw_imagenet_normalized() {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(1, 1, Rgb([255, 0, 127])));
        let tensor = preprocess(&image, 2, 2).expect("preprocess");
        assert_eq!(tensor.shape, [1, 3, 2, 2]);
        assert!((tensor.values[0] - ((1.0 - 0.485) / 0.229)).abs() < 1e-5);
        assert!((tensor.values[4] - ((0.0 - 0.456) / 0.224)).abs() < 1e-5);
        assert!((tensor.values[8] - ((127.0 / 255.0 - 0.406) / 0.225)).abs() < 1e-5);
    }

    #[test]
    fn decoder_uses_sigmoid_flattened_topk_background_and_normalized_cxcywh() {
        let boxes = NamedTensor {
            name: "dets".to_owned(),
            shape: vec![1, 2, 4],
            data: TensorData::Float32(vec![0.5, 0.5, 0.4, 0.2, 0.2, 0.2, 0.1, 0.1]),
        };
        let logits = NamedTensor {
            name: "labels".to_owned(),
            shape: vec![1, 2, 3],
            data: TensorData::Float32(vec![4.0, 3.0, 20.0, 2.0, -2.0, 20.0]),
        };
        let detections = decode_outputs(&boxes, &logits, &options()).expect("decode");
        assert_eq!(detections.len(), 2);
        assert_eq!((detections[0].query_index, detections[0].class_id), (0, 0));
        assert_eq!((detections[1].query_index, detections[1].class_id), (0, 1));
        assert!((detections[0].bbox.x() - 0.3).abs() < 1e-6);
        assert!((detections[0].bbox.y() - 0.4).abs() < 1e-6);
        assert!((detections[0].bbox.width() - 0.4).abs() < 1e-6);
        assert!((detections[0].bbox.height() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn dataset_identity_is_required_by_the_frozen_node_configuration() {
        let error = decode_options(&BTreeMap::new()).expect_err("dataset version is required");
        assert!(error.to_string().contains("training_dataset_version"));
        let options = decode_options(&BTreeMap::from([(
            "training_dataset_version".to_owned(),
            serde_json::json!("custom-coco@sha256:abc"),
        )]))
        .expect("options");
        assert_eq!(options.training_dataset_version, "custom-coco@sha256:abc");
        assert_eq!(options.background_class_id, Some(-1));
        let options = decode_options(&BTreeMap::from([
            (
                "training_dataset_version".to_owned(),
                serde_json::json!("custom-coco@sha256:abc"),
            ),
            ("background_class_id".to_owned(), serde_json::Value::Null),
        ]))
        .expect("sparse class layout");
        assert_eq!(options.background_class_id, None);
    }
}
