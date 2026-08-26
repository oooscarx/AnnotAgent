use std::collections::BTreeMap;

use annotagent_core::{
    Annotation, AnnotationRefiner, AnnotationValue, CoreError, CoreResult, IssueSeverity,
    NormalizedPoint, RefinementContext, RefinementResult, SuggestedAction, ValidationContext,
    ValidationEvidence, ValidationIssue,
};
use annotagent_image_tools::{point_in_polygon, simplify_polyline, white_response};

pub struct FieldContainmentValidator;

impl annotagent_core::AnnotationValidator for FieldContainmentValidator {
    fn id(&self) -> &str {
        "field_containment"
    }

    fn validate(&self, context: &ValidationContext<'_>) -> CoreResult<Vec<ValidationIssue>> {
        let Some(ring) = field_ring(context.related_annotations) else {
            return Ok(vec![ValidationIssue {
                code: "field_region_missing".to_owned(),
                severity: IssueSeverity::Warning,
                annotation_ids: vec![context.candidate.id],
                message: "field containment was downgraded because no field region is available"
                    .to_owned(),
                suggested_action: SuggestedAction::HumanReview,
                evidence: ValidationEvidence::MissingDependency {
                    task_id: "field_region".into(),
                },
            }]);
        };
        let ratio = containment_ratio(&context.candidate.value, ring);
        let required = match context.candidate.value {
            AnnotationValue::Polyline { .. } => 0.75,
            _ => 0.5,
        };
        if ratio + 0.05 >= required {
            Ok(Vec::new())
        } else {
            Ok(vec![ValidationIssue {
                code: "outside_field_region".to_owned(),
                severity: IssueSeverity::Warning,
                annotation_ids: vec![context.candidate.id],
                message: format!(
                    "only {:.0}% of annotation evidence lies on the field",
                    ratio * 100.0
                ),
                suggested_action: SuggestedAction::HumanReview,
                evidence: ValidationEvidence::Geometry {
                    metric: "field_containment_ratio".to_owned(),
                    value: f64::from(ratio),
                    threshold: f64::from(required),
                },
            }])
        }
    }
}

fn containment_ratio(value: &AnnotationValue, ring: &[NormalizedPoint]) -> f32 {
    let points: Vec<NormalizedPoint> = match value {
        AnnotationValue::BoundingBox { rect } => vec![rect.center()],
        AnnotationValue::Keypoints { points } => points.iter().map(|point| point.point).collect(),
        AnnotationValue::Polyline { points } => points.clone(),
        AnnotationValue::Polygon { rings } => rings.first().cloned().unwrap_or_default(),
        _ => return 1.0,
    };
    if points.is_empty() {
        return 0.0;
    }
    let inside = points
        .iter()
        .filter(|point| point_in_polygon(**point, ring))
        .count();
    inside as f32 / points.len() as f32
}

fn field_ring(annotations: &[Annotation]) -> Option<&[NormalizedPoint]> {
    annotations.iter().find_map(|annotation| {
        if annotation
            .label
            .as_ref()
            .is_some_and(|label| label.as_str() == "field")
        {
            if let AnnotationValue::Polygon { rings } = &annotation.value {
                return rings.first().map(Vec::as_slice);
            }
        }
        None
    })
}

#[derive(Debug, Clone)]
pub struct RoboCupFieldLineRefiner {
    pub search_radius_pixels: i32,
    pub samples_per_segment: u32,
    pub white_threshold: f32,
}

impl Default for RoboCupFieldLineRefiner {
    fn default() -> Self {
        Self {
            search_radius_pixels: 12,
            samples_per_segment: 12,
            white_threshold: 0.62,
        }
    }
}

impl AnnotationRefiner for RoboCupFieldLineRefiner {
    fn id(&self) -> &str {
        "robocup_field_line_refiner"
    }

    fn refine(&self, context: &RefinementContext<'_>) -> CoreResult<RefinementResult> {
        let AnnotationValue::Polyline { points } = &context.candidate.value else {
            return Err(CoreError::Refinement(
                "field-line refiner requires a polyline candidate".to_owned(),
            ));
        };
        let ring = field_ring(context.related_annotations);
        let (refined, support, continuity) = refine_points(
            context.image,
            points,
            ring,
            self.search_radius_pixels,
            self.samples_per_segment,
            self.white_threshold,
        )?;
        let mut annotation = context.candidate.clone();
        annotation.value = AnnotationValue::Polyline { points: refined };
        annotation.confidence = Some(
            annotation
                .confidence
                .unwrap_or(0.5)
                .mul_add(0.4, support * 0.4 + continuity * 0.2)
                .clamp(0.0, 1.0),
        );
        let issues = if support < 0.5 {
            vec![ValidationIssue {
                code: "weak_pixel_support".to_owned(),
                severity: IssueSeverity::Warning,
                annotation_ids: vec![annotation.id],
                message: format!("white-pixel support is {:.0}%", support * 100.0),
                suggested_action: SuggestedAction::HumanReview,
                evidence: ValidationEvidence::Geometry {
                    metric: "pixel_support".to_owned(),
                    value: f64::from(support),
                    threshold: 0.5,
                },
            }]
        } else {
            Vec::new()
        };
        Ok(RefinementResult {
            annotation,
            confidence: support * continuity,
            issues,
            summary: format!(
                "refined line with {:.0}% white support and {:.0}% continuity",
                support * 100.0,
                continuity * 100.0
            ),
        })
    }
}

pub(crate) fn refine_points(
    image: &annotagent_core::ImageFrame,
    coarse: &[NormalizedPoint],
    field: Option<&[NormalizedPoint]>,
    radius: i32,
    samples_per_segment: u32,
    threshold: f32,
) -> CoreResult<(Vec<NormalizedPoint>, f32, f32)> {
    if coarse.len() < 2 {
        return Err(CoreError::Refinement(
            "coarse polyline needs at least two points".to_owned(),
        ));
    }
    let width = image.metadata.width as f32;
    let height = image.metadata.height as f32;
    let mut samples = Vec::new();
    let mut supported = 0_u32;
    let mut search_offsets: Vec<i32> = (-radius..=radius).collect();
    search_offsets.sort_by_key(|offset| offset.abs());
    for (segment_index, pair) in coarse.windows(2).enumerate() {
        let start = pair[0];
        let end = pair[1];
        let dx = (end.x() - start.x()) * width;
        let dy = (end.y() - start.y()) * height;
        let length = dx.hypot(dy).max(1.0);
        let normal_x = -dy / length;
        let normal_y = dx / length;
        for index in 0..=samples_per_segment {
            if segment_index > 0 && index == 0 {
                continue;
            }
            let fraction = index as f32 / samples_per_segment as f32;
            let base_x = (start.x() + (end.x() - start.x()) * fraction) * width;
            let base_y = (start.y() + (end.y() - start.y()) * fraction) * height;
            let mut best = (base_x, base_y, 0.0_f32);
            for offset in &search_offsets {
                let x = base_x + normal_x * *offset as f32;
                let y = base_y + normal_y * *offset as f32;
                let candidate = NormalizedPoint::new(
                    (x / width).clamp(0.0, 1.0),
                    (y / height).clamp(0.0, 1.0),
                )?;
                if field.is_some_and(|ring| !point_in_polygon(candidate, ring)) {
                    continue;
                }
                let response = white_response(image, x.round() as i32, y.round() as i32);
                if response > best.2 {
                    best = (x, y, response);
                }
            }
            let has_support = best.2 >= threshold;
            supported += u32::from(has_support);
            samples.push((
                NormalizedPoint::new(
                    (best.0 / width).clamp(0.0, 1.0),
                    (best.1 / height).clamp(0.0, 1.0),
                )?,
                has_support,
            ));
        }
    }
    let resolved = interpolate_unsupported(&samples)?;
    let smoothed = smooth(&resolved)?;
    let simplified = simplify_polyline(&smoothed, 0.0025);
    let sample_count = u32::try_from(samples.len()).unwrap_or(u32::MAX);
    let support = supported as f32 / sample_count.max(1) as f32;
    let continuity = continuity_score(&smoothed);
    Ok((simplified, support, continuity))
}

fn interpolate_unsupported(
    samples: &[(NormalizedPoint, bool)],
) -> CoreResult<Vec<NormalizedPoint>> {
    samples
        .iter()
        .enumerate()
        .map(|(index, (point, supported))| {
            if *supported {
                return Ok(*point);
            }
            let previous = samples[..index]
                .iter()
                .rposition(|(_, supported)| *supported);
            let next = samples[index + 1..]
                .iter()
                .position(|(_, supported)| *supported)
                .map(|offset| index + 1 + offset);
            match (previous, next) {
                (Some(previous), Some(next)) => {
                    let fraction = (index - previous) as f32 / (next - previous) as f32;
                    NormalizedPoint::new(
                        samples[previous].0.x()
                            + (samples[next].0.x() - samples[previous].0.x()) * fraction,
                        samples[previous].0.y()
                            + (samples[next].0.y() - samples[previous].0.y()) * fraction,
                    )
                }
                (Some(previous), None) => Ok(samples[previous].0),
                (None, Some(next)) => Ok(samples[next].0),
                (None, None) => Ok(*point),
            }
        })
        .collect()
}

fn smooth(points: &[NormalizedPoint]) -> CoreResult<Vec<NormalizedPoint>> {
    if points.len() < 3 {
        return Ok(points.to_vec());
    }
    let mut output = vec![points[0]];
    for window in points.windows(3) {
        output.push(NormalizedPoint::new(
            (window[0].x() + 2.0 * window[1].x() + window[2].x()) / 4.0,
            (window[0].y() + 2.0 * window[1].y() + window[2].y()) / 4.0,
        )?);
    }
    output.push(points[points.len() - 1]);
    Ok(output)
}

fn continuity_score(points: &[NormalizedPoint]) -> f32 {
    if points.len() < 2 {
        return 0.0;
    }
    let distances: Vec<f32> = points
        .windows(2)
        .map(|pair| (pair[1].x() - pair[0].x()).hypot(pair[1].y() - pair[0].y()))
        .collect();
    let mean = distances.iter().sum::<f32>() / distances.len() as f32;
    if mean <= f32::EPSILON {
        return 0.0;
    }
    let outliers = distances
        .iter()
        .filter(|distance| **distance > mean * 2.5)
        .count();
    1.0 - outliers as f32 / distances.len() as f32
}

#[derive(Debug, Clone)]
pub struct WhiteLineAppearanceValidator {
    pub minimum_support: f32,
}

impl Default for WhiteLineAppearanceValidator {
    fn default() -> Self {
        Self {
            minimum_support: 0.5,
        }
    }
}

impl annotagent_core::AnnotationValidator for WhiteLineAppearanceValidator {
    fn id(&self) -> &str {
        "white_line_appearance"
    }

    fn validate(&self, context: &ValidationContext<'_>) -> CoreResult<Vec<ValidationIssue>> {
        let (Some(image), AnnotationValue::Polyline { points }) =
            (context.image, &context.candidate.value)
        else {
            return Ok(Vec::new());
        };
        let mut supported = 0_u32;
        let mut sampled = 0_u32;
        for pair in points.windows(2) {
            for index in 0..=16_u32 {
                let fraction = index as f32 / 16.0;
                let point = NormalizedPoint::new(
                    pair[0].x() + (pair[1].x() - pair[0].x()) * fraction,
                    pair[0].y() + (pair[1].y() - pair[0].y()) * fraction,
                )?;
                let (x, y) = point.to_pixel(image.metadata.width, image.metadata.height);
                supported +=
                    u32::from(white_response(image, x.round() as i32, y.round() as i32) >= 0.58);
                sampled += 1;
            }
        }
        let ratio = supported as f32 / sampled.max(1) as f32;
        if ratio >= self.minimum_support {
            Ok(Vec::new())
        } else {
            Ok(vec![line_issue(
                context.candidate,
                "weak_pixel_support",
                "pixel_support",
                ratio,
                self.minimum_support,
            )])
        }
    }
}

#[derive(Debug, Clone)]
pub struct PolylineContinuityValidator {
    pub maximum_jump: f32,
}

impl Default for PolylineContinuityValidator {
    fn default() -> Self {
        Self { maximum_jump: 0.2 }
    }
}

impl annotagent_core::AnnotationValidator for PolylineContinuityValidator {
    fn id(&self) -> &str {
        "polyline_continuity"
    }

    fn validate(&self, context: &ValidationContext<'_>) -> CoreResult<Vec<ValidationIssue>> {
        let AnnotationValue::Polyline { points } = &context.candidate.value else {
            return Ok(Vec::new());
        };
        if points.len() < 2 {
            return Ok(vec![line_issue(
                context.candidate,
                "polyline_too_short",
                "point_count",
                points.len() as f32,
                2.0,
            )]);
        }
        let mut distances: Vec<f32> = points
            .windows(2)
            .map(|pair| (pair[1].x() - pair[0].x()).hypot(pair[1].y() - pair[0].y()))
            .collect();
        let total = distances.iter().sum::<f32>();
        if total < 0.01 {
            return Ok(vec![line_issue(
                context.candidate,
                "polyline_too_short",
                "total_normalized_length",
                total,
                0.01,
            )]);
        }
        if distances.len() < 2 {
            Ok(Vec::new())
        } else {
            distances.sort_by(f32::total_cmp);
            let median = distances[distances.len() / 2].max(0.001);
            let longest = *distances.last().unwrap_or(&0.0);
            if longest > self.maximum_jump && longest > median * 4.0 {
                Ok(vec![line_issue(
                    context.candidate,
                    "polyline_discontinuity",
                    "maximum_normalized_jump",
                    longest,
                    self.maximum_jump.max(median * 4.0),
                )])
            } else {
                Ok(Vec::new())
            }
        }
    }
}

fn line_issue(
    annotation: &Annotation,
    code: &str,
    metric: &str,
    value: f32,
    threshold: f32,
) -> ValidationIssue {
    ValidationIssue {
        code: code.to_owned(),
        severity: IssueSeverity::Warning,
        annotation_ids: vec![annotation.id],
        message: format!("{metric}={value:.3} does not meet threshold {threshold:.3}"),
        suggested_action: SuggestedAction::HumanReview,
        evidence: ValidationEvidence::Geometry {
            metric: metric.to_owned(),
            value: f64::from(value),
            threshold: f64::from(threshold),
        },
    }
}

pub(crate) fn measurements(values: &[(&str, f64)]) -> BTreeMap<String, f64> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), *value))
        .collect()
}
