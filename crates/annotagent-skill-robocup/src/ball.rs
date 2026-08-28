use std::collections::BTreeMap;

use annotagent_core::{
    AnnotationRefiner, AnnotationValue, CoreError, CoreResult, IssueSeverity, NormalizedPoint,
    NormalizedRect, RefinementContext, RefinementResult, SuggestedAction, ValidationContext,
    ValidationEvidence, ValidationIssue,
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

/// Backward-compatible name used by the original broad `RoboCup` Skill.
pub type BallHardNegativeValidator = RoboCupBallHardNegativeValidator;

/// Tightens a VLM-proposed ball box with deterministic foreground evidence.
///
/// This is deliberately implemented in the `RoboCup` Skill rather than Core. It is a small,
/// dependency-free fallback for installations without a prompted-segmentation worker; the same
/// refiner slot can later be backed by SAM through the versioned HTTP Vision Protocol.
#[derive(Debug, Clone)]
pub struct RoboCupBallForegroundRefiner {
    pub padding_ratio: f32,
    pub minimum_foreground_pixels: usize,
    pub minimum_dense_axis_pixels: usize,
}

impl Default for RoboCupBallForegroundRefiner {
    fn default() -> Self {
        Self {
            padding_ratio: 0.55,
            minimum_foreground_pixels: 24,
            minimum_dense_axis_pixels: 3,
        }
    }
}

impl AnnotationRefiner for RoboCupBallForegroundRefiner {
    fn id(&self) -> &str {
        "ball_foreground_refiner"
    }

    fn refine(&self, context: &RefinementContext<'_>) -> CoreResult<RefinementResult> {
        if context
            .candidate
            .label
            .as_ref()
            .is_none_or(|label| label.as_str() != "ball")
        {
            return Err(CoreError::Refinement(
                "ball foreground refiner requires a ball candidate".to_owned(),
            ));
        }
        let AnnotationValue::BoundingBox { rect: coarse } = context.candidate.value else {
            return Err(CoreError::Refinement(
                "ball foreground refiner requires a bounding-box candidate".to_owned(),
            ));
        };

        match tighten_ball_box(
            context.image,
            coarse,
            self.padding_ratio,
            self.minimum_foreground_pixels,
            self.minimum_dense_axis_pixels,
        )? {
            Some(measurement) => {
                let mut annotation = context.candidate.clone();
                annotation.value = AnnotationValue::BoundingBox {
                    rect: measurement.rect,
                };
                Ok(RefinementResult {
                    annotation,
                    confidence: measurement.quality,
                    issues: Vec::new(),
                    summary: format!(
                        "tightened VLM ball box with local foreground segmentation (quality {:.0}%, foreground {:.0}%)",
                        measurement.quality * 100.0,
                        measurement.foreground_ratio * 100.0,
                    ),
                })
            }
            None => Ok(RefinementResult {
                annotation: context.candidate.clone(),
                confidence: 0.0,
                issues: vec![ValidationIssue {
                    code: "ball_foreground_refiner_fallback".to_owned(),
                    severity: IssueSeverity::Warning,
                    annotation_ids: vec![context.candidate.id],
                    message: "local foreground evidence was inconclusive; the original VLM box was preserved"
                        .to_owned(),
                    suggested_action: SuggestedAction::HumanReview,
                    evidence: ValidationEvidence::Rule {
                        facts: BTreeMap::from([
                            ("refiner".to_owned(), self.id().to_owned()),
                            ("fallback".to_owned(), "original_bbox".to_owned()),
                        ]),
                    },
                }],
                summary: "foreground segmentation was inconclusive; preserved original VLM box"
                    .to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BallForegroundMeasurement {
    rect: NormalizedRect,
    quality: f32,
    foreground_ratio: f32,
}

fn tighten_ball_box(
    image: &annotagent_core::ImageFrame,
    coarse: NormalizedRect,
    padding_ratio: f32,
    minimum_foreground_pixels: usize,
    minimum_dense_axis_pixels: usize,
) -> CoreResult<Option<BallForegroundMeasurement>> {
    image.validate()?;
    let width = image.metadata.width as usize;
    let height = image.metadata.height as usize;
    if width == 0 || height == 0 {
        return Ok(None);
    }

    let image_width = width as f32;
    let image_height = height as f32;
    let coarse_left = coarse.x() * image_width;
    let coarse_top = coarse.y() * image_height;
    let coarse_width = coarse.width() * image_width;
    let coarse_height = coarse.height() * image_height;
    let coarse_right = coarse_left + coarse_width;
    let coarse_bottom = coarse_top + coarse_height;
    let left = (coarse_left - coarse_width * padding_ratio)
        .floor()
        .max(0.0) as usize;
    let top = (coarse_top - coarse_height * padding_ratio)
        .floor()
        .max(0.0) as usize;
    let right = (coarse_right + coarse_width * padding_ratio)
        .ceil()
        .min(image_width) as usize;
    let bottom = (coarse_bottom + coarse_height * padding_ratio)
        .ceil()
        .min(image_height) as usize;
    if right <= left + 2 || bottom <= top + 2 {
        return Ok(None);
    }

    let search_width = right - left;
    let search_height = bottom - top;
    let center_x = coarse_left + coarse_width / 2.0;
    let center_y = coarse_top + coarse_height / 2.0;
    let radius_x = (coarse_width * (0.5 + padding_ratio)).max(2.0);
    let radius_y = (coarse_height * (0.5 + padding_ratio)).max(2.0);
    let mut foreground_mask = vec![false; search_width * search_height];
    let mut row_support = vec![0_usize; search_height];
    let mut foreground_pixels = 0_usize;

    for y in top..bottom {
        for x in left..right {
            let normalized_x = (x as f32 + 0.5 - center_x) / radius_x;
            let normalized_y = (y as f32 + 0.5 - center_y) / radius_y;
            if normalized_x.mul_add(normalized_x, normalized_y * normalized_y) > 1.0 {
                continue;
            }
            let offset = (y * width + x) * 3;
            let red = image.rgb[offset];
            let green = image.rgb[offset + 1];
            let blue = image.rgb[offset + 2];
            if is_field_green(red, green, blue) {
                continue;
            }
            foreground_pixels += 1;
            foreground_mask[(y - top) * search_width + (x - left)] = true;
            row_support[y - top] += 1;
        }
    }
    if foreground_pixels < minimum_foreground_pixels {
        return Ok(None);
    }

    // A painted field line is usually only a few pixels thick. Requiring support along both axes
    // keeps that line from widening the ball box while retaining the dense, roughly round object.
    let row_threshold = minimum_dense_axis_pixels.max((coarse_width * 0.22).ceil() as usize);
    let center_column = center_x.round().clamp(left as f32, (right - 1) as f32) as usize - left;
    let center_row = center_y.round().clamp(top as f32, (bottom - 1) as f32) as usize - top;
    let Some((dense_top, dense_bottom)) = dense_span(&row_support, row_threshold, center_row)
    else {
        return Ok(None);
    };
    // Restrict horizontal support to the selected object-height band. This prevents an unrelated
    // robot foot or sideline background above the ball from widening an oversized VLM proposal.
    let band_top = dense_top.saturating_sub(1);
    let band_bottom = (dense_bottom + 1).min(search_height - 1);
    let band_height = band_bottom - band_top + 1;
    let mut column_support = vec![0_usize; search_width];
    for y in band_top..=band_bottom {
        for (x, support) in column_support.iter_mut().enumerate() {
            *support += usize::from(foreground_mask[y * search_width + x]);
        }
    }
    let column_threshold =
        minimum_dense_axis_pixels.max((band_height as f32 * 0.35).ceil() as usize);
    let Some((dense_left, dense_right)) =
        dense_span(&column_support, column_threshold, center_column)
    else {
        return Ok(None);
    };

    let refined_left = left.saturating_add(dense_left).saturating_sub(2);
    let refined_top = top.saturating_add(dense_top).saturating_sub(2);
    let refined_right = (left + dense_right + 3).min(width);
    let refined_bottom = (top + dense_bottom + 3).min(height);
    let refined_width = refined_right.saturating_sub(refined_left) as f32;
    let refined_height = refined_bottom.saturating_sub(refined_top) as f32;
    if refined_width < 3.0 || refined_height < 3.0 {
        return Ok(None);
    }

    let width_ratio = refined_width / coarse_width.max(1.0);
    let height_ratio = refined_height / coarse_height.max(1.0);
    let aspect_ratio = refined_width / refined_height;
    let refined_center_x = (refined_left + refined_right) as f32 / 2.0;
    let refined_center_y = (refined_top + refined_bottom) as f32 / 2.0;
    let center_shift = ((refined_center_x - center_x) / coarse_width.max(1.0))
        .hypot((refined_center_y - center_y) / coarse_height.max(1.0));
    if !(0.35..=1.6).contains(&width_ratio)
        || !(0.35..=1.6).contains(&height_ratio)
        || !(0.5..=1.9).contains(&aspect_ratio)
        || center_shift > 0.45
    {
        return Ok(None);
    }

    let rect = NormalizedRect::new(
        refined_left as f32 / image_width,
        refined_top as f32 / image_height,
        refined_width / image_width,
        refined_height / image_height,
    )?;
    let foreground_ratio =
        foreground_pixels as f32 / (std::f32::consts::PI * radius_x * radius_y).max(1.0);
    let roundness = 1.0 - ((aspect_ratio - 1.0).abs() / 0.9).clamp(0.0, 1.0);
    let size_consistency =
        1.0 - (((width_ratio - 1.0).abs() + (height_ratio - 1.0).abs()) / 1.2).clamp(0.0, 1.0);
    let density_quality = (foreground_ratio / 0.55).clamp(0.0, 1.0);
    let quality =
        (roundness * 0.35 + size_consistency * 0.35 + density_quality * 0.3).clamp(0.0, 1.0);
    if quality < 0.45 {
        return Ok(None);
    }

    Ok(Some(BallForegroundMeasurement {
        rect,
        quality,
        foreground_ratio,
    }))
}

fn is_field_green(red: u8, green: u8, blue: u8) -> bool {
    let red = i16::from(red);
    let green = i16::from(green);
    let blue = i16::from(blue);
    green >= 38 && green - red >= 7 && green - blue >= 4
}

fn dense_span(support: &[usize], threshold: usize, center: usize) -> Option<(usize, usize)> {
    let dense: Vec<bool> = support.iter().map(|value| *value >= threshold).collect();
    let mut spans = Vec::new();
    let mut index = 0;
    while index < dense.len() {
        if !dense[index] {
            index += 1;
            continue;
        }
        let start = index;
        let mut last_dense = index;
        index += 1;
        while index < dense.len() {
            if dense[index] {
                last_dense = index;
                index += 1;
            } else if index + 1 < dense.len() && dense[index + 1] {
                index += 2;
                last_dense = index - 1;
            } else {
                break;
            }
        }
        spans.push((start, last_dense));
    }
    spans.into_iter().max_by(|left, right| {
        let left_distance = distance_to_span(center, *left);
        let right_distance = distance_to_span(center, *right);
        right_distance
            .cmp(&left_distance)
            .then_with(|| (left.1 - left.0).cmp(&(right.1 - right.0)))
    })
}

fn distance_to_span(point: usize, span: (usize, usize)) -> usize {
    if point < span.0 {
        span.0 - point
    } else {
        point.saturating_sub(span.1)
    }
}

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
