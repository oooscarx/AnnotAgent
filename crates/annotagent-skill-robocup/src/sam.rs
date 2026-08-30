use std::{collections::BTreeMap, time::Duration};

use annotagent_core::{
    AnnotationRefiner, AnnotationValue, ArtifactId, ArtifactProvenance, ArtifactRole,
    ArtifactValidationState, CoreError, CoreResult, IssueSeverity, LabelId, MaskEncoding,
    ModelImage, NormalizedPoint, NormalizedRect, RefinementContext, RefinementResult,
    SuggestedAction, ValidationEvidence, ValidationIssue, VisionArtifact, VisionArtifactValue,
    VisionCapability, VisionInferenceRequest, VisionModelBackend,
};
use annotagent_image_tools::encode_png;
use annotagent_provider::{HttpJsonVisionBackend, HttpJsonVisionBackendConfig};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use uuid::Uuid;

use crate::RoboCupBallForegroundRefiner;

const DEFAULT_SAM_ENDPOINT: &str = "http://127.0.0.1:8790/v1/infer";
const DEFAULT_SAM_MODEL: &str = "sam2.1-hiera-tiny";

/// Refines a VLM-proposed ball box with a real prompted-segmentation worker.
///
/// The worker is deliberately outside the Rust process: this keeps Core model-agnostic and uses
/// the same versioned HTTP Vision Protocol as other vision backends. A deterministic local
/// foreground refiner remains available only as an explicit, review-visible fallback.
pub struct RoboCupSamHttpRefiner {
    backend: HttpJsonVisionBackend,
    model_id: String,
    fallback: RoboCupBallForegroundRefiner,
}

impl RoboCupSamHttpRefiner {
    pub fn from_env() -> CoreResult<Self> {
        let endpoint = std::env::var("ANNOTAGENT_SAM_ENDPOINT")
            .unwrap_or_else(|_| DEFAULT_SAM_ENDPOINT.to_owned());
        let model_id =
            std::env::var("ANNOTAGENT_SAM_MODEL").unwrap_or_else(|_| DEFAULT_SAM_MODEL.to_owned());
        Self::new(endpoint, model_id)
    }

    pub fn new(endpoint: impl Into<String>, model_id: impl Into<String>) -> CoreResult<Self> {
        let backend = HttpJsonVisionBackend::new(HttpJsonVisionBackendConfig {
            id: "sam2_http_worker".to_owned(),
            endpoint: endpoint.into(),
            capabilities: vec![VisionCapability::PromptedSegmentation],
            request_timeout: Duration::from_secs(120),
            authorization: None,
            expected_model_identity: None,
            max_retries: 0,
            max_response_bytes: 2_000_000,
            allow_remote: false,
        })?;
        Ok(Self {
            backend,
            model_id: model_id.into(),
            fallback: RoboCupBallForegroundRefiner::default(),
        })
    }

    async fn fallback(
        &self,
        context: &RefinementContext<'_>,
        detail: &str,
    ) -> CoreResult<RefinementResult> {
        let mut result = self.fallback.refine(context).await?;
        result.issues.push(ValidationIssue {
            code: "sam_prompted_refiner_unavailable".to_owned(),
            severity: IssueSeverity::Warning,
            annotation_ids: vec![context.candidate.id],
            message: format!(
                "SAM prompted segmentation was unavailable; used local foreground fallback ({})",
                detail.chars().take(240).collect::<String>()
            ),
            suggested_action: SuggestedAction::HumanReview,
            evidence: ValidationEvidence::Rule {
                facts: BTreeMap::from([
                    ("requested_refiner".to_owned(), self.id().to_owned()),
                    ("fallback".to_owned(), self.fallback.id().to_owned()),
                ]),
            },
        });
        result.summary = format!(
            "SAM unavailable; {}. Result requires review.",
            result.summary
        );
        Ok(result)
    }
}

#[async_trait::async_trait]
impl AnnotationRefiner for RoboCupSamHttpRefiner {
    fn id(&self) -> &str {
        "sam_prompted_refiner"
    }

    async fn refine(&self, context: &RefinementContext<'_>) -> CoreResult<RefinementResult> {
        if context
            .candidate
            .label
            .as_ref()
            .is_none_or(|label| label.as_str() != "ball")
        {
            return Err(CoreError::Refinement(
                "SAM ball refiner requires a ball candidate".to_owned(),
            ));
        }
        let AnnotationValue::BoundingBox { rect: coarse } = context.candidate.value else {
            return Err(CoreError::Refinement(
                "SAM ball refiner requires a bounding-box candidate".to_owned(),
            ));
        };
        let seed_result = self.fallback.refine(context).await?;
        let seeded_prompt = match seed_result.annotation.value {
            AnnotationValue::BoundingBox { rect }
                if !seed_result
                    .issues
                    .iter()
                    .any(|issue| issue.code == "ball_foreground_refiner_fallback") =>
            {
                Some(rect)
            }
            _ => None,
        };
        let sam_prompts = ball_search_prompts(coarse, seeded_prompt)?;
        let sam_prompt = sam_prompts[0];
        let input_artifact = VisionArtifact {
            id: ArtifactId::new(),
            image_id: context.candidate.image_id,
            task_id: Some(context.candidate.task_id.clone()),
            label: Some(LabelId::from("ball")),
            role: ArtifactRole::Candidate,
            value: VisionArtifactValue::BoundingBox { rect: sam_prompt },
            source_node: format!("{}.input", self.id()),
            confidence: context.candidate.confidence,
            metadata: BTreeMap::new(),
            validation_state: ArtifactValidationState::Unvalidated,
            provenance: ArtifactProvenance::default(),
            revision: 1,
            replaces_artifact_id: None,
            created_at: chrono::Utc::now(),
        };
        let png = encode_png(context.image)?;
        let request = VisionInferenceRequest {
            protocol_version: annotagent_core::VISION_WORKER_PROTOCOL_VERSION,
            request_id: Uuid::new_v4().to_string(),
            operation: VisionCapability::PromptedSegmentation,
            run_id: context.run_id,
            image_id: context.candidate.image_id,
            task_id: context.candidate.task_id.clone(),
            node_id: self.id().to_owned(),
            model_id: self.model_id.clone(),
            image: Some(ModelImage {
                id: context.candidate.image_id.to_string(),
                mime_type: "image/png".to_owned(),
                data_base64: STANDARD.encode(png),
            }),
            input_artifacts: vec![input_artifact],
            prompt: Some(
                "segment the compact non-field football; return multiple candidates for bounded selection"
                    .to_owned(),
            ),
            parameters: BTreeMap::from([
                ("box_prompt".to_owned(), serde_json::json!(sam_prompt)),
                ("box_prompts".to_owned(), serde_json::json!(sam_prompts)),
                ("multimask_output".to_owned(), serde_json::json!(true)),
            ]),
            timeout_ms: Some(120_000),
            cancellation_requested: context.cancellation.is_cancelled(),
        };
        let response = match self
            .backend
            .infer(request, context.cancellation.clone())
            .await
        {
            Ok(response) => response,
            Err(error) => return self.fallback(context, &error.to_string()).await,
        };
        let mut artifacts = response.artifacts;
        let mut candidates = artifacts
            .iter()
            .enumerate()
            .filter_map(|(index, artifact)| {
                let VisionArtifactValue::InstanceMask { mask } = &artifact.value else {
                    return None;
                };
                let rect = tight_bbox(mask).ok()?;
                let score = ball_mask_score(
                    context.image,
                    coarse,
                    rect,
                    mask,
                    artifact.confidence.unwrap_or(0.0),
                )
                .ok()??;
                Some((index, rect, score))
            })
            .collect::<Vec<_>>();
        for (index, _, score) in &candidates {
            artifacts[*index]
                .metadata
                .insert("ball_selection_score".to_owned(), serde_json::json!(score));
        }
        candidates.sort_by(|left, right| right.2.total_cmp(&left.2));
        let Some((selected_index, refined, selection_score)) = candidates.first().copied() else {
            return self
                .fallback(
                    context,
                    "worker returned no geometrically and visually plausible football mask",
                )
                .await;
        };
        let sam_confidence = artifacts[selected_index].confidence.unwrap_or(0.0);
        artifacts[selected_index]
            .metadata
            .insert("selected".to_owned(), serde_json::json!(true));
        let confidence = (selection_score * 0.7 + sam_confidence * 0.3).clamp(0.0, 1.0);
        let mut annotation = context.candidate.clone();
        annotation.value = AnnotationValue::BoundingBox { rect: refined };
        annotation.confidence = Some(confidence);
        Ok(RefinementResult {
            annotation,
            confidence,
            issues: if confidence >= 0.5 {
                Vec::new()
            } else {
                vec![ValidationIssue {
                    code: "sam_low_confidence".to_owned(),
                    severity: IssueSeverity::Warning,
                    annotation_ids: vec![context.candidate.id],
                    message: "SAM returned a low-confidence mask; inspect the refined box"
                        .to_owned(),
                    suggested_action: SuggestedAction::HumanReview,
                    evidence: ValidationEvidence::Geometry {
                        metric: "sam_score".to_owned(),
                        value: f64::from(confidence),
                        threshold: 0.5,
                    },
                }]
            },
            summary: format!(
                "selected a football mask from {} SAM candidates with {} (SAM {:.0}%, selection {:.0}%)",
                candidates.len(),
                response.model_identity.as_deref().unwrap_or(&self.model_id),
                sam_confidence * 100.0,
                selection_score * 100.0,
            ),
            artifacts,
        })
    }
}

fn tight_bbox(mask: &MaskEncoding) -> CoreResult<NormalizedRect> {
    match mask {
        MaskEncoding::CocoRle {
            width,
            height,
            counts,
        } => bbox_from_uncompressed_rle(*width, *height, counts),
        MaskEncoding::Polygon { rings } => {
            let points = rings.iter().flatten().collect::<Vec<&NormalizedPoint>>();
            if points.is_empty() {
                return Err(CoreError::Refinement(
                    "SAM polygon mask is empty".to_owned(),
                ));
            }
            let min_x = points.iter().map(|point| point.x()).fold(1.0_f32, f32::min);
            let min_y = points.iter().map(|point| point.y()).fold(1.0_f32, f32::min);
            let max_x = points.iter().map(|point| point.x()).fold(0.0_f32, f32::max);
            let max_y = points.iter().map(|point| point.y()).fold(0.0_f32, f32::max);
            NormalizedRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
        }
    }
}

fn bbox_from_uncompressed_rle(width: u32, height: u32, counts: &str) -> CoreResult<NormalizedRect> {
    let runs = parse_rle_runs(width, height, counts)?;
    let mut cursor = 0_u64;
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for (index, length) in runs.into_iter().enumerate() {
        if index % 2 == 1 && length > 0 {
            for offset in cursor..cursor + length {
                let x = (offset / u64::from(height)) as u32;
                let y = (offset % u64::from(height)) as u32;
                bounds = Some(bounds.map_or((x, y, x, y), |(l, t, r, b)| {
                    (l.min(x), t.min(y), r.max(x), b.max(y))
                }));
            }
        }
        cursor += length;
    }
    let Some((left, top, right, bottom)) = bounds else {
        return Err(CoreError::Refinement("SAM mask is empty".to_owned()));
    };
    NormalizedRect::new(
        left as f32 / width as f32,
        top as f32 / height as f32,
        (right - left + 1) as f32 / width as f32,
        (bottom - top + 1) as f32 / height as f32,
    )
}

fn parse_rle_runs(width: u32, height: u32, counts: &str) -> CoreResult<Vec<u64>> {
    if width == 0 || height == 0 {
        return Err(CoreError::Refinement(
            "SAM mask has zero dimensions".to_owned(),
        ));
    }
    let runs = counts
        .split_whitespace()
        .map(|value| {
            value.parse::<u64>().map_err(|_| {
                CoreError::Refinement("SAM returned invalid uncompressed COCO RLE".to_owned())
            })
        })
        .collect::<CoreResult<Vec<_>>>()?;
    let expected = u64::from(width) * u64::from(height);
    if runs.iter().sum::<u64>() != expected {
        return Err(CoreError::Refinement(
            "SAM mask RLE dimensions do not match its counts".to_owned(),
        ));
    }
    Ok(runs)
}

fn ball_search_prompts(
    coarse: NormalizedRect,
    seeded: Option<NormalizedRect>,
) -> CoreResult<Vec<NormalizedRect>> {
    let mut prompts = Vec::new();
    if let Some(seed) = seeded {
        prompts.push(seed);
    }
    prompts.push(coarse);
    for (width_scale, height_scale, vertical_shift) in [
        (2.0, 2.5, -0.75),
        (3.0, 3.5, -1.5),
        (4.0, 5.0, -2.0),
        (3.0, 3.0, 0.0),
    ] {
        let width = (coarse.width() * width_scale).min(1.0);
        let height = (coarse.height() * height_scale).min(1.0);
        let center = coarse.center();
        let center_y = center.y() + coarse.height() * vertical_shift;
        let left = (center.x() - width / 2.0).clamp(0.0, 1.0 - width);
        let top = (center_y - height / 2.0).clamp(0.0, 1.0 - height);
        let prompt = NormalizedRect::new(left, top, width, height)?;
        if !prompts.contains(&prompt) {
            prompts.push(prompt);
        }
    }
    Ok(prompts)
}

fn ball_mask_score(
    image: &annotagent_core::ImageFrame,
    coarse: NormalizedRect,
    refined: NormalizedRect,
    mask: &MaskEncoding,
    sam_confidence: f32,
) -> CoreResult<Option<f32>> {
    let area_ratio = refined.area() / coarse.area().max(f32::EPSILON);
    let aspect = refined.width() / refined.height().max(f32::EPSILON);
    let coarse_center = coarse.center();
    let refined_center = refined.center();
    let distance = ((refined_center.x() - coarse_center.x()) / coarse.width().max(0.005))
        .hypot((refined_center.y() - coarse_center.y()) / coarse.height().max(0.005));
    if !(0.15..=4.5).contains(&area_ratio)
        || !(0.35..=2.5).contains(&aspect)
        || distance > 5.0
        || refined.area() > 0.15
    {
        return Ok(None);
    }
    let (non_field_ratio, distinctive_ratio) = mask_appearance(image, mask)?;
    if non_field_ratio < 0.25 || distinctive_ratio < 0.04 {
        return Ok(None);
    }
    let aspect_score = (1.0 - aspect.ln().abs() / 1.2).clamp(0.0, 1.0);
    let size_score = (1.0 - area_ratio.ln().abs() / 2.5).clamp(0.0, 1.0);
    let proximity_score = (1.0 - distance / 5.0).clamp(0.0, 1.0);
    let overlap_score =
        coarse.intersection_area(refined) / coarse.area().min(refined.area()).max(f32::EPSILON);
    Ok(Some(
        (sam_confidence.clamp(0.0, 1.0) * 0.35
            + aspect_score * 0.1
            + size_score * 0.15
            + proximity_score * 0.08
            + overlap_score * 0.12
            + non_field_ratio * 0.12
            + distinctive_ratio * 0.08)
            .clamp(0.0, 1.0),
    ))
}

fn mask_appearance(
    image: &annotagent_core::ImageFrame,
    mask: &MaskEncoding,
) -> CoreResult<(f32, f32)> {
    let MaskEncoding::CocoRle {
        width,
        height,
        counts,
    } = mask
    else {
        return Ok((0.5, 0.25));
    };
    if *width != image.metadata.width || *height != image.metadata.height {
        return Err(CoreError::Refinement(
            "SAM mask dimensions do not match the source image".to_owned(),
        ));
    }
    let runs = parse_rle_runs(*width, *height, counts)?;
    let mut cursor = 0_u64;
    let mut foreground = 0_u64;
    let mut non_field = 0_u64;
    let mut distinctive = 0_u64;
    for (index, length) in runs.into_iter().enumerate() {
        if index % 2 == 1 {
            for offset in cursor..cursor + length {
                let x = (offset / u64::from(*height)) as usize;
                let y = (offset % u64::from(*height)) as usize;
                let pixel = (y * *width as usize + x) * 3;
                let red = image.rgb[pixel];
                let green = image.rgb[pixel + 1];
                let blue = image.rgb[pixel + 2];
                foreground += 1;
                let field_green = green >= 38
                    && i16::from(green) - i16::from(red) >= 7
                    && i16::from(green) - i16::from(blue) >= 4;
                non_field += u64::from(!field_green);
                let maximum = red.max(green).max(blue);
                let minimum = red.min(green).min(blue);
                distinctive += u64::from(maximum < 105 || maximum - minimum > 32);
            }
        }
        cursor += length;
    }
    if foreground == 0 {
        return Err(CoreError::Refinement("SAM mask is empty".to_owned()));
    }
    Ok((
        non_field as f32 / foreground as f32,
        distinctive as f32 / foreground as f32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_tight_box_from_column_major_uncompressed_coco_rle() {
        let rect = bbox_from_uncompressed_rle(4, 3, "4 2 1 2 3").expect("bbox");
        assert!((rect.x() - 0.25).abs() < f32::EPSILON);
        assert!((rect.y() - (1.0 / 3.0)).abs() < f32::EPSILON);
        assert!((rect.width() - 0.5).abs() < f32::EPSILON);
        assert!((rect.height() - (2.0 / 3.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn rejects_malformed_or_empty_sam_masks() {
        assert!(bbox_from_uncompressed_rle(4, 3, "12").is_err());
        assert!(bbox_from_uncompressed_rle(4, 3, "4 nope 8").is_err());
        assert!(bbox_from_uncompressed_rle(4, 3, "4 2").is_err());
    }

    #[test]
    fn search_prompts_cover_a_ball_above_an_imprecise_vlm_box() {
        let coarse = NormalizedRect::new(0.44, 0.41, 0.035, 0.04).expect("coarse bbox");
        let actual_ball = NormalizedRect::new(0.4375, 0.357, 0.039, 0.051).expect("ball bbox");
        let prompts = ball_search_prompts(coarse, None).expect("search prompts");

        assert!(prompts.len() > 1);
        assert!(prompts.iter().any(|prompt| {
            let center = actual_ball.center();
            center.x() >= prompt.x()
                && center.x() <= prompt.x() + prompt.width()
                && center.y() >= prompt.y()
                && center.y() <= prompt.y() + prompt.height()
        }));
    }

    #[test]
    fn high_confidence_tight_mask_beats_a_large_distractor() {
        let image = annotagent_core::ImageFrame {
            metadata: annotagent_core::ImageMetadata {
                width: 10,
                height: 10,
                mime_type: "image/png".to_owned(),
                sha256: "fixture".to_owned(),
            },
            rgb: vec![0; 10 * 10 * 3],
        };
        let coarse = NormalizedRect::new(0.4, 0.4, 0.04, 0.04).expect("coarse");
        let tight = NormalizedRect::new(0.4, 0.39, 0.04, 0.05).expect("tight");
        let distractor = NormalizedRect::new(0.38, 0.32, 0.09, 0.09).expect("distractor");
        let polygon = MaskEncoding::Polygon { rings: Vec::new() };

        let tight_score = ball_mask_score(&image, coarse, tight, &polygon, 0.91)
            .expect("score")
            .expect("plausible tight mask");
        assert!(
            ball_mask_score(&image, coarse, distractor, &polygon, 0.62)
                .expect("score")
                .is_none(),
            "a mask over five times the VLM box area must not outrank a tight high-confidence mask"
        );
        assert!(tight_score > 0.7);
    }
}
