//! Domain-neutral bounded localization-recovery policies and scale evidence.

use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult, NormalizedRect};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegionExpansionPolicy {
    ImageFraction {
        x: f32,
        y: f32,
    },
    RelativeToCandidate {
        width_factor: f32,
        height_factor: f32,
        minimum_width_px: u32,
        minimum_height_px: u32,
        maximum_image_fraction: f32,
    },
    DirectionAware {
        base_factor: f32,
        offset_x: f32,
        offset_y: f32,
        minimum_size_px: u32,
    },
}

impl RegionExpansionPolicy {
    pub fn expand(
        &self,
        candidate: NormalizedRect,
        image_width: u32,
        image_height: u32,
    ) -> CoreResult<NormalizedRect> {
        if image_width == 0 || image_height == 0 {
            return Err(CoreError::InvalidGeometry(
                "region expansion requires non-zero image dimensions".to_owned(),
            ));
        }
        let (width, height, offset_x, offset_y) = match *self {
            Self::ImageFraction { x, y } => {
                validate_non_negative_finite(x, "image-fraction x")?;
                validate_non_negative_finite(y, "image-fraction y")?;
                (
                    candidate.width() + x * 2.0,
                    candidate.height() + y * 2.0,
                    0.0,
                    0.0,
                )
            }
            Self::RelativeToCandidate {
                width_factor,
                height_factor,
                minimum_width_px,
                minimum_height_px,
                maximum_image_fraction,
            } => {
                validate_positive_finite(width_factor, "candidate width factor")?;
                validate_positive_finite(height_factor, "candidate height factor")?;
                validate_positive_finite(maximum_image_fraction, "maximum image fraction")?;
                if minimum_width_px == 0 || minimum_height_px == 0 {
                    return Err(CoreError::InvalidGeometry(
                        "minimum search-region dimensions must be non-zero".to_owned(),
                    ));
                }
                let maximum = maximum_image_fraction.min(1.0);
                (
                    (candidate.width() * width_factor)
                        .max(minimum_width_px as f32 / image_width as f32)
                        .min(maximum),
                    (candidate.height() * height_factor)
                        .max(minimum_height_px as f32 / image_height as f32)
                        .min(maximum),
                    0.0,
                    0.0,
                )
            }
            Self::DirectionAware {
                base_factor,
                offset_x,
                offset_y,
                minimum_size_px,
            } => {
                validate_positive_finite(base_factor, "direction-aware factor")?;
                validate_finite(offset_x, "direction-aware x offset")?;
                validate_finite(offset_y, "direction-aware y offset")?;
                if minimum_size_px == 0 {
                    return Err(CoreError::InvalidGeometry(
                        "minimum search-region size must be non-zero".to_owned(),
                    ));
                }
                (
                    (candidate.width() * base_factor)
                        .max(minimum_size_px as f32 / image_width as f32)
                        .min(1.0),
                    (candidate.height() * base_factor)
                        .max(minimum_size_px as f32 / image_height as f32)
                        .min(1.0),
                    offset_x * candidate.width(),
                    offset_y * candidate.height(),
                )
            }
        };
        centered_clamped_region(
            candidate,
            width.min(1.0),
            height.min(1.0),
            offset_x,
            offset_y,
        )
    }
}

fn centered_clamped_region(
    candidate: NormalizedRect,
    width: f32,
    height: f32,
    offset_x: f32,
    offset_y: f32,
) -> CoreResult<NormalizedRect> {
    let center = candidate.center();
    let x = (center.x() + offset_x - width / 2.0).clamp(0.0, 1.0 - width);
    let y = (center.y() + offset_y - height / 2.0).clamp(0.0, 1.0 - height);
    NormalizedRect::new(x, y, width, height)
}

fn validate_finite(value: f32, name: &str) -> CoreResult<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(CoreError::InvalidGeometry(format!("{name} must be finite")))
    }
}

fn validate_non_negative_finite(value: f32, name: &str) -> CoreResult<()> {
    validate_finite(value, name)?;
    if value < 0.0 {
        return Err(CoreError::InvalidGeometry(format!(
            "{name} must be non-negative"
        )));
    }
    Ok(())
}

fn validate_positive_finite(value: f32, name: &str) -> CoreResult<()> {
    validate_finite(value, name)?;
    if value <= 0.0 {
        return Err(CoreError::InvalidGeometry(format!(
            "{name} must be positive"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetScaleSource {
    CurrentCandidate,
    HistoricalAnnotation,
    ProjectStatistics,
    DomainSkillPrior,
    UserSetting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetSizeBucket {
    Tiny,
    Small,
    Medium,
    Large,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetScaleProfile {
    pub source: TargetScaleSource,
    pub normalized_area: Option<f32>,
    pub minimum_dimension_px: Option<f32>,
    pub size_bucket: TargetSizeBucket,
}

impl TargetScaleProfile {
    #[must_use]
    pub fn from_candidate(
        candidate: NormalizedRect,
        image_width: u32,
        image_height: u32,
        tiny_max_dimension_px: f32,
        small_max_dimension_px: f32,
        medium_max_dimension_px: f32,
    ) -> Self {
        let minimum_dimension_px =
            (candidate.width() * image_width as f32).min(candidate.height() * image_height as f32);
        let size_bucket = if minimum_dimension_px <= tiny_max_dimension_px {
            TargetSizeBucket::Tiny
        } else if minimum_dimension_px <= small_max_dimension_px {
            TargetSizeBucket::Small
        } else if minimum_dimension_px <= medium_max_dimension_px {
            TargetSizeBucket::Medium
        } else {
            TargetSizeBucket::Large
        };
        Self {
            source: TargetScaleSource::CurrentCandidate,
            normalized_area: Some(candidate.area()),
            minimum_dimension_px: Some(minimum_dimension_px),
            size_bucket,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_relative_region_obeys_pixel_minimum_and_clamps_at_edges() {
        let policy = RegionExpansionPolicy::RelativeToCandidate {
            width_factor: 4.0,
            height_factor: 4.0,
            minimum_width_px: 96,
            minimum_height_px: 96,
            maximum_image_fraction: 0.5,
        };
        let candidate = NormalizedRect::new(0.92, 0.01, 0.03, 0.03).expect("candidate");
        let region = policy.expand(candidate, 544, 448).expect("expanded region");
        assert!((region.width() * 544.0 - 96.0).abs() < 0.001);
        assert!((region.height() * 448.0 - 96.0).abs() < 0.001);
        assert!(region.x() + region.width() <= 1.0 + f32::EPSILON);
        assert!(region.y() <= f32::EPSILON);
    }

    #[test]
    fn target_scale_uses_observed_pixels_not_a_model_confidence() {
        let candidate =
            NormalizedRect::new(0.5, 0.4, 16.0 / 544.0, 16.0 / 448.0).expect("candidate");
        let profile = TargetScaleProfile::from_candidate(candidate, 544, 448, 20.0, 64.0, 160.0);
        assert_eq!(profile.minimum_dimension_px, Some(16.0));
        assert_eq!(profile.size_bucket, TargetSizeBucket::Tiny);
    }
}
