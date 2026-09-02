#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use annotagent_core::{
    ArtifactKind, ArtifactRef, ArtifactValidationState, BoxPrompt, BoxPromptSetArtifact,
    DetectionScore, ImageArtifact, MaskArtifactItem, MaskEncoding, MaskSetArtifact,
    PIPELINE_VISION_PROTOCOL_VERSION, PipelineArtifact, PipelineInferenceRequest,
    PipelineInferenceResponse, PointPrompt, PointPromptSetArtifact, ScoreSemantics,
    VisionCapability,
};
use annotagent_model_runtime_common::{BinaryMask, TensorF32, resize_mask, threshold_mask};
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
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use sha2::{Digest, Sha256};

const MODEL_ID: &str = "sam-vit-b-onnx";
const ENCODER_FILENAME: &str = "sam_image_encoder.onnx";
const DECODER_FILENAME: &str = "sam_mask_decoder.onnx";
const ENCODER_SIZE: u32 = 1024;
const EMBEDDING_CHANNELS: usize = 256;
const EMBEDDING_SIZE: usize = 64;
const EMBEDDING_CHANNELS_I64: i64 = 256;
const EMBEDDING_SIZE_I64: i64 = 64;
const LOW_RESOLUTION: usize = 256;
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_CACHE_VALUES: usize = 8 * 1024 * 1024;
const CACHE_MAGIC: &[u8; 8] = b"AASEMB1\0";

pub struct SamOnnxPlugin {
    manifest: PluginManifest,
    loaded: RwLock<Option<LoadedSam>>,
    embeddings: RwLock<HashMap<String, Arc<CachedEmbedding>>>,
}

struct LoadedSam {
    encoder: Arc<OnnxSession>,
    decoder: Arc<OnnxSession>,
    encoder_sha256: String,
    checkpoint_sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
struct CachedEmbedding {
    shape: Vec<usize>,
    values: Vec<f32>,
}

#[derive(Clone, Copy, Debug)]
struct SamImageTransform {
    source_width: u32,
    source_height: u32,
    resized_width: u32,
    resized_height: u32,
}

#[derive(Clone, Copy, Debug)]
struct DecodeOptions {
    multi_mask: bool,
    maximum_masks: usize,
    mask_threshold: f32,
}

enum PromptInput<'a> {
    Boxes(&'a BoxPromptSetArtifact),
    Points(&'a PointPromptSetArtifact),
}

impl SamOnnxPlugin {
    pub fn load() -> Result<Self, PluginSdkError> {
        let manifest = PluginManifest::from_toml(include_str!("../annotagent-plugin.toml"))
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        Ok(Self {
            manifest,
            loaded: RwLock::new(None),
            embeddings: RwLock::new(HashMap::new()),
        })
    }

    fn descriptor_model(&self) -> &annotagent_plugin_api::PluginModelManifest {
        &self.manifest.models[0]
    }

    fn loaded_model(
        &self,
    ) -> Result<(Arc<OnnxSession>, Arc<OnnxSession>, String, String), PluginSdkError> {
        let loaded = self
            .loaded
            .read()
            .map_err(|_| PluginSdkError::Plugin("model state lock was poisoned".to_owned()))?;
        let model = loaded
            .as_ref()
            .ok_or_else(|| PluginSdkError::Plugin("model weights are not loaded".to_owned()))?;
        Ok((
            Arc::clone(&model.encoder),
            Arc::clone(&model.decoder),
            model.encoder_sha256.clone(),
            model.checkpoint_sha256.clone(),
        ))
    }

    async fn embedding(
        &self,
        image: &DynamicImage,
        image_digest: &str,
        cache_dir: &Path,
        encoder: Arc<OnnxSession>,
        encoder_sha256: &str,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(Arc<CachedEmbedding>, SamImageTransform, bool), PluginSdkError> {
        let (tensor, transform) = preprocess(image)?;
        let cache_key = embedding_key(encoder_sha256, image_digest);
        if let Some(cached) = self
            .embeddings
            .read()
            .map_err(|_| PluginSdkError::Plugin("embedding cache lock was poisoned".to_owned()))?
            .get(&cache_key)
            .cloned()
        {
            return Ok((cached, transform, true));
        }
        let cache_path = cache_dir
            .join("sam-embeddings")
            .join(format!("{cache_key}.bin"));
        if cache_path.is_file() {
            let path = cache_path.clone();
            let cached = tokio::task::spawn_blocking(move || read_embedding(&path))
                .await
                .map_err(|error| {
                    PluginSdkError::Plugin(format!("embedding cache task failed: {error}"))
                })??;
            validate_embedding(&cached)?;
            let cached = Arc::new(cached);
            self.embeddings
                .write()
                .map_err(|_| {
                    PluginSdkError::Plugin("embedding cache lock was poisoned".to_owned())
                })?
                .insert(cache_key, Arc::clone(&cached));
            return Ok((cached, transform, true));
        }
        if cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
        }
        let input = NamedTensor::float32("input_image", tensor);
        let outputs = tokio::task::spawn_blocking(move || {
            encoder.infer(&[input], &InferenceCancellation::default())
        })
        .await
        .map_err(|error| PluginSdkError::Plugin(format!("encoder task failed: {error}")))?
        .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        if cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
        }
        let tensor = outputs
            .tensors
            .iter()
            .find(|tensor| tensor.name == "image_embeddings")
            .ok_or_else(|| {
                PluginSdkError::Plugin("encoder did not return image_embeddings".to_owned())
            })?;
        let TensorData::Float32(values) = &tensor.data else {
            return Err(PluginSdkError::Plugin(
                "image_embeddings must be float32".to_owned(),
            ));
        };
        let cached = CachedEmbedding {
            shape: tensor.shape.clone(),
            values: values.clone(),
        };
        validate_embedding(&cached)?;
        let parent = cache_path.parent().ok_or_else(|| {
            PluginSdkError::Plugin("embedding cache path has no parent".to_owned())
        })?;
        std::fs::create_dir_all(parent)?;
        let write_path = cache_path;
        let write_value = cached.clone();
        tokio::task::spawn_blocking(move || write_embedding(&write_path, &write_value))
            .await
            .map_err(|error| {
                PluginSdkError::Plugin(format!("embedding cache task failed: {error}"))
            })??;
        let cached = Arc::new(cached);
        self.embeddings
            .write()
            .map_err(|_| PluginSdkError::Plugin("embedding cache lock was poisoned".to_owned()))?
            .insert(cache_key, Arc::clone(&cached));
        Ok((cached, transform, false))
    }
}

#[async_trait]
impl ExpertModelPlugin for SamOnnxPlugin {
    async fn setup(&self, context: PluginRuntimeContext) -> Result<(), PluginSdkError> {
        let encoder_path = find_component(&context.weights_dir, ENCODER_FILENAME)?;
        let decoder_path = find_component(&context.weights_dir, DECODER_FILENAME)?;
        let (encoder, decoder) = tokio::task::spawn_blocking(move || {
            let options = SessionOptions::default();
            Ok::<_, annotagent_model_runtime_onnx::OnnxRuntimeError>((
                OnnxSession::load(encoder_path, &options)?,
                OnnxSession::load(decoder_path, &options)?,
            ))
        })
        .await
        .map_err(|error| PluginSdkError::Plugin(format!("model setup task failed: {error}")))?
        .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
        validate_encoder_contract(&encoder)?;
        validate_decoder_contract(&decoder)?;
        let encoder_sha256 = encoder.descriptor().sha256.clone();
        let decoder_sha256 = decoder.descriptor().sha256.clone();
        let checkpoint_sha256 = combined_checkpoint(&encoder_sha256, &decoder_sha256);
        std::fs::create_dir_all(context.cache_dir.join("sam-embeddings"))?;
        *self
            .loaded
            .write()
            .map_err(|_| PluginSdkError::Plugin("model state lock was poisoned".to_owned()))? =
            Some(LoadedSam {
                encoder: Arc::new(encoder),
                decoder: Arc::new(decoder),
                encoder_sha256,
                checkpoint_sha256,
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
        let (encoder, decoder, _, _) = self.loaded_model()?;
        tokio::task::spawn_blocking(move || {
            let cancellation = InferenceCancellation::default();
            let encoder_input = NamedTensor {
                name: "input_image".to_owned(),
                shape: vec![1, 3, ENCODER_SIZE as usize, ENCODER_SIZE as usize],
                data: TensorData::Float32(vec![
                    0.0;
                    3 * ENCODER_SIZE as usize * ENCODER_SIZE as usize
                ]),
            };
            encoder.warmup(&[encoder_input], 1, &cancellation)?;
            decoder.warmup(&warmup_decoder_inputs(), 1, &cancellation)?;
            Ok::<_, annotagent_model_runtime_onnx::OnnxRuntimeError>(())
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
            || request.operation != VisionCapability::PromptedSegmentation
        {
            return Err(PluginSdkError::Plugin(
                "request does not match the SAM model capability".to_owned(),
            ));
        }
        if context.cancellation.is_cancelled() {
            return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
        }
        let image_input = request
            .image
            .as_ref()
            .ok_or_else(|| PluginSdkError::InvalidImage("image is required".to_owned()))?;
        let image_digest = request_image_digest(image_input)?;
        let image = decode_image(image_input, MAX_IMAGE_BYTES)?;
        let image_artifact = one_image_artifact(&request)?;
        if image_artifact.image_id != request.image_id
            || image.dimensions() != (image_artifact.width, image_artifact.height)
        {
            return Err(PluginSdkError::Plugin(
                "request image does not match its Image Artifact lineage".to_owned(),
            ));
        }
        let prompts = one_prompt_input(&request)?;
        let options = decode_options(&request.parameters)?;
        let (encoder, decoder, encoder_sha256, checkpoint_sha256) = self.loaded_model()?;
        let (embedding, transform, cache_hit) = self
            .embedding(
                &image,
                &image_digest,
                &context.cache_dir,
                encoder,
                &encoder_sha256,
                &context.cancellation,
            )
            .await?;
        let masks = match prompts {
            PromptInput::Boxes(prompts) => {
                let mut masks = Vec::new();
                for prompt in &prompts.prompts {
                    masks.extend(
                        run_prompt(
                            Arc::clone(&decoder),
                            Arc::clone(&embedding),
                            PromptSpec::Box(prompt),
                            &prompts.reference,
                            transform,
                            options,
                            &context.cancellation,
                        )
                        .await?,
                    );
                }
                (prompts.reference.clone(), masks)
            }
            PromptInput::Points(prompts) => {
                let mut masks = Vec::new();
                for prompt in &prompts.prompts {
                    masks.extend(
                        run_prompt(
                            Arc::clone(&decoder),
                            Arc::clone(&embedding),
                            PromptSpec::Point(prompt),
                            &prompts.reference,
                            transform,
                            options,
                            &context.cancellation,
                        )
                        .await?,
                    );
                }
                (prompts.reference.clone(), masks)
            }
        };
        let reference = ArtifactRef {
            artifact_id: format!(
                "mask-set:{}:{}:{}",
                request.run_id, request.image_id, request.node_id
            ),
            source_node: request.node_id.clone(),
            port: "masks".to_owned(),
            artifact_type: ArtifactKind::MaskSet,
            item_id: None,
        };
        let artifact = MaskSetArtifact {
            reference,
            image_id: request.image_id,
            model_binding: request.model_id.clone(),
            source_prompts: masks.0,
            validation_state: ArtifactValidationState::Unvalidated,
            masks: masks.1,
            metadata: BTreeMap::from([
                (
                    "checkpoint_sha256".to_owned(),
                    serde_json::json!(checkpoint_sha256),
                ),
                (
                    "embedding_cache_hit".to_owned(),
                    serde_json::json!(cache_hit),
                ),
                ("runtime".to_owned(), serde_json::json!("rust-onnx-cpu")),
            ]),
        };
        artifact.validate().map_err(PluginSdkError::Plugin)?;
        Ok(PipelineInferenceResponse {
            protocol_version: PIPELINE_VISION_PROTOCOL_VERSION,
            request_id: Some(request.request_id),
            model_identity: Some(request.model_id),
            artifacts: vec![PipelineArtifact::MaskSet(artifact)],
            metadata: BTreeMap::from([
                (
                    "plugin_id".to_owned(),
                    serde_json::json!("org.annotagent.sam-onnx"),
                ),
                (
                    "contract".to_owned(),
                    serde_json::json!("sam-vit-b-encoder-decoder-v1"),
                ),
            ]),
            ..PipelineInferenceResponse::default()
        })
    }

    async fn cancel(&self, _request_id: &str) -> Result<(), PluginSdkError> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum PromptSpec<'a> {
    Box(&'a BoxPrompt),
    Point(&'a PointPrompt),
}

async fn run_prompt(
    decoder: Arc<OnnxSession>,
    embedding: Arc<CachedEmbedding>,
    prompt: PromptSpec<'_>,
    prompt_set: &ArtifactRef,
    transform: SamImageTransform,
    options: DecodeOptions,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<Vec<MaskArtifactItem>, PluginSdkError> {
    if cancellation.is_cancelled() {
        return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
    }
    let prompt_id = match &prompt {
        PromptSpec::Box(prompt) => prompt.id.clone(),
        PromptSpec::Point(prompt) => prompt.id.clone(),
    };
    let inputs = decoder_inputs(&embedding, prompt, transform);
    let outputs = tokio::task::spawn_blocking(move || {
        decoder.infer(&inputs, &InferenceCancellation::default())
    })
    .await
    .map_err(|error| PluginSdkError::Plugin(format!("decoder task failed: {error}")))?
    .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
    if cancellation.is_cancelled() {
        return Err(PluginSdkError::Plugin("request cancelled".to_owned()));
    }
    decode_masks(&outputs.tensors, &prompt_id, prompt_set, transform, options)
}

fn preprocess(image: &DynamicImage) -> Result<(TensorF32, SamImageTransform), PluginSdkError> {
    let (source_width, source_height) = image.dimensions();
    if source_width == 0 || source_height == 0 {
        return Err(PluginSdkError::InvalidImage(
            "image dimensions must be non-zero".to_owned(),
        ));
    }
    let scale = ENCODER_SIZE as f32 / source_width.max(source_height) as f32;
    let resized_width = ((source_width as f32 * scale).round() as u32).clamp(1, ENCODER_SIZE);
    let resized_height = ((source_height as f32 * scale).round() as u32).clamp(1, ENCODER_SIZE);
    let resized = image
        .resize_exact(resized_width, resized_height, FilterType::Triangle)
        .to_rgb8();
    let plane = ENCODER_SIZE as usize * ENCODER_SIZE as usize;
    let mut values = vec![0.0; plane * 3];
    let mean = [123.675_f32, 116.28, 103.53];
    let std = [58.395_f32, 57.12, 57.375];
    for (x, y, pixel) in resized.enumerate_pixels() {
        let index = y as usize * ENCODER_SIZE as usize + x as usize;
        for channel in 0..3 {
            values[channel * plane + index] =
                (f32::from(pixel[channel]) - mean[channel]) / std[channel];
        }
    }
    Ok((
        TensorF32::new(
            vec![1, 3, ENCODER_SIZE as usize, ENCODER_SIZE as usize],
            values,
        )
        .map_err(|error| PluginSdkError::Plugin(error.to_string()))?,
        SamImageTransform {
            source_width,
            source_height,
            resized_width,
            resized_height,
        },
    ))
}

fn decoder_inputs(
    embedding: &CachedEmbedding,
    prompt: PromptSpec<'_>,
    transform: SamImageTransform,
) -> Vec<NamedTensor> {
    let (coordinates, labels) = match prompt {
        PromptSpec::Box(prompt) => {
            let left = prompt.bbox.x() * transform.source_width as f32;
            let top = prompt.bbox.y() * transform.source_height as f32;
            let right = (prompt.bbox.x() + prompt.bbox.width()) * transform.source_width as f32;
            let bottom = (prompt.bbox.y() + prompt.bbox.height()) * transform.source_height as f32;
            (
                vec![
                    left * transform.x_scale(),
                    top * transform.y_scale(),
                    right * transform.x_scale(),
                    bottom * transform.y_scale(),
                ],
                vec![2.0, 3.0],
            )
        }
        PromptSpec::Point(prompt) => {
            let mut coordinates = Vec::with_capacity((prompt.points.len() + 1) * 2);
            let mut labels = Vec::with_capacity(prompt.points.len() + 1);
            for point in &prompt.points {
                coordinates
                    .push(point.point.x() * transform.source_width as f32 * transform.x_scale());
                coordinates
                    .push(point.point.y() * transform.source_height as f32 * transform.y_scale());
                labels.push(if point.positive { 1.0 } else { 0.0 });
            }
            coordinates.extend([0.0, 0.0]);
            labels.push(-1.0);
            (coordinates, labels)
        }
    };
    let point_count = labels.len();
    vec![
        NamedTensor {
            name: "image_embeddings".to_owned(),
            shape: embedding.shape.clone(),
            data: TensorData::Float32(embedding.values.clone()),
        },
        NamedTensor {
            name: "point_coords".to_owned(),
            shape: vec![1, point_count, 2],
            data: TensorData::Float32(coordinates),
        },
        NamedTensor {
            name: "point_labels".to_owned(),
            shape: vec![1, point_count],
            data: TensorData::Float32(labels),
        },
        NamedTensor {
            name: "mask_input".to_owned(),
            shape: vec![1, 1, LOW_RESOLUTION, LOW_RESOLUTION],
            data: TensorData::Float32(vec![0.0; LOW_RESOLUTION * LOW_RESOLUTION]),
        },
        NamedTensor {
            name: "has_mask_input".to_owned(),
            shape: vec![1],
            data: TensorData::Float32(vec![0.0]),
        },
        NamedTensor {
            name: "orig_im_size".to_owned(),
            shape: vec![2],
            data: TensorData::Float32(vec![
                transform.source_height as f32,
                transform.source_width as f32,
            ]),
        },
    ]
}

impl SamImageTransform {
    fn x_scale(self) -> f32 {
        self.resized_width as f32 / self.source_width as f32
    }

    fn y_scale(self) -> f32 {
        self.resized_height as f32 / self.source_height as f32
    }
}

fn decode_masks(
    outputs: &[NamedTensor],
    prompt_id: &str,
    prompt_set: &ArtifactRef,
    transform: SamImageTransform,
    options: DecodeOptions,
) -> Result<Vec<MaskArtifactItem>, PluginSdkError> {
    let masks = outputs
        .iter()
        .find(|tensor| tensor.name == "masks")
        .ok_or_else(|| PluginSdkError::Plugin("decoder did not return masks".to_owned()))?;
    let scores = outputs
        .iter()
        .find(|tensor| tensor.name == "iou_predictions")
        .ok_or_else(|| {
            PluginSdkError::Plugin("decoder did not return iou_predictions".to_owned())
        })?;
    let TensorData::Float32(mask_values) = &masks.data else {
        return Err(PluginSdkError::Plugin("masks must be float32".to_owned()));
    };
    let TensorData::Float32(score_values) = &scores.data else {
        return Err(PluginSdkError::Plugin(
            "iou_predictions must be float32".to_owned(),
        ));
    };
    let (mask_count, mask_height, mask_width) = match masks.shape.as_slice() {
        [1, count, height, width] | [count, height, width] => (*count, *height, *width),
        _ => {
            return Err(PluginSdkError::Plugin(format!(
                "masks has unsupported shape {:?}",
                masks.shape
            )));
        }
    };
    let per_mask = mask_height
        .checked_mul(mask_width)
        .ok_or_else(|| PluginSdkError::Plugin("mask tensor dimensions overflow".to_owned()))?;
    if mask_count == 0
        || mask_values.len() != mask_count.saturating_mul(per_mask)
        || score_values.len() < mask_count
    {
        return Err(PluginSdkError::Plugin(
            "mask and score tensor lengths do not match their shapes".to_owned(),
        ));
    }
    let mut selected = (0..mask_count).collect::<Vec<_>>();
    selected.sort_by(|left, right| score_values[*right].total_cmp(&score_values[*left]));
    selected.truncate(if options.multi_mask {
        options.maximum_masks.min(mask_count)
    } else {
        1
    });
    selected
        .into_iter()
        .enumerate()
        .map(|(rank, mask_index)| {
            let start = mask_index * per_mask;
            let mask = threshold_mask(
                &mask_values[start..start + per_mask],
                u32::try_from(mask_width)
                    .map_err(|_| PluginSdkError::Plugin("mask width exceeds u32".to_owned()))?,
                u32::try_from(mask_height)
                    .map_err(|_| PluginSdkError::Plugin("mask height exceeds u32".to_owned()))?,
                options.mask_threshold,
            )
            .map_err(|error| PluginSdkError::Plugin(error.to_string()))?;
            let restored =
                if mask.width == transform.source_width && mask.height == transform.source_height {
                    mask
                } else {
                    resize_mask(&mask, transform.source_width, transform.source_height)
                        .map_err(|error| PluginSdkError::Plugin(error.to_string()))?
                };
            let score = score_values[mask_index];
            Ok(MaskArtifactItem {
                mask_id: format!("mask:{prompt_id}:{rank}"),
                prompt: prompt_set.item(prompt_id),
                mask: binary_mask_to_rle(&restored),
                score: DetectionScore::new(Some(score), ScoreSemantics::RelativeConfidence)
                    .map_err(PluginSdkError::Plugin)?,
                attributes: BTreeMap::from([
                    (
                        "decoder_mask_index".to_owned(),
                        serde_json::json!(mask_index),
                    ),
                    ("decoder_rank".to_owned(), serde_json::json!(rank)),
                ]),
            })
        })
        .collect()
}

fn binary_mask_to_rle(mask: &BinaryMask) -> MaskEncoding {
    let mut runs = Vec::<u64>::new();
    let mut foreground = false;
    let mut length = 0_u64;
    for x in 0..mask.width {
        for y in 0..mask.height {
            let next = mask.get(x, y);
            if next == foreground {
                length += 1;
            } else {
                runs.push(length);
                length = 1;
                foreground = next;
            }
        }
    }
    runs.push(length);
    MaskEncoding::CocoRle {
        width: mask.width,
        height: mask.height,
        counts: runs
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(" "),
    }
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
            "SAM requires exactly one Image Artifact".to_owned(),
        )),
    }
}

fn one_prompt_input(request: &PipelineInferenceRequest) -> Result<PromptInput<'_>, PluginSdkError> {
    let mut prompts = request
        .input_artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PipelineArtifact::BoxPromptSet(prompts) => Some(PromptInput::Boxes(prompts)),
            PipelineArtifact::PointPromptSet(prompts) => Some(PromptInput::Points(prompts)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if prompts.len() != 1 {
        return Err(PluginSdkError::Plugin(
            "SAM requires exactly one BoxPromptSet or PointPromptSet".to_owned(),
        ));
    }
    let prompt = prompts.remove(0);
    let image_id = match &prompt {
        PromptInput::Boxes(prompts) => prompts.image_id,
        PromptInput::Points(prompts) => prompts.image_id,
    };
    if image_id != request.image_id {
        return Err(PluginSdkError::Plugin(
            "prompt set belongs to another image".to_owned(),
        ));
    }
    Ok(prompt)
}

fn decode_options(
    parameters: &BTreeMap<String, serde_json::Value>,
) -> Result<DecodeOptions, PluginSdkError> {
    let multi_mask = parameters
        .get("multi_mask")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let maximum_masks = parameters
        .get("maximum_masks")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(3);
    let maximum_masks = usize::try_from(maximum_masks)
        .ok()
        .filter(|value| (1..=4).contains(value))
        .ok_or_else(|| PluginSdkError::Plugin("maximum_masks must be within [1,4]".to_owned()))?;
    let mask_threshold = parameters
        .get("mask_threshold")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    if !mask_threshold.is_finite() || !(-100.0..=100.0).contains(&mask_threshold) {
        return Err(PluginSdkError::Plugin(
            "mask_threshold must be finite and within [-100,100]".to_owned(),
        ));
    }
    Ok(DecodeOptions {
        multi_mask,
        maximum_masks,
        mask_threshold,
    })
}

fn validate_encoder_contract(session: &OnnxSession) -> Result<(), PluginSdkError> {
    let descriptor = session.descriptor();
    if descriptor.inputs.len() != 1
        || descriptor.inputs[0].name != "input_image"
        || descriptor.inputs[0].element_type != "f32"
        || descriptor.inputs[0].shape != [1, 3, i64::from(ENCODER_SIZE), i64::from(ENCODER_SIZE)]
        || descriptor.outputs.len() != 1
        || descriptor.outputs[0].name != "image_embeddings"
        || descriptor.outputs[0].element_type != "f32"
        || descriptor.outputs[0].shape
            != [
                1,
                EMBEDDING_CHANNELS_I64,
                EMBEDDING_SIZE_I64,
                EMBEDDING_SIZE_I64,
            ]
    {
        return Err(PluginSdkError::Plugin(format!(
            "checkpoint does not match the SAM image encoder contract: inputs={:?}, outputs={:?}",
            descriptor.inputs, descriptor.outputs
        )));
    }
    Ok(())
}

fn validate_decoder_contract(session: &OnnxSession) -> Result<(), PluginSdkError> {
    let descriptor = session.descriptor();
    let expected_inputs = [
        "image_embeddings",
        "point_coords",
        "point_labels",
        "mask_input",
        "has_mask_input",
        "orig_im_size",
    ];
    if descriptor.inputs.len() != expected_inputs.len()
        || expected_inputs.iter().any(|name| {
            !descriptor
                .inputs
                .iter()
                .any(|input| input.name == *name && input.element_type == "f32")
        })
        || !descriptor
            .outputs
            .iter()
            .any(|output| output.name == "masks" && output.element_type == "f32")
        || !descriptor
            .outputs
            .iter()
            .any(|output| output.name == "iou_predictions" && output.element_type == "f32")
    {
        return Err(PluginSdkError::Plugin(format!(
            "checkpoint does not match the SAM mask decoder contract: inputs={:?}, outputs={:?}",
            descriptor.inputs, descriptor.outputs
        )));
    }
    Ok(())
}

fn validate_embedding(embedding: &CachedEmbedding) -> Result<(), PluginSdkError> {
    if embedding.shape != [1, EMBEDDING_CHANNELS, EMBEDDING_SIZE, EMBEDDING_SIZE]
        || embedding.values.len() != EMBEDDING_CHANNELS * EMBEDDING_SIZE * EMBEDDING_SIZE
        || embedding.values.len() > MAX_CACHE_VALUES
        || embedding.values.iter().any(|value| !value.is_finite())
    {
        return Err(PluginSdkError::Plugin(
            "cached embedding does not match the SAM contract".to_owned(),
        ));
    }
    Ok(())
}

fn request_image_digest(image: &annotagent_core::ModelImage) -> Result<String, PluginSdkError> {
    let bytes = STANDARD
        .decode(&image.data_base64)
        .map_err(|_| PluginSdkError::InvalidImage("input is not valid base64".to_owned()))?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(PluginSdkError::InvalidImage(
            "decoded image is empty or exceeds the configured limit".to_owned(),
        ));
    }
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn embedding_key(encoder_sha256: &str, image_digest: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(encoder_sha256.as_bytes());
    hasher.update(b"\0");
    hasher.update(image_digest.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn combined_checkpoint(encoder_sha256: &str, decoder_sha256: &str) -> String {
    let identity = format!("image_encoder:{encoder_sha256}\nmask_decoder:{decoder_sha256}");
    format!("{:x}", Sha256::digest(identity.as_bytes()))
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

fn write_embedding(path: &Path, embedding: &CachedEmbedding) -> Result<(), PluginSdkError> {
    validate_embedding(embedding)?;
    let parent = path
        .parent()
        .ok_or_else(|| PluginSdkError::Plugin("embedding cache path has no parent".to_owned()))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".embedding-{}", std::process::id()));
    let mut file = File::create(&temporary)?;
    file.write_all(CACHE_MAGIC)?;
    file.write_all(&(embedding.shape.len() as u32).to_le_bytes())?;
    for dimension in &embedding.shape {
        file.write_all(&(*dimension as u64).to_le_bytes())?;
    }
    file.write_all(&(embedding.values.len() as u64).to_le_bytes())?;
    for value in &embedding.values {
        file.write_all(&value.to_le_bytes())?;
    }
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn read_embedding(path: &Path) -> Result<CachedEmbedding, PluginSdkError> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
        return Err(PluginSdkError::Plugin(
            "embedding cache file is invalid or too large".to_owned(),
        ));
    }
    let mut file = File::open(path)?;
    let mut magic = [0_u8; 8];
    file.read_exact(&mut magic)?;
    if &magic != CACHE_MAGIC {
        return Err(PluginSdkError::Plugin(
            "embedding cache magic does not match".to_owned(),
        ));
    }
    let dimensions = usize::try_from(read_u32(&mut file)?)
        .map_err(|_| PluginSdkError::Plugin("embedding rank exceeds usize".to_owned()))?;
    if dimensions == 0 || dimensions > 8 {
        return Err(PluginSdkError::Plugin(
            "embedding cache rank is invalid".to_owned(),
        ));
    }
    let mut shape = Vec::with_capacity(dimensions);
    for _ in 0..dimensions {
        shape.push(
            usize::try_from(read_u64(&mut file)?).map_err(|_| {
                PluginSdkError::Plugin("embedding dimension exceeds usize".to_owned())
            })?,
        );
    }
    let value_count = usize::try_from(read_u64(&mut file)?)
        .ok()
        .filter(|count| *count <= MAX_CACHE_VALUES)
        .ok_or_else(|| PluginSdkError::Plugin("embedding value count is invalid".to_owned()))?;
    let mut values = Vec::with_capacity(value_count);
    for _ in 0..value_count {
        let mut bytes = [0_u8; 4];
        file.read_exact(&mut bytes)?;
        values.push(f32::from_le_bytes(bytes));
    }
    let embedding = CachedEmbedding { shape, values };
    validate_embedding(&embedding)?;
    Ok(embedding)
}

fn read_u32(reader: &mut impl Read) -> Result<u32, PluginSdkError> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Result<u64, PluginSdkError> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn warmup_decoder_inputs() -> Vec<NamedTensor> {
    vec![
        NamedTensor {
            name: "image_embeddings".to_owned(),
            shape: vec![1, EMBEDDING_CHANNELS, EMBEDDING_SIZE, EMBEDDING_SIZE],
            data: TensorData::Float32(vec![
                0.0;
                EMBEDDING_CHANNELS * EMBEDDING_SIZE * EMBEDDING_SIZE
            ]),
        },
        NamedTensor {
            name: "point_coords".to_owned(),
            shape: vec![1, 2, 2],
            data: TensorData::Float32(vec![0.0; 4]),
        },
        NamedTensor {
            name: "point_labels".to_owned(),
            shape: vec![1, 2],
            data: TensorData::Float32(vec![1.0, -1.0]),
        },
        NamedTensor {
            name: "mask_input".to_owned(),
            shape: vec![1, 1, LOW_RESOLUTION, LOW_RESOLUTION],
            data: TensorData::Float32(vec![0.0; LOW_RESOLUTION * LOW_RESOLUTION]),
        },
        NamedTensor {
            name: "has_mask_input".to_owned(),
            shape: vec![1],
            data: TensorData::Float32(vec![0.0]),
        },
        NamedTensor {
            name: "orig_im_size".to_owned(),
            shape: vec![2],
            data: TensorData::Float32(vec![ENCODER_SIZE as f32; 2]),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use annotagent_core::{NormalizedPoint, NormalizedRect, PromptPoint, mask_tight_bbox};
    use image::{Rgb, RgbImage};

    fn transform() -> SamImageTransform {
        SamImageTransform {
            source_width: 4,
            source_height: 2,
            resized_width: 1024,
            resized_height: 512,
        }
    }

    #[test]
    fn manifest_declares_two_weight_components_and_only_prompted_segmentation() {
        let plugin = SamOnnxPlugin::load().expect("plugin");
        assert_eq!(plugin.manifest.weights.components.len(), 2);
        assert_eq!(
            plugin.descriptor_model().capabilities,
            [annotagent_core::ModelCapability::PromptedSegmentation]
        );
        assert_eq!(
            plugin.descriptor_model().output_contracts[0].data_type,
            annotagent_core::ContractDataType::Artifact(ArtifactKind::MaskSet)
        );
    }

    #[test]
    fn preprocess_matches_sam_normalization_and_coordinate_scaling() {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 2, Rgb([124, 116, 104])));
        let (tensor, transform) = preprocess(&image).expect("preprocess");
        assert_eq!(tensor.shape, [1, 3, 1024, 1024]);
        assert!((transform.x_scale() - 256.0).abs() < f32::EPSILON);
        assert!((transform.y_scale() - 256.0).abs() < f32::EPSILON);
        assert!(tensor.values[0].abs() < 0.01);
        assert!(tensor.values[1023 * 1024].abs() < f32::EPSILON);
    }

    #[test]
    fn point_and_box_prompts_preserve_sam_labels() {
        let bbox = BoxPrompt {
            id: "box-1".to_owned(),
            subject: ArtifactRef {
                artifact_id: "detections".to_owned(),
                source_node: "detect".to_owned(),
                port: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                item_id: Some("ball".to_owned()),
            },
            bbox: NormalizedRect::new(0.25, 0.25, 0.5, 0.5).expect("bbox"),
            attributes: BTreeMap::new(),
        };
        let embedding = CachedEmbedding {
            shape: vec![1, 256, 64, 64],
            values: vec![0.0; 256 * 64 * 64],
        };
        let box_inputs = decoder_inputs(&embedding, PromptSpec::Box(&bbox), transform());
        assert_eq!(box_inputs[2].data, TensorData::Float32(vec![2.0, 3.0]));

        let point = PointPrompt {
            id: "point-1".to_owned(),
            subject: bbox.subject.clone(),
            points: vec![PromptPoint {
                point: NormalizedPoint::new(0.5, 0.5).expect("point"),
                positive: true,
            }],
            attributes: BTreeMap::new(),
        };
        let point_inputs = decoder_inputs(&embedding, PromptSpec::Point(&point), transform());
        assert_eq!(point_inputs[2].data, TensorData::Float32(vec![1.0, -1.0]));
    }

    #[test]
    fn decoder_selects_best_mask_and_emits_geometry_safe_rle() {
        let outputs = vec![
            NamedTensor {
                name: "masks".to_owned(),
                shape: vec![1, 2, 2, 4],
                data: TensorData::Float32(vec![
                    -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0,
                    1.0, 1.0, -1.0,
                ]),
            },
            NamedTensor {
                name: "iou_predictions".to_owned(),
                shape: vec![1, 2],
                data: TensorData::Float32(vec![0.2, 0.9]),
            },
        ];
        let prompt_set = ArtifactRef {
            artifact_id: "prompts".to_owned(),
            source_node: "prompt".to_owned(),
            port: "prompts".to_owned(),
            artifact_type: ArtifactKind::BoxPromptSet,
            item_id: None,
        };
        let masks = decode_masks(
            &outputs,
            "box-1",
            &prompt_set,
            transform(),
            DecodeOptions {
                multi_mask: false,
                maximum_masks: 3,
                mask_threshold: 0.0,
            },
        )
        .expect("masks");
        assert_eq!(masks.len(), 1);
        assert_eq!(masks[0].prompt, prompt_set.item("box-1"));
        assert_eq!(masks[0].score.value, Some(0.9));
        let bbox = mask_tight_bbox(&masks[0].mask).expect("bbox");
        assert!(bbox.width() > 0.0 && bbox.height() > 0.0);
    }

    #[test]
    fn embedding_cache_round_trip_is_bounded_and_exact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("embedding.bin");
        let embedding = CachedEmbedding {
            shape: vec![1, 256, 64, 64],
            values: vec![0.25; 256 * 64 * 64],
        };
        write_embedding(&path, &embedding).expect("write");
        assert_eq!(read_embedding(&path).expect("read"), embedding);
    }
}
