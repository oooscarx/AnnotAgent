use annotagent_core::{
    AnnotationValue, CoreResult, IssueSeverity, NormalizedPoint, SuggestedAction,
    ValidationContext, ValidationEvidence, ValidationIssue,
};
use annotagent_image_tools::{color_statistics, point_segment_distance};

use crate::field::measurements;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoboCupBallIssueCode {
    PossibleWhiteShoe,
    PossibleWhiteSock,
    PossiblePenaltyMark,
    PossibleFieldLineIntersection,
    MissedSmallBall,
    DuplicateBall,
    InaccurateBoundingBox,
    OutsideField,
    MissingFieldEvidence,
}

#[derive(Debug, Clone)]
pub struct RoboCupBallHardNegativeValidator {
    pub lower_body_margin: f32,
    pub point_distance: f32,
    pub line_distance: f32,
}

impl Default for RoboCupBallHardNegativeValidator {
    fn default() -> Self {
        Self {
            lower_body_margin: 0.035,
            point_distance: 0.035,
            line_distance: 0.018,
        }
    }
}

impl annotagent_core::AnnotationValidator for RoboCupBallHardNegativeValidator {
    fn id(&self) -> &str {
        "ball_hard_negative"
    }

    fn validate(&self, context: &ValidationContext<'_>) -> CoreResult<Vec<ValidationIssue>> {
        if context
            .candidate
            .label
            .as_ref()
            .is_none_or(|label| label.as_str() != "ball")
        {
            return Ok(Vec::new());
        }
        let AnnotationValue::BoundingBox { rect } = context.candidate.value else {
            return Ok(Vec::new());
        };
        let center = rect.center();
        let white_ratio = context
            .image
            .and_then(|image| color_statistics(image, rect).ok())
            .map_or(0.0, |statistics| statistics.white_ratio);
        let mut issues = Vec::new();
        let mut lower_body_overlap = 0.0_f32;
        for annotation in context.related_annotations {
            if annotation
                .label
                .as_ref()
                .is_some_and(|label| label.as_str() == "robot")
                && let AnnotationValue::BoundingBox { rect: robot } = annotation.value
            {
                let overlap = rect.intersection_area(robot) / rect.area().max(f32::EPSILON);
                let robot_bottom = robot.y() + robot.height();
                let near_feet = center.x() >= robot.x() - self.lower_body_margin
                    && center.x() <= robot.x() + robot.width() + self.lower_body_margin
                    && center.y() >= robot.y() + robot.height() * 0.62 - self.lower_body_margin
                    && center.y() <= robot_bottom + self.lower_body_margin;
                if overlap > 0.05 || near_feet {
                    lower_body_overlap = lower_body_overlap.max(overlap.max(0.5));
                }
            }
        }
        if lower_body_overlap > 0.0 && white_ratio >= 0.25 {
            issues.push(risk_issue(
                context,
                "possible_white_shoe",
                "white candidate overlaps or lies next to a robot's lower body",
                "ball_candidate",
                &[
                    ("lower_body_risk", f64::from(lower_body_overlap)),
                    ("white_ratio", f64::from(white_ratio)),
                ],
            ));
        }

        let duplicate_overlap = context
            .related_annotations
            .iter()
            .filter(|annotation| {
                annotation
                    .label
                    .as_ref()
                    .is_some_and(|label| label.as_str() == "ball")
                    && annotation.id != context.candidate.id
            })
            .filter_map(|annotation| match annotation.value {
                AnnotationValue::BoundingBox { rect: other } => {
                    let intersection = rect.intersection_area(other);
                    let union = rect.area() + other.area() - intersection;
                    (union > f32::EPSILON).then_some(intersection / union)
                }
                _ => None,
            })
            .max_by(f32::total_cmp);
        if duplicate_overlap.is_some_and(|overlap| overlap >= 0.65) {
            issues.push(risk_issue(
                context,
                "duplicate_ball",
                "candidate substantially overlaps an existing ball annotation",
                "ball_candidate",
                &[(
                    "intersection_over_union",
                    f64::from(duplicate_overlap.unwrap_or_default()),
                )],
            ));
        }

        let penalty_distance = context
            .related_annotations
            .iter()
            .filter(|annotation| {
                annotation
                    .label
                    .as_ref()
                    .is_some_and(|label| label.as_str() == "penalty_mark")
            })
            .filter_map(|annotation| match &annotation.value {
                AnnotationValue::Keypoints { points } => points
                    .iter()
                    .map(|point| distance(center, point.point))
                    .min_by(f32::total_cmp),
                _ => None,
            })
            .min_by(f32::total_cmp);
        if penalty_distance.is_some_and(|distance| distance <= self.point_distance) {
            issues.push(risk_issue(
                context,
                "possible_penalty_mark",
                "ball candidate is too close to an existing penalty mark",
                "ball_candidate",
                &[(
                    "penalty_distance",
                    f64::from(penalty_distance.unwrap_or_default()),
                )],
            ));
        }

        let line_distance = context
            .related_annotations
            .iter()
            .filter_map(|annotation| match &annotation.value {
                AnnotationValue::Polyline { points } => Some(points),
                _ => None,
            })
            .flat_map(|points| points.windows(2))
            .map(|pair| point_segment_distance(center, pair[0], pair[1]))
            .min_by(f32::total_cmp);
        if line_distance.is_some_and(|distance| distance <= self.line_distance)
            && white_ratio >= 0.3
        {
            issues.push(risk_issue(
                context,
                "possible_field_line_intersection",
                "small white candidate lies on a field-line segment",
                "ball_candidate",
                &[(
                    "field_line_distance",
                    f64::from(line_distance.unwrap_or_default()),
                )],
            ));
        }

        let aspect_ratio = rect.width() / rect.height();
        if !(0.55..=1.8).contains(&aspect_ratio) || rect.area() > 0.035 {
            issues.push(risk_issue(
                context,
                "unlikely_ball_geometry",
                "candidate shape or relative area is unusual for a ball",
                "ball_candidate",
                &[
                    ("aspect_ratio", f64::from(aspect_ratio)),
                    ("relative_area", f64::from(rect.area())),
                ],
            ));
        }
        if context.correction_risk >= 0.2 {
            issues.push(risk_issue(
                context,
                "frequent_ball_correction",
                "recent project corrections raise the risk for this label",
                "correction_memory",
                &[("recent_frequency", f64::from(context.correction_risk))],
            ));
        }
        Ok(issues)
    }
}

/// Backward-compatible name used by the original broad RoboCup Skill.
pub type BallHardNegativeValidator = RoboCupBallHardNegativeValidator;

#[derive(Debug, Clone, Default)]
pub struct RoboCupBallFieldRelationValidator;

impl annotagent_core::AnnotationValidator for RoboCupBallFieldRelationValidator {
    fn id(&self) -> &str {
        "robocup_ball_field_relation"
    }

    fn validate(&self, context: &ValidationContext<'_>) -> CoreResult<Vec<ValidationIssue>> {
        if context
            .candidate
            .label
            .as_ref()
            .is_none_or(|label| label.as_str() != "ball")
        {
            return Ok(Vec::new());
        }
        let AnnotationValue::BoundingBox { rect } = context.candidate.value else {
            return Ok(Vec::new());
        };
        let field_ring = context
            .related_annotations
            .iter()
            .find(|annotation| {
                annotation
                    .label
                    .as_ref()
                    .is_some_and(|label| label.as_str() == "field")
            })
            .and_then(|annotation| match &annotation.value {
                AnnotationValue::Polygon { rings } => rings.first(),
                _ => None,
            });
        let Some(field_ring) = field_ring else {
            return Ok(vec![risk_issue(
                context,
                "missing_field_evidence",
                "field geometry is unavailable; field relation remains unverified",
                "field_relation",
                &[],
            )]);
        };
        if point_in_ring(rect.center(), field_ring) {
            Ok(Vec::new())
        } else {
            Ok(vec![risk_issue(
                context,
                "ball_outside_field",
                "ball center lies outside the available field polygon",
                "field_relation",
                &[],
            )])
        }
    }
}

fn point_in_ring(point: NormalizedPoint, ring: &[NormalizedPoint]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = ring[ring.len() - 1];
    for &current in ring {
        let crosses = (current.y() > point.y()) != (previous.y() > point.y());
        if crosses {
            let denominator = previous.y() - current.y();
            if denominator.abs() > f32::EPSILON {
                let intersection_x = (previous.x() - current.x()) * (point.y() - current.y())
                    / denominator
                    + current.x();
                if point.x() < intersection_x {
                    inside = !inside;
                }
            }
        }
        previous = current;
    }
    inside
}

fn distance(left: NormalizedPoint, right: NormalizedPoint) -> f32 {
    (left.x() - right.x()).hypot(left.y() - right.y())
}

fn risk_issue(
    context: &ValidationContext<'_>,
    code: &str,
    message: &str,
    region: &str,
    values: &[(&str, f64)],
) -> ValidationIssue {
    ValidationIssue {
        code: code.to_owned(),
        severity: IssueSeverity::Warning,
        annotation_ids: vec![context.candidate.id],
        message: message.to_owned(),
        suggested_action: SuggestedAction::Retry,
        evidence: ValidationEvidence::ImageStatistics {
            region: region.to_owned(),
            measurements: measurements(values),
        },
    }
}
