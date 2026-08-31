//! Structured annotation failure and geometry-quality evidence.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    AnnotationSnapshot, AnnotationValue, DetectionArtifactItem, GeometrySemantics, NormalizedRect,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationFailureClass {
    InfrastructureFailure,
    ProviderFailure,
    NoCandidate,
    SemanticError,
    GeometryError,
    MissingScore,
    DomainRisk,
    InvalidArtifact,
    BudgetLimit,
}

/// Classifies an already-structured Runtime/validation code. This is intentionally conservative:
/// unknown errors remain invalid-artifact failures instead of being guessed as geometry errors.
#[must_use]
pub fn classify_annotation_failure(code: &str, message: &str) -> AnnotationFailureClass {
    let evidence = format!("{code} {message}").to_ascii_lowercase();
    if contains_any(&evidence, &["budget", "cost_limit", "token_limit"]) {
        AnnotationFailureClass::BudgetLimit
    } else if contains_any(
        &evidence,
        &[
            "provider",
            "api key",
            "api_key",
            "credential",
            "rate_limit",
            "rate limit",
            "model request",
            "model timeout",
            "qwen",
            "openai",
        ],
    ) {
        AnnotationFailureClass::ProviderFailure
    } else if contains_any(
        &evidence,
        &[
            "worker",
            "unreachable",
            "connection",
            "connect",
            "service unavailable",
            "backend unavailable",
            "not started",
        ],
    ) {
        AnnotationFailureClass::InfrastructureFailure
    } else if contains_any(
        &evidence,
        &[
            "no_candidate",
            "no candidate",
            "empty_detection",
            "empty result",
        ],
    ) {
        AnnotationFailureClass::NoCandidate
    } else if contains_any(
        &evidence,
        &["missing_score", "missing score", "score_not_provided"],
    ) {
        AnnotationFailureClass::MissingScore
    } else if contains_any(
        &evidence,
        &[
            "semantic",
            "wrong_label",
            "label_conflict",
            "classification",
            "false_positive",
        ],
    ) {
        AnnotationFailureClass::SemanticError
    } else if contains_any(
        &evidence,
        &[
            "geometry",
            "bounding_box",
            "bbox",
            "mask",
            "iou",
            "aspect_ratio",
            "center_shift",
        ],
    ) {
        AnnotationFailureClass::GeometryError
    } else if contains_any(
        &evidence,
        &["domain", "correction_risk", "validator", "hard_negative"],
    ) {
        AnnotationFailureClass::DomainRisk
    } else {
        AnnotationFailureClass::InvalidArtifact
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryQualityReport {
    /// Pipeline Artifact IDs are stable strings and may not be UUIDs.
    pub artifact_id: String,
    pub geometry_semantics: GeometrySemantics,
    pub clipped_to_image: bool,
    pub aspect_ratio_outlier: bool,
    pub area_ratio: Option<f32>,
    pub foreground_occupancy: Option<f32>,
    pub edge_support: Option<f32>,
    pub mask_support: Option<f32>,
    pub center_shift_from_refiner: Option<f32>,
    pub area_change_from_refiner: Option<f32>,
    pub iou_with_refiner: Option<f32>,
    pub manual_center_shift: Option<f32>,
    pub manual_area_change: Option<f32>,
    pub historical_correction_rate: Option<f32>,
    pub issue_codes: Vec<String>,
}

impl GeometryQualityReport {
    #[must_use]
    pub fn from_detection(
        artifact_id: impl Into<String>,
        detection: &DetectionArtifactItem,
    ) -> Self {
        let bbox = detection.bbox;
        let aspect_ratio = bbox.width() / bbox.height();
        let aspect_ratio_outlier = !(0.2..=5.0).contains(&aspect_ratio);
        let clipped_to_image = detection
            .attributes
            .get("clipped_to_image")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let mut report = Self {
            artifact_id: artifact_id.into(),
            geometry_semantics: detection.geometry_semantics,
            clipped_to_image,
            aspect_ratio_outlier,
            area_ratio: Some(bbox.area()),
            foreground_occupancy: None,
            edge_support: None,
            mask_support: None,
            center_shift_from_refiner: None,
            area_change_from_refiner: None,
            iou_with_refiner: None,
            manual_center_shift: None,
            manual_area_change: None,
            historical_correction_rate: None,
            issue_codes: Vec::new(),
        };
        if report.clipped_to_image {
            report
                .issue_codes
                .push("geometry_clipped_to_image".to_owned());
        }
        if touches_image_boundary(bbox) {
            report
                .issue_codes
                .push("geometry_touches_image_boundary".to_owned());
        }
        if aspect_ratio_outlier {
            report
                .issue_codes
                .push("geometry_aspect_ratio_outlier".to_owned());
        }
        if bbox.area() < 0.000_1 {
            report.issue_codes.push("geometry_very_small".to_owned());
        } else if bbox.area() > 0.8 {
            report.issue_codes.push("geometry_very_large".to_owned());
        }
        if let Some(refinement) = detection.attributes.get("geometry_refinement") {
            let original = refinement
                .get("original_bbox")
                .cloned()
                .and_then(|value| serde_json::from_value::<NormalizedRect>(value).ok());
            let refined = refinement
                .get("refined_bbox")
                .cloned()
                .and_then(|value| serde_json::from_value::<NormalizedRect>(value).ok())
                .unwrap_or(bbox);
            if let Some(original) = original {
                report.center_shift_from_refiner = Some(center_shift(original, refined));
                report.area_change_from_refiner = Some(relative_area_change(original, refined));
                report.iou_with_refiner = Some(rect_iou(original, refined));
            } else {
                report
                    .issue_codes
                    .push("refiner_lineage_invalid".to_owned());
            }
        }
        report
    }

    #[must_use]
    pub fn has_geometry_issue(&self) -> bool {
        self.aspect_ratio_outlier
            || self
                .issue_codes
                .iter()
                .any(|code| code.starts_with("geometry_"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GeometryQualitySummary {
    pub total_candidates: u32,
    pub coarse_geometry_count: u32,
    pub geometry_review_count: u32,
    pub human_adjustment_count: u32,
    pub mean_manual_center_shift: Option<f32>,
    pub mean_manual_area_change: Option<f32>,
    pub mean_refiner_iou: Option<f32>,
    pub inaccurate_bbox_reason_count: u32,
}

impl GeometryQualitySummary {
    pub fn add_report(&mut self, report: &GeometryQualityReport, needs_review: bool) {
        self.total_candidates = self.total_candidates.saturating_add(1);
        if report.geometry_semantics == GeometrySemantics::CoarseHypothesis {
            self.coarse_geometry_count = self.coarse_geometry_count.saturating_add(1);
        }
        if needs_review && report.has_geometry_issue() {
            self.geometry_review_count = self.geometry_review_count.saturating_add(1);
        }
        if report.has_geometry_issue() {
            self.inaccurate_bbox_reason_count = self.inaccurate_bbox_reason_count.saturating_add(1);
        }
    }

    pub fn add_manual_adjustment(&mut self, center_shift: f32, area_change: f32) {
        self.human_adjustment_count = self.human_adjustment_count.saturating_add(1);
        let count = self.human_adjustment_count;
        push_mean(
            &mut self.mean_manual_center_shift,
            Some(center_shift),
            count,
        );
        push_mean(&mut self.mean_manual_area_change, Some(area_change), count);
    }
}

fn push_mean(mean: &mut Option<f32>, value: Option<f32>, count: u32) {
    let Some(value) = value else { return };
    *mean = Some(match *mean {
        Some(current) if count > 1 => current + (value - current) / count as f32,
        _ => value,
    });
}

#[must_use]
pub fn manual_geometry_metrics(
    before: &AnnotationSnapshot,
    after: &AnnotationSnapshot,
) -> Option<(f32, f32, f32)> {
    let (
        AnnotationValue::BoundingBox { rect: before },
        AnnotationValue::BoundingBox { rect: after },
    ) = (&before.value, &after.value)
    else {
        return None;
    };
    (*before != *after).then(|| {
        (
            center_shift(*before, *after),
            relative_area_change(*before, *after),
            rect_iou(*before, *after),
        )
    })
}

#[must_use]
pub fn manual_geometry_feature_map(
    before: &AnnotationSnapshot,
    after: &AnnotationSnapshot,
) -> BTreeMap<String, f64> {
    manual_geometry_metrics(before, after).map_or_else(BTreeMap::new, |(shift, area, iou)| {
        BTreeMap::from([
            ("manual_center_shift".to_owned(), f64::from(shift)),
            ("manual_area_change".to_owned(), f64::from(area)),
            ("manual_iou".to_owned(), f64::from(iou)),
        ])
    })
}

#[must_use]
pub fn center_shift(left: NormalizedRect, right: NormalizedRect) -> f32 {
    let left = left.center();
    let right = right.center();
    (left.x() - right.x()).hypot(left.y() - right.y())
}

#[must_use]
pub fn relative_area_change(left: NormalizedRect, right: NormalizedRect) -> f32 {
    ((right.area() - left.area()) / left.area()).abs()
}

#[must_use]
pub fn rect_iou(left: NormalizedRect, right: NormalizedRect) -> f32 {
    let intersection = left.intersection_area(right);
    intersection / (left.area() + right.area() - intersection).max(f32::EPSILON)
}

fn touches_image_boundary(rect: NormalizedRect) -> bool {
    rect.x() <= f32::EPSILON
        || rect.y() <= f32::EPSILON
        || rect.x() + rect.width() >= 1.0 - f32::EPSILON
        || rect.y() + rect.height() >= 1.0 - f32::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DetectionScore, DetectionSource, LabelId, ReviewStatus, VisionCapability};

    #[test]
    fn provider_failure_is_not_geometry_evidence() {
        assert_eq!(
            classify_annotation_failure("provider_timeout", "Qwen did not respond"),
            AnnotationFailureClass::ProviderFailure
        );
        assert_eq!(
            classify_annotation_failure("no_candidate", "detector returned no boxes"),
            AnnotationFailureClass::NoCandidate
        );
        assert_eq!(
            classify_annotation_failure("bbox_too_large", "candidate needs tightening"),
            AnnotationFailureClass::GeometryError
        );
        assert_eq!(
            classify_annotation_failure("worker_unreachable", "service not started"),
            AnnotationFailureClass::InfrastructureFailure
        );
        assert_eq!(
            classify_annotation_failure("wrong_label", "shoe is not a ball"),
            AnnotationFailureClass::SemanticError
        );
        assert_eq!(
            classify_annotation_failure("missing_score", "score not provided"),
            AnnotationFailureClass::MissingScore
        );
        assert_eq!(
            classify_annotation_failure("domain_risk", "hard negative"),
            AnnotationFailureClass::DomainRisk
        );
        assert_eq!(
            classify_annotation_failure("artifact_schema", "malformed payload"),
            AnnotationFailureClass::InvalidArtifact
        );
        assert_eq!(
            classify_annotation_failure("cost_budget", "budget exhausted"),
            AnnotationFailureClass::BudgetLimit
        );
    }

    #[test]
    fn vlm_bbox_is_coarse_and_refiner_metrics_are_separate_from_score() {
        let original = NormalizedRect::new(0.1, 0.1, 0.6, 0.6).expect("original");
        let refined = NormalizedRect::new(0.2, 0.2, 0.3, 0.3).expect("refined");
        let mut detection = DetectionArtifactItem::from_source(
            "ball-1",
            None,
            Some("ball".to_owned()),
            Some(LabelId::from("ball")),
            refined,
            DetectionScore::relative(0.99).expect("score"),
            DetectionSource {
                model_id: "vlm".to_owned(),
                capability: VisionCapability::VisionLanguage,
                artifact_id: "detections".to_owned(),
            },
        )
        .expect("detection");
        detection.attributes.insert(
            "geometry_refinement".to_owned(),
            serde_json::json!({"original_bbox": original, "refined_bbox": refined}),
        );
        let report = GeometryQualityReport::from_detection("refined", &detection);
        assert_eq!(
            report.geometry_semantics,
            GeometrySemantics::CoarseHypothesis
        );
        assert!(report.iou_with_refiner.expect("IoU") < 1.0);
        assert_eq!(detection.score.comparable_confidence(), Some(0.99));
    }

    #[test]
    fn manual_bbox_adjustment_produces_geometry_metrics() {
        let snapshot = |rect| AnnotationSnapshot {
            label: Some(LabelId::from("ball")),
            value: AnnotationValue::BoundingBox { rect },
            attributes: BTreeMap::new(),
            confidence: None,
            review_status: ReviewStatus::NeedsReview,
        };
        let before = snapshot(NormalizedRect::new(0.1, 0.1, 0.6, 0.6).expect("before"));
        let after = snapshot(NormalizedRect::new(0.2, 0.2, 0.3, 0.3).expect("after"));
        let features = manual_geometry_feature_map(&before, &after);
        assert!(features["manual_center_shift"] > 0.0);
        assert!(features["manual_area_change"] > 0.0);
        assert!(features["manual_iou"] < 1.0);
    }
}
