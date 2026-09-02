#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ArtifactValidationState, DetectionArtifactItem, DetectionScore,
    DetectionSetArtifact, DetectionSource, LabelId, NormalizedRect, PipelineArtifact,
    PipelineInferenceRequest, PipelineInferenceResponse, ScoreSemantics, VisionCapability,
};
use annotagent_model_runtime_common::{BoundingBox, ScoredBox, TensorF32, non_maximum_suppression};
use annotagent_model_runtime_onnx::{
    InferenceCancellation, NamedTensor, OnnxSession, SessionOptions, TensorData,
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
use image::{DynamicImage, GenericImageView, Rgb, RgbImage, imageops::FilterType};

const MODEL_ID: &str = "yolox-nano-coco-onnx";
const INPUT_WIDTH: u32 = 416;
const INPUT_HEIGHT: u32 = 416;
const OUTPUT_COLUMNS: usize = 85;
const OUTPUT_COLUMNS_I64: i64 = 85;
const STRIDES: [usize; 3] = [8, 16, 32];
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;

pub struct YoloOnnxPlugin {
    manifest: PluginManifest,
    loaded: RwLock<Option<LoadedModel>>,
}

struct LoadedModel {
    session: Arc<OnnxSession>,
    checkpoint_sha256: String,
    input_name: String,
}

#[derive(Clone, Copy, Debug)]
struct PreprocessTransform {
    source_width: u32,
    source_height: u32,
    scale: f32,
}

#[derive(Clone, Debug)]
struct DecodeOptions {
    confidence_threshold: f32,
    iou_threshold: f32,
    maximum_detections: usize,
    class_mapping: BTreeMap<String, LabelId>,
}

#[derive(Clone, Debug)]
struct DecodedDetection {
    bbox: BoundingBox,
    score: f32,
    class_id: usize,
}

impl YoloOnnxPlugin {
    pub fn load() -> Result<Self, PluginSdkError> {
        let manifest = PluginManifest::from_toml(include_str!("../annotagent-plugin.toml"))
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        Ok(Self {
            manifest,
            loaded: RwLock::new(None),
        })
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

    fn descriptor_model(&self) -> &annotagent_plugin_api::PluginModelManifest {
        &self.manifest.models[0]
    }
}

#[async_trait]
impl ExpertModelPlugin for YoloOnnxPlugin {
    async fn setup(&self, context: PluginRuntimeContext) -> Result<(), PluginSdkError> {
        let model_path = find_single_onnx(&context.weights_dir)?;
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
        let input = NamedTensor {
            name: input_name,
            shape: vec![1, 3, INPUT_HEIGHT as usize, INPUT_WIDTH as usize],
            data: TensorData::Float32(vec![
                114.0;
                3 * INPUT_HEIGHT as usize * INPUT_WIDTH as usize
            ]),
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
        if request.model_id != MODEL_ID {
            return Err(PluginSdkError::Plugin("unknown model".to_owned()));
        }
        if context.cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
        }
        let image = request
            .image
            .as_ref()
            .ok_or_else(|| PluginSdkError::InvalidImage("image is required".to_owned()))?;
        let image = decode_image(image, MAX_IMAGE_BYTES)?;
        let (tensor, transform) = preprocess(&image)?;
        let options = decode_options(&request.parameters)?;
        let (session, checkpoint_sha256, input_name) = self.loaded_model()?;
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
        let output = outputs
            .tensors
            .iter()
            .find(|tensor| matches!(tensor.data, TensorData::Float32(_)))
            .ok_or_else(|| {
                PluginSdkError::Plugin("model returned no float32 output tensor".to_owned())
            })?;
        let detections = decode_output(output, transform, &options)?;
        build_response(request, &checkpoint_sha256, transform, detections)
    }

    async fn cancel(&self, _request_id: &str) -> Result<(), PluginSdkError> {
        Ok(())
    }
}

fn validate_contract(session: &OnnxSession) -> Result<(), PluginSdkError> {
    let descriptor = session.descriptor();
    if descriptor.inputs.len() != 1
        || descriptor.inputs[0].element_type != "f32"
        || descriptor.inputs[0].shape != [1, 3, i64::from(INPUT_HEIGHT), i64::from(INPUT_WIDTH)]
        || descriptor.outputs.len() != 1
        || descriptor.outputs[0].element_type != "f32"
        || descriptor.outputs[0].shape.last().copied() != Some(OUTPUT_COLUMNS_I64)
    {
        return Err(PluginSdkError::Plugin(format!(
            "checkpoint tensor contract does not match YOLOX Nano: inputs={:?}, outputs={:?}",
            descriptor.inputs, descriptor.outputs
        )));
    }
    Ok(())
}

fn find_single_onnx(root: &Path) -> Result<PathBuf, PluginSdkError> {
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
        for entry in std::fs::read_dir(&directory)? {
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
                && path.extension().and_then(std::ffi::OsStr::to_str) == Some("onnx")
            {
                matches.push(path);
            }
        }
    }
    if matches.len() != 1 {
        return Err(PluginSdkError::Plugin(format!(
            "expected exactly one ONNX checkpoint, found {}",
            matches.len()
        )));
    }
    Ok(matches.remove(0))
}

fn preprocess(image: &DynamicImage) -> Result<(TensorF32, PreprocessTransform), PluginSdkError> {
    let (source_width, source_height) = image.dimensions();
    if source_width == 0 || source_height == 0 {
        return Err(PluginSdkError::InvalidImage(
            "image dimensions must be non-zero".to_owned(),
        ));
    }
    let scale =
        (INPUT_WIDTH as f32 / source_width as f32).min(INPUT_HEIGHT as f32 / source_height as f32);
    let resized_width = ((source_width as f32 * scale) as u32).clamp(1, INPUT_WIDTH);
    let resized_height = ((source_height as f32 * scale) as u32).clamp(1, INPUT_HEIGHT);
    let resized = image
        .resize_exact(resized_width, resized_height, FilterType::Triangle)
        .to_rgb8();
    let mut canvas = RgbImage::from_pixel(INPUT_WIDTH, INPUT_HEIGHT, Rgb([114; 3]));
    image::imageops::replace(&mut canvas, &resized, 0, 0);
    let plane = INPUT_WIDTH as usize * INPUT_HEIGHT as usize;
    let mut values = vec![0.0; plane * 3];
    for (index, pixel) in canvas.pixels().enumerate() {
        values[index] = f32::from(pixel[2]);
        values[plane + index] = f32::from(pixel[1]);
        values[plane * 2 + index] = f32::from(pixel[0]);
    }
    let tensor = TensorF32::new(
        vec![1, 3, INPUT_HEIGHT as usize, INPUT_WIDTH as usize],
        values,
    )
    .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
    Ok((
        tensor,
        PreprocessTransform {
            source_width,
            source_height,
            scale,
        },
    ))
}

fn decode_options(
    parameters: &BTreeMap<String, serde_json::Value>,
) -> Result<DecodeOptions, PluginSdkError> {
    let confidence_threshold = numeric_parameter(parameters, "confidence_threshold", 0.25)?;
    let iou_threshold = numeric_parameter(parameters, "iou_threshold", 0.45)?;
    let maximum_detections = parameters
        .get("max_detections")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(300);
    if maximum_detections == 0 || maximum_detections > 10_000 {
        return Err(PluginSdkError::Plugin(
            "max_detections must be within 1..=10000".to_owned(),
        ));
    }
    let class_mapping = parameters
        .get("class_mapping")
        .and_then(serde_json::Value::as_object)
        .map(|mapping| {
            mapping
                .iter()
                .map(|(model_label, project_label)| {
                    let project_label = project_label.as_str().ok_or_else(|| {
                        PluginSdkError::Plugin("class_mapping values must be strings".to_owned())
                    })?;
                    Ok((model_label.clone(), LabelId::new(project_label)))
                })
                .collect::<Result<BTreeMap<_, _>, PluginSdkError>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(DecodeOptions {
        confidence_threshold,
        iou_threshold,
        maximum_detections: maximum_detections as usize,
        class_mapping,
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

fn decode_output(
    output: &NamedTensor,
    transform: PreprocessTransform,
    options: &DecodeOptions,
) -> Result<Vec<DecodedDetection>, PluginSdkError> {
    let TensorData::Float32(values) = &output.data else {
        return Err(PluginSdkError::Plugin(
            "YOLOX output must be float32".to_owned(),
        ));
    };
    let rows = match output.shape.as_slice() {
        [1, rows, OUTPUT_COLUMNS] | [rows, OUTPUT_COLUMNS] => *rows,
        shape => {
            return Err(PluginSdkError::Plugin(format!(
                "unexpected YOLOX output shape {shape:?}"
            )));
        }
    };
    let grids = output_grids();
    if rows != grids.len() || values.len() != rows * OUTPUT_COLUMNS {
        return Err(PluginSdkError::Plugin(format!(
            "YOLOX output row count {rows} does not match the 416 stride contract {}",
            grids.len()
        )));
    }
    let mut candidates = Vec::new();
    for (row_index, (grid_x, grid_y, stride)) in grids.into_iter().enumerate() {
        let row = &values[row_index * OUTPUT_COLUMNS..(row_index + 1) * OUTPUT_COLUMNS];
        let objectness = row[4];
        if !objectness.is_finite() || !(0.0..=1.0).contains(&objectness) {
            continue;
        }
        let Some((class_id, class_value)) = row[5..]
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, value)| value.is_finite() && (0.0..=1.0).contains(value))
            .max_by(|left, right| left.1.total_cmp(&right.1))
        else {
            continue;
        };
        let score = objectness * class_value;
        if score < options.confidence_threshold {
            continue;
        }
        let center_x = (row[0] + grid_x as f32) * stride as f32;
        let center_y = (row[1] + grid_y as f32) * stride as f32;
        let width = row[2].exp() * stride as f32;
        let height = row[3].exp() * stride as f32;
        if ![center_x, center_y, width, height]
            .iter()
            .all(|value| value.is_finite())
        {
            continue;
        }
        let bbox = BoundingBox::from_cxcywh(center_x, center_y, width, height);
        let bbox = BoundingBox {
            x_min: bbox.x_min / transform.scale,
            y_min: bbox.y_min / transform.scale,
            x_max: bbox.x_max / transform.scale,
            y_max: bbox.y_max / transform.scale,
        }
        .clip(
            transform.source_width as f32,
            transform.source_height as f32,
        );
        if bbox.area() <= f32::EPSILON {
            continue;
        }
        candidates.push(ScoredBox {
            bbox,
            score,
            class_id: i64::try_from(class_id).map_err(|_| {
                PluginSdkError::Plugin("model class id exceeds the supported range".to_owned())
            })?,
        });
    }
    let mut kept = non_maximum_suppression(
        &candidates,
        options.confidence_threshold,
        options.iou_threshold,
        false,
    );
    kept.truncate(options.maximum_detections);
    Ok(kept
        .into_iter()
        .map(|candidate| DecodedDetection {
            bbox: candidate.bbox,
            score: candidate.score,
            class_id: candidate.class_id as usize,
        })
        .collect())
}

fn output_grids() -> Vec<(usize, usize, usize)> {
    let mut grids = Vec::with_capacity(3_549);
    for stride in STRIDES {
        let width = INPUT_WIDTH as usize / stride;
        let height = INPUT_HEIGHT as usize / stride;
        for y in 0..height {
            for x in 0..width {
                grids.push((x, y, stride));
            }
        }
    }
    grids
}

fn build_response(
    request: PipelineInferenceRequest,
    checkpoint_sha256: &str,
    transform: PreprocessTransform,
    detections: Vec<DecodedDetection>,
) -> Result<PipelineInferenceResponse, PluginSdkError> {
    let artifact_id = format!("yolox-detections:{}", request.request_id);
    let artifact_width = image_dimension(&request, true)?;
    let artifact_height = image_dimension(&request, false)?;
    if artifact_width != transform.source_width || artifact_height != transform.source_height {
        return Err(PluginSdkError::Plugin(
            "Image Artifact dimensions do not match the decoded image".to_owned(),
        ));
    }
    let source_width = artifact_width as f32;
    let source_height = artifact_height as f32;
    let reference = ArtifactRef {
        artifact_id: artifact_id.clone(),
        source_node: request.node_id.clone(),
        port: "detections".to_owned(),
        artifact_type: ArtifactKind::DetectionSet,
        item_id: None,
    };
    let options = decode_options(&request.parameters)?;
    let items = detections
        .into_iter()
        .enumerate()
        .map(|(index, detection)| {
            let label = COCO_LABELS[detection.class_id].to_owned();
            let x_min = detection.bbox.x_min / source_width;
            let y_min = detection.bbox.y_min / source_height;
            let x_max = detection.bbox.x_max / source_width;
            let y_max = detection.bbox.y_max / source_height;
            let bbox = NormalizedRect::new(
                x_min.clamp(0.0, 1.0),
                y_min.clamp(0.0, 1.0),
                x_max.clamp(0.0, 1.0) - x_min.clamp(0.0, 1.0),
                y_max.clamp(0.0, 1.0) - y_min.clamp(0.0, 1.0),
            )
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
            let mut item = DetectionArtifactItem::from_source(
                format!("yolox-{index}"),
                None,
                Some(label.clone()),
                options.class_mapping.get(&label).cloned(),
                bbox,
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
            ("label_space".to_owned(), serde_json::json!("coco-80")),
            ("runtime".to_owned(), serde_json::json!("rust-onnx-cpu")),
        ]),
    };
    artifact
        .validate()
        .map_err(|error| PluginSdkError::Plugin(error.clone()))?;
    Ok(PipelineInferenceResponse {
        request_id: Some(request.request_id),
        model_identity: Some(request.model_id),
        artifacts: vec![PipelineArtifact::DetectionSet(artifact)],
        metadata: BTreeMap::from([
            (
                "plugin_id".to_owned(),
                serde_json::json!("org.annotagent.yolo-onnx"),
            ),
            (
                "input_contract".to_owned(),
                serde_json::json!("yolox-nano-416-coco-v1"),
            ),
        ]),
        ..PipelineInferenceResponse::default()
    })
}

fn image_dimension(request: &PipelineInferenceRequest, width: bool) -> Result<u32, PluginSdkError> {
    let image = request
        .input_artifacts
        .iter()
        .find_map(|artifact| match artifact {
            PipelineArtifact::Image(image) => Some(image),
            _ => None,
        })
        .ok_or_else(|| PluginSdkError::Plugin("Image Artifact input is required".to_owned()))?;
    if image.image_id != request.image_id {
        return Err(PluginSdkError::Plugin(
            "Image Artifact belongs to another request image".to_owned(),
        ));
    }
    let dimension = if width { image.width } else { image.height };
    if dimension == 0 {
        return Err(PluginSdkError::Plugin(
            "Image Artifact dimensions must be non-zero".to_owned(),
        ));
    }
    Ok(dimension)
}

const COCO_LABELS: [&str; 80] = [
    "person",
    "bicycle",
    "car",
    "motorcycle",
    "airplane",
    "bus",
    "train",
    "truck",
    "boat",
    "traffic light",
    "fire hydrant",
    "stop sign",
    "parking meter",
    "bench",
    "bird",
    "cat",
    "dog",
    "horse",
    "sheep",
    "cow",
    "elephant",
    "bear",
    "zebra",
    "giraffe",
    "backpack",
    "umbrella",
    "handbag",
    "tie",
    "suitcase",
    "frisbee",
    "skis",
    "snowboard",
    "sports ball",
    "kite",
    "baseball bat",
    "baseball glove",
    "skateboard",
    "surfboard",
    "tennis racket",
    "bottle",
    "wine glass",
    "cup",
    "fork",
    "knife",
    "spoon",
    "bowl",
    "banana",
    "apple",
    "sandwich",
    "orange",
    "broccoli",
    "carrot",
    "hot dog",
    "pizza",
    "donut",
    "cake",
    "chair",
    "couch",
    "potted plant",
    "bed",
    "dining table",
    "toilet",
    "tv",
    "laptop",
    "mouse",
    "remote",
    "keyboard",
    "cell phone",
    "microwave",
    "oven",
    "toaster",
    "sink",
    "refrigerator",
    "book",
    "clock",
    "vase",
    "scissors",
    "teddy bear",
    "hair drier",
    "toothbrush",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn transform() -> PreprocessTransform {
        PreprocessTransform {
            source_width: 416,
            source_height: 416,
            scale: 1.0,
        }
    }

    fn options() -> DecodeOptions {
        DecodeOptions {
            confidence_threshold: 0.25,
            iou_threshold: 0.5,
            maximum_detections: 100,
            class_mapping: BTreeMap::new(),
        }
    }

    #[test]
    fn preprocessing_is_bgr_nchw_with_top_left_padding() {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 1, Rgb([10, 20, 30])));
        let (tensor, transform) = preprocess(&image).expect("preprocess");
        assert_eq!(tensor.shape, [1, 3, 416, 416]);
        assert!((tensor.values[0] - 30.0).abs() < f32::EPSILON);
        assert!((tensor.values[416 * 416] - 20.0).abs() < f32::EPSILON);
        assert!((tensor.values[2 * 416 * 416] - 10.0).abs() < f32::EPSILON);
        assert!((tensor.values[415 * 416] - 114.0).abs() < f32::EPSILON);
        assert_eq!(transform.source_width, 2);
    }

    #[test]
    fn decoder_applies_grid_scores_class_aware_nms_and_original_geometry() {
        let rows = output_grids().len();
        let mut values = vec![0.0; rows * OUTPUT_COLUMNS];
        values[0] = 5.0;
        values[1] = 5.0;
        values[4] = 0.9;
        values[5 + 32] = 0.8;
        let second = OUTPUT_COLUMNS;
        values[second] = 4.0;
        values[second + 1] = 5.0;
        values[second + 4] = 0.8;
        values[second + 5 + 32] = 0.8;
        let output = NamedTensor {
            name: "output".to_owned(),
            shape: vec![1, rows, OUTPUT_COLUMNS],
            data: TensorData::Float32(values),
        };
        let detections = decode_output(&output, transform(), &options()).expect("decode");
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].class_id, 32);
        assert!((detections[0].score - 0.72).abs() < 0.001);
        assert!((detections[0].bbox.x_min - 36.0).abs() < 0.001);
        assert!((detections[0].bbox.x_max - 44.0).abs() < 0.001);
    }

    #[test]
    fn decoder_rejects_another_yolo_tensor_contract() {
        let output = NamedTensor {
            name: "output".to_owned(),
            shape: vec![1, 10, 84],
            data: TensorData::Float32(vec![0.0; 840]),
        };
        assert!(decode_output(&output, transform(), &options()).is_err());
    }

    #[test]
    fn manifest_requires_weights_and_never_claims_crop_or_commit() {
        let plugin = YoloOnnxPlugin::load().expect("plugin");
        assert!(plugin.manifest.weights.required);
        assert_eq!(
            plugin.descriptor_model().capabilities,
            [annotagent_core::ModelCapability::ObjectDetection]
        );
        let manifest = include_str!("../annotagent-plugin.toml").to_ascii_lowercase();
        assert!(!manifest.contains("crop"));
        assert!(!manifest.contains("commit"));
    }
}
