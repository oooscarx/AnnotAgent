//! Domain-neutral executable Core nodes for Label Pipeline intermediate Artifacts.

use std::{collections::BTreeMap, sync::OnceLock};

use annotagent_core::{
    AnnotationCandidateSet, ArtifactKind, ArtifactRef, ArtifactValidationState,
    ClassificationSetArtifact, CropSetArtifact, DetectionSetArtifact, LabelId, PipelineArtifact,
    TaskId,
};
use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{DagNodeContext, DagNodeFailure, DagNodeOutput, DagNodeRunner};

pub const CORE_CROP: &str = "core.crop";
pub const CORE_FILTER: &str = "core.filter";
pub const CORE_MAP_LABEL: &str = "core.map_label";
pub const CORE_ATTACH_RESULT: &str = "core.attach_result";
pub const CORE_ATTACH_ATTRIBUTE: &str = "core.attach_attribute";
pub const CORE_CONFIDENCE_GATE: &str = "core.confidence_gate";

#[derive(Debug, Default, Clone, Copy)]
pub struct CorePipelineRunner;

#[async_trait]
impl DagNodeRunner for CorePipelineRunner {
    async fn run(&self, context: DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
        match context.node.node_type.as_str() {
            CORE_CROP => run_crop(&context),
            CORE_FILTER => run_filter(&context),
            CORE_MAP_LABEL => run_map_label(&context),
            CORE_ATTACH_RESULT => run_attach_result(&context),
            CORE_ATTACH_ATTRIBUTE => run_attach_attribute(&context),
            CORE_CONFIDENCE_GATE => run_confidence_gate(&context),
            operation => Err(DagNodeFailure::terminal(
                "unsupported_core_pipeline_node",
                format!("Core Pipeline runner does not implement {operation:?}"),
            )),
        }
    }
}

fn run_crop(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    require_image(context)?;
    let detections = one_detection_set(context)?;
    let padding = number_parameter(context, "padding", 0.0)? as f32;
    let reference = output_reference(context, "crops", ArtifactKind::CropSet)?;
    let crops = CropSetArtifact::fan_out(reference, detections, padding, |detection| {
        Some(format!(
            "artifact-cache://{}/{}",
            context.node.id, detection.id
        ))
    })
    .map_err(|error| DagNodeFailure::terminal("crop_failed", error))?;
    Ok(output(PipelineArtifact::CropSet(crops)))
}

fn run_filter(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let source = one_detection_set(context)?;
    let minimum = number_parameter(context, "minimum_confidence", 0.0)? as f32;
    let class_ids = string_list_parameter(context, "class_ids")?;
    let labels = string_list_parameter(context, "labels")?;
    let mut filtered = source.clone();
    filtered.reference = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    filtered.detections.retain(|detection| {
        detection.confidence >= minimum
            && (class_ids.is_empty() || class_ids.contains(&detection.class_id))
            && (labels.is_empty()
                || detection
                    .label
                    .as_ref()
                    .is_some_and(|label| labels.iter().any(|item| item == label.as_str())))
    });
    filtered
        .validate()
        .map_err(|error| DagNodeFailure::terminal("filter_output_invalid", error))?;
    Ok(output(PipelineArtifact::DetectionSet(filtered)))
}

fn run_map_label(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let source = one_detection_set(context)?;
    let mapping = object_parameter(context, "class_mapping")?;
    let mut mapped = source.clone();
    mapped.reference = output_reference(context, "detections", ArtifactKind::DetectionSet)?;
    for detection in &mut mapped.detections {
        if let Some(label) = mapping
            .get(&detection.class_id)
            .and_then(Value::as_str)
            .filter(|label| !label.trim().is_empty())
        {
            detection.label = Some(LabelId::from(label));
        }
    }
    mapped
        .validate()
        .map_err(|error| DagNodeFailure::terminal("map_label_output_invalid", error))?;
    Ok(output(PipelineArtifact::DetectionSet(mapped)))
}

fn run_attach_result(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let detections = one_detection_set(context)?;
    let classifications = one_classification_set(context)?;
    let task_id = context
        .node
        .parameters
        .get("task_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            DagNodeFailure::terminal("missing_task_id", "Attach Result requires task_id")
        })?;
    let mapping = object_parameter(context, "class_mapping")?
        .iter()
        .filter_map(|(source, target)| {
            target
                .as_str()
                .map(|target| (LabelId::from(source.as_str()), LabelId::from(target)))
        })
        .collect::<BTreeMap<_, _>>();
    let candidates = AnnotationCandidateSet::fan_in(
        output_reference(context, "candidates", ArtifactKind::AnnotationCandidateSet)?,
        detections,
        classifications,
        &TaskId::from(task_id),
        &mapping,
    )
    .map_err(|error| DagNodeFailure::terminal("attach_result_failed", error))?;
    Ok(output(PipelineArtifact::AnnotationCandidateSet(candidates)))
}

fn run_attach_attribute(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let source = one_candidate_set(context)?;
    let name = context
        .node
        .parameters
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            DagNodeFailure::terminal("missing_attribute_name", "Attach Attribute requires name")
        })?;
    let value = context
        .node
        .parameters
        .get("value")
        .cloned()
        .ok_or_else(|| {
            DagNodeFailure::terminal("missing_attribute_value", "Attach Attribute requires value")
        })?;
    let mut candidates = source.clone();
    candidates.reference =
        output_reference(context, "candidates", ArtifactKind::AnnotationCandidateSet)?;
    for candidate in &mut candidates.candidates {
        candidate.attributes.insert(name.to_owned(), value.clone());
    }
    candidates
        .validate()
        .map_err(|error| DagNodeFailure::terminal("attach_attribute_output_invalid", error))?;
    Ok(output(PipelineArtifact::AnnotationCandidateSet(candidates)))
}

fn run_confidence_gate(context: &DagNodeContext<'_>) -> Result<DagNodeOutput, DagNodeFailure> {
    let threshold = number_parameter(context, "threshold", 0.5)? as f32;
    if !(0.0..=1.0).contains(&threshold) {
        return Err(DagNodeFailure::terminal(
            "invalid_threshold",
            "Confidence Gate threshold must be within [0,1]",
        ));
    }
    let mut artifacts = context.input_pipeline_artifacts.clone();
    let confidence = artifacts
        .iter()
        .flat_map(artifact_confidences)
        .reduce(f32::min)
        .unwrap_or(1.0);
    let route = if confidence >= threshold {
        set_candidate_state(&mut artifacts, ArtifactValidationState::Valid);
        "pass"
    } else {
        set_candidate_state(&mut artifacts, ArtifactValidationState::NeedsReview);
        "review"
    };
    Ok(DagNodeOutput {
        pipeline_artifacts: artifacts,
        route: Some(route.to_owned()),
        metadata: BTreeMap::from([
            ("confidence".to_owned(), serde_json::json!(confidence)),
            ("threshold".to_owned(), serde_json::json!(threshold)),
        ]),
        ..DagNodeOutput::default()
    })
}

fn set_candidate_state(artifacts: &mut [PipelineArtifact], state: ArtifactValidationState) {
    for artifact in artifacts {
        match artifact {
            PipelineArtifact::DetectionSet(detections) => detections.validation_state = state,
            PipelineArtifact::ClassificationSet(classifications) => {
                classifications.validation_state = state;
            }
            PipelineArtifact::AnnotationCandidateSet(candidates) => {
                for candidate in &mut candidates.candidates {
                    candidate.validation_state = Some(state);
                }
            }
            PipelineArtifact::Image(_) | PipelineArtifact::CropSet(_) => {}
        }
    }
}

fn artifact_confidences(artifact: &PipelineArtifact) -> Vec<f32> {
    match artifact {
        PipelineArtifact::DetectionSet(artifact) => artifact
            .detections
            .iter()
            .map(|detection| detection.confidence)
            .collect(),
        PipelineArtifact::ClassificationSet(artifact) => artifact
            .classifications
            .iter()
            .map(|classification| classification.confidence)
            .collect(),
        PipelineArtifact::AnnotationCandidateSet(artifact) => artifact
            .candidates
            .iter()
            .filter_map(|candidate| candidate.confidence)
            .collect(),
        PipelineArtifact::Image(_) | PipelineArtifact::CropSet(_) => Vec::new(),
    }
}

fn require_image(context: &DagNodeContext<'_>) -> Result<(), DagNodeFailure> {
    if context
        .input_pipeline_artifacts
        .iter()
        .any(|artifact| matches!(artifact, PipelineArtifact::Image(_)))
    {
        Ok(())
    } else {
        Err(DagNodeFailure::terminal(
            "missing_image_input",
            "Crop requires Image input",
        ))
    }
}

fn one_detection_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a DetectionSetArtifact, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::DetectionSet(value) => Some(value),
            _ => None,
        },
        "DetectionSet",
    )
}

fn one_classification_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a ClassificationSetArtifact, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::ClassificationSet(value) => Some(value),
            _ => None,
        },
        "ClassificationSet",
    )
}

fn one_candidate_set<'a>(
    context: &'a DagNodeContext<'_>,
) -> Result<&'a AnnotationCandidateSet, DagNodeFailure> {
    exactly_one(
        context,
        |artifact| match artifact {
            PipelineArtifact::AnnotationCandidateSet(value) => Some(value),
            _ => None,
        },
        "AnnotationCandidateSet",
    )
}

fn exactly_one<'a, T>(
    context: &'a DagNodeContext<'_>,
    extract: impl Fn(&'a PipelineArtifact) -> Option<&'a T>,
    name: &str,
) -> Result<&'a T, DagNodeFailure> {
    let mut values = context.input_pipeline_artifacts.iter().filter_map(extract);
    let first = values.next().ok_or_else(|| {
        DagNodeFailure::terminal("missing_pipeline_input", format!("node requires {name}"))
    })?;
    if values.next().is_some() {
        return Err(DagNodeFailure::terminal(
            "ambiguous_pipeline_input",
            format!("node received multiple {name} Artifacts"),
        ));
    }
    Ok(first)
}

fn output(artifact: PipelineArtifact) -> DagNodeOutput {
    DagNodeOutput {
        pipeline_artifacts: vec![artifact],
        ..DagNodeOutput::default()
    }
}

fn output_reference(
    context: &DagNodeContext<'_>,
    preferred_port: &str,
    artifact_type: ArtifactKind,
) -> Result<ArtifactRef, DagNodeFailure> {
    let port = context
        .node
        .outputs
        .iter()
        .find(|port| port.id == preferred_port && port.artifact_type == artifact_type)
        .or_else(|| {
            context
                .node
                .outputs
                .iter()
                .find(|port| port.artifact_type == artifact_type)
        })
        .ok_or_else(|| {
            DagNodeFailure::terminal(
                "missing_output_port",
                format!("node does not declare a {artifact_type:?} output"),
            )
        })?;
    let material = serde_json::to_vec(&serde_json::json!({
        "run_id": context.run_id,
        "image_id": context.image_id,
        "node": context.node.id,
        "port": port.id,
        "inputs": context
            .input_pipeline_artifacts
            .iter()
            .map(PipelineArtifact::reference)
            .collect::<Vec<_>>(),
        "parameters": context.node.parameters,
    }))
    .map_err(|error| DagNodeFailure::terminal("artifact_identity_failed", error.to_string()))?;
    Ok(ArtifactRef {
        artifact_id: format!("sha256:{:x}", Sha256::digest(material)),
        source_node: context.node.id.clone(),
        port: port.id.clone(),
        artifact_type,
        item_id: None,
    })
}

fn number_parameter(
    context: &DagNodeContext<'_>,
    name: &str,
    default: f64,
) -> Result<f64, DagNodeFailure> {
    context
        .node
        .parameters
        .get(name)
        .map_or(Ok(default), |value| {
            value.as_f64().ok_or_else(|| {
                DagNodeFailure::terminal(
                    "invalid_node_parameter",
                    format!("parameter {name:?} must be a number"),
                )
            })
        })
}

fn string_list_parameter(
    context: &DagNodeContext<'_>,
    name: &str,
) -> Result<Vec<String>, DagNodeFailure> {
    context.node.parameters.get(name).map_or_else(
        || Ok(Vec::new()),
        |value| {
            value
                .as_array()
                .ok_or_else(|| {
                    DagNodeFailure::terminal(
                        "invalid_node_parameter",
                        format!("parameter {name:?} must be an array"),
                    )
                })?
                .iter()
                .map(|item| {
                    item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                        DagNodeFailure::terminal(
                            "invalid_node_parameter",
                            format!("parameter {name:?} must contain strings"),
                        )
                    })
                })
                .collect()
        },
    )
}

fn object_parameter<'a>(
    context: &'a DagNodeContext<'_>,
    name: &str,
) -> Result<&'a serde_json::Map<String, Value>, DagNodeFailure> {
    static EMPTY: OnceLock<serde_json::Map<String, Value>> = OnceLock::new();
    context.node.parameters.get(name).map_or_else(
        || Ok(EMPTY.get_or_init(serde_json::Map::new)),
        |value| {
            value.as_object().ok_or_else(|| {
                DagNodeFailure::terminal(
                    "invalid_node_parameter",
                    format!("parameter {name:?} must be an object"),
                )
            })
        },
    )
}
