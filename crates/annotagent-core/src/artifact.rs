//! Domain-neutral, typed outputs exchanged between vision workflow nodes.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    AnnotationValue, ArtifactId, ArtifactRef, AttributeValue, CoreResult, ImageId, Keypoint,
    LabelId, MaskEncoding, NormalizedPoint, NormalizedRect, PipelineArtifact, ProjectId,
    RelationValue, RunId, TaskId,
};

pub const ARTIFACT_ENVELOPE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArtifactEnvelopeRef {
    pub artifact_id: String,
    #[serde(default)]
    pub item_id: Option<String>,
}

impl From<&ArtifactRef> for ArtifactEnvelopeRef {
    fn from(reference: &ArtifactRef) -> Self {
        Self {
            artifact_id: reference.artifact_id.clone(),
            item_id: reference.item_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", content = "payload", rename_all = "snake_case")]
pub enum ArtifactPayload {
    Vision(VisionArtifact),
    Pipeline(PipelineArtifact),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactEnvelope {
    pub schema_version: u32,
    pub artifact_id: String,
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub image_id: ImageId,
    pub node_id: String,
    pub payload: ArtifactPayload,
    #[serde(default)]
    pub parents: Vec<ArtifactEnvelopeRef>,
    pub provenance: ArtifactProvenance,
    pub created_at: DateTime<Utc>,
    pub cache_key: Option<String>,
}

impl ArtifactEnvelope {
    #[must_use]
    pub fn from_vision(
        project_id: ProjectId,
        run_id: RunId,
        node_id: impl Into<String>,
        artifact: VisionArtifact,
        cache_key: Option<String>,
    ) -> Self {
        let artifact_id = artifact.id.to_string();
        let image_id = artifact.image_id;
        let parents = artifact
            .provenance
            .input_artifact_ids
            .iter()
            .map(|id| ArtifactEnvelopeRef {
                artifact_id: id.to_string(),
                item_id: None,
            })
            .collect();
        let provenance = artifact.provenance.clone();
        let created_at = artifact.created_at;
        Self {
            schema_version: ARTIFACT_ENVELOPE_SCHEMA_VERSION,
            artifact_id,
            project_id,
            run_id,
            image_id,
            node_id: node_id.into(),
            payload: ArtifactPayload::Vision(artifact),
            parents,
            provenance,
            created_at,
            cache_key,
        }
    }

    #[must_use]
    pub fn from_pipeline(
        project_id: ProjectId,
        run_id: RunId,
        node_id: impl Into<String>,
        artifact: PipelineArtifact,
        provenance: ArtifactProvenance,
        cache_key: Option<String>,
    ) -> Self {
        let artifact_id = artifact.reference().artifact_id.clone();
        let image_id = artifact.image_id();
        let parents = pipeline_parents(&artifact);
        Self {
            schema_version: ARTIFACT_ENVELOPE_SCHEMA_VERSION,
            artifact_id,
            project_id,
            run_id,
            image_id,
            node_id: node_id.into(),
            payload: ArtifactPayload::Pipeline(artifact),
            parents,
            provenance,
            created_at: Utc::now(),
            cache_key,
        }
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != ARTIFACT_ENVELOPE_SCHEMA_VERSION {
            return Err(crate::CoreError::Validation(format!(
                "unsupported artifact envelope schema version {}",
                self.schema_version
            )));
        }
        if self.node_id.trim().is_empty() || self.artifact_id.trim().is_empty() {
            return Err(crate::CoreError::Validation(
                "artifact envelope requires node_id and artifact_id".to_owned(),
            ));
        }
        let (payload_id, payload_image) = match &self.payload {
            ArtifactPayload::Vision(artifact) => {
                artifact.validate()?;
                (artifact.id.to_string(), artifact.image_id)
            }
            ArtifactPayload::Pipeline(artifact) => {
                artifact.validate().map_err(crate::CoreError::Validation)?;
                (
                    artifact.reference().artifact_id.clone(),
                    artifact.image_id(),
                )
            }
        };
        if payload_id != self.artifact_id || payload_image != self.image_id {
            return Err(crate::CoreError::Validation(
                "artifact envelope scope does not match its payload".to_owned(),
            ));
        }
        let mut parents = std::collections::BTreeSet::new();
        for parent in &self.parents {
            if parent.artifact_id == self.artifact_id && parent.item_id.is_none() {
                return Err(crate::CoreError::Validation(
                    "artifact envelope cannot be its own parent".to_owned(),
                ));
            }
            if !parents.insert(parent) {
                return Err(crate::CoreError::Validation(
                    "artifact envelope parents must be unique".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

fn pipeline_parents(artifact: &PipelineArtifact) -> Vec<ArtifactEnvelopeRef> {
    let references = match artifact {
        PipelineArtifact::Image(_) | PipelineArtifact::DetectionSet(_) => Vec::new(),
        PipelineArtifact::CandidateClusterSet(candidates) => {
            candidates.source_detection_sets.iter().collect()
        }
        PipelineArtifact::CropSet(crops) => vec![&crops.source_detections],
        PipelineArtifact::ClassificationSet(classifications) => classifications
            .classifications
            .iter()
            .flat_map(|classification| {
                std::iter::once(&classification.subject).chain(classification.parent.iter())
            })
            .collect(),
        PipelineArtifact::AnnotationCandidateSet(candidates) => candidates
            .candidates
            .iter()
            .flat_map(|candidate| {
                std::iter::once(&candidate.subject).chain(candidate.evidence.iter())
            })
            .collect(),
    };
    references
        .into_iter()
        .map(ArtifactEnvelopeRef::from)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

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
    use crate::{
        ArtifactKind, CandidateAgreement, CandidateCluster, CandidateClusterSetArtifact,
        DetectionEvidence, DetectionScore,
    };

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

    #[test]
    fn strong_envelope_validates_scope_lineage_and_model_safe_reference() {
        let parent = ArtifactId::new();
        let artifact = VisionArtifact {
            id: ArtifactId::new(),
            image_id: ImageId::new(),
            task_id: Some(TaskId::from("object")),
            label: Some(LabelId::from("target")),
            role: ArtifactRole::Candidate,
            value: VisionArtifactValue::BoundingBox {
                rect: NormalizedRect::new(0.1, 0.2, 0.3, 0.4).expect("rect"),
            },
            source_node: "detector".to_owned(),
            confidence: Some(0.9),
            metadata: BTreeMap::new(),
            validation_state: ArtifactValidationState::Unvalidated,
            provenance: ArtifactProvenance {
                input_artifact_ids: vec![parent],
                ..ArtifactProvenance::default()
            },
            revision: 1,
            replaces_artifact_id: None,
            created_at: Utc::now(),
        };
        let mut envelope = ArtifactEnvelope::from_vision(
            ProjectId::new(),
            RunId::new(),
            "detector",
            artifact,
            Some("sha256:fixture".to_owned()),
        );
        assert!(envelope.validate().is_ok());
        assert_eq!(envelope.parents[0].artifact_id, parent.to_string());
        envelope.image_id = ImageId::new();
        assert!(envelope.validate().is_err());
    }

    #[test]
    fn candidate_cluster_envelope_retains_both_detection_set_parents() {
        let source = |artifact_id: &str| ArtifactRef {
            artifact_id: artifact_id.to_owned(),
            source_node: format!("{artifact_id}-node"),
            port: "detections".to_owned(),
            artifact_type: ArtifactKind::DetectionSet,
            item_id: None,
        };
        let bbox = NormalizedRect::new(0.1, 0.2, 0.3, 0.4).expect("bbox");
        let artifact = PipelineArtifact::CandidateClusterSet(CandidateClusterSetArtifact {
            reference: ArtifactRef {
                artifact_id: "clusters".to_owned(),
                source_node: "matcher".to_owned(),
                port: "candidates".to_owned(),
                artifact_type: ArtifactKind::CandidateClusterSet,
                item_id: None,
            },
            image_id: ImageId::new(),
            source_detection_sets: vec![source("set-a"), source("set-b")],
            candidates: vec![CandidateCluster {
                id: "candidate".to_owned(),
                target_label: LabelId::from("ball"),
                representative_bbox: bbox,
                members: vec![
                    DetectionEvidence {
                        source_model_id: "model-a".to_owned(),
                        source_artifact_id: "set-a".to_owned(),
                        bbox,
                        score: DetectionScore::relative(0.8).expect("score"),
                        query_id: None,
                        model_label: Some("sports ball".to_owned()),
                        raw_output_ref: None,
                    },
                    DetectionEvidence {
                        source_model_id: "model-b".to_owned(),
                        source_artifact_id: "set-b".to_owned(),
                        bbox,
                        score: DetectionScore::not_provided(),
                        query_id: Some("target object".to_owned()),
                        model_label: None,
                        raw_output_ref: None,
                    },
                ],
                agreement: CandidateAgreement::MultiSourceAgreement {
                    minimum_iou: 1.0,
                    mean_iou: 1.0,
                },
            }],
        });
        let envelope = ArtifactEnvelope::from_pipeline(
            ProjectId::new(),
            RunId::new(),
            "matcher",
            artifact,
            ArtifactProvenance::default(),
            None,
        );
        envelope.validate().expect("valid cluster envelope");
        assert_eq!(
            envelope.parents,
            vec![
                ArtifactEnvelopeRef {
                    artifact_id: "set-a".to_owned(),
                    item_id: None,
                },
                ArtifactEnvelopeRef {
                    artifact_id: "set-b".to_owned(),
                    item_id: None,
                },
            ]
        );
    }
}
