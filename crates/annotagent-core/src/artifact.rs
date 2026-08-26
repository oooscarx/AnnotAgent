//! Domain-neutral, typed outputs exchanged between vision workflow nodes.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    AnnotationValue, ArtifactId, AttributeValue, CoreResult, ImageId, Keypoint, LabelId,
    MaskEncoding, NormalizedPoint, NormalizedRect, RelationValue, TaskId,
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
    SemanticMask {
        mask: MaskEncoding,
    },
    InstanceMask {
        mask: MaskEncoding,
    },
    Attributes {
        values: BTreeMap<String, AttributeValue>,
    },
    Relations {
        relations: Vec<RelationValue>,
    },
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
            Self::SemanticMask { mask } => AnnotationValue::SemanticMask { mask: mask.clone() },
            Self::InstanceMask { mask } => AnnotationValue::InstanceMask { mask: mask.clone() },
            Self::Attributes { values } => AnnotationValue::Attributes {
                values: values.clone(),
            },
            Self::Relations { relations } => AnnotationValue::Relations {
                relations: relations.clone(),
            },
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
            Self::Attributes { .. } => "attributes",
            Self::Relations { .. } => "relations",
        }
    }

    #[must_use]
    pub fn from_annotation_value(value: &AnnotationValue) -> Self {
        match value {
            AnnotationValue::Classification { labels } => Self::Classification {
                labels: labels.clone(),
            },
            AnnotationValue::BoundingBox { rect } => Self::BoundingBox { rect: *rect },
            AnnotationValue::Keypoints { points } => Self::Keypoints {
                points: points.clone(),
            },
            AnnotationValue::Polyline { points } => Self::Polyline {
                points: points.clone(),
            },
            AnnotationValue::Polygon { rings } => Self::Polygon {
                rings: rings.clone(),
            },
            AnnotationValue::SemanticMask { mask } => Self::SemanticMask { mask: mask.clone() },
            AnnotationValue::InstanceMask { mask } => Self::InstanceMask { mask: mask.clone() },
            AnnotationValue::Attributes { values } => Self::Attributes {
                values: values.clone(),
            },
            AnnotationValue::Relation {
                source,
                predicate,
                target,
            } => Self::Relations {
                relations: vec![RelationValue {
                    source: crate::RelationEndpoint::Annotation(*source),
                    predicate: predicate.clone(),
                    target: crate::RelationEndpoint::Annotation(*target),
                }],
            },
            AnnotationValue::Relations { relations } => Self::Relations {
                relations: relations.clone(),
            },
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
    #[serde(default = "default_artifact_revision")]
    pub revision: u32,
    #[serde(default)]
    pub replaces_artifact_id: Option<ArtifactId>,
    pub created_at: DateTime<Utc>,
}

const fn default_artifact_revision() -> u32 {
    1
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
        if self.revision == 0 {
            return Err(crate::CoreError::Validation(
                "artifact revision must start at one".to_owned(),
            ));
        }
        if self.replaces_artifact_id == Some(self.id) {
            return Err(crate::CoreError::Validation(
                "artifact cannot replace itself".to_owned(),
            ));
        }
        if self.revision > 1 && self.replaces_artifact_id.is_none() {
            return Err(crate::CoreError::Validation(
                "artifact revisions after one must name the artifact they replace".to_owned(),
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
            VisionArtifactValue::Attributes {
                values: BTreeMap::from([(
                    "quality".to_owned(),
                    AttributeValue::String("verified".to_owned()),
                )]),
            },
            VisionArtifactValue::Relations {
                relations: vec![RelationValue {
                    source: crate::RelationEndpoint::Artifact(ArtifactId::new()),
                    predicate: "contains".to_owned(),
                    target: crate::RelationEndpoint::Artifact(ArtifactId::new()),
                }],
            },
        ];
        assert!(values.iter().all(|value| value.validate().is_ok()));
    }

    #[test]
    fn artifact_revision_requires_explicit_replacement_lineage() {
        let original_id = ArtifactId::new();
        let mut artifact = VisionArtifact {
            id: ArtifactId::new(),
            image_id: ImageId::new(),
            task_id: Some(TaskId::from("attributes")),
            label: None,
            role: ArtifactRole::RefinedCandidate,
            value: VisionArtifactValue::Attributes {
                values: BTreeMap::from([("verified".to_owned(), AttributeValue::Boolean(true))]),
            },
            source_node: "generic.refiner".to_owned(),
            confidence: Some(1.0),
            metadata: BTreeMap::new(),
            validation_state: ArtifactValidationState::Valid,
            provenance: ArtifactProvenance {
                input_artifact_ids: vec![original_id],
                ..ArtifactProvenance::default()
            },
            revision: 2,
            replaces_artifact_id: Some(original_id),
            created_at: Utc::now(),
        };
        assert!(artifact.validate().is_ok());
        artifact.replaces_artifact_id = None;
        assert!(artifact.validate().is_err());
        artifact.replaces_artifact_id = Some(artifact.id);
        assert!(artifact.validate().is_err());
    }
}
