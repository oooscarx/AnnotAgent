//! Domain-neutral, typed outputs exchanged between vision workflow nodes.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    AnnotationValue, ArtifactId, CoreResult, ImageId, Keypoint, LabelId, MaskEncoding,
    NormalizedPoint, NormalizedRect, TaskId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    Evidence,
    Candidate,
    RefinedCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactValidationState {
    Unvalidated,
    Valid,
    Invalid,
    NeedsReview,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ArtifactProvenance {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tool: Option<String>,
    pub request_id: Option<String>,
    pub model_digest: Option<String>,
    pub input_artifact_ids: Vec<ArtifactId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VisionArtifactValue {
    Classification { labels: Vec<LabelId> },
    BoundingBox { rect: NormalizedRect },
    Keypoints { points: Vec<Keypoint> },
    Polyline { points: Vec<NormalizedPoint> },
    Polygon { rings: Vec<Vec<NormalizedPoint>> },
    SemanticMask { mask: MaskEncoding },
    InstanceMask { mask: MaskEncoding },
}

impl VisionArtifactValue {
    pub fn validate(&self) -> CoreResult<()> {
        self.as_annotation_value().validate()
    }

    #[must_use]
    pub fn as_annotation_value(&self) -> AnnotationValue {
        match self {
            Self::Classification { labels } => AnnotationValue::Classification {
                labels: labels.clone(),
            },
            Self::BoundingBox { rect } => AnnotationValue::BoundingBox { rect: *rect },
            Self::Keypoints { points } => AnnotationValue::Keypoints {
                points: points.clone(),
            },
            Self::Polyline { points } => AnnotationValue::Polyline {
                points: points.clone(),
            },
            Self::Polygon { rings } => AnnotationValue::Polygon {
                rings: rings.clone(),
            },
            Self::SemanticMask { mask } | Self::InstanceMask { mask } => {
                AnnotationValue::InstanceMask { mask: mask.clone() }
            }
        }
    }

    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Classification { .. } => "classification",
            Self::BoundingBox { .. } => "bounding_box",
            Self::Keypoints { .. } => "keypoints",
            Self::Polyline { .. } => "polyline",
            Self::Polygon { .. } => "polygon",
            Self::SemanticMask { .. } => "semantic_mask",
            Self::InstanceMask { .. } => "instance_mask",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisionArtifact {
    pub id: ArtifactId,
    pub image_id: ImageId,
    pub task_id: Option<TaskId>,
    pub label: Option<LabelId>,
    pub role: ArtifactRole,
    pub value: VisionArtifactValue,
    pub source_node: String,
    pub confidence: Option<f32>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub validation_state: ArtifactValidationState,
    pub provenance: ArtifactProvenance,
    pub created_at: DateTime<Utc>,
}

impl VisionArtifact {
    pub fn validate(&self) -> CoreResult<()> {
        self.value.validate()?;
        if self.source_node.trim().is_empty() {
            return Err(crate::CoreError::Validation(
                "artifact source_node cannot be empty".to_owned(),
            ));
        }
        if let Some(confidence) = self.confidence
            && (!confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
        {
            return Err(crate::CoreError::Validation(
                "artifact confidence must be finite and within [0,1]".to_owned(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn reference(&self) -> ArtifactReference {
        ArtifactReference {
            artifact_id: self.id,
            kind: self.value.kind_name().to_owned(),
            source_node: self.source_node.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReference {
    pub artifact_id: ArtifactId,
    pub kind: String,
    pub source_node: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_required_artifact_shape_is_typed_and_validated() {
        let values = [
            VisionArtifactValue::Classification {
                labels: vec![LabelId::from("field")],
            },
            VisionArtifactValue::BoundingBox {
                rect: NormalizedRect::new(0.1, 0.1, 0.2, 0.2).expect("rect"),
            },
            VisionArtifactValue::Keypoints {
                points: vec![Keypoint {
                    name: "mark".to_owned(),
                    point: NormalizedPoint::new(0.5, 0.5).expect("point"),
                    visible: true,
                }],
            },
            VisionArtifactValue::Polyline {
                points: vec![
                    NormalizedPoint::new(0.1, 0.5).expect("point"),
                    NormalizedPoint::new(0.9, 0.5).expect("point"),
                ],
            },
            VisionArtifactValue::Polygon {
                rings: vec![vec![
                    NormalizedPoint::new(0.1, 0.1).expect("point"),
                    NormalizedPoint::new(0.9, 0.1).expect("point"),
                    NormalizedPoint::new(0.9, 0.9).expect("point"),
                ]],
            },
            VisionArtifactValue::SemanticMask {
                mask: MaskEncoding::CocoRle {
                    width: 2,
                    height: 2,
                    counts: "4".to_owned(),
                },
            },
            VisionArtifactValue::InstanceMask {
                mask: MaskEncoding::CocoRle {
                    width: 2,
                    height: 2,
                    counts: "4".to_owned(),
                },
            },
        ];
        assert!(values.iter().all(|value| value.validate().is_ok()));
    }
}
