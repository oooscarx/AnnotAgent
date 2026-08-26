//! Domain-neutral annotation values and revision records.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    AnnotationId, AnnotationRevisionId, CoreError, CoreResult, ImageId, LabelId, NormalizedPoint,
    NormalizedRect, RunStepId, TaskId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Classification,
    BoundingBox,
    Keypoints,
    Polyline,
    Polygon,
    InstanceMask,
    Attributes,
    Relations,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keypoint {
    pub name: String,
    pub point: NormalizedPoint,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "encoding", rename_all = "snake_case")]
pub enum MaskEncoding {
    Polygon {
        rings: Vec<Vec<NormalizedPoint>>,
    },
    CocoRle {
        width: u32,
        height: u32,
        counts: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnnotationValue {
    Classification {
        labels: Vec<LabelId>,
    },
    BoundingBox {
        rect: NormalizedRect,
    },
    Keypoints {
        points: Vec<Keypoint>,
    },
    Polyline {
        points: Vec<NormalizedPoint>,
    },
    Polygon {
        rings: Vec<Vec<NormalizedPoint>>,
    },
    InstanceMask {
        mask: MaskEncoding,
    },
    Relation {
        source: AnnotationId,
        predicate: String,
        target: AnnotationId,
    },
}

impl AnnotationValue {
    pub fn validate(&self) -> CoreResult<()> {
        match self {
            Self::Classification { labels } if labels.is_empty() => Err(
                CoreError::InvalidGeometry("classification needs at least one label".to_owned()),
            ),
            Self::Keypoints { points } if points.is_empty() => Err(CoreError::InvalidGeometry(
                "keypoint annotation needs at least one point".to_owned(),
            )),
            Self::Polyline { points } if points.len() < 2 => Err(CoreError::InvalidGeometry(
                "polyline needs at least two points".to_owned(),
            )),
            Self::Polygon { rings }
            | Self::InstanceMask {
                mask: MaskEncoding::Polygon { rings },
            } => validate_rings(rings),
            Self::InstanceMask {
                mask:
                    MaskEncoding::CocoRle {
                        width,
                        height,
                        counts,
                    },
            } if *width == 0 || *height == 0 || counts.trim().is_empty() => {
                Err(CoreError::InvalidGeometry(
                    "COCO RLE needs non-zero dimensions and counts".to_owned(),
                ))
            }
            Self::Relation {
                source,
                predicate,
                target,
            } if source == target || predicate.trim().is_empty() => {
                Err(CoreError::InvalidGeometry(
                    "relation needs distinct endpoints and a predicate".to_owned(),
                ))
            }
            _ => Ok(()),
        }
    }

    #[must_use]
    pub const fn task_kind(&self) -> TaskKind {
        match self {
            Self::Classification { .. } => TaskKind::Classification,
            Self::BoundingBox { .. } => TaskKind::BoundingBox,
            Self::Keypoints { .. } => TaskKind::Keypoints,
            Self::Polyline { .. } => TaskKind::Polyline,
            Self::Polygon { .. } => TaskKind::Polygon,
            Self::InstanceMask { .. } => TaskKind::InstanceMask,
            Self::Relation { .. } => TaskKind::Relations,
        }
    }
}

fn validate_rings(rings: &[Vec<NormalizedPoint>]) -> CoreResult<()> {
    if rings.is_empty() || rings.iter().any(|ring| ring.len() < 3) {
        return Err(CoreError::InvalidGeometry(
            "polygon needs at least one ring with three points".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Boolean(bool),
    StringList(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationSource {
    Model,
    DeterministicTool,
    ModelAndTool,
    Human,
    Imported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Draft,
    AutoAccepted,
    NeedsReview,
    HumanAccepted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AnnotationProvenance {
    pub run_step_id: Option<RunStepId>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tool_names: Vec<String>,
    pub parent_annotation_id: Option<AnnotationId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: AnnotationId,
    pub image_id: ImageId,
    pub task_id: TaskId,
    pub label: Option<LabelId>,
    pub value: AnnotationValue,
    pub attributes: BTreeMap<String, AttributeValue>,
    pub confidence: Option<f32>,
    pub source: AnnotationSource,
    pub review_status: ReviewStatus,
    pub provenance: AnnotationProvenance,
    pub created_at: DateTime<Utc>,
}

impl Annotation {
    pub fn validate(&self) -> CoreResult<()> {
        self.value.validate()?;
        if let Some(confidence) = self.confidence
            && (!confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
        {
            return Err(CoreError::InvalidGeometry(format!(
                "confidence must be finite and within [0, 1], got {confidence}"
            )));
        }
        Ok(())
    }

    #[must_use]
    pub fn snapshot(&self) -> AnnotationSnapshot {
        AnnotationSnapshot {
            label: self.label.clone(),
            value: self.value.clone(),
            attributes: self.attributes.clone(),
            confidence: self.confidence,
            review_status: self.review_status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationSnapshot {
    pub label: Option<LabelId>,
    pub value: AnnotationValue,
    pub attributes: BTreeMap<String, AttributeValue>,
    pub confidence: Option<f32>,
    pub review_status: ReviewStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionActor {
    Human,
    Runtime,
    Import,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationRevision {
    pub revision_id: AnnotationRevisionId,
    pub annotation_id: AnnotationId,
    pub parent_revision_id: Option<AnnotationRevisionId>,
    pub before: Option<AnnotationSnapshot>,
    pub after: Option<AnnotationSnapshot>,
    pub actor: RevisionActor,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl AnnotationRevision {
    pub fn validate(&self) -> CoreResult<()> {
        if self.before == self.after {
            return Err(CoreError::Validation(
                "revision must change, create, or delete an annotation".to_owned(),
            ));
        }
        if self.before.is_none() && self.after.is_none() {
            return Err(CoreError::Validation(
                "revision cannot have both snapshots empty".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub mime_type: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageFrame {
    pub metadata: ImageMetadata,
    /// Interleaved RGB8 pixels, without file container metadata.
    pub rgb: Vec<u8>,
}

impl ImageFrame {
    pub fn validate(&self) -> CoreResult<()> {
        let expected = usize::try_from(self.metadata.width)
            .ok()
            .and_then(|width| {
                usize::try_from(self.metadata.height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(3))
            .ok_or_else(|| CoreError::InvalidGeometry("image dimensions overflow".to_owned()))?;
        if self.rgb.len() != expected {
            return Err(CoreError::InvalidGeometry(format!(
                "RGB byte length mismatch: expected {expected}, got {}",
                self.rgb.len()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, y: f32) -> NormalizedPoint {
        NormalizedPoint::new(x, y).expect("test point")
    }

    #[test]
    fn polyline_and_polygon_invariants() {
        assert!(
            AnnotationValue::Polyline {
                points: vec![point(0.1, 0.1)]
            }
            .validate()
            .is_err()
        );
        assert!(
            AnnotationValue::Polygon {
                rings: vec![vec![point(0.0, 0.0), point(1.0, 0.0), point(0.0, 1.0)]]
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn coco_mask_needs_dimensions_and_counts() {
        let value = AnnotationValue::InstanceMask {
            mask: MaskEncoding::CocoRle {
                width: 0,
                height: 10,
                counts: String::new(),
            },
        };
        assert!(value.validate().is_err());
    }

    #[test]
    fn no_op_revision_is_rejected() {
        let snapshot = AnnotationSnapshot {
            label: None,
            value: AnnotationValue::Classification {
                labels: vec![LabelId::from("ok")],
            },
            attributes: BTreeMap::new(),
            confidence: Some(1.0),
            review_status: ReviewStatus::HumanAccepted,
        };
        let revision = AnnotationRevision {
            revision_id: AnnotationRevisionId::new(),
            annotation_id: AnnotationId::new(),
            parent_revision_id: None,
            before: Some(snapshot.clone()),
            after: Some(snapshot),
            actor: RevisionActor::Human,
            reason: None,
            created_at: Utc::now(),
        };
        assert!(revision.validate().is_err());
    }
}
