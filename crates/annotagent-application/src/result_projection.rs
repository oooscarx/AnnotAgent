use std::collections::{BTreeMap, BTreeSet};

use annotagent_core::{
    AnnotationFailureClass, ArtifactId, ArtifactRef, ArtifactValidationState,
    CandidateGeometryQualityReport, FinalCandidateProjection, LineageStageProjection,
    PipelineArtifact, ResultExplanation, ResultLineageStage, ResultProjection, SampleTestOutcome,
    SampleTestOutcomeStatus, VisionArtifactValue, WorkflowDraft, WorkflowNodeKind,
};
use annotagent_runtime::{DagNodeStatus, DagRunResult, DagRunStatus};
use uuid::Uuid;

pub(crate) fn project_sandbox_result(
    draft: &WorkflowDraft,
    result: &DagRunResult,
    historical_geometry_correction_rate: Option<f32>,
) -> ResultProjection {
    let terminal_artifacts = terminal_artifacts(draft, result);
    let terminal_refs = terminal_artifacts
        .iter()
        .map(|artifact| artifact.reference().artifact_id.as_str())
        .collect::<BTreeSet<_>>();
    let review = result.status == DagRunStatus::AwaitingReview;
    let mut projected = BTreeMap::<String, FinalCandidateProjection>::new();
    for artifact in &terminal_artifacts {
        for candidate in artifact_candidates(artifact, review, historical_geometry_correction_rate)
        {
            projected.insert(candidate.lineage_id.clone(), candidate);
        }
    }

    let mut debug_stages = Vec::new();
    let mut seen_debug = BTreeSet::new();
    let mut all_artifact_ids = BTreeSet::new();
    for trace in &result.checkpoint.traces {
        let Some(node) = draft.nodes.iter().find(|node| node.id == trace.node_id) else {
            continue;
        };
        for artifact in &trace.output_pipeline_artifacts {
            all_artifact_ids.insert(projection_artifact_id(artifact.reference()));
            append_debug_stages(&mut debug_stages, &mut seen_debug, artifact, node, false);
        }
    }
    for artifact in &terminal_artifacts {
        all_artifact_ids.insert(projection_artifact_id(artifact.reference()));
        let node = draft
            .nodes
            .iter()
            .find(|node| node.id == artifact.reference().source_node)
            .or_else(|| {
                draft.nodes.iter().find(|node| {
                    matches!(
                        node.kind,
                        WorkflowNodeKind::HumanReview | WorkflowNodeKind::Commit
                    )
                })
            });
        if let Some(node) = node {
            append_debug_stages(&mut debug_stages, &mut seen_debug, artifact, node, true);
        }
    }
    debug_stages
        .sort_by_key(|stage| (stage.stage, stage.node_id.clone(), stage.lineage_id.clone()));

    let terminal_ids = terminal_refs
        .into_iter()
        .map(stable_projection_id)
        .collect::<BTreeSet<_>>();
    let intermediate_artifact_ids = all_artifact_ids
        .difference(&terminal_ids)
        .copied()
        .collect::<Vec<_>>();
    let no_target = result.status == DagRunStatus::Completed
        && projected.is_empty()
        && result
            .checkpoint
            .traces
            .iter()
            .all(|trace| trace.error.is_none());

    if review {
        let review_candidates = projected
            .into_values()
            .map(|candidate| annotagent_core::ReviewCandidateProjection {
                explanation: review_explanation(&candidate),
                candidate,
            })
            .collect();
        ResultProjection {
            review_candidates,
            no_target,
            intermediate_artifact_ids,
            debug_stages,
            ..ResultProjection::default()
        }
    } else {
        ResultProjection {
            final_candidates: projected.into_values().collect(),
            no_target,
            intermediate_artifact_ids,
            debug_stages,
            ..ResultProjection::default()
        }
    }
}

fn terminal_artifacts<'a>(
    draft: &'a WorkflowDraft,
    result: &'a DagRunResult,
) -> Vec<&'a PipelineArtifact> {
    if result.status == DagRunStatus::AwaitingReview {
        return result
            .checkpoint
            .traces
            .iter()
            .filter(|trace| trace.status == DagNodeStatus::AwaitingReview)
            .filter(|trace| {
                draft.nodes.iter().any(|node| {
                    node.id == trace.node_id && node.kind == WorkflowNodeKind::HumanReview
                })
            })
            .flat_map(|trace| &trace.input_pipeline_artifacts)
            .filter(|artifact| is_candidate_artifact(artifact))
            .collect();
    }
    result
        .committed_pipeline_artifacts
        .iter()
        .filter(|artifact| is_candidate_artifact(artifact))
        .collect()
}

const fn is_candidate_artifact(artifact: &PipelineArtifact) -> bool {
    matches!(
        artifact,
        PipelineArtifact::DetectionSet(_)
            | PipelineArtifact::ClassificationSet(_)
            | PipelineArtifact::AnnotationCandidateSet(_)
            | PipelineArtifact::CandidateClusterSet(_)
    )
}

fn artifact_candidates(
    artifact: &PipelineArtifact,
    force_review: bool,
    historical_geometry_correction_rate: Option<f32>,
) -> Vec<FinalCandidateProjection> {
    let source_artifact_id = projection_artifact_id(artifact.reference());
    let source_artifact_ref = artifact.reference().artifact_id.clone();
    match artifact {
        PipelineArtifact::DetectionSet(set) => set
            .detections
            .iter()
            .map(|detection| {
                let status = if force_review {
                    SampleTestOutcomeStatus::NeedsReview
                } else {
                    outcome_status(Some(set.validation_state))
                };
                let mut geometry_quality = CandidateGeometryQualityReport::from_detection(
                    set.reference.artifact_id.clone(),
                    detection,
                );
                geometry_quality.historical_correction_rate = historical_geometry_correction_rate;
                let mut failure_classes = Vec::new();
                if detection.score.value.is_none() {
                    failure_classes.push(AnnotationFailureClass::MissingScore);
                }
                if status == SampleTestOutcomeStatus::NeedsReview
                    && geometry_quality.has_geometry_issue()
                {
                    failure_classes.push(AnnotationFailureClass::GeometryError);
                }
                let rejected_refiner = detection
                    .attributes
                    .contains_key("rejected_refined_geometry");
                let lineage_id = detection_lineage_id(detection);
                FinalCandidateProjection {
                    source_artifact_id,
                    source_artifact_ref: source_artifact_ref.clone(),
                    lineage_id: format!("detection:{lineage_id}"),
                    outcome: SampleTestOutcome {
                        id: detection.detection_id.clone(),
                        label: detection.project_label.as_ref().map_or_else(
                            || {
                                detection
                                    .model_label
                                    .clone()
                                    .unwrap_or_else(|| "unlabeled".to_owned())
                            },
                            ToString::to_string,
                        ),
                        confidence: detection.score.comparable_confidence(),
                        status,
                        value: Some(VisionArtifactValue::BoundingBox {
                            rect: detection.bbox,
                        }),
                        failure_classes,
                        geometry_quality: Some(geometry_quality),
                    },
                    localization: if has_projected_localization(detection) {
                        "Re-localized in expanded crop".to_owned()
                    } else {
                        "Whole-image localization".to_owned()
                    },
                    geometry: if rejected_refiner {
                        "Prompted-segmentation refinement rejected; coarse evidence retained"
                            .to_owned()
                    } else if detection
                        .attributes
                        .contains_key("geometry_quality_evaluation")
                    {
                        "Refined by prompted segmentation".to_owned()
                    } else {
                        "Model-proposed geometry".to_owned()
                    },
                    final_status: if force_review || rejected_refiner {
                        "Prompt coverage uncertain / geometry drift".to_owned()
                    } else {
                        "Ready to accept".to_owned()
                    },
                }
            })
            .collect(),
        PipelineArtifact::ClassificationSet(set) => set
            .classifications
            .iter()
            .map(|classification| FinalCandidateProjection {
                source_artifact_id,
                source_artifact_ref: source_artifact_ref.clone(),
                lineage_id: format!(
                    "classification:{}:{}",
                    classification.subject.artifact_id,
                    classification
                        .subject
                        .item_id
                        .as_deref()
                        .unwrap_or(&classification.id)
                ),
                outcome: SampleTestOutcome {
                    id: classification.id.clone(),
                    label: classification.label.to_string(),
                    confidence: Some(classification.confidence),
                    status: if force_review {
                        SampleTestOutcomeStatus::NeedsReview
                    } else {
                        outcome_status(Some(set.validation_state))
                    },
                    value: Some(VisionArtifactValue::Classification {
                        labels: vec![classification.label.clone()],
                    }),
                    failure_classes: Vec::new(),
                    geometry_quality: None,
                },
                localization: "Whole image or referenced subject".to_owned(),
                geometry: "Not applicable".to_owned(),
                final_status: if force_review {
                    "Needs human review".to_owned()
                } else {
                    "Ready to accept".to_owned()
                },
            })
            .collect(),
        PipelineArtifact::AnnotationCandidateSet(set) => set
            .candidates
            .iter()
            .map(|candidate| FinalCandidateProjection {
                source_artifact_id,
                source_artifact_ref: source_artifact_ref.clone(),
                lineage_id: format!(
                    "candidate:{}:{}",
                    candidate.subject.artifact_id,
                    candidate
                        .subject
                        .item_id
                        .as_deref()
                        .unwrap_or(&candidate.id)
                ),
                outcome: SampleTestOutcome {
                    id: candidate.id.clone(),
                    label: candidate.label.to_string(),
                    confidence: candidate.confidence,
                    status: if force_review {
                        SampleTestOutcomeStatus::NeedsReview
                    } else {
                        outcome_status(candidate.validation_state)
                    },
                    value: candidate.value.clone(),
                    failure_classes: Vec::new(),
                    geometry_quality: None,
                },
                localization: "Referenced subject".to_owned(),
                geometry: "Projected annotation geometry".to_owned(),
                final_status: if force_review {
                    "Needs human review".to_owned()
                } else {
                    "Ready to accept".to_owned()
                },
            })
            .collect(),
        PipelineArtifact::CandidateClusterSet(set) => set
            .candidates
            .iter()
            .map(|candidate| {
                let source_models = candidate
                    .members
                    .iter()
                    .map(|member| &member.source_model_id)
                    .collect::<BTreeSet<_>>();
                FinalCandidateProjection {
                    source_artifact_id,
                    source_artifact_ref: source_artifact_ref.clone(),
                    lineage_id: format!("cluster:{}", candidate.id),
                    outcome: SampleTestOutcome {
                        id: candidate.id.clone(),
                        label: candidate.target_label.to_string(),
                        confidence: (source_models.len() == 1)
                            .then(|| {
                                candidate
                                    .members
                                    .first()
                                    .and_then(|member| member.score.comparable_confidence())
                            })
                            .flatten(),
                        status: if force_review {
                            SampleTestOutcomeStatus::NeedsReview
                        } else {
                            outcome_status(Some(set.validation_state))
                        },
                        value: Some(VisionArtifactValue::BoundingBox {
                            rect: candidate.representative_bbox,
                        }),
                        failure_classes: Vec::new(),
                        geometry_quality: None,
                    },
                    localization: "Multi-source candidate merge".to_owned(),
                    geometry: "Representative candidate geometry".to_owned(),
                    final_status: if force_review {
                        "Needs human review".to_owned()
                    } else {
                        "Ready to accept".to_owned()
                    },
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn detection_lineage_id(detection: &annotagent_core::DetectionArtifactItem) -> String {
    detection
        .attributes
        .get("geometry_quality_evaluation")
        .and_then(|value| value.pointer("/trace/source_detection/item_id"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            detection
                .attributes
                .get("geometry_refinement")
                .and_then(|value| value.pointer("/source_detection/item_id"))
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or(&detection.detection_id)
        .to_owned()
}

fn has_projected_localization(detection: &annotagent_core::DetectionArtifactItem) -> bool {
    detection.attributes.contains_key("coordinate_projection")
        || detection
            .evidence
            .iter()
            .any(|evidence| evidence.source_artifact_id.contains("project"))
}

fn review_explanation(candidate: &FinalCandidateProjection) -> ResultExplanation {
    let refiner_drift = candidate.geometry.contains("rejected");
    if refiner_drift {
        return ResultExplanation {
            title: "Prompted-segmentation refinement was rejected".to_owned(),
            summary: "The refinement moved or expanded beyond the configured geometry safety limits. The earlier localization remains evidence and the unsafe refined box is not a final result."
                .to_owned(),
            recommendation: Some(
                "Re-localize the object inside a bounded crop before trying prompted segmentation again."
                    .to_owned(),
            ),
        };
    }
    ResultExplanation {
        title: "This terminal candidate needs a human decision".to_owned(),
        summary: "The automation preserved one final candidate for this object lineage instead of exposing every intermediate detection as a separate result."
            .to_owned(),
        recommendation: Some(
            "Review the final geometry or open Diagnostics to compare its localization stages."
                .to_owned(),
        ),
    }
}

fn append_debug_stages(
    stages: &mut Vec<LineageStageProjection>,
    seen: &mut BTreeSet<(ResultLineageStage, String, String)>,
    artifact: &PipelineArtifact,
    node: &annotagent_core::WorkflowDraftNode,
    terminal: bool,
) {
    let artifact_id = projection_artifact_id(artifact.reference());
    let artifact_ref = artifact.reference().artifact_id.clone();
    let stage = if terminal {
        ResultLineageStage::Final
    } else {
        stage_for(node, artifact)
    };
    let source = source_for(node, artifact);
    let mut push = |lineage_id: String,
                    label: Option<String>,
                    confidence: Option<f32>,
                    value: Option<VisionArtifactValue>,
                    detail: Option<String>| {
        if seen.insert((stage, artifact_ref.clone(), lineage_id.clone())) {
            stages.push(LineageStageProjection {
                artifact_id,
                artifact_ref: artifact_ref.clone(),
                node_id: node.id.clone(),
                lineage_id,
                stage,
                source: source.clone(),
                label,
                confidence,
                value,
                terminal,
                detail,
            });
        }
    };
    match artifact {
        PipelineArtifact::DetectionSet(set) => {
            for detection in &set.detections {
                push(
                    format!("detection:{}", detection_lineage_id(detection)),
                    detection
                        .project_label
                        .as_ref()
                        .map(ToString::to_string)
                        .or_else(|| detection.model_label.clone()),
                    detection.score.comparable_confidence(),
                    Some(VisionArtifactValue::BoundingBox {
                        rect: detection.bbox,
                    }),
                    detection
                        .attributes
                        .get("localization_failure_class")
                        .and_then(serde_json::Value::as_str)
                        .map(|value| value.replace('_', " ")),
                );
            }
        }
        PipelineArtifact::CropSet(set) => {
            for crop in &set.crops {
                push(
                    format!(
                        "detection:{}",
                        crop.parent.item_id.as_deref().unwrap_or(&crop.id)
                    ),
                    None,
                    None,
                    Some(VisionArtifactValue::BoundingBox { rect: crop.rect }),
                    Some(format!(
                        "{}×{} search crop",
                        crop.crop_width, crop.crop_height
                    )),
                );
            }
        }
        PipelineArtifact::PromptCoverage(coverage) => push(
            coverage.reference.item_id.as_ref().map_or_else(
                || "prompt-coverage".to_owned(),
                |id| format!("detection:{id}"),
            ),
            None,
            None,
            None,
            Some(format!(
                "{:?} · {:?}",
                coverage.state, coverage.recommended_action
            )),
        ),
        PipelineArtifact::MaskSet(set) => {
            for mask in &set.masks {
                push(
                    format!(
                        "detection:{}",
                        mask.prompt.item_id.as_deref().unwrap_or(&mask.mask_id)
                    ),
                    None,
                    mask.score.comparable_confidence(),
                    None,
                    Some("Prompted-segmentation mask".to_owned()),
                );
            }
        }
        PipelineArtifact::ClassificationSet(set) => {
            for classification in &set.classifications {
                push(
                    format!(
                        "classification:{}:{}",
                        classification.subject.artifact_id,
                        classification
                            .subject
                            .item_id
                            .as_deref()
                            .unwrap_or(&classification.id)
                    ),
                    Some(classification.label.to_string()),
                    Some(classification.confidence),
                    Some(VisionArtifactValue::Classification {
                        labels: vec![classification.label.clone()],
                    }),
                    None,
                );
            }
        }
        PipelineArtifact::AnnotationCandidateSet(set) => {
            for candidate in &set.candidates {
                push(
                    format!(
                        "candidate:{}:{}",
                        candidate.subject.artifact_id,
                        candidate
                            .subject
                            .item_id
                            .as_deref()
                            .unwrap_or(&candidate.id)
                    ),
                    Some(candidate.label.to_string()),
                    candidate.confidence,
                    candidate.value.clone(),
                    None,
                );
            }
        }
        PipelineArtifact::CandidateClusterSet(set) => {
            for candidate in &set.candidates {
                push(
                    format!("cluster:{}", candidate.id),
                    Some(candidate.target_label.to_string()),
                    None,
                    Some(VisionArtifactValue::BoundingBox {
                        rect: candidate.representative_bbox,
                    }),
                    Some(format!("{} evidence source(s)", candidate.members.len())),
                );
            }
        }
        PipelineArtifact::Image(_)
        | PipelineArtifact::BoxPromptSet(_)
        | PipelineArtifact::PointPromptSet(_)
        | PipelineArtifact::SemanticMask(_)
        | PipelineArtifact::PolygonSet(_) => {}
    }
}

fn stage_for(
    node: &annotagent_core::WorkflowDraftNode,
    artifact: &PipelineArtifact,
) -> ResultLineageStage {
    if matches!(artifact, PipelineArtifact::PromptCoverage(_)) {
        ResultLineageStage::PromptCoverage
    } else if matches!(artifact, PipelineArtifact::MaskSet(_)) {
        ResultLineageStage::Mask
    } else if node.node_type == "core.expand_region" || node.node_type == "core.crop" {
        ResultLineageStage::SearchRegion
    } else if node.node_type == "core.project_coordinates" {
        ResultLineageStage::Relocalized
    } else if node.node_type == "core.mask_to_bbox"
        || node.node_type == "core.geometry_quality_evaluation"
    {
        ResultLineageStage::Refined
    } else if node.node_type == "core.geometry_decision" {
        ResultLineageStage::Final
    } else {
        ResultLineageStage::Coarse
    }
}

fn source_for(node: &annotagent_core::WorkflowDraftNode, artifact: &PipelineArtifact) -> String {
    if node.kind == WorkflowNodeKind::HumanReview {
        "Human correction".to_owned()
    } else if matches!(artifact, PipelineArtifact::MaskSet(_)) {
        "Prompted segmentation".to_owned()
    } else if node.node_type == "core.project_coordinates" {
        "Local vision model".to_owned()
    } else if matches!(
        node.kind,
        WorkflowNodeKind::VisionLanguageModel | WorkflowNodeKind::VisionModel
    ) {
        node.model_profile_binding.as_ref().map_or_else(
            || {
                node.model_binding
                    .clone()
                    .unwrap_or_else(|| "Vision model".to_owned())
            },
            |binding| binding.model_profile_id.to_string(),
        )
    } else {
        node.node_type.replace('_', " ")
    }
}

fn projection_artifact_id(reference: &ArtifactRef) -> ArtifactId {
    stable_projection_id(&reference.artifact_id)
}

fn stable_projection_id(value: &str) -> ArtifactId {
    ArtifactId(Uuid::new_v5(&Uuid::NAMESPACE_URL, value.as_bytes()))
}

const fn outcome_status(state: Option<ArtifactValidationState>) -> SampleTestOutcomeStatus {
    match state {
        Some(ArtifactValidationState::Valid) => SampleTestOutcomeStatus::ReadyToAccept,
        Some(ArtifactValidationState::Invalid) => SampleTestOutcomeStatus::Invalid,
        Some(ArtifactValidationState::Unvalidated | ArtifactValidationState::NeedsReview)
        | None => SampleTestOutcomeStatus::NeedsReview,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use super::*;
    use annotagent_core::{
        DETECTION_ARTIFACT_SCHEMA_VERSION, DetectionArtifactItem, DetectionScore,
        DetectionSetArtifact, DetectionSource, ModelId, NormalizedRect, ScoreSemantics,
        VisionCapability, WorkflowDraftNode, WorkflowDraftStatus,
    };
    use annotagent_runtime::{DagCheckpoint, DagNodeOutput, DagNodeTrace, DagNodeUsage};

    #[test]
    fn projection_ids_are_stable_and_do_not_claim_pipeline_refs_are_uuids() {
        assert_eq!(
            stable_projection_id("coarse:set"),
            stable_projection_id("coarse:set")
        );
        assert_ne!(
            stable_projection_id("coarse:set"),
            stable_projection_id("refined:set")
        );
    }

    #[test]
    fn one_lineage_projects_to_one_result_and_four_debug_stages() {
        let provisional_fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/execution/fixtures/small-object-localization/bhuman-provisional.json"
        ))
        .expect("B-Human provisional fixture");
        let fixture_rect = |field: &str| {
            let coordinates = provisional_fixture[field]
                .as_array()
                .expect("fixture xywh coordinates");
            NormalizedRect::new(
                coordinates[0].as_f64().expect("x") as f32,
                coordinates[1].as_f64().expect("y") as f32,
                coordinates[2].as_f64().expect("width") as f32,
                coordinates[3].as_f64().expect("height") as f32,
            )
            .expect("fixture bbox")
        };
        let now = Utc::now();
        let detection_node = |id: &str, node_type: &str, kind| WorkflowDraftNode {
            id: id.to_owned(),
            node_type: node_type.to_owned(),
            kind,
            ..WorkflowDraftNode::default()
        };
        let draft = WorkflowDraft {
            schema_version: annotagent_core::WORKFLOW_SCHEMA_VERSION,
            id: "projection-draft".to_owned(),
            project_id: "projection-project".to_owned(),
            name: "Projection regression".to_owned(),
            status: WorkflowDraftStatus::Validated,
            revision: 1,
            content_hash: "projection-hash".to_owned(),
            nodes: vec![
                detection_node(
                    "coarse",
                    "capability.detect",
                    WorkflowNodeKind::VisionLanguageModel,
                ),
                detection_node(
                    "local",
                    "core.project_coordinates",
                    WorkflowNodeKind::Transform,
                ),
                detection_node("refined", "core.mask_to_bbox", WorkflowNodeKind::Transform),
                detection_node("review", "core.human_review", WorkflowNodeKind::HumanReview),
            ],
            edges: Vec::new(),
            enabled_skills: BTreeMap::new(),
            resource_versions: BTreeMap::new(),
            runtime_policies: BTreeMap::new(),
            allow_unvalidated_commit: false,
            geometry_risk_acceptance: None,
            label_pipeline: None,
            created_at: now,
            updated_at: now,
        };
        let image_id = annotagent_core::ImageId::new();
        let set = |artifact_ref: &str,
                   source_node: &str,
                   bbox: NormalizedRect,
                   state: ArtifactValidationState| {
            let source = DetectionSource {
                model_id: ModelId::from("projection-model"),
                capability: VisionCapability::VisionLanguage,
                artifact_id: artifact_ref.to_owned(),
            };
            PipelineArtifact::DetectionSet(DetectionSetArtifact {
                schema_version: DETECTION_ARTIFACT_SCHEMA_VERSION,
                reference: ArtifactRef {
                    artifact_id: artifact_ref.to_owned(),
                    source_node: source_node.to_owned(),
                    port: "detections".to_owned(),
                    artifact_type: annotagent_core::ArtifactKind::DetectionSet,
                    item_id: None,
                },
                image_id,
                model_binding: "projection-model".to_owned(),
                validation_state: state,
                detections: vec![
                    DetectionArtifactItem::from_source(
                        "ball-1",
                        Some("ball".to_owned()),
                        Some("ball".to_owned()),
                        Some(annotagent_core::LabelId::from("ball")),
                        bbox,
                        DetectionScore::new(Some(0.8), ScoreSemantics::SemanticConfidence)
                            .expect("score"),
                        source,
                    )
                    .expect("detection"),
                ],
                metadata: BTreeMap::new(),
            })
        };
        let coarse = set(
            "coarse-set",
            "coarse",
            fixture_rect("coarse_detection_xywh_normalized"),
            ArtifactValidationState::Unvalidated,
        );
        let local = set(
            "local-set",
            "local",
            NormalizedRect::new(0.47, 0.49, 0.04, 0.04).expect("local bbox"),
            ArtifactValidationState::Unvalidated,
        );
        let refined = set(
            "refined-set",
            "refined",
            fixture_rect("refined_detection_xywh_normalized"),
            ArtifactValidationState::NeedsReview,
        );
        let trace = |node_id: &str,
                     operation: &str,
                     status: DagNodeStatus,
                     inputs: Vec<PipelineArtifact>,
                     outputs: Vec<PipelineArtifact>| DagNodeTrace {
            node_id: node_id.to_owned(),
            operation: operation.to_owned(),
            status,
            attempt_count: 1,
            cache_key: None,
            cache_hit: false,
            input_artifacts: Vec::new(),
            output_artifacts: Vec::new(),
            input_pipeline_artifacts: inputs,
            output_pipeline_artifacts: outputs,
            input_envelopes: Vec::new(),
            output_envelopes: Vec::new(),
            route: None,
            usage: DagNodeUsage {
                input_tokens: 0,
                output_tokens: 0,
                cost: Decimal::ZERO,
            },
            error: None,
            started_at: now,
            finished_at: now,
        };
        let traces = vec![
            trace(
                "coarse",
                "capability.detect",
                DagNodeStatus::Succeeded,
                Vec::new(),
                vec![coarse],
            ),
            trace(
                "local",
                "core.project_coordinates",
                DagNodeStatus::Succeeded,
                Vec::new(),
                vec![local],
            ),
            trace(
                "refined",
                "core.mask_to_bbox",
                DagNodeStatus::Succeeded,
                Vec::new(),
                vec![refined.clone()],
            ),
            trace(
                "review",
                "core.human_review",
                DagNodeStatus::AwaitingReview,
                vec![refined],
                Vec::new(),
            ),
        ];
        let result = DagRunResult {
            status: DagRunStatus::AwaitingReview,
            checkpoint: DagCheckpoint {
                workflow_content_hash: "projection-hash".to_owned(),
                node_statuses: BTreeMap::from([
                    ("coarse".to_owned(), DagNodeStatus::Succeeded),
                    ("local".to_owned(), DagNodeStatus::Succeeded),
                    ("refined".to_owned(), DagNodeStatus::Succeeded),
                    ("review".to_owned(), DagNodeStatus::AwaitingReview),
                ]),
                node_outputs: BTreeMap::<String, DagNodeOutput>::new(),
                traces,
                activated_fallbacks: BTreeSet::new(),
                approved_review_nodes: BTreeSet::new(),
                usage: DagNodeUsage::default(),
            },
            committed: Vec::new(),
            committed_pipeline_artifacts: Vec::new(),
        };

        let projection = project_sandbox_result(&draft, &result, None);

        assert!(projection.final_candidates.is_empty());
        assert_eq!(provisional_fixture["legacy_results_count"], 2);
        assert_eq!(
            projection.review_candidates.len() as u64,
            provisional_fixture["expected_terminal_results_count"]
                .as_u64()
                .expect("expected terminal count")
        );
        assert_eq!(
            projection.review_candidates[0].candidate.outcome.id,
            "ball-1"
        );
        assert_eq!(projection.debug_stages.len(), 4);
        assert_eq!(
            projection
                .debug_stages
                .iter()
                .map(|stage| stage.stage)
                .collect::<Vec<_>>(),
            vec![
                ResultLineageStage::Coarse,
                ResultLineageStage::Relocalized,
                ResultLineageStage::Refined,
                ResultLineageStage::Final,
            ]
        );
    }
}
