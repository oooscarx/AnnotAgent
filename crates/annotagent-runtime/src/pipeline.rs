//! Domain-neutral executable Core nodes for Label Pipeline intermediate Artifacts.

use std::{collections::BTreeMap, sync::OnceLock};

use annotagent_core::{
    AnnotationCandidateSet, ArtifactId, ArtifactKind, ArtifactProvenance, ArtifactRef,
    ArtifactRole, ArtifactValidationState, AttributeValue, BoxPromptSetArtifact,
    CandidateAgreement, CandidateCluster, CandidateClusterSetArtifact, ClassificationSetArtifact,
    CorrectionRisk, CropSetArtifact, Detection, DetectionEvidence, DetectionSetArtifact,
    EvidenceAcceptRule, EvidenceFallbackRule, EvidenceGateConfig, EvidenceGateDecision,
    EvidenceGateInput, EvidenceGateReason, EvidenceGateReport, EvidenceRejectRule,
    EvidenceReviewRule, GEOMETRY_REFINEMENT_TRACE_SCHEMA_VERSION, GeometryRefinementThresholds,
    GeometryRefinementTrace, IssueSeverity, LabelId, MaskEncoding, MaskSetArtifact,
    PipelineArtifact, PolygonArtifactItem, PolygonSetArtifact, SuggestedAction, TaskId,
    ValidationEvidence, ValidationIssue, VisionArtifact, VisionArtifactValue, VisionCapability,
    evaluate_geometry_refinement, mask_tight_bbox,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};

pub const CORE_CROP: &str = "core.crop";
pub const CORE_EXISTING_ANNOTATIONS: &str = "core.existing_annotations";
pub const CORE_DETECTIONS_TO_BOX_PROMPTS: &str = "core.detections_to_box_prompts";
pub const CORE_MASK_TO_BBOX: &str = "core.mask_to_bbox";
pub const CORE_GEOMETRY_QUALITY_EVALUATION: &str = "core.geometry_quality_evaluation";
pub const CORE_GEOMETRY_DECISION: &str = "core.geometry_decision";
pub const CORE_MASK_TO_POLYGON: &str = "core.mask_to_polygon";
pub const CORE_RESIZE: &str = "core.resize";
pub const CORE_TILE: &str = "core.tile";
pub const CORE_FILTER: &str = "core.filter";
pub const CORE_MAP_LABEL: &str = "core.map_label";
pub const CORE_SELECT_AND_MAP: &str = "core.select_and_map";
pub const CORE_PROJECT_COORDINATES: &str = "core.project_coordinates";
pub const CORE_ATTACH_RESULT: &str = "core.attach_result";
pub const CORE_ATTACH_ATTRIBUTE: &str = "core.attach_attribute";
pub const CORE_CONFIDENCE_GATE: &str = "core.confidence_gate";
pub const CORE_CANDIDATE_MATCH: &str = "core.match_detection_sets";
pub const CORE_COMBINE_EVIDENCE: &str = "core.combine_evidence";
pub const CORE_EVIDENCE_GATE: &str = "core.evidence_gate";
pub const CORE_DECISION: &str = "core.decision";
pub const CORE_PROJECT_CANDIDATES: &str = "core.project_detection_candidates";
pub const CORE_REJECT: &str = "core.reject_candidates";
pub const CORE_ARTIFACT_CACHE: &str = "core.artifact_cache";
pub const CORE_IMAGE_STATISTICS: &str = "core.compute_image_statistics";

#[derive(Debug, Default, Clone, Copy)]
pub struct CorePipelineRunner;

#[async_trait]
impl DagNodeRunner for CorePipelineRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        match context.node.node_type.as_str() {
            CORE_RESIZE => run_resize(&context),
            CORE_TILE => run_tile(&context),
            CORE_CROP => run_crop(&context),
            CORE_DETECTIONS_TO_BOX_PROMPTS => run_detections_to_box_prompts(&context),
            CORE_MASK_TO_BBOX => run_mask_to_bbox(&context),
            CORE_GEOMETRY_QUALITY_EVALUATION => run_geometry_quality_evaluation(&context),
            CORE_GEOMETRY_DECISION => run_geometry_decision(&context),
            CORE_MASK_TO_POLYGON => run_mask_to_polygon(&context),
            CORE_FILTER => run_filter(&context),
            CORE_MAP_LABEL => run_map_label(&context),
            CORE_SELECT_AND_MAP => run_select_and_map(&context),
            CORE_PROJECT_COORDINATES => run_project_coordinates(&context),
            CORE_ATTACH_RESULT => run_attach_result(&context),
            CORE_ATTACH_ATTRIBUTE => run_attach_attribute(&context),
            CORE_CONFIDENCE_GATE => run_confidence_gate(&context),
            CORE_CANDIDATE_MATCH | CORE_COMBINE_EVIDENCE => run_candidate_match(&context),
            CORE_EVIDENCE_GATE => run_evidence_gate(&context),
            CORE_DECISION => run_decision(&context),
            CORE_PROJECT_CANDIDATES => run_project_candidates(&context),
            CORE_REJECT => run_reject(&context),
            CORE_IMAGE_STATISTICS => run_image_statistics(&context),
            CORE_ARTIFACT_CACHE => Ok(DagNodeOutput {
                pipeline_artifacts: context.input_pipeline_artifacts,
                metadata: BTreeMap::from([("cached".to_owned(), serde_json::json!(true))]),
                ..DagNodeOutput::default()
            }),
            operation => Err(DagNodeFailure::terminal(
                "unsupported_core_pipeline_node",
                format!("Core Pipeline runner does not implement {operation:?}"),
            )),
        }
    }
}

fn run_resize(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let image = one_image(context)?;
    image
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_image", error))?;
    let target_width = optional_u32_parameter(context, "target_width")?;
    let target_height = optional_u32_parameter(context, "target_height")?;
    let max_edge = optional_u32_parameter(context, "max_edge")?;
    let maximum_pixels = optional_u64_parameter(context, "maximum_pixels")?;
    if target_width.is_none()
        && target_height.is_none()
        && max_edge.is_none()
        && maximum_pixels.is_none()
    {
        return Err(DagNodeFailure::terminal(
            "resize_target_missing",
            "Resize requires target_width, target_height, max_edge, or maximum_pixels",
        ));
    }
    let mut requested_scales = Vec::new();
    if let Some(width) = target_width {
        requested_scales.push(f64::from(width) / f64::from(image.width));
    }
    if let Some(height) = target_height {
        requested_scales.push(f64::from(height) / f64::from(image.height));
    }
    if let Some(edge) = max_edge {
        requested_scales.push(f64::from(edge) / f64::from(image.width.max(image.height)));
    }
    if let Some(pixels) = maximum_pixels {
        let source_pixels = f64::from(image.width) * f64::from(image.height);
        requested_scales.push((pixels as f64 / source_pixels).sqrt());
    }
    let mut scale = requested_scales.into_iter().fold(f64::INFINITY, f64::min);
    if !boolean_parameter(context, "allow_upscale", false)? {
        scale = scale.min(1.0);
    }
    if !scale.is_finite() || scale <= 0.0 {
        return Err(DagNodeFailure::terminal(
            "invalid_resize_scale",
            "Resize computed an invalid scale",
        ));
    }
    let reference = output_reference(context, "image", ArtifactKind::Image)?;
    let resized = annotagent_core::ImageArtifact {
        reference: reference.clone(),
        image_id: image.image_id,
        width: (f64::from(image.width) * scale).round().max(1.0) as u32,
        height: (f64::from(image.height) * scale).round().max(1.0) as u32,
        mime_type: image.mime_type.clone(),
        blob_ref: format!("virtual-resize://{}", reference.artifact_id),
        parent: Some(image.reference.clone()),
        root_region: image.root_region,
    };
    resized
        .validate()
        .map_err(|error| DagNodeFailure::terminal("resize_failed", error))?;
    Ok(output(PipelineArtifact::Image(resized)))
}

fn run_tile(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let image = one_image(context)?;
    image
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_image", error))?;
    let square = optional_u32_parameter(context, "tile_size")?;
    let tile_width = optional_u32_parameter(context, "tile_width")?
        .or(square)
        .unwrap_or(1024)
        .min(image.width);
    let tile_height = optional_u32_parameter(context, "tile_height")?
        .or(square)
        .unwrap_or(1024)
        .min(image.height);
    let overlap = number_parameter(context, "overlap", 0.15)?;
    if !(0.0..0.9).contains(&overlap) {
        return Err(DagNodeFailure::terminal(
            "invalid_tile_overlap",
            "Tile overlap must be within [0,0.9)",
        ));
    }
    let maximum_tiles = optional_u32_parameter(context, "maximum_tiles")?.unwrap_or(64) as usize;
    if maximum_tiles == 0 {
        return Err(DagNodeFailure::terminal(
            "invalid_maximum_tiles",
            "maximum_tiles must be greater than zero",
        ));
    }
    let x_offsets = tile_offsets(image.width, tile_width, overlap);
    let y_offsets = tile_offsets(image.height, tile_height, overlap);
    if x_offsets.len().saturating_mul(y_offsets.len()) > maximum_tiles {
        return Err(DagNodeFailure::terminal(
            "tile_limit_exceeded",
            format!(
                "Tile would produce {} images, exceeding maximum_tiles={maximum_tiles}",
                x_offsets.len().saturating_mul(y_offsets.len())
            ),
        ));
    }
    let output = output_reference(context, "images", ArtifactKind::Image)?;
    let parent_region = image.root_region.unwrap_or(
        annotagent_core::NormalizedRect::new(0.0, 0.0, 1.0, 1.0)
            .map_err(|error| DagNodeFailure::terminal("tile_failed", error.to_string()))?,
    );
    let mut tiles = Vec::new();
    for (row, y) in y_offsets.iter().enumerate() {
        for (column, x) in x_offsets.iter().enumerate() {
            let local_x = *x as f32 / image.width as f32;
            let local_y = *y as f32 / image.height as f32;
            let local_width = tile_width as f32 / image.width as f32;
            let local_height = tile_height as f32 / image.height as f32;
            let root_region = annotagent_core::NormalizedRect::new(
                parent_region.x() + local_x * parent_region.width(),
                parent_region.y() + local_y * parent_region.height(),
                local_width * parent_region.width(),
                local_height * parent_region.height(),
            )
            .map_err(|error| DagNodeFailure::terminal("tile_failed", error.to_string()))?;
            let tile_id = format!("r{row}-c{column}");
            let reference = output.item(tile_id.clone());
            tiles.push(PipelineArtifact::Image(annotagent_core::ImageArtifact {
                reference: reference.clone(),
                image_id: image.image_id,
                width: tile_width,
                height: tile_height,
                mime_type: image.mime_type.clone(),
                blob_ref: format!("virtual-tile://{}/{}", output.artifact_id, tile_id),
                parent: Some(image.reference.clone()),
                root_region: Some(root_region),
            }));
        }
    }
    Ok(DagNodeOutput {
        pipeline_artifacts: tiles,
        metadata: BTreeMap::from([(
            "tile_count".to_owned(),
            serde_json::json!(x_offsets.len() * y_offsets.len()),
        )]),
        ..DagNodeOutput::default()
    })
}

fn tile_offsets(total: u32, tile: u32, overlap: f64) -> Vec<u32> {
    if tile >= total {
        return vec![0];
    }
    let step = ((f64::from(tile) * (1.0 - overlap)).round() as u32).max(1);
    let mut offsets = Vec::new();
    let mut offset = 0_u32;
    while offset.saturating_add(tile) < total {
        offsets.push(offset);
        offset = offset.saturating_add(step);
    }
    offsets.push(total - tile);
    offsets.sort_unstable();
    offsets.dedup();
    offsets
}

/// Converts evidence clusters back into a `DetectionSet` for downstream Core Crop fan-out while
/// retaining every source evidence item and its original score semantics.
fn run_project_candidates(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let clusters = one_candidate_cluster_set(context)?;
    clusters
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_candidate_clusters", error))?;
    let reference = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    let detections = clusters
        .candidates
        .iter()
        .map(|candidate| {
            let representative = candidate
                .members
                .iter()
                .find(|member| member.bbox == candidate.representative_bbox)
                .or_else(|| candidate.members.first())
                .ok_or_else(|| {
                    DagNodeFailure::terminal(
                        "candidate_evidence_missing",
                        "candidate projection requires source evidence",
                    )
                })?;
            let mut detection = Detection::from_source(
                candidate.id.clone(),
                representative.query_id.clone(),
                representative.model_label.clone(),
                Some(candidate.target_label.clone()),
                candidate.representative_bbox,
                representative.score,
                annotagent_core::DetectionSource {
                    model_id: representative.source_model_id.clone(),
                    capability: representative.source_capability,
                    artifact_id: reference.artifact_id.clone(),
                },
            )
            .map_err(|error| DagNodeFailure::terminal("candidate_projection_failed", error))?;
            detection.evidence.clone_from(&candidate.members);
            Ok(detection)
        })
        .collect::<Result<Vec<_>, DagNodeFailure>>()?;
    let model_binding = clusters
        .candidates
        .iter()
        .flat_map(|candidate| &candidate.members)
        .map(|member| member.source_model_id.as_str())
        .next()
        .unwrap_or("evidence-projection")
        .to_owned();
    let projected = DetectionSetArtifact {
        schema_version: annotagent_core::DETECTION_ARTIFACT_SCHEMA_VERSION,
        reference,
        image_id: clusters.image_id,
        model_binding,
        validation_state: clusters.validation_state,
        detections,
        metadata: BTreeMap::from([
            (
                "projected_from".to_owned(),
                serde_json::json!(clusters.reference.artifact_id),
            ),
            (
                "source_detection_sets".to_owned(),
                serde_json::to_value(&clusters.source_detection_sets)
                    .unwrap_or_else(|_| serde_json::json!([])),
            ),
        ]),
    };
    projected
        .validate()
        .map_err(|error| DagNodeFailure::terminal("candidate_projection_failed", error))?;
    let mut metadata = propagated_evidence_metadata(context)?;
    let mut projection_issues = collected_validation_issues(context)?;
    projection_issues.extend(clusters.candidates.iter().filter_map(|candidate| {
        let (code, message) = match candidate.agreement {
            CandidateAgreement::GeometryConflict => (
                "geometry_conflict",
                "detector sources disagree on candidate geometry",
            ),
            CandidateAgreement::LabelConflict => (
                "label_conflict",
                "detector sources disagree on the Project Label",
            ),
            CandidateAgreement::SingleSource | CandidateAgreement::MultiSourceAgreement { .. } => {
                return None;
            }
        };
        Some(ValidationIssue {
            code: code.to_owned(),
            severity: IssueSeverity::Warning,
            annotation_ids: Vec::new(),
            message: message.to_owned(),
            suggested_action: SuggestedAction::HumanReview,
            evidence: ValidationEvidence::Rule {
                facts: BTreeMap::from([("candidate_id".to_owned(), candidate.id.clone())]),
            },
        })
    }));
    if !projection_issues.is_empty() {
        metadata.insert(
            "validation_issues".to_owned(),
            serde_json::to_value(projection_issues).unwrap_or_else(|_| serde_json::json!([])),
        );
    }
    for source in context.input_metadata.values() {
        if let Some(report) = source.get("recovery_agent") {
            metadata.insert("recovery_agent".to_owned(), report.clone());
        }
    }
    Ok(DagNodeOutput {
        pipeline_artifacts: vec![PipelineArtifact::DetectionSet(projected)],
        metadata,
        ..DagNodeOutput::default()
    })
}

fn run_reject(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    if context.input_pipeline_artifacts.is_empty() {
        return Err(DagNodeFailure::terminal(
            "reject_input_missing",
            "Reject Candidates requires candidate evidence",
        ));
    }
    Ok(DagNodeOutput {
        pipeline_artifacts: context.input_pipeline_artifacts.clone(),
        metadata: BTreeMap::from([
            ("decision".to_owned(), serde_json::json!("reject")),
            (
                "reason".to_owned(),
                context
                    .node
                    .parameters
                    .get("reason")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!("candidate rejected by workflow policy")),
            ),
        ]),
        ..DagNodeOutput::default()
    })
}

fn run_image_statistics(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let image = one_image(context)?;
    let aspect_ratio = f64::from(image.width) / f64::from(image.height);
    let artifact = VisionArtifact {
        id: ArtifactId::new(),
        image_id: image.image_id,
        task_id: None,
        label: None,
        role: ArtifactRole::Evidence,
        value: VisionArtifactValue::Attributes {
            values: BTreeMap::from([
                (
                    "width".to_owned(),
                    AttributeValue::Number(f64::from(image.width)),
                ),
                (
                    "height".to_owned(),
                    AttributeValue::Number(f64::from(image.height)),
                ),
                (
                    "aspect_ratio".to_owned(),
                    AttributeValue::Number(aspect_ratio),
                ),
            ]),
        },
        source_node: context.node.id.clone(),
        confidence: Some(1.0),
        metadata: BTreeMap::from([("blob_ref".to_owned(), serde_json::json!(image.blob_ref))]),
        validation_state: ArtifactValidationState::Valid,
        provenance: ArtifactProvenance {
            tool: Some(CORE_IMAGE_STATISTICS.to_owned()),
            ..ArtifactProvenance::default()
        },
        revision: 1,
        replaces_artifact_id: None,
        created_at: Utc::now(),
    };
    artifact
        .validate()
        .map_err(|error| DagNodeFailure::terminal("image_statistics_failed", error.to_string()))?;
    Ok(DagNodeOutput {
        artifacts: vec![artifact],
        ..DagNodeOutput::default()
    })
}

fn run_crop(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let image = one_image(context)?;
    let detections = one_detection_set(context)?;
    let padding = number_parameter(context, "padding", 0.0)? as f32;
    let reference = output_reference(context, "crops", ArtifactKind::CropSet)?;
    let mut crops = CropSetArtifact::fan_out(reference, detections, padding, |detection| {
        Some(format!(
            "artifact-cache://{}/{}",
            context.node.id, detection.detection_id
        ))
    })
    .map_err(|error| DagNodeFailure::terminal("crop_failed", error))?;
    for crop in &mut crops.crops {
        crop.source_width = image.width;
        crop.source_height = image.height;
        crop.crop_width = ((crop.rect.width() * image.width as f32).round() as u32).max(1);
        crop.crop_height = ((crop.rect.height() * image.height as f32).round() as u32).max(1);
        crop.mime_type = Some(image.mime_type.clone());
        let material = format!(
            "{}:{}:{}:{}:{}:{}",
            image.blob_ref,
            crop.parent.artifact_id,
            crop.parent.item_id.as_deref().unwrap_or_default(),
            crop.rect.x(),
            crop.rect.y(),
            padding
        );
        crop.cache_key = Some(format!("{:x}", Sha256::digest(material.as_bytes())));
    }
    crops
        .validate()
        .map_err(|error| DagNodeFailure::terminal("crop_failed", error))?;
    Ok(output(PipelineArtifact::CropSet(crops)))
}

fn run_detections_to_box_prompts(
    context: &DagNodeContext<'_>,
) -> Result<DagNodeOutput, DagNodeFailure> {
    let detections = one_detection_set(context)?;
    let padding = number_parameter(context, "padding", 0.0)? as f32;
    let prompts = BoxPromptSetArtifact::from_detections(
        output_reference(context, "prompts", ArtifactKind::BoxPromptSet)?,
        detections,
        padding,
    )
    .map_err(|error| DagNodeFailure::terminal("box_prompt_conversion_failed", error))?;
    Ok(DagNodeOutput {
        metadata: BTreeMap::from([
            (
                "conversion".to_owned(),
                serde_json::json!(CORE_DETECTIONS_TO_BOX_PROMPTS),
            ),
            (
                "source_detection_count".to_owned(),
                serde_json::json!(detections.detections.len()),
            ),
            (
                "prompt_count".to_owned(),
                serde_json::json!(prompts.prompts.len()),
            ),
        ]),
        pipeline_artifacts: vec![PipelineArtifact::BoxPromptSet(prompts)],
        ..DagNodeOutput::default()
    })
}

fn run_mask_to_bbox(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let masks = one_mask_set(context)?;
    let prompts = one_box_prompt_set(context)?;
    masks
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_mask_set", error))?;
    prompts
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_box_prompt_set", error))?;
    if masks.image_id != prompts.image_id
        || masks.source_prompts.artifact_id != prompts.reference.artifact_id
    {
        return Err(DagNodeFailure::terminal(
            "mask_prompt_scope_mismatch",
            "MaskSet and BoxPromptSet must belong to the same image and lineage",
        ));
    }
    let reference = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    let mut detections = Vec::with_capacity(masks.masks.len());
    for mask in &masks.masks {
        let prompt_id = mask.prompt.item_id.as_deref().ok_or_else(|| {
            DagNodeFailure::terminal("mask_prompt_missing", "Mask does not identify its prompt")
        })?;
        let prompt = prompts
            .prompts
            .iter()
            .find(|prompt| prompt.id == prompt_id)
            .ok_or_else(|| {
                DagNodeFailure::terminal(
                    "mask_prompt_missing",
                    format!("Mask references unknown Box Prompt {prompt_id:?}"),
                )
            })?;
        let original = prompt
            .attributes
            .get("source_detection")
            .cloned()
            .and_then(|value| serde_json::from_value::<Detection>(value).ok())
            .ok_or_else(|| {
                DagNodeFailure::terminal(
                    "source_detection_missing",
                    "Box Prompt does not preserve its source Detection",
                )
            })?;
        let bbox = mask_tight_bbox(&mask.mask)
            .map_err(|error| DagNodeFailure::terminal("mask_to_bbox_failed", error))?;
        let score = if mask.score.value.is_some() {
            mask.score
        } else {
            original.score
        };
        let refined_detection_id = format!("refined:{}", original.detection_id);
        let mut refined = Detection::from_source(
            refined_detection_id.clone(),
            original.query_id.clone(),
            original.model_label.clone(),
            original.project_label.clone(),
            bbox,
            score,
            annotagent_core::DetectionSource {
                model_id: masks.model_binding.clone(),
                capability: VisionCapability::PromptedSegmentation,
                artifact_id: masks.reference.artifact_id.clone(),
            },
        )
        .map_err(|error| DagNodeFailure::terminal("mask_to_bbox_failed", error))?;
        refined.evidence.extend(original.evidence.clone());
        refined.attributes.clone_from(&original.attributes);
        let trace = GeometryRefinementTrace {
            schema_version: GEOMETRY_REFINEMENT_TRACE_SCHEMA_VERSION,
            method: "mask_to_bbox".to_owned(),
            source_detection: prompt.subject.clone(),
            box_prompt: mask.prompt.clone(),
            mask: masks.reference.item(&mask.mask_id),
            refined_detection: reference.item(&refined_detection_id),
            original_bbox: original.bbox,
            refined_bbox: bbox,
            mask_score: mask.score,
        };
        trace
            .validate()
            .map_err(|error| DagNodeFailure::terminal("mask_to_bbox_failed", error))?;
        refined.attributes.insert(
            "geometry_refinement".to_owned(),
            serde_json::to_value(trace).map_err(|error| {
                DagNodeFailure::terminal("mask_to_bbox_failed", error.to_string())
            })?,
        );
        detections.push(refined);
    }
    let refined = DetectionSetArtifact {
        schema_version: annotagent_core::DETECTION_ARTIFACT_SCHEMA_VERSION,
        reference,
        image_id: masks.image_id,
        model_binding: masks.model_binding.clone(),
        validation_state: masks.validation_state,
        detections,
        metadata: BTreeMap::from([
            (
                "conversion".to_owned(),
                serde_json::json!(CORE_MASK_TO_BBOX),
            ),
            (
                "source_mask_set".to_owned(),
                serde_json::json!(masks.reference.artifact_id),
            ),
            (
                "source_prompt_set".to_owned(),
                serde_json::json!(prompts.reference.artifact_id),
            ),
        ]),
    };
    refined
        .validate()
        .map_err(|error| DagNodeFailure::terminal("mask_to_bbox_failed", error))?;
    Ok(output(PipelineArtifact::DetectionSet(refined)))
}

fn run_geometry_quality_evaluation(
    context: &DagNodeContext<'_>,
) -> Result<DagNodeOutput, DagNodeFailure> {
    let mut detections = one_detection_set(context)?.clone();
    detections
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_detection_set", error))?;
    let thresholds: GeometryRefinementThresholds = serde_json::from_value(
        serde_json::to_value(&context.node.parameters).map_err(|error| {
            DagNodeFailure::terminal("invalid_geometry_thresholds", error.to_string())
        })?,
    )
    .map_err(|error| DagNodeFailure::terminal("invalid_geometry_thresholds", error.to_string()))?;
    thresholds
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_geometry_thresholds", error))?;
    let mut evaluations = Vec::with_capacity(detections.detections.len());
    let mut unstable_count = 0_u64;
    for detection in &mut detections.detections {
        let trace = detection
            .attributes
            .get("geometry_refinement")
            .cloned()
            .ok_or_else(|| {
                DagNodeFailure::terminal(
                    "geometry_refinement_missing",
                    format!(
                        "Detection {:?} has no prompted-refinement trace",
                        detection.detection_id
                    ),
                )
            })
            .and_then(|value| {
                serde_json::from_value::<GeometryRefinementTrace>(value).map_err(|error| {
                    DagNodeFailure::terminal(
                        "geometry_refinement_invalid",
                        format!(
                            "Detection {:?} has invalid prompted-refinement lineage: {error}",
                            detection.detection_id
                        ),
                    )
                })
            })?;
        if trace.refined_detection.artifact_id != detections.reference.artifact_id
            || trace.refined_detection.item_id.as_deref() != Some(&detection.detection_id)
            || trace.refined_bbox != detection.bbox
        {
            return Err(DagNodeFailure::terminal(
                "geometry_refinement_lineage_mismatch",
                format!(
                    "Detection {:?} does not match its prompted-refinement output reference and geometry",
                    detection.detection_id
                ),
            ));
        }
        let evaluation = evaluate_geometry_refinement(trace, thresholds)
            .map_err(|error| DagNodeFailure::terminal("geometry_evaluation_failed", error))?;
        if !evaluation.stable {
            unstable_count = unstable_count.saturating_add(1);
        }
        detection.attributes.insert(
            "geometry_quality_evaluation".to_owned(),
            serde_json::to_value(&evaluation).map_err(|error| {
                DagNodeFailure::terminal("geometry_evaluation_failed", error.to_string())
            })?,
        );
        evaluations.push(evaluation);
    }
    detections.reference = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    detections.validation_state = if unstable_count == 0 {
        ArtifactValidationState::Unvalidated
    } else {
        ArtifactValidationState::NeedsReview
    };
    detections.metadata.insert(
        "geometry_evaluations".to_owned(),
        serde_json::to_value(&evaluations).map_err(|error| {
            DagNodeFailure::terminal("geometry_evaluation_failed", error.to_string())
        })?,
    );
    detections
        .validate()
        .map_err(|error| DagNodeFailure::terminal("geometry_evaluation_failed", error))?;
    Ok(DagNodeOutput {
        pipeline_artifacts: vec![PipelineArtifact::DetectionSet(detections)],
        metadata: BTreeMap::from([
            (
                "evaluated_detection_count".to_owned(),
                serde_json::json!(evaluations.len()),
            ),
            (
                "unstable_detection_count".to_owned(),
                serde_json::json!(unstable_count),
            ),
            ("semantic_score_used".to_owned(), serde_json::json!(false)),
        ]),
        ..DagNodeOutput::default()
    })
}

fn run_geometry_decision(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let mut detections = one_detection_set(context)?.clone();
    detections
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_detection_set", error))?;
    let mut missing_evaluation_count = 0_u64;
    let mut unstable_count = 0_u64;
    for detection in &detections.detections {
        let Some(value) = detection.attributes.get("geometry_quality_evaluation") else {
            missing_evaluation_count = missing_evaluation_count.saturating_add(1);
            continue;
        };
        match serde_json::from_value::<annotagent_core::GeometryRefinementEvaluation>(value.clone())
        {
            Ok(evaluation)
                if evaluation.validate().is_ok()
                    && evaluation.stable
                    && evaluation.trace.refined_detection.item_id.as_deref()
                        == Some(&detection.detection_id)
                    && evaluation.trace.refined_bbox == detection.bbox => {}
            Ok(_) | Err(_) => unstable_count = unstable_count.saturating_add(1),
        }
    }
    let accept =
        !detections.detections.is_empty() && missing_evaluation_count == 0 && unstable_count == 0;
    detections.reference = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    detections.validation_state = if accept {
        ArtifactValidationState::Valid
    } else {
        ArtifactValidationState::NeedsReview
    };
    detections.metadata.insert(
        "geometry_decision".to_owned(),
        serde_json::json!({
            "route": if accept { "accept" } else { "review" },
            "evaluated_detection_count": detections.detections.len(),
            "missing_evaluation_count": missing_evaluation_count,
            "unstable_detection_count": unstable_count,
            "semantic_score_used": false,
        }),
    );
    detections
        .validate()
        .map_err(|error| DagNodeFailure::terminal("geometry_decision_failed", error))?;
    Ok(DagNodeOutput {
        pipeline_artifacts: vec![PipelineArtifact::DetectionSet(detections)],
        route: Some(if accept { "accept" } else { "review" }.to_owned()),
        metadata: BTreeMap::from([
            (
                "missing_evaluation_count".to_owned(),
                serde_json::json!(missing_evaluation_count),
            ),
            (
                "unstable_detection_count".to_owned(),
                serde_json::json!(unstable_count),
            ),
            ("semantic_score_used".to_owned(), serde_json::json!(false)),
        ]),
        ..DagNodeOutput::default()
    })
}

fn run_mask_to_polygon(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let masks = one_mask_set(context)?;
    masks
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_mask_set", error))?;
    let reference = output_reference(context, "polygons", ArtifactKind::PolygonSet)?;
    let polygons = masks
        .masks
        .iter()
        .map(|mask| {
            let MaskEncoding::Polygon { rings } = &mask.mask else {
                return Err(DagNodeFailure::terminal(
                    "mask_polygon_encoding_required",
                    "Mask to Polygon currently requires a polygon-encoded Mask Artifact",
                ));
            };
            Ok(PolygonArtifactItem {
                polygon_id: format!("polygon:{}", mask.mask_id),
                parent: masks.reference.item(&mask.mask_id),
                rings: rings.clone(),
                score: mask.score,
            })
        })
        .collect::<Result<Vec<_>, DagNodeFailure>>()?;
    let polygons = PolygonSetArtifact {
        reference,
        image_id: masks.image_id,
        source_masks: masks.reference.clone(),
        polygons,
    };
    polygons
        .validate()
        .map_err(|error| DagNodeFailure::terminal("mask_to_polygon_failed", error))?;
    Ok(output(PipelineArtifact::PolygonSet(polygons)))
}

fn run_filter(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let source = one_detection_set(context)?;
    let minimum = number_parameter(context, "minimum_confidence", 0.0)? as f32;
    let class_ids = string_list_parameter(context, "class_ids")?;
    let labels = string_list_parameter(context, "labels")?;
    let mut filtered = source.clone();
    filtered.reference = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    filtered.detections.retain(|detection| {
        detection
            .score
            .comparable_confidence()
            .is_none_or(|confidence| confidence >= minimum)
            && (class_ids.is_empty()
                || detection
                    .model_label
                    .as_ref()
                    .is_some_and(|model_label| class_ids.contains(model_label)))
            && (labels.is_empty()
                || detection
                    .project_label
                    .as_ref()
                    .is_some_and(|label| labels.iter().any(|item| item == label.as_str())))
    });
    filtered
        .validate()
        .map_err(|error| DagNodeFailure::terminal("filter_output_invalid", error))?;
    Ok(output(PipelineArtifact::DetectionSet(filtered)))
}

fn run_map_label(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let source = one_detection_set(context)?;
    let mapping = object_parameter(context, "class_mapping")?;
    let mut mapped = source.clone();
    mapped.reference = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    for detection in &mut mapped.detections {
        if let Some(label) = mapping
            .get(detection.model_label.as_deref().unwrap_or_default())
            .and_then(Value::as_str)
            .filter(|label| !label.trim().is_empty())
        {
            detection.project_label = Some(LabelId::from(label));
        }
    }
    mapped
        .validate()
        .map_err(|error| DagNodeFailure::terminal("map_label_output_invalid", error))?;
    Ok(output(PipelineArtifact::DetectionSet(mapped)))
}

fn run_select_and_map(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let source = one_detection_set(context)?;
    let minimum = number_parameter(context, "minimum_confidence", 0.0)? as f32;
    if !(0.0..=1.0).contains(&minimum) {
        return Err(DagNodeFailure::terminal(
            "invalid_minimum_confidence",
            "minimum_confidence must be within [0,1]",
        ));
    }
    let class_ids = string_list_parameter(context, "class_ids")?;
    let labels = string_list_parameter(context, "labels")?;
    let queries = string_list_parameter(context, "queries")?;
    let mapping = object_parameter(context, "class_mapping")?;
    let drop_unknown = boolean_parameter(context, "drop_unknown_labels", false)?;
    let mut selected = source.clone();
    selected.reference = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    selected.detections.retain_mut(|detection| {
        if let Some(label) = mapping
            .get(detection.model_label.as_deref().unwrap_or_default())
            .and_then(Value::as_str)
            .filter(|label| !label.trim().is_empty())
        {
            detection.project_label = Some(LabelId::from(label));
        }
        detection
            .score
            .comparable_confidence()
            .is_none_or(|confidence| confidence >= minimum)
            && (class_ids.is_empty()
                || detection
                    .model_label
                    .as_ref()
                    .is_some_and(|value| class_ids.contains(value)))
            && (labels.is_empty()
                || detection
                    .project_label
                    .as_ref()
                    .is_some_and(|value| labels.iter().any(|label| label == value.as_str())))
            && (queries.is_empty()
                || detection
                    .query_id
                    .as_ref()
                    .is_some_and(|query| queries.contains(query)))
            && (!drop_unknown || detection.project_label.is_some())
    });
    selected
        .validate()
        .map_err(|error| DagNodeFailure::terminal("select_and_map_output_invalid", error))?;
    Ok(output(PipelineArtifact::DetectionSet(selected)))
}

fn run_project_coordinates(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let images = context
        .input_pipeline_artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PipelineArtifact::Image(image) => Some(image),
            _ => None,
        })
        .collect::<Vec<_>>();
    if images.is_empty() {
        return Err(DagNodeFailure::terminal(
            "coordinate_source_missing",
            "Coordinate Projection requires the source Crop or Tile Image Artifact",
        ));
    }
    let sets = detection_sets(context)?;
    let output = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    let mut projected = Vec::new();
    for (index, source) in sets.into_iter().enumerate() {
        let source_artifact_id = source
            .metadata
            .get("source_image_artifact_id")
            .and_then(Value::as_str);
        let source_item_id = source
            .metadata
            .get("source_image_item_id")
            .and_then(Value::as_str);
        let image = if images.len() == 1 {
            images[0]
        } else {
            images
                .iter()
                .copied()
                .find(|image| {
                    source_artifact_id == Some(image.reference.artifact_id.as_str())
                        && source_item_id == image.reference.item_id.as_deref()
                })
                .ok_or_else(|| {
                    DagNodeFailure::terminal(
                        "coordinate_lineage_ambiguous",
                        "Each local DetectionSet must identify source_image_artifact_id and source_image_item_id",
                    )
                })?
        };
        let region = image.root_region.ok_or_else(|| {
            DagNodeFailure::terminal(
                "coordinate_mapping_missing",
                "Source Image Artifact has no Crop/Tile root coordinate mapping",
            )
        })?;
        let mut next = source.clone();
        next.reference = ArtifactRef {
            artifact_id: format!("{}:set-{index}", output.artifact_id),
            source_node: output.source_node.clone(),
            port: output.port.clone(),
            artifact_type: output.artifact_type,
            item_id: None,
        };
        for detection in &mut next.detections {
            let local = detection.bbox;
            detection.bbox = annotagent_core::NormalizedRect::new(
                region.x() + local.x() * region.width(),
                region.y() + local.y() * region.height(),
                local.width() * region.width(),
                local.height() * region.height(),
            )
            .map_err(|error| {
                DagNodeFailure::terminal("coordinate_projection_failed", error.to_string())
            })?;
        }
        next.metadata.insert(
            "projected_from_image_artifact_id".to_owned(),
            serde_json::json!(image.reference.artifact_id),
        );
        next.metadata.insert(
            "coordinate_space".to_owned(),
            serde_json::json!("root_image"),
        );
        next.validate()
            .map_err(|error| DagNodeFailure::terminal("coordinate_projection_failed", error))?;
        projected.push(PipelineArtifact::DetectionSet(next));
    }
    Ok(DagNodeOutput {
        pipeline_artifacts: projected,
        ..DagNodeOutput::default()
    })
}

fn run_attach_result(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let detections = one_detection_set(context)?;
    let classifications = one_classification_set(context)?;
    let task_id = context
        .node
        .parameters
        .get("task_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            DagNodeFailure::terminal("missing_task_id", "Attach Result requires task_id")
        })?;
    let mapping = object_parameter(context, "class_mapping")?
        .iter()
        .filter_map(|(source, target)| {
            target
                .as_str()
                .map(|target| (LabelId::from(source.as_str()), LabelId::from(target)))
        })
        .collect::<BTreeMap<_, _>>();
    let candidates = AnnotationCandidateSet::fan_in(
        output_reference(context, "candidates", ArtifactKind::AnnotationCandidateSet)?,
        detections,
        classifications,
        &TaskId::from(task_id),
        &mapping,
    )
    .map_err(|error| DagNodeFailure::terminal("attach_result_failed", error))?;
    Ok(output(PipelineArtifact::AnnotationCandidateSet(candidates)))
}

fn run_attach_attribute(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let source = one_candidate_set(context)?;
    let name = context
        .node
        .parameters
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            DagNodeFailure::terminal("missing_attribute_name", "Attach Attribute requires name")
        })?;
    let value = context
        .node
        .parameters
        .get("value")
        .cloned()
        .ok_or_else(|| {
            DagNodeFailure::terminal("missing_attribute_value", "Attach Attribute requires value")
        })?;
    let mut candidates = source.clone();
    candidates.reference =
        output_reference(context, "candidates", ArtifactKind::AnnotationCandidateSet)?;
    for candidate in &mut candidates.candidates {
        candidate.attributes.insert(name.to_owned(), value.clone());
    }
    candidates
        .validate()
        .map_err(|error| DagNodeFailure::terminal("attach_attribute_output_invalid", error))?;
    Ok(output(PipelineArtifact::AnnotationCandidateSet(candidates)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MatchKind {
    Agreement,
    GeometryConflict,
    LabelConflict,
}

#[derive(Debug, Clone, Copy)]
struct MatchPair {
    left: usize,
    right: usize,
    iou: f32,
    kind: MatchKind,
}

fn run_candidate_match(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let sets = detection_sets(context)?;
    if sets.len() != 2 {
        return Err(DagNodeFailure::terminal(
            "invalid_match_input",
            "Match Detection Sets requires exactly two DetectionSet Artifacts",
        ));
    }
    for set in &sets {
        set.validate()
            .map_err(|error| DagNodeFailure::terminal("invalid_detection_set", error))?;
    }
    if sets[0].image_id != sets[1].image_id {
        return Err(DagNodeFailure::terminal(
            "image_scope_mismatch",
            "DetectionSets must belong to the same image",
        ));
    }
    let method = string_parameter(context, "method", "iou")?;
    if method != "iou" {
        return Err(DagNodeFailure::terminal(
            "unsupported_match_method",
            "Match Detection Sets currently supports only method=iou",
        ));
    }
    let minimum_iou = number_parameter(context, "minimum_iou", 0.5)? as f32;
    if !(0.0..=1.0).contains(&minimum_iou) {
        return Err(DagNodeFailure::terminal(
            "invalid_minimum_iou",
            "minimum_iou must be within [0,1]",
        ));
    }
    let preserve_unmatched = boolean_parameter(context, "preserve_unmatched", true)?;
    let artifact = match_detection_sets(
        output_reference(context, "candidates", ArtifactKind::CandidateClusterSet)?,
        sets[0],
        sets[1],
        minimum_iou,
        preserve_unmatched,
    )
    .map_err(|error| DagNodeFailure::terminal("candidate_match_output_invalid", error))?;
    let mut metadata = propagated_evidence_metadata(context)?;
    metadata.insert(
        "source_summaries".to_owned(),
        serde_json::json!(
            sets.iter()
                .map(|set| serde_json::json!({
                    "artifact_id": set.reference.artifact_id,
                    "model_id": set.model_binding,
                    "detection_count": set.detections.len(),
                }))
                .collect::<Vec<_>>()
        ),
    );
    metadata.insert(
        "candidate_match".to_owned(),
        serde_json::json!({
            "method": method,
            "minimum_iou": minimum_iou,
            "preserve_unmatched": preserve_unmatched,
            "candidate_count": artifact.candidates.len(),
        }),
    );
    Ok(DagNodeOutput {
        pipeline_artifacts: vec![PipelineArtifact::CandidateClusterSet(artifact)],
        metadata,
        ..DagNodeOutput::default()
    })
}

/// Combines two independent detector outputs while preserving every source score and provenance.
/// The representative geometry is stable (left source first); this function never averages scores.
pub fn match_detection_sets(
    reference: ArtifactRef,
    left: &DetectionSetArtifact,
    right: &DetectionSetArtifact,
    minimum_iou: f32,
    preserve_unmatched: bool,
) -> Result<CandidateClusterSetArtifact, String> {
    left.validate()?;
    right.validate()?;
    if left.image_id != right.image_id {
        return Err("DetectionSets must belong to the same image".to_owned());
    }
    if !(0.0..=1.0).contains(&minimum_iou) {
        return Err("minimum_iou must be within [0,1]".to_owned());
    }
    let sets = [left, right];
    let left_labels = project_labels(left)?;
    let right_labels = project_labels(right)?;
    let mut left_matched = vec![false; sets[0].detections.len()];
    let mut right_matched = vec![false; sets[1].detections.len()];
    let mut selected = Vec::new();

    select_pairs(
        pair_candidates(
            sets[0],
            sets[1],
            &left_labels,
            &right_labels,
            |same, iou| same && iou >= minimum_iou,
            MatchKind::Agreement,
        ),
        &mut left_matched,
        &mut right_matched,
        &mut selected,
    );
    select_pairs(
        pair_candidates(
            sets[0],
            sets[1],
            &left_labels,
            &right_labels,
            |same, iou| !same && iou > 0.0,
            MatchKind::LabelConflict,
        ),
        &mut left_matched,
        &mut right_matched,
        &mut selected,
    );
    select_pairs(
        pair_candidates(
            sets[0],
            sets[1],
            &left_labels,
            &right_labels,
            |same, iou| same && iou > 0.0 && iou < minimum_iou,
            MatchKind::GeometryConflict,
        ),
        &mut left_matched,
        &mut right_matched,
        &mut selected,
    );

    selected.sort_by_key(|pair| (pair.left, pair.right, pair.kind));
    let mut pending = selected
        .into_iter()
        .map(|pair| {
            let left = &sets[0].detections[pair.left];
            let right = &sets[1].detections[pair.right];
            let mut members = detection_evidence(sets[0], left);
            members.extend(detection_evidence(sets[1], right));
            let agreement = match pair.kind {
                MatchKind::Agreement => CandidateAgreement::MultiSourceAgreement {
                    minimum_iou: pair.iou,
                    mean_iou: pair.iou,
                },
                MatchKind::GeometryConflict => CandidateAgreement::GeometryConflict,
                MatchKind::LabelConflict => CandidateAgreement::LabelConflict,
            };
            (
                pair.left,
                pair.right,
                left_labels[pair.left].clone(),
                left.bbox,
                members,
                agreement,
            )
        })
        .collect::<Vec<_>>();

    if preserve_unmatched {
        pending.extend(
            sets[0]
                .detections
                .iter()
                .enumerate()
                .filter(|(index, _)| !left_matched[*index])
                .map(|(index, detection)| {
                    (
                        index,
                        usize::MAX,
                        left_labels[index].clone(),
                        detection.bbox,
                        detection_evidence(sets[0], detection),
                        CandidateAgreement::SingleSource,
                    )
                }),
        );
        pending.extend(
            sets[1]
                .detections
                .iter()
                .enumerate()
                .filter(|(index, _)| !right_matched[*index])
                .map(|(index, detection)| {
                    (
                        usize::MAX,
                        index,
                        right_labels[index].clone(),
                        detection.bbox,
                        detection_evidence(sets[1], detection),
                        CandidateAgreement::SingleSource,
                    )
                }),
        );
    }
    pending.sort_by_key(|(left, right, ..)| (*left, *right));
    let candidates = pending
        .into_iter()
        .enumerate()
        .map(
            |(index, (_, _, target_label, representative_bbox, members, agreement))| {
                CandidateCluster {
                    id: format!("cluster-{index:04}"),
                    target_label,
                    representative_bbox,
                    members,
                    agreement,
                }
            },
        )
        .collect::<Vec<_>>();
    let artifact = CandidateClusterSetArtifact {
        reference,
        image_id: sets[0].image_id,
        source_detection_sets: sets.iter().map(|set| set.reference.clone()).collect(),
        validation_state: ArtifactValidationState::Unvalidated,
        candidates,
    };
    artifact.validate()?;
    Ok(artifact)
}

/// Projects a single detector output into evidence clusters without inventing a second source.
pub fn cluster_single_detection_set(
    reference: ArtifactRef,
    set: &DetectionSetArtifact,
) -> Result<CandidateClusterSetArtifact, String> {
    set.validate()?;
    let labels = project_labels(set)?;
    let artifact = CandidateClusterSetArtifact {
        reference,
        image_id: set.image_id,
        source_detection_sets: vec![set.reference.clone()],
        validation_state: ArtifactValidationState::Unvalidated,
        candidates: set
            .detections
            .iter()
            .zip(labels)
            .enumerate()
            .map(|(index, (detection, target_label))| CandidateCluster {
                id: format!("cluster-{index:04}"),
                target_label,
                representative_bbox: detection.bbox,
                members: detection_evidence(set, detection),
                agreement: CandidateAgreement::SingleSource,
            })
            .collect(),
    };
    artifact.validate()?;
    Ok(artifact)
}

fn project_labels(set: &DetectionSetArtifact) -> Result<Vec<LabelId>, String> {
    set.detections
        .iter()
        .map(|detection| {
            detection.project_label.clone().ok_or_else(|| {
                "Match Detection Sets requires every Detection to carry a Project Label".to_owned()
            })
        })
        .collect()
}

fn pair_candidates(
    left: &DetectionSetArtifact,
    right: &DetectionSetArtifact,
    left_labels: &[LabelId],
    right_labels: &[LabelId],
    include: impl Fn(bool, f32) -> bool + Copy,
    kind: MatchKind,
) -> Vec<MatchPair> {
    let mut pairs =
        left.detections
            .iter()
            .enumerate()
            .flat_map(|(left_index, left_detection)| {
                right.detections.iter().enumerate().filter_map(
                    move |(right_index, right_detection)| {
                        let iou = rect_iou(left_detection.bbox, right_detection.bbox);
                        include(left_labels[left_index] == right_labels[right_index], iou)
                            .then_some(MatchPair {
                                left: left_index,
                                right: right_index,
                                iou,
                                kind,
                            })
                    },
                )
            })
            .collect::<Vec<_>>();
    pairs.sort_by(|left, right| {
        right
            .iou
            .total_cmp(&left.iou)
            .then_with(|| left.left.cmp(&right.left))
            .then_with(|| left.right.cmp(&right.right))
    });
    pairs
}

fn select_pairs(
    pairs: Vec<MatchPair>,
    left_matched: &mut [bool],
    right_matched: &mut [bool],
    selected: &mut Vec<MatchPair>,
) {
    for pair in pairs {
        if left_matched[pair.left] || right_matched[pair.right] {
            continue;
        }
        left_matched[pair.left] = true;
        right_matched[pair.right] = true;
        selected.push(pair);
    }
}

fn rect_iou(left: annotagent_core::NormalizedRect, right: annotagent_core::NormalizedRect) -> f32 {
    let intersection = left.intersection_area(right);
    let union = left.area() + right.area() - intersection;
    if union <= 0.0 {
        0.0
    } else {
        (intersection / union).clamp(0.0, 1.0)
    }
}

fn detection_evidence(set: &DetectionSetArtifact, detection: &Detection) -> Vec<DetectionEvidence> {
    let mut evidence = detection.evidence.clone();
    for member in &mut evidence {
        member
            .source_artifact_id
            .clone_from(&set.reference.artifact_id);
        if member.project_label.is_none() {
            member.project_label.clone_from(&detection.project_label);
        }
        if member.source_model_id == detection.source_model_id {
            member.source_capability = detection.source_capability;
        }
    }
    evidence
}

fn run_evidence_gate(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let mut artifact = one_candidate_cluster_set(context)?.clone();
    artifact
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_candidate_clusters", error))?;
    let config: EvidenceGateConfig =
        serde_json::from_value(serde_json::to_value(&context.node.parameters).map_err(
            |error| DagNodeFailure::terminal("invalid_evidence_config", error.to_string()),
        )?)
        .map_err(|error| DagNodeFailure::terminal("invalid_evidence_config", error.to_string()))?;
    config
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_evidence_config", error))?;
    let input = EvidenceGateInput {
        candidates: artifact.candidates.clone(),
        validation_issues: collected_validation_issues(context)?,
        correction_risk: collected_correction_risk(context)?,
    };
    input
        .validate()
        .map_err(|error| DagNodeFailure::terminal("invalid_evidence_input", error))?;
    let source_summaries = collected_source_summaries(context);
    let report = decide_evidence(&input, &config, &source_summaries);
    artifact.validation_state = match report.decision {
        EvidenceGateDecision::Accept => ArtifactValidationState::Valid,
        EvidenceGateDecision::Fallback => ArtifactValidationState::Unvalidated,
        EvidenceGateDecision::Review => ArtifactValidationState::NeedsReview,
        EvidenceGateDecision::Reject => ArtifactValidationState::Invalid,
    };
    let route = report.decision.route().to_owned();
    Ok(DagNodeOutput {
        pipeline_artifacts: vec![PipelineArtifact::CandidateClusterSet(artifact)],
        route: Some(route),
        metadata: BTreeMap::from([
            (
                "evidence_gate".to_owned(),
                serde_json::to_value(&report).unwrap_or_else(|_| serde_json::json!({})),
            ),
            (
                "validation_issues".to_owned(),
                serde_json::to_value(&input.validation_issues)
                    .unwrap_or_else(|_| serde_json::json!([])),
            ),
            (
                "correction_risk".to_owned(),
                serde_json::to_value(&input.correction_risk).unwrap_or(serde_json::Value::Null),
            ),
        ]),
        ..DagNodeOutput::default()
    })
}

#[derive(Debug, Clone)]
struct EvidenceSourceSummary {
    model_id: String,
    detection_count: usize,
}

fn propagated_evidence_metadata(
    context: &DagNodeContext<'_>,
) -> Result<BTreeMap<String, serde_json::Value>, DagNodeFailure> {
    let issues = collected_validation_issues(context)?;
    let risk = collected_correction_risk(context)?;
    let mut metadata = BTreeMap::new();
    if !issues.is_empty() {
        metadata.insert(
            "validation_issues".to_owned(),
            serde_json::to_value(issues).unwrap_or_else(|_| serde_json::json!([])),
        );
    }
    if risk.is_some() {
        metadata.insert(
            "correction_risk".to_owned(),
            serde_json::to_value(risk).unwrap_or(serde_json::Value::Null),
        );
    }
    Ok(metadata)
}

fn collected_validation_issues(
    context: &DagNodeContext<'_>,
) -> Result<Vec<ValidationIssue>, DagNodeFailure> {
    let values = context
        .input_metadata
        .values()
        .filter_map(|metadata| metadata.get("validation_issues"))
        .chain(context.node.parameters.get("validation_issues"));
    let mut issues = Vec::new();
    for value in values {
        let next =
            serde_json::from_value::<Vec<ValidationIssue>>(value.clone()).map_err(|error| {
                DagNodeFailure::terminal(
                    "invalid_validation_evidence",
                    format!("validation_issues metadata is invalid: {error}"),
                )
            })?;
        issues.extend(next);
    }
    let mut identities = std::collections::BTreeSet::new();
    issues.retain(|issue| identities.insert((issue.code.clone(), issue.message.clone())));
    Ok(issues)
}

fn collected_correction_risk(
    context: &DagNodeContext<'_>,
) -> Result<Option<CorrectionRisk>, DagNodeFailure> {
    let value = context.node.parameters.get("correction_risk").or_else(|| {
        context
            .input_metadata
            .values()
            .find_map(|metadata| metadata.get("correction_risk"))
    });
    value
        .map(|value| {
            if let Some(score) = value.as_f64() {
                Ok(CorrectionRisk {
                    score: score as f32,
                    reasons: Vec::new(),
                })
            } else {
                serde_json::from_value::<CorrectionRisk>(value.clone()).map_err(|error| {
                    DagNodeFailure::terminal(
                        "invalid_correction_risk",
                        format!("correction_risk metadata is invalid: {error}"),
                    )
                })
            }
        })
        .transpose()
}

fn collected_source_summaries(context: &DagNodeContext<'_>) -> Vec<EvidenceSourceSummary> {
    let summaries = context
        .input_metadata
        .values()
        .filter_map(|metadata| metadata.get("source_summaries"))
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .filter_map(|value| {
            Some(EvidenceSourceSummary {
                model_id: value.get("model_id")?.as_str()?.to_owned(),
                detection_count: value.get("detection_count")?.as_u64()?.try_into().ok()?,
            })
        })
        .collect::<Vec<_>>();
    let mut seen = std::collections::BTreeSet::new();
    summaries
        .into_iter()
        .filter(|summary| seen.insert(summary.model_id.clone()))
        .collect()
}

fn decide_evidence(
    input: &EvidenceGateInput,
    config: &EvidenceGateConfig,
    source_summaries: &[EvidenceSourceSummary],
) -> EvidenceGateReport {
    if let Some(reason) = config
        .reject_when
        .iter()
        .find_map(|rule| reject_reason(input, rule))
    {
        return evidence_report(EvidenceGateDecision::Reject, vec![reason], input);
    }
    if let Some(reason) = config
        .fallback_when
        .iter()
        .find_map(|rule| fallback_reason(input, rule, source_summaries))
    {
        return evidence_report(EvidenceGateDecision::Fallback, vec![reason], input);
    }
    if let Some(candidate) = input.candidates.iter().find(|candidate| {
        matches!(
            candidate.agreement,
            CandidateAgreement::GeometryConflict | CandidateAgreement::LabelConflict
        )
    }) {
        let (code, message) = match candidate.agreement {
            CandidateAgreement::GeometryConflict => (
                "geometry_conflict",
                "Detector boxes overlap but do not meet the configured agreement threshold",
            ),
            CandidateAgreement::LabelConflict => (
                "label_conflict",
                "Detectors assigned different Project Labels to overlapping boxes",
            ),
            CandidateAgreement::SingleSource | CandidateAgreement::MultiSourceAgreement { .. } => {
                unreachable!("candidate was selected as a conflict")
            }
        };
        return evidence_report(
            EvidenceGateDecision::Review,
            vec![candidate_reason(code, message, candidate)],
            input,
        );
    }
    if let Some(reason) = config
        .review_when
        .iter()
        .find_map(|rule| review_reason(input, rule))
    {
        return evidence_report(EvidenceGateDecision::Review, vec![reason], input);
    }
    if !input.candidates.is_empty()
        && !config.accept_when.is_empty()
        && input.candidates.iter().all(|candidate| {
            config.accept_when.iter().any(|rule| {
                accept_rule_matches(candidate, rule, input.validation_issues.is_empty())
            })
        })
    {
        let reasons = input
            .candidates
            .iter()
            .map(accepted_candidate_reason)
            .collect();
        return evidence_report(EvidenceGateDecision::Accept, reasons, input);
    }
    let reason = if input.candidates.is_empty() {
        simple_reason(
            "empty_result",
            "No detection candidates were produced; human review is required",
        )
    } else {
        simple_reason(
            "insufficient_evidence",
            "Candidates did not satisfy an explicit accept rule",
        )
    };
    evidence_report(EvidenceGateDecision::Review, vec![reason], input)
}

/// Evaluates an Evidence Gate outside the fixed Core node, for example inside a bounded Recovery
/// Agent. Source counts are explicit so an empty specialist result remains distinguishable from a
/// detector that was never run.
#[must_use]
pub fn evaluate_detection_evidence(
    input: &EvidenceGateInput,
    config: &EvidenceGateConfig,
    source_counts: &[(String, usize)],
) -> EvidenceGateReport {
    let source_summaries = source_counts
        .iter()
        .map(|(model_id, detection_count)| EvidenceSourceSummary {
            model_id: model_id.clone(),
            detection_count: *detection_count,
        })
        .collect::<Vec<_>>();
    decide_evidence(input, config, &source_summaries)
}

fn reject_reason(
    input: &EvidenceGateInput,
    rule: &EvidenceRejectRule,
) -> Option<EvidenceGateReason> {
    let mut checks = Vec::new();
    if rule.empty_result {
        checks.push(input.candidates.is_empty());
    }
    if !rule.domain_issue_codes.is_empty() {
        checks.push(input.validation_issues.iter().any(|issue| {
            rule.domain_issue_codes
                .iter()
                .any(|code| code == &issue.code)
        }));
    }
    (!checks.is_empty() && checks.into_iter().all(|matched| matched)).then(|| {
        simple_reason(
            "reject_rule_matched",
            "Evidence matched an explicit reject rule",
        )
    })
}

fn fallback_reason(
    input: &EvidenceGateInput,
    rule: &EvidenceFallbackRule,
    source_summaries: &[EvidenceSourceSummary],
) -> Option<EvidenceGateReason> {
    let source = rule.source.as_deref().or_else(|| {
        source_summaries
            .first()
            .map(|summary| summary.model_id.as_str())
    });
    let source_members = source.map(|source| {
        input
            .candidates
            .iter()
            .flat_map(|candidate| &candidate.members)
            .filter(|member| member.source_model_id == source)
            .collect::<Vec<_>>()
    });
    let mut checks = Vec::new();
    if rule.empty_specialist_result {
        let empty = source.map_or(input.candidates.is_empty(), |source| {
            source_summaries
                .iter()
                .find(|summary| summary.model_id == source)
                .is_some_and(|summary| summary.detection_count == 0)
                || source_members.as_ref().is_some_and(Vec::is_empty)
        });
        checks.push(empty);
    }
    if let Some(threshold) = rule.specialist_score_below {
        let comparable = source_members
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|member| member.score.comparable_confidence())
            .collect::<Vec<_>>();
        checks.push(!comparable.is_empty() && comparable.iter().any(|score| *score < threshold));
    }
    if rule.domain_issue {
        checks.push(!input.validation_issues.is_empty());
    }
    if let Some(threshold) = rule.correction_risk_above {
        checks.push(
            input
                .correction_risk
                .as_ref()
                .is_some_and(|risk| risk.score >= threshold),
        );
    }
    if checks.is_empty() || !checks.into_iter().all(|matched| matched) {
        return None;
    }
    let source_ids = source
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let (code, message, metrics) = if rule.empty_specialist_result {
        (
            "empty_source_result",
            format!(
                "{} returned no candidates; fallback requested",
                source.unwrap_or("configured source")
            ),
            BTreeMap::new(),
        )
    } else if let Some(threshold) = rule.specialist_score_below {
        let score = source_members
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|member| member.score.comparable_confidence())
            .fold(1.0_f32, f32::min);
        (
            "source_score_below_threshold",
            format!(
                "{} score {score:.2} is below {threshold:.2}; fallback requested",
                source.unwrap_or("configured source")
            ),
            BTreeMap::from([
                ("score".to_owned(), f64::from(score)),
                ("threshold".to_owned(), f64::from(threshold)),
            ]),
        )
    } else if rule.domain_issue {
        (
            "domain_issue",
            "Domain validation reported an issue; fallback requested".to_owned(),
            BTreeMap::new(),
        )
    } else {
        let risk = input
            .correction_risk
            .as_ref()
            .map_or(0.0, |risk| risk.score);
        (
            "correction_risk",
            format!("Correction risk {risk:.2} requires fallback evidence"),
            BTreeMap::from([("correction_risk".to_owned(), f64::from(risk))]),
        )
    };
    Some(EvidenceGateReason {
        code: code.to_owned(),
        message,
        candidate_id: None,
        source_model_ids: source_ids,
        metrics,
    })
}

fn review_reason(
    input: &EvidenceGateInput,
    rule: &EvidenceReviewRule,
) -> Option<EvidenceGateReason> {
    let mut checks = Vec::new();
    if rule.geometry_conflict {
        checks.push(
            input
                .candidates
                .iter()
                .any(|candidate| candidate.agreement == CandidateAgreement::GeometryConflict),
        );
    }
    if rule.label_conflict {
        checks.push(
            input
                .candidates
                .iter()
                .any(|candidate| candidate.agreement == CandidateAgreement::LabelConflict),
        );
    }
    if rule.open_vocab_only {
        checks.push(
            !input.candidates.is_empty()
                && input.candidates.iter().all(|candidate| {
                    candidate.members.iter().all(|member| {
                        matches!(
                            member.source_capability,
                            VisionCapability::OpenVocabularyDetection
                                | VisionCapability::PhraseGrounding
                        )
                    })
                }),
        );
    }
    if rule.score_missing {
        checks.push(input.candidates.iter().any(|candidate| {
            candidate
                .members
                .iter()
                .any(|member| member.score.comparable_confidence().is_none())
        }));
    }
    if rule.empty_result {
        checks.push(input.candidates.is_empty());
    }
    if let Some(threshold) = rule.correction_risk_above {
        checks.push(
            input
                .correction_risk
                .as_ref()
                .is_some_and(|risk| risk.score >= threshold),
        );
    }
    if checks.is_empty() || !checks.into_iter().all(|matched| matched) {
        return None;
    }
    if rule.score_missing {
        Some(simple_reason(
            "score_not_comparable",
            "Confidence was not provided or is not comparable; human review is required",
        ))
    } else if rule.open_vocab_only {
        Some(simple_reason(
            "open_vocabulary_only",
            "Only open-vocabulary evidence is available; human review is required",
        ))
    } else if rule.empty_result {
        Some(simple_reason(
            "empty_result",
            "No candidates were produced; human review is required",
        ))
    } else if rule.correction_risk_above.is_some() {
        Some(simple_reason(
            "correction_risk",
            "Correction history requires human review",
        ))
    } else {
        Some(simple_reason(
            "evidence_conflict",
            "Detector evidence conflicts; human review is required",
        ))
    }
}

fn accept_rule_matches(
    candidate: &CandidateCluster,
    rule: &EvidenceAcceptRule,
    no_domain_issue: bool,
) -> bool {
    let mut checks = Vec::new();
    if let Some(minimum_sources) = rule.minimum_sources {
        checks.push(
            candidate
                .members
                .iter()
                .map(|member| &member.source_model_id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                >= minimum_sources,
        );
    }
    if let Some(minimum_iou) = rule.minimum_iou {
        checks.push(matches!(
            candidate.agreement,
            CandidateAgreement::MultiSourceAgreement {
                minimum_iou: actual,
                ..
            } if actual >= minimum_iou
        ));
    }
    if let Some(source) = rule.source.as_deref() {
        checks.push(
            candidate
                .members
                .iter()
                .any(|member| member.source_model_id == source),
        );
        if let Some(minimum_score) = rule.minimum_score {
            checks.push(candidate.members.iter().any(|member| {
                member.source_model_id == source
                    && member
                        .score
                        .comparable_confidence()
                        .is_some_and(|score| score >= minimum_score)
            }));
        }
    } else if let Some(minimum_score) = rule.minimum_score {
        checks.push(candidate.members.iter().any(|member| {
            member
                .score
                .comparable_confidence()
                .is_some_and(|score| score >= minimum_score)
        }));
    }
    if rule.no_domain_issue {
        checks.push(no_domain_issue);
    }
    !checks.is_empty() && checks.into_iter().all(|matched| matched)
}

fn accepted_candidate_reason(candidate: &CandidateCluster) -> EvidenceGateReason {
    match candidate.agreement {
        CandidateAgreement::MultiSourceAgreement { minimum_iou, .. } => {
            let source_count = candidate
                .members
                .iter()
                .map(|member| &member.source_model_id)
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            let mut reason = candidate_reason(
                "multi_source_agreement",
                format!("{source_count} detector sources agree at IoU {minimum_iou:.2}"),
                candidate,
            );
            reason
                .metrics
                .insert("minimum_iou".to_owned(), f64::from(minimum_iou));
            reason
        }
        CandidateAgreement::SingleSource => candidate_reason(
            "source_rule_matched",
            "Single-source candidate satisfied an explicit score rule",
            candidate,
        ),
        CandidateAgreement::GeometryConflict | CandidateAgreement::LabelConflict => {
            unreachable!("conflicts are routed to review before acceptance")
        }
    }
}

fn evidence_report(
    decision: EvidenceGateDecision,
    reasons: Vec<EvidenceGateReason>,
    input: &EvidenceGateInput,
) -> EvidenceGateReport {
    EvidenceGateReport {
        decision,
        reasons,
        candidate_count: input.candidates.len(),
        validation_issue_count: input.validation_issues.len(),
    }
}

fn candidate_reason(
    code: impl Into<String>,
    message: impl Into<String>,
    candidate: &CandidateCluster,
) -> EvidenceGateReason {
    EvidenceGateReason {
        code: code.into(),
        message: message.into(),
        candidate_id: Some(candidate.id.clone()),
        source_model_ids: candidate
            .members
            .iter()
            .map(|member| member.source_model_id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect(),
        metrics: BTreeMap::new(),
    }
}

fn simple_reason(code: impl Into<String>, message: impl Into<String>) -> EvidenceGateReason {
    EvidenceGateReason {
        code: code.into(),
        message: message.into(),
        candidate_id: None,
        source_model_ids: Vec::new(),
        metrics: BTreeMap::new(),
    }
}

fn run_confidence_gate(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let threshold = number_parameter(context, "threshold", 0.5)? as f32;
    if !(0.0..=1.0).contains(&threshold) {
        return Err(DagNodeFailure::terminal(
            "invalid_threshold",
            "Confidence Gate threshold must be within [0,1]",
        ));
    }
    let mut artifacts = context.input_pipeline_artifacts.clone();
    let confidence = artifacts
        .iter()
        .flat_map(artifact_confidences)
        .reduce(f32::min);
    let already_requires_review = artifacts.iter().any(|artifact| match artifact {
        PipelineArtifact::DetectionSet(value) => matches!(
            value.validation_state,
            ArtifactValidationState::NeedsReview | ArtifactValidationState::Invalid
        ),
        PipelineArtifact::ClassificationSet(value) => matches!(
            value.validation_state,
            ArtifactValidationState::NeedsReview | ArtifactValidationState::Invalid
        ),
        PipelineArtifact::CandidateClusterSet(value) => matches!(
            value.validation_state,
            ArtifactValidationState::NeedsReview | ArtifactValidationState::Invalid
        ),
        PipelineArtifact::AnnotationCandidateSet(value) => value.candidates.iter().any(|item| {
            matches!(
                item.validation_state,
                Some(ArtifactValidationState::NeedsReview | ArtifactValidationState::Invalid)
            )
        }),
        PipelineArtifact::MaskSet(value) => matches!(
            value.validation_state,
            ArtifactValidationState::NeedsReview | ArtifactValidationState::Invalid
        ),
        PipelineArtifact::SemanticMask(value) => matches!(
            value.validation_state,
            ArtifactValidationState::NeedsReview | ArtifactValidationState::Invalid
        ),
        PipelineArtifact::Image(_)
        | PipelineArtifact::BoxPromptSet(_)
        | PipelineArtifact::PointPromptSet(_)
        | PipelineArtifact::PolygonSet(_)
        | PipelineArtifact::CropSet(_) => false,
    });
    let route = if !already_requires_review
        && confidence.is_some_and(|confidence| confidence >= threshold)
    {
        set_candidate_state(&mut artifacts, ArtifactValidationState::Valid);
        "pass"
    } else {
        set_candidate_state(&mut artifacts, ArtifactValidationState::NeedsReview);
        "review"
    };
    rebind_pipeline_outputs(context, &mut artifacts, "candidates")?;
    Ok(DagNodeOutput {
        pipeline_artifacts: artifacts,
        route: Some(route.to_owned()),
        metadata: BTreeMap::from([
            ("confidence".to_owned(), serde_json::json!(confidence)),
            ("threshold".to_owned(), serde_json::json!(threshold)),
            (
                "score_state".to_owned(),
                serde_json::json!(if confidence.is_some() {
                    "comparable"
                } else {
                    "not_comparable"
                }),
            ),
        ]),
        ..DagNodeOutput::default()
    })
}

fn rebind_pipeline_outputs(
    context: &DagNodeContext<'_>,
    artifacts: &mut [PipelineArtifact],
    preferred_port: &str,
) -> Result<(), DagNodeFailure> {
    let artifact_count = artifacts.len();
    for (index, artifact) in artifacts.iter_mut().enumerate() {
        let mut reference = output_reference(context, preferred_port, artifact.artifact_type())?;
        if artifact_count > 1 {
            reference.artifact_id = format!("{}:{index}", reference.artifact_id);
        }
        *artifact.reference_mut() = reference;
    }
    Ok(())
}

fn run_decision(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let mode = string_parameter(context, "mode", "confidence")?;
    match mode.as_str() {
        "confidence" => run_confidence_gate(context),
        "evidence" => run_evidence_gate(context),
        "domain_policy" => {
            let has_invalid =
                context
                    .input_pipeline_artifacts
                    .iter()
                    .any(|artifact| match artifact {
                        PipelineArtifact::DetectionSet(value) => {
                            value.validation_state == ArtifactValidationState::Invalid
                        }
                        PipelineArtifact::ClassificationSet(value) => {
                            value.validation_state == ArtifactValidationState::Invalid
                        }
                        PipelineArtifact::CandidateClusterSet(value) => {
                            value.validation_state == ArtifactValidationState::Invalid
                        }
                        PipelineArtifact::AnnotationCandidateSet(value) => {
                            value.candidates.iter().any(|candidate| {
                                candidate.validation_state == Some(ArtifactValidationState::Invalid)
                            })
                        }
                        PipelineArtifact::MaskSet(value) => {
                            value.validation_state == ArtifactValidationState::Invalid
                        }
                        PipelineArtifact::SemanticMask(value) => {
                            value.validation_state == ArtifactValidationState::Invalid
                        }
                        PipelineArtifact::Image(_)
                        | PipelineArtifact::BoxPromptSet(_)
                        | PipelineArtifact::PointPromptSet(_)
                        | PipelineArtifact::PolygonSet(_)
                        | PipelineArtifact::CropSet(_) => false,
                    });
            let has_review =
                context
                    .input_pipeline_artifacts
                    .iter()
                    .any(|artifact| match artifact {
                        PipelineArtifact::DetectionSet(value) => {
                            value.validation_state == ArtifactValidationState::NeedsReview
                        }
                        PipelineArtifact::ClassificationSet(value) => {
                            value.validation_state == ArtifactValidationState::NeedsReview
                        }
                        PipelineArtifact::CandidateClusterSet(value) => {
                            value.validation_state == ArtifactValidationState::NeedsReview
                        }
                        PipelineArtifact::AnnotationCandidateSet(value) => {
                            value.candidates.iter().any(|candidate| {
                                candidate.validation_state
                                    == Some(ArtifactValidationState::NeedsReview)
                            })
                        }
                        PipelineArtifact::MaskSet(value) => {
                            value.validation_state == ArtifactValidationState::NeedsReview
                        }
                        PipelineArtifact::SemanticMask(value) => {
                            value.validation_state == ArtifactValidationState::NeedsReview
                        }
                        PipelineArtifact::Image(_)
                        | PipelineArtifact::BoxPromptSet(_)
                        | PipelineArtifact::PointPromptSet(_)
                        | PipelineArtifact::PolygonSet(_)
                        | PipelineArtifact::CropSet(_) => false,
                    });
            Ok(DagNodeOutput {
                pipeline_artifacts: context.input_pipeline_artifacts.clone(),
                route: Some(
                    if has_invalid {
                        "reject"
                    } else if has_review {
                        "review"
                    } else {
                        "accept"
                    }
                    .to_owned(),
                ),
                ..DagNodeOutput::default()
            })
        }
        _ => Err(DagNodeFailure::terminal(
            "invalid_decision_mode",
            "Decision mode must be confidence, evidence, or domain_policy",
        )),
    }
}

fn set_candidate_state(artifacts: &mut [PipelineArtifact], state: ArtifactValidationState) {
    for artifact in artifacts {
        match artifact {
            PipelineArtifact::DetectionSet(detections) => detections.validation_state = state,
            PipelineArtifact::ClassificationSet(classifications) => {
                classifications.validation_state = state;
            }
            PipelineArtifact::CandidateClusterSet(candidates) => {
                candidates.validation_state = state;
            }
            PipelineArtifact::MaskSet(masks) => masks.validation_state = state,
            PipelineArtifact::SemanticMask(mask) => mask.validation_state = state,
            PipelineArtifact::AnnotationCandidateSet(candidates) => {
                for candidate in &mut candidates.candidates {
                    candidate.validation_state = Some(state);
                }
            }
            PipelineArtifact::Image(_)
            | PipelineArtifact::BoxPromptSet(_)
            | PipelineArtifact::PointPromptSet(_)
            | PipelineArtifact::PolygonSet(_)
            | PipelineArtifact::CropSet(_) => {}
        }
    }
}

fn artifact_confidences(artifact: &PipelineArtifact) -> Vec<f32> {
    match artifact {
        PipelineArtifact::DetectionSet(artifact) => artifact
            .detections
            .iter()
            .filter_map(|detection| detection.score.comparable_confidence())
            .collect(),
        PipelineArtifact::ClassificationSet(artifact) => artifact
            .classifications
            .iter()
            .map(|classification| classification.confidence)
            .collect(),
        PipelineArtifact::AnnotationCandidateSet(artifact) => artifact
            .candidates
            .iter()
            .filter_map(|candidate| candidate.confidence)
            .collect(),
        PipelineArtifact::MaskSet(artifact) => artifact
            .masks
            .iter()
            .filter_map(|mask| mask.score.comparable_confidence())
            .collect(),
        PipelineArtifact::SemanticMask(_)
        | PipelineArtifact::Image(_)
        | PipelineArtifact::BoxPromptSet(_)
        | PipelineArtifact::PointPromptSet(_)
        | PipelineArtifact::PolygonSet(_)
        | PipelineArtifact::CandidateClusterSet(_)
        | PipelineArtifact::CropSet(_) => Vec::new(),
    }
}

fn one_image<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a annotagent_core::ImageArtifact, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::Image(value) => Some(value),
            _ => None,
        },
        "Image",
    )
}

fn one_detection_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a DetectionSetArtifact, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::DetectionSet(value) => Some(value),
            _ => None,
        },
        "DetectionSet",
    )
}

fn one_box_prompt_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a BoxPromptSetArtifact, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::BoxPromptSet(value) => Some(value),
            _ => None,
        },
        "BoxPromptSet",
    )
}

fn one_mask_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a MaskSetArtifact, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::MaskSet(value) => Some(value),
            _ => None,
        },
        "MaskSet",
    )
}

fn detection_sets<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<Vec<&'a DetectionSetArtifact>, DagNodeFailure> {
    let sets = context
        .input_pipeline_artifacts
        .iter()
        .filter_map(|artifact| match artifact {
            PipelineArtifact::DetectionSet(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    if sets.is_empty() {
        return Err(DagNodeFailure::terminal(
            "missing_pipeline_input",
            "node requires DetectionSet Artifacts",
        ));
    }
    Ok(sets)
}

fn one_candidate_cluster_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a CandidateClusterSetArtifact, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::CandidateClusterSet(value) => Some(value),
            _ => None,
        },
        "CandidateClusterSet",
    )
}

fn one_classification_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a ClassificationSetArtifact, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::ClassificationSet(value) => Some(value),
            _ => None,
        },
        "ClassificationSet",
    )
}

fn one_candidate_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a AnnotationCandidateSet, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::AnnotationCandidateSet(value) => Some(value),
            _ => None,
        },
        "AnnotationCandidateSet",
    )
}

fn exactly_one<'a, T>(
    context: &'a DagNodeContext<'_>,
    extract: impl Fn(&'a PipelineArtifact) -> Option<&'a T>,
    name: &str,
) -> Result<&'a T, DagNodeFailure> {
    let mut values = context.input_pipeline_artifacts.iter().filter_map(extract);
    let first = values.next().ok_or_else(|| {
        DagNodeFailure::terminal("missing_pipeline_input", format!("node requires {name}"))
    })?;
    if values.next().is_some() {
        return Err(DagNodeFailure::terminal(
            "ambiguous_pipeline_input",
            format!("node received multiple {name} Artifacts"),
        ));
    }
    Ok(first)
}

fn output(artifact: PipelineArtifact) -> DagNodeOutput {
    DagNodeOutput {
        pipeline_artifacts: vec![artifact],
        ..DagNodeOutput::default()
    }
}

fn output_reference(
    context: &DagNodeContext<'_>,
    preferred_port: &str,
    artifact_type: ArtifactKind,
) -> Result<ArtifactRef, DagNodeFailure> {
    let port = context
        .node
        .outputs
        .iter()
        .find(|port| port.id == preferred_port && port.artifact_type == artifact_type)
        .or_else(|| {
            context
                .node
                .outputs
                .iter()
                .find(|port| port.artifact_type == artifact_type)
        })
        .ok_or_else(|| {
            DagNodeFailure::terminal(
                "missing_output_port",
                format!("node does not declare a {artifact_type:?} output"),
            )
        })?;
    let material = serde_json::to_vec(&serde_json::json!({
        "run_id": context.run_id,
        "image_id": context.image_id,
        "node": context.node.id,
        "port": port.id,
        "inputs": context
            .input_pipeline_artifacts
            .iter()
            .map(PipelineArtifact::reference)
            .collect::<Vec<_>>(),
        "parameters": context.node.parameters,
    }))
    .map_err(|error| DagNodeFailure::terminal("artifact_identity_failed", error.to_string()))?;
    Ok(ArtifactRef {
        artifact_id: format!("sha256:{:x}", Sha256::digest(material)),
        source_node: context.node.id.clone(),
        port: port.id.clone(),
        artifact_type,
        item_id: None,
    })
}

fn number_parameter(
    context: &DagNodeContext<'_>,
    name: &str,
    default: f64,
) -> Result<f64, DagNodeFailure> {
    context
        .node
        .parameters
        .get(name)
        .map_or(Ok(default), |value| {
            value.as_f64().ok_or_else(|| {
                DagNodeFailure::terminal(
                    "invalid_node_parameter",
                    format!("parameter {name:?} must be a number"),
                )
            })
        })
}

fn optional_u32_parameter(
    context: &DagNodeContext<'_>,
    name: &str,
) -> Result<Option<u32>, DagNodeFailure> {
    context
        .node
        .parameters
        .get(name)
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    DagNodeFailure::terminal(
                        "invalid_node_parameter",
                        format!("parameter {name:?} must be a positive 32-bit integer"),
                    )
                })
        })
        .transpose()
}

fn optional_u64_parameter(
    context: &DagNodeContext<'_>,
    name: &str,
) -> Result<Option<u64>, DagNodeFailure> {
    context
        .node
        .parameters
        .get(name)
        .map(|value| {
            value.as_u64().filter(|value| *value > 0).ok_or_else(|| {
                DagNodeFailure::terminal(
                    "invalid_node_parameter",
                    format!("parameter {name:?} must be a positive integer"),
                )
            })
        })
        .transpose()
}

fn boolean_parameter(
    context: &DagNodeContext<'_>,
    name: &str,
    default: bool,
) -> Result<bool, DagNodeFailure> {
    context
        .node
        .parameters
        .get(name)
        .map_or(Ok(default), |value| {
            value.as_bool().ok_or_else(|| {
                DagNodeFailure::terminal(
                    "invalid_node_parameter",
                    format!("parameter {name:?} must be a boolean"),
                )
            })
        })
}

fn string_parameter(
    context: &DagNodeContext<'_>,
    name: &str,
    default: &str,
) -> Result<String, DagNodeFailure> {
    context.node.parameters.get(name).map_or_else(
        || Ok(default.to_owned()),
        |value| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .ok_or_else(|| {
                    DagNodeFailure::terminal(
                        "invalid_node_parameter",
                        format!("parameter {name:?} must be a non-empty string"),
                    )
                })
        },
    )
}

fn string_list_parameter(
    context: &DagNodeContext<'_>,
    name: &str,
) -> Result<Vec<String>, DagNodeFailure> {
    context.node.parameters.get(name).map_or_else(
        || Ok(Vec::new()),
        |value| {
            value
                .as_array()
                .ok_or_else(|| {
                    DagNodeFailure::terminal(
                        "invalid_node_parameter",
                        format!("parameter {name:?} must be an array"),
                    )
                })?
                .iter()
                .map(|item| {
                    item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                        DagNodeFailure::terminal(
                            "invalid_node_parameter",
                            format!("parameter {name:?} must contain strings"),
                        )
                    })
                })
                .collect()
        },
    )
}

fn object_parameter<'a>(
    context: &'a DagNodeContext<'_>,
    name: &str,
) -> Result<&'a serde_json::Map<String, Value>, DagNodeFailure> {
    static EMPTY: OnceLock<serde_json::Map<String, Value>> = OnceLock::new();
    context.node.parameters.get(name).map_or_else(
        || Ok(EMPTY.get_or_init(serde_json::Map::new)),
        |value| {
            value.as_object().ok_or_else(|| {
                DagNodeFailure::terminal(
                    "invalid_node_parameter",
                    format!("parameter {name:?} must be an object"),
                )
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use annotagent_core::{
        DETECTION_ARTIFACT_SCHEMA_VERSION, DetectionScore, DetectionSource, ImageId, IssueSeverity,
        NodePort, NormalizedRect, ProjectId, RunId, ScoreSemantics, SuggestedAction,
        ValidationEvidence, WorkflowDraftNode, WorkflowNodeKind,
    };
    use tokio_util::sync::CancellationToken;

    use super::*;

    #[test]
    fn ordinary_confidence_gate_cannot_compare_unknown_or_missing_scores() {
        assert_eq!(DetectionScore::not_provided().comparable_confidence(), None);
        assert_eq!(
            DetectionScore::new(Some(0.99), ScoreSemantics::Unknown)
                .expect("unknown score")
                .comparable_confidence(),
            None
        );
        assert_eq!(
            DetectionScore::new(Some(0.99), ScoreSemantics::RankingScore)
                .expect("ranking score")
                .comparable_confidence(),
            None
        );
    }

    #[tokio::test]
    async fn candidate_match_is_one_to_one_stable_and_preserves_conflicts_and_scores() {
        let image_id = ImageId::new();
        let left = detection_set(
            image_id,
            "set-a",
            "specialist",
            vec![
                detection(
                    "set-a",
                    "a1",
                    "ball",
                    [0.10, 0.10, 0.20, 0.20],
                    Some(0.91),
                    "specialist",
                    VisionCapability::ObjectDetection,
                ),
                detection(
                    "set-a",
                    "a2",
                    "ball",
                    [0.55, 0.55, 0.20, 0.20],
                    Some(0.72),
                    "specialist",
                    VisionCapability::ObjectDetection,
                ),
                detection(
                    "set-a",
                    "a3",
                    "robot",
                    [0.30, 0.60, 0.15, 0.20],
                    Some(0.88),
                    "specialist",
                    VisionCapability::ObjectDetection,
                ),
            ],
        );
        let right = detection_set(
            image_id,
            "set-b",
            "open",
            vec![
                detection(
                    "set-b",
                    "b1",
                    "ball",
                    [0.11, 0.11, 0.20, 0.20],
                    None,
                    "open",
                    VisionCapability::OpenVocabularyDetection,
                ),
                detection(
                    "set-b",
                    "b2",
                    "ball",
                    [0.62, 0.62, 0.20, 0.20],
                    None,
                    "open",
                    VisionCapability::OpenVocabularyDetection,
                ),
                detection(
                    "set-b",
                    "b3",
                    "person",
                    [0.31, 0.61, 0.15, 0.20],
                    None,
                    "open",
                    VisionCapability::OpenVocabularyDetection,
                ),
                detection(
                    "set-b",
                    "b4",
                    "ball",
                    [0.80, 0.10, 0.10, 0.10],
                    None,
                    "open",
                    VisionCapability::OpenVocabularyDetection,
                ),
            ],
        );
        let node = match_node(0.6);
        let first = CorePipelineRunner
            .run(node_context(
                &node,
                vec![left.clone(), right.clone()],
                BTreeMap::new(),
            ))
            .await
            .expect("candidate match");
        let second = CorePipelineRunner
            .run(node_context(&node, vec![left, right], BTreeMap::new()))
            .await
            .expect("stable candidate match");
        let PipelineArtifact::CandidateClusterSet(clusters) = &first.pipeline_artifacts[0] else {
            panic!("expected CandidateClusterSet")
        };
        let PipelineArtifact::CandidateClusterSet(second_clusters) = &second.pipeline_artifacts[0]
        else {
            panic!("expected CandidateClusterSet")
        };
        assert_eq!(clusters.candidates, second_clusters.candidates);
        assert_eq!(clusters.candidates.len(), 4);
        assert!(
            matches!(clusters.candidates[0].agreement, CandidateAgreement::MultiSourceAgreement { minimum_iou, .. } if minimum_iou > 0.8)
        );
        assert_eq!(clusters.candidates[0].members[0].score.value, Some(0.91));
        assert_eq!(
            clusters.candidates[0].members[1].score,
            DetectionScore::not_provided()
        );
        assert_eq!(
            clusters.candidates[1].agreement,
            CandidateAgreement::GeometryConflict
        );
        assert_eq!(
            clusters.candidates[2].agreement,
            CandidateAgreement::LabelConflict
        );
        assert_eq!(
            clusters.candidates[2].members[0].project_label,
            Some(LabelId::from("robot"))
        );
        assert_eq!(
            clusters.candidates[2].members[1].project_label,
            Some(LabelId::from("person"))
        );
        assert_eq!(
            clusters.candidates[3].agreement,
            CandidateAgreement::SingleSource
        );
        assert_eq!(
            clusters.candidates[3].members[0].model_label.as_deref(),
            Some("ball")
        );
    }

    #[tokio::test]
    async fn evidence_gate_accepts_agreement_without_averaging_scores() {
        let image_id = ImageId::new();
        let left = detection_set(
            image_id,
            "set-a",
            "specialist",
            vec![detection(
                "set-a",
                "a1",
                "ball",
                [0.1, 0.1, 0.2, 0.2],
                Some(0.93),
                "specialist",
                VisionCapability::ObjectDetection,
            )],
        );
        let right = detection_set(
            image_id,
            "set-b",
            "open",
            vec![detection(
                "set-b",
                "b1",
                "ball",
                [0.11, 0.11, 0.2, 0.2],
                None,
                "open",
                VisionCapability::OpenVocabularyDetection,
            )],
        );
        let match_node = match_node(0.6);
        let matched = CorePipelineRunner
            .run(node_context(
                &match_node,
                vec![left, right],
                BTreeMap::new(),
            ))
            .await
            .expect("candidate match");
        let gate = gate_node(&serde_json::json!({
            "accept_when": [{"minimum_sources": 2, "minimum_iou": 0.6}],
            "review_when": [{"score_missing": true}]
        }));
        let output = CorePipelineRunner
            .run(node_context(
                &gate,
                matched.pipeline_artifacts,
                BTreeMap::from([("match".to_owned(), matched.metadata)]),
            ))
            .await
            .expect("evidence gate");
        assert_eq!(
            output.route.as_deref(),
            Some("review"),
            "explicit score-missing review precedes acceptance"
        );
        let report: EvidenceGateReport =
            serde_json::from_value(output.metadata["evidence_gate"].clone()).expect("report");
        assert_eq!(report.reasons[0].code, "score_not_comparable");

        let gate = gate_node(&serde_json::json!({
            "accept_when": [{"minimum_sources": 2, "minimum_iou": 0.6}]
        }));
        let output = CorePipelineRunner
            .run(node_context(
                &gate,
                output.pipeline_artifacts,
                BTreeMap::new(),
            ))
            .await
            .expect("agreement accepts");
        assert_eq!(output.route.as_deref(), Some("accept"));
        let PipelineArtifact::CandidateClusterSet(clusters) = &output.pipeline_artifacts[0] else {
            panic!("clusters")
        };
        assert_eq!(clusters.validation_state, ArtifactValidationState::Valid);
        assert_eq!(clusters.candidates[0].members[0].score.value, Some(0.93));
        assert_eq!(clusters.candidates[0].members[1].score.value, None);
        let report: EvidenceGateReport =
            serde_json::from_value(output.metadata["evidence_gate"].clone()).expect("report");
        assert_eq!(report.decision, EvidenceGateDecision::Accept);
        assert_eq!(report.reasons[0].code, "multi_source_agreement");
    }

    #[tokio::test]
    async fn evidence_gate_requests_fallback_for_empty_source_and_domain_issue() {
        let image_id = ImageId::new();
        let specialist = detection_set(image_id, "set-a", "specialist", Vec::new());
        let open = detection_set(
            image_id,
            "set-b",
            "open",
            vec![detection(
                "set-b",
                "b1",
                "ball",
                [0.1, 0.1, 0.2, 0.2],
                None,
                "open",
                VisionCapability::OpenVocabularyDetection,
            )],
        );
        let match_node = match_node(0.5);
        let matched = CorePipelineRunner
            .run(node_context(
                &match_node,
                vec![specialist, open],
                BTreeMap::new(),
            ))
            .await
            .expect("candidate match");
        let issue = ValidationIssue {
            code: "domain-risk".to_owned(),
            severity: IssueSeverity::Warning,
            annotation_ids: Vec::new(),
            message: "domain validator requested more evidence".to_owned(),
            suggested_action: SuggestedAction::Refine,
            evidence: ValidationEvidence::Rule {
                facts: BTreeMap::new(),
            },
        };
        let mut match_metadata = matched.metadata;
        match_metadata.insert("validation_issues".to_owned(), serde_json::json!([issue]));
        let gate = gate_node(&serde_json::json!({
            "fallback_when": [
                {"source": "specialist", "empty_specialist_result": true},
                {"domain_issue": true}
            ]
        }));
        let output = CorePipelineRunner
            .run(node_context(
                &gate,
                matched.pipeline_artifacts,
                BTreeMap::from([("match".to_owned(), match_metadata)]),
            ))
            .await
            .expect("fallback decision");
        assert_eq!(output.route.as_deref(), Some("fallback"));
        let report: EvidenceGateReport =
            serde_json::from_value(output.metadata["evidence_gate"].clone()).expect("report");
        assert_eq!(report.reasons[0].code, "empty_source_result");
        assert_eq!(report.validation_issue_count, 1);
    }

    #[tokio::test]
    async fn evidence_gate_rejects_only_from_an_explicit_rule() {
        let image_id = ImageId::new();
        let empty_a = detection_set(image_id, "set-a", "a", Vec::new());
        let empty_b = detection_set(image_id, "set-b", "b", Vec::new());
        let match_step = match_node(0.5);
        let matched = CorePipelineRunner
            .run(node_context(
                &match_step,
                vec![empty_a, empty_b],
                BTreeMap::new(),
            ))
            .await
            .expect("valid empty match");
        let gate = gate_node(&serde_json::json!({"reject_when": [{"empty_result": true}]}));
        let output = CorePipelineRunner
            .run(node_context(
                &gate,
                matched.pipeline_artifacts,
                BTreeMap::from([("match".to_owned(), matched.metadata)]),
            ))
            .await
            .expect("reject decision");
        assert_eq!(output.route.as_deref(), Some("reject"));
        let PipelineArtifact::CandidateClusterSet(clusters) = &output.pipeline_artifacts[0] else {
            panic!("clusters")
        };
        assert_eq!(clusters.validation_state, ArtifactValidationState::Invalid);
    }

    #[tokio::test]
    async fn resize_and_tile_preserve_explicit_coordinate_lineage() {
        let image_id = ImageId::new();
        let image = pipeline_image(image_id, 100, 80, None);
        let resize = WorkflowDraftNode {
            id: "resize".to_owned(),
            node_type: CORE_RESIZE.to_owned(),
            kind: WorkflowNodeKind::Transform,
            outputs: vec![NodePort {
                id: "image".to_owned(),
                artifact_type: ArtifactKind::Image,
                required: true,
                multiple: false,
            }],
            parameters: BTreeMap::from([("max_edge".to_owned(), serde_json::json!(50))]),
            ..WorkflowDraftNode::default()
        };
        let resized = CorePipelineRunner
            .run(node_context(&resize, vec![image.clone()], BTreeMap::new()))
            .await
            .expect("resize");
        let PipelineArtifact::Image(resized_image) = &resized.pipeline_artifacts[0] else {
            panic!("image")
        };
        assert_eq!((resized_image.width, resized_image.height), (50, 40));
        assert_eq!(resized_image.parent.as_ref(), Some(image.reference()));

        let tile = WorkflowDraftNode {
            id: "tile".to_owned(),
            node_type: CORE_TILE.to_owned(),
            kind: WorkflowNodeKind::Transform,
            outputs: vec![NodePort {
                id: "images".to_owned(),
                artifact_type: ArtifactKind::Image,
                required: true,
                multiple: true,
            }],
            parameters: BTreeMap::from([
                ("tile_width".to_owned(), serde_json::json!(60)),
                ("tile_height".to_owned(), serde_json::json!(60)),
                ("overlap".to_owned(), serde_json::json!(0)),
                ("maximum_tiles".to_owned(), serde_json::json!(4)),
            ]),
            ..WorkflowDraftNode::default()
        };
        let tiled = CorePipelineRunner
            .run(node_context(&tile, vec![image], BTreeMap::new()))
            .await
            .expect("tile");
        assert_eq!(tiled.pipeline_artifacts.len(), 4);
        let PipelineArtifact::Image(last) = &tiled.pipeline_artifacts[3] else {
            panic!("tile")
        };
        let region = last.root_region.expect("root region");
        assert_eq!((region.x(), region.y()), (0.4, 0.25));
        assert_eq!(last.reference.item_id.as_deref(), Some("r1-c1"));
    }

    #[tokio::test]
    async fn coordinate_projection_maps_local_detections_to_root_image() {
        let image_id = ImageId::new();
        let root_region = NormalizedRect::new(0.4, 0.2, 0.5, 0.5).expect("region");
        let tile = pipeline_image(image_id, 100, 100, Some(root_region));
        let mut detections = detection_set(
            image_id,
            "local-set",
            "detector",
            vec![detection(
                "local-set",
                "ball",
                "ball",
                [0.2, 0.4, 0.4, 0.2],
                Some(0.9),
                "detector",
                VisionCapability::ObjectDetection,
            )],
        );
        let PipelineArtifact::DetectionSet(set) = &mut detections else {
            panic!("detections")
        };
        set.metadata.insert(
            "source_image_artifact_id".to_owned(),
            serde_json::json!(tile.reference().artifact_id),
        );
        let project = WorkflowDraftNode {
            id: "project".to_owned(),
            node_type: CORE_PROJECT_COORDINATES.to_owned(),
            kind: WorkflowNodeKind::Transform,
            outputs: vec![NodePort {
                id: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                required: true,
                multiple: true,
            }],
            ..WorkflowDraftNode::default()
        };
        let output = CorePipelineRunner
            .run(node_context(
                &project,
                vec![tile, detections],
                BTreeMap::new(),
            ))
            .await
            .expect("project");
        let PipelineArtifact::DetectionSet(set) = &output.pipeline_artifacts[0] else {
            panic!("detections")
        };
        let rect = set.detections[0].bbox;
        assert!((rect.x() - 0.5).abs() < f32::EPSILON);
        assert!((rect.y() - 0.4).abs() < f32::EPSILON);
        assert!((rect.width() - 0.2).abs() < f32::EPSILON);
        assert!((rect.height() - 0.1).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn select_and_map_is_one_public_transform() {
        let image_id = ImageId::new();
        let source = detection_set(
            image_id,
            "raw",
            "detector",
            vec![
                detection(
                    "raw",
                    "keep",
                    "sports-ball",
                    [0.1, 0.1, 0.2, 0.2],
                    Some(0.9),
                    "detector",
                    VisionCapability::ObjectDetection,
                ),
                detection(
                    "raw",
                    "drop",
                    "robot",
                    [0.4, 0.4, 0.2, 0.2],
                    Some(0.4),
                    "detector",
                    VisionCapability::ObjectDetection,
                ),
            ],
        );
        let node = WorkflowDraftNode {
            id: "select".to_owned(),
            node_type: CORE_SELECT_AND_MAP.to_owned(),
            kind: WorkflowNodeKind::Transform,
            outputs: vec![NodePort {
                id: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                required: true,
                multiple: true,
            }],
            parameters: BTreeMap::from([
                ("minimum_confidence".to_owned(), serde_json::json!(0.5)),
                (
                    "class_mapping".to_owned(),
                    serde_json::json!({"sports-ball": "ball"}),
                ),
                ("labels".to_owned(), serde_json::json!(["ball"])),
            ]),
            ..WorkflowDraftNode::default()
        };
        let output = CorePipelineRunner
            .run(node_context(&node, vec![source], BTreeMap::new()))
            .await
            .expect("select and map");
        let PipelineArtifact::DetectionSet(set) = &output.pipeline_artifacts[0] else {
            panic!("detections")
        };
        assert_eq!(set.detections.len(), 1);
        assert_eq!(
            set.detections[0]
                .project_label
                .as_ref()
                .map(LabelId::as_str),
            Some("ball")
        );
    }

    #[tokio::test]
    async fn sam_artifact_chain_preserves_original_prompt_mask_and_refined_box() {
        let image_id = ImageId::new();
        let detections = detection_set(
            image_id,
            "coarse-detections",
            "coarse-vlm",
            vec![detection(
                "coarse-detections",
                "ball-1",
                "ball",
                [0.10, 0.20, 0.40, 0.40],
                Some(0.82),
                "coarse-vlm",
                VisionCapability::ObjectDetection,
            )],
        );
        let prompt_node = WorkflowDraftNode {
            id: "prompts".to_owned(),
            node_type: CORE_DETECTIONS_TO_BOX_PROMPTS.to_owned(),
            kind: WorkflowNodeKind::Transform,
            outputs: vec![NodePort {
                id: "prompts".to_owned(),
                artifact_type: ArtifactKind::BoxPromptSet,
                required: true,
                multiple: true,
            }],
            ..WorkflowDraftNode::default()
        };
        let prompt_output = CorePipelineRunner
            .run(node_context(
                &prompt_node,
                vec![detections],
                BTreeMap::new(),
            ))
            .await
            .expect("box prompts");
        let PipelineArtifact::BoxPromptSet(prompts) = &prompt_output.pipeline_artifacts[0] else {
            panic!("box prompts")
        };
        assert_eq!(
            prompts.prompts[0].subject.item_id.as_deref(),
            Some("ball-1")
        );

        let point = |x, y| annotagent_core::NormalizedPoint::new(x, y).expect("point");
        let masks = PipelineArtifact::MaskSet(MaskSetArtifact {
            reference: ArtifactRef {
                artifact_id: "sam-masks".to_owned(),
                source_node: "sam".to_owned(),
                port: "masks".to_owned(),
                artifact_type: ArtifactKind::MaskSet,
                item_id: None,
            },
            image_id,
            model_binding: "sam2.1-worker".to_owned(),
            source_prompts: prompts.reference.clone(),
            validation_state: ArtifactValidationState::Valid,
            masks: vec![annotagent_core::MaskArtifactItem {
                mask_id: "ball-mask".to_owned(),
                prompt: prompts.reference.item(&prompts.prompts[0].id),
                mask: MaskEncoding::Polygon {
                    rings: vec![vec![
                        point(0.16, 0.26),
                        point(0.42, 0.26),
                        point(0.42, 0.52),
                        point(0.16, 0.52),
                    ]],
                },
                score: DetectionScore::relative(0.96).expect("score"),
                attributes: BTreeMap::new(),
            }],
            metadata: BTreeMap::new(),
        });
        let bbox_node = WorkflowDraftNode {
            id: "mask-to-bbox".to_owned(),
            node_type: CORE_MASK_TO_BBOX.to_owned(),
            kind: WorkflowNodeKind::Transform,
            outputs: vec![NodePort {
                id: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                required: true,
                multiple: true,
            }],
            ..WorkflowDraftNode::default()
        };
        let refined_output = CorePipelineRunner
            .run(node_context(
                &bbox_node,
                vec![prompt_output.pipeline_artifacts[0].clone(), masks],
                BTreeMap::new(),
            ))
            .await
            .expect("mask to bbox");
        let PipelineArtifact::DetectionSet(refined) = &refined_output.pipeline_artifacts[0] else {
            panic!("refined detections")
        };
        let detection = &refined.detections[0];
        assert_eq!(
            detection.source_capability,
            VisionCapability::PromptedSegmentation
        );
        assert_eq!(detection.source_model_id, "sam2.1-worker");
        assert_eq!(detection.evidence.len(), 2);
        assert!((detection.bbox.x() - 0.16).abs() < f32::EPSILON);
        let audit = detection
            .attributes
            .get("geometry_refinement")
            .expect("audit trail");
        assert_eq!(audit["method"], "mask_to_bbox");
        assert_eq!(audit["mask"]["artifact_id"], "sam-masks");
        assert_eq!(audit["source_detection"]["item_id"], "ball-1");
        assert_eq!(audit["box_prompt"]["item_id"], "box-prompt:ball-1");
        assert_eq!(audit["refined_detection"]["item_id"], "refined:ball-1");

        let refined_artifact = refined_output.pipeline_artifacts[0].clone();
        let evaluation_node = WorkflowDraftNode {
            id: "geometry-quality".to_owned(),
            node_type: CORE_GEOMETRY_QUALITY_EVALUATION.to_owned(),
            kind: WorkflowNodeKind::Validator,
            outputs: vec![NodePort {
                id: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                required: true,
                multiple: true,
            }],
            ..WorkflowDraftNode::default()
        };
        let evaluated = CorePipelineRunner
            .run(node_context(
                &evaluation_node,
                vec![refined_artifact.clone()],
                BTreeMap::new(),
            ))
            .await
            .expect("geometry quality evaluation");
        assert_eq!(evaluated.metadata["unstable_detection_count"], 0);
        assert_eq!(evaluated.metadata["semantic_score_used"], false);

        let decision_node = WorkflowDraftNode {
            id: "geometry-decision".to_owned(),
            node_type: CORE_GEOMETRY_DECISION.to_owned(),
            kind: WorkflowNodeKind::Gate,
            outputs: vec![NodePort {
                id: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                required: true,
                multiple: true,
            }],
            ..WorkflowDraftNode::default()
        };
        let accepted = CorePipelineRunner
            .run(node_context(
                &decision_node,
                evaluated.pipeline_artifacts,
                BTreeMap::new(),
            ))
            .await
            .expect("geometry decision");
        assert_eq!(accepted.route.as_deref(), Some("accept"));
        assert_eq!(accepted.metadata["semantic_score_used"], false);

        let strict_evaluation_node = WorkflowDraftNode {
            parameters: BTreeMap::from([(
                "maximum_center_shift".to_owned(),
                serde_json::json!(0.005),
            )]),
            ..evaluation_node
        };
        let unstable = CorePipelineRunner
            .run(node_context(
                &strict_evaluation_node,
                vec![refined_artifact],
                BTreeMap::new(),
            ))
            .await
            .expect("strict geometry quality evaluation");
        assert_eq!(unstable.metadata["unstable_detection_count"], 1);
        let reviewed = CorePipelineRunner
            .run(node_context(
                &decision_node,
                unstable.pipeline_artifacts,
                BTreeMap::new(),
            ))
            .await
            .expect("review geometry decision");
        assert_eq!(reviewed.route.as_deref(), Some("review"));
    }

    fn pipeline_image(
        image_id: ImageId,
        width: u32,
        height: u32,
        root_region: Option<NormalizedRect>,
    ) -> PipelineArtifact {
        PipelineArtifact::Image(annotagent_core::ImageArtifact {
            reference: ArtifactRef {
                artifact_id: format!("image:{image_id}"),
                source_node: "image".to_owned(),
                port: "image".to_owned(),
                artifact_type: ArtifactKind::Image,
                item_id: root_region.map(|_| "tile".to_owned()),
            },
            image_id,
            width,
            height,
            mime_type: "image/png".to_owned(),
            blob_ref: "workspace://fixture".to_owned(),
            parent: None,
            root_region,
        })
    }

    fn detection_set(
        image_id: ImageId,
        artifact_id: &str,
        model_id: &str,
        detections: Vec<Detection>,
    ) -> PipelineArtifact {
        PipelineArtifact::DetectionSet(DetectionSetArtifact {
            schema_version: DETECTION_ARTIFACT_SCHEMA_VERSION,
            reference: ArtifactRef {
                artifact_id: artifact_id.to_owned(),
                source_node: format!("{model_id}-node"),
                port: "detections".to_owned(),
                artifact_type: ArtifactKind::DetectionSet,
                item_id: None,
            },
            image_id,
            model_binding: model_id.to_owned(),
            validation_state: ArtifactValidationState::Unvalidated,
            detections,
            metadata: BTreeMap::new(),
        })
    }

    fn detection(
        artifact_id: &str,
        id: &str,
        label: &str,
        bbox: [f32; 4],
        score: Option<f32>,
        model_id: &str,
        capability: VisionCapability,
    ) -> Detection {
        Detection::from_source(
            id,
            (capability != VisionCapability::ObjectDetection).then(|| format!("query-{label}")),
            Some(label.to_owned()),
            Some(LabelId::from(label)),
            NormalizedRect::new(bbox[0], bbox[1], bbox[2], bbox[3]).expect("bbox"),
            score.map_or_else(DetectionScore::not_provided, |score| {
                DetectionScore::relative(score).expect("score")
            }),
            DetectionSource {
                model_id: model_id.to_owned(),
                capability,
                artifact_id: artifact_id.to_owned(),
            },
        )
        .expect("detection")
    }

    fn match_node(minimum_iou: f32) -> WorkflowDraftNode {
        WorkflowDraftNode {
            id: "match".to_owned(),
            node_type: CORE_CANDIDATE_MATCH.to_owned(),
            kind: WorkflowNodeKind::CandidateMerge,
            outputs: vec![NodePort {
                id: "candidates".to_owned(),
                artifact_type: ArtifactKind::CandidateClusterSet,
                required: true,
                multiple: false,
            }],
            parameters: BTreeMap::from([
                ("method".to_owned(), serde_json::json!("iou")),
                ("minimum_iou".to_owned(), serde_json::json!(minimum_iou)),
                ("preserve_unmatched".to_owned(), serde_json::json!(true)),
            ]),
            ..WorkflowDraftNode::default()
        }
    }

    fn gate_node(parameters: &serde_json::Value) -> WorkflowDraftNode {
        WorkflowDraftNode {
            id: "evidence".to_owned(),
            node_type: CORE_EVIDENCE_GATE.to_owned(),
            kind: WorkflowNodeKind::Gate,
            parameters: parameters
                .as_object()
                .expect("gate config")
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            ..WorkflowDraftNode::default()
        }
    }

    fn node_context(
        node: &WorkflowDraftNode,
        input_pipeline_artifacts: Vec<PipelineArtifact>,
        input_metadata: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    ) -> DagNodeContext<'_> {
        let image_id = input_pipeline_artifacts
            .first()
            .map_or_else(ImageId::new, PipelineArtifact::image_id);
        DagNodeContext {
            project_id: ProjectId::new(),
            run_id: RunId::new(),
            image_id,
            node,
            input_artifacts: Vec::new(),
            input_pipeline_artifacts,
            input_metadata,
            cancellation: CancellationToken::new(),
        }
    }
}
