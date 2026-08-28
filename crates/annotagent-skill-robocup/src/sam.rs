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
        let sam_prompt = match seed_result.annotation.value {
            AnnotationValue::BoundingBox { rect }
                if !seed_result
                    .issues
                    .iter()
                    .any(|issue| issue.code == "ball_foreground_refiner_fallback") =>
            {
                rect
            }
            _ => coarse,
        };
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
            prompt: Some("segment the football inside the supplied bounding box".to_owned()),
            parameters: BTreeMap::from([
                ("box_prompt".to_owned(), serde_json::json!(sam_prompt)),
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
        let Some(mask_artifact) = response
            .artifacts
            .iter()
            .find(|artifact| matches!(artifact.value, VisionArtifactValue::InstanceMask { .. }))
        else {
            return self
                .fallback(context, "worker returned no instance-mask artifact")
                .await;
        };
        let VisionArtifactValue::InstanceMask { mask } = &mask_artifact.value else {
            unreachable!();
        };
        let refined = match tight_bbox(mask) {
            Ok(rect) if plausible_refinement(coarse, rect) => rect,
            Ok(_) => {
                return self
                    .fallback(context, "SAM mask geometry was outside safety bounds")
                    .await;
            }
            Err(error) => return self.fallback(context, &error.to_string()).await,
        };
        let confidence = mask_artifact.confidence.unwrap_or(0.0);
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
                "refined foreground-seeded VLM box with {} prompted mask (score {:.0}%)",
                response.model_identity.as_deref().unwrap_or(&self.model_id),
                confidence * 100.0
            ),
            artifacts: response.artifacts,
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

fn plausible_refinement(coarse: NormalizedRect, refined: NormalizedRect) -> bool {
    let center = refined.center();
    let aspect = refined.width() / refined.height();
    coarse.contains(center, coarse.width().max(coarse.height()) * 0.2)
        && refined.area() <= coarse.area() * 1.35
        && (0.3..=3.0).contains(&aspect)
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
}
