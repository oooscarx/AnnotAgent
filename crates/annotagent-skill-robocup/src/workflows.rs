use std::collections::BTreeMap;

use annotagent_core::{
    ArtifactKind, FallbackPolicy, NodePort, ResourceRequirements, RetryPolicy, ReviewGate,
    WorkflowDraftNode, WorkflowEdge, WorkflowNodeKind, WorkflowTemplate,
};
use serde_json::json;

const SKILL_ID: &str = "robocup";

#[must_use]
pub fn workflow_templates() -> Vec<WorkflowTemplate> {
    vec![vlm_bootstrap(), detector_first(), accurate_hybrid()]
}

fn vlm_bootstrap() -> WorkflowTemplate {
    let kinds = [
        ArtifactKind::Classification,
        ArtifactKind::Polygon,
        ArtifactKind::Polyline,
        ArtifactKind::Keypoints,
        ArtifactKind::BoundingBox,
        ArtifactKind::Attributes,
    ];
    let bootstrap = node(
        "vlm_bootstrap",
        "vision_language",
        WorkflowNodeKind::VisionLanguageModel,
        &[],
        &kinds,
        Some("default-vision"),
        &[],
        &[],
    );
    let mut validate = node(
        "robocup_validate",
        "static_validator",
        WorkflowNodeKind::Validator,
        &kinds,
        &kinds,
        None,
        &[
            "field_containment",
            "white_line_appearance",
            "ball_hard_negative",
            "robot_attribute_rules",
        ],
        &["robocup_field_line_refiner"],
    );
    validate.parameters.insert(
        "contract".to_owned(),
        json!("validate typed VLM candidates; absent optional targets remain succeeded_empty"),
    );
    template(
        "vlm-bootstrap",
        "VLM bootstrap",
        "Initial RoboCup annotation without specialist detector dependencies.",
        vec![bootstrap, validate, review_node(&kinds), commit_node()],
        chain_edges(&[
            ("vlm_bootstrap", "robocup_validate", kinds.as_slice()),
            ("robocup_validate", "review", kinds.as_slice()),
        ]),
    )
}

fn detector_first() -> WorkflowTemplate {
    let mask = [ArtifactKind::SemanticMask];
    let boxes = [ArtifactKind::BoundingBox];
    let semantics = [ArtifactKind::Classification];
    let nodes = vec![
        model_node(
            "field_candidates",
            "semantic_segmentation",
            WorkflowNodeKind::VisionModel,
            &[],
            &mask,
            "semantic_segmentation",
        ),
        model_node(
            "object_candidates",
            "object_detection",
            WorkflowNodeKind::VisionModel,
            &[],
            &boxes,
            "object_detection",
        ),
        node(
            "hard_negative_check",
            "static_validator",
            WorkflowNodeKind::Validator,
            &boxes,
            &boxes,
            None,
            &["field_containment", "ball_hard_negative"],
            &[],
        ),
        node(
            "vlm_hard_sample_review",
            "vision_language",
            WorkflowNodeKind::VisionLanguageModel,
            &boxes,
            &semantics,
            Some("default-vision"),
            &[],
            &[],
        ),
        review_node(&[
            ArtifactKind::SemanticMask,
            ArtifactKind::BoundingBox,
            ArtifactKind::Classification,
        ]),
        commit_node(),
    ];
    let mut edges = Vec::new();
    edges.extend(edge_set("object_candidates", "hard_negative_check", &boxes));
    edges.extend(edge_set(
        "hard_negative_check",
        "vlm_hard_sample_review",
        &boxes,
    ));
    edges.extend(edge_set("field_candidates", "review", &mask));
    edges.extend(edge_set("hard_negative_check", "review", &boxes));
    edges.extend(edge_set("vlm_hard_sample_review", "review", &semantics));
    template(
        "detector-first",
        "Detector first",
        "Specialist candidates first; the VLM reviews only semantic hard samples and never rewrites geometry.",
        nodes,
        edges,
    )
}

fn accurate_hybrid() -> WorkflowTemplate {
    let mask = [ArtifactKind::SemanticMask];
    let polygon = [ArtifactKind::Polygon];
    let boxes = [ArtifactKind::BoundingBox];
    let class = [ArtifactKind::Classification];
    let attributes = [ArtifactKind::Attributes];
    let line = [ArtifactKind::Polyline];
    let instance = [ArtifactKind::InstanceMask];
    let mut field_geometry = node(
        "field_geometry",
        "deterministic_cv",
        WorkflowNodeKind::DeterministicTool,
        &mask,
        &polygon,
        None,
        &[],
        &[],
    );
    field_geometry.parameters.insert(
        "operation".to_owned(),
        json!("mask_to_normalized_field_polygon"),
    );
    let mut verify = node(
        "vlm_verification",
        "vision_language",
        WorkflowNodeKind::VisionLanguageModel,
        &boxes,
        &class,
        Some("default-vision"),
        &[],
        &[],
    );
    verify.parameters.insert(
        "geometry_policy".to_owned(),
        json!("read_only; output semantic verification only"),
    );
    let mut robot_attributes = node(
        "robot_attributes",
        "vision_language",
        WorkflowNodeKind::VisionLanguageModel,
        &boxes,
        &attributes,
        Some("default-vision"),
        &["robot_attribute_rules"],
        &[],
    );
    robot_attributes.parameters.insert(
        "geometry_policy".to_owned(),
        json!("read_only; output attributes only"),
    );
    let mut nodes = vec![
        model_node(
            "field_segmentation",
            "semantic_segmentation",
            WorkflowNodeKind::VisionModel,
            &[],
            &mask,
            "semantic_segmentation",
        ),
        field_geometry,
        model_node(
            "object_candidates",
            "object_detection",
            WorkflowNodeKind::VisionModel,
            &[],
            &boxes,
            "object_detection",
        ),
        node(
            "hard_negative_validator",
            "static_validator",
            WorkflowNodeKind::Validator,
            &boxes,
            &boxes,
            None,
            &["field_containment", "ball_hard_negative"],
            &[],
        ),
        verify,
        robot_attributes,
        node(
            "coarse_field_line",
            "deterministic_cv",
            WorkflowNodeKind::DeterministicTool,
            &mask,
            &line,
            None,
            &[],
            &[],
        ),
        node(
            "field_line_refiner",
            "field_line_refiner",
            WorkflowNodeKind::Refiner,
            &line,
            &line,
            None,
            &["white_line_appearance"],
            &["robocup_field_line_refiner"],
        ),
        model_node(
            "prompted_segmentation",
            "prompted_segmentation",
            WorkflowNodeKind::VisionModel,
            &boxes,
            &instance,
            "prompted_segmentation",
        ),
        review_node(&[
            ArtifactKind::Polygon,
            ArtifactKind::BoundingBox,
            ArtifactKind::Classification,
            ArtifactKind::Attributes,
            ArtifactKind::Polyline,
            ArtifactKind::InstanceMask,
        ]),
        commit_node(),
    ];
    let prompted = nodes
        .iter_mut()
        .find(|node| node.id == "prompted_segmentation")
        .expect("template node exists");
    prompted
        .parameters
        .insert("optional".to_owned(), json!(true));
    for port in &mut prompted.inputs {
        port.required = false;
    }
    let review = nodes
        .iter_mut()
        .find(|node| node.id == "review")
        .expect("template review exists");
    for port in &mut review.inputs {
        if port.artifact_type == ArtifactKind::InstanceMask {
            port.required = false;
        }
    }
    let mut edges = Vec::new();
    for (from, to, kinds) in [
        ("field_segmentation", "field_geometry", mask.as_slice()),
        ("field_segmentation", "coarse_field_line", mask.as_slice()),
        ("coarse_field_line", "field_line_refiner", line.as_slice()),
        (
            "object_candidates",
            "hard_negative_validator",
            boxes.as_slice(),
        ),
        (
            "hard_negative_validator",
            "vlm_verification",
            boxes.as_slice(),
        ),
        (
            "hard_negative_validator",
            "robot_attributes",
            boxes.as_slice(),
        ),
        (
            "hard_negative_validator",
            "prompted_segmentation",
            boxes.as_slice(),
        ),
        ("field_geometry", "review", polygon.as_slice()),
        ("hard_negative_validator", "review", boxes.as_slice()),
        ("vlm_verification", "review", class.as_slice()),
        ("robot_attributes", "review", attributes.as_slice()),
        ("field_line_refiner", "review", line.as_slice()),
        ("prompted_segmentation", "review", instance.as_slice()),
    ] {
        edges.extend(edge_set(from, to, kinds));
    }
    template(
        "accurate-hybrid",
        "Accurate hybrid",
        "Field CV, specialist detection, deterministic line refinement, semantic-only VLM verification, RoboCup validation, and human review.",
        nodes,
        edges,
    )
}

fn template(
    id: &str,
    name: &str,
    description: &str,
    mut nodes: Vec<WorkflowDraftNode>,
    mut edges: Vec<WorkflowEdge>,
) -> WorkflowTemplate {
    edges.push(WorkflowEdge {
        from_node: "review".to_owned(),
        from_port: String::new(),
        to_node: "commit".to_owned(),
        to_port: String::new(),
        route: Some("approved".to_owned()),
    });
    for node in &mut nodes {
        node.depends_on = edges
            .iter()
            .filter(|edge| edge.to_node == node.id)
            .map(|edge| edge.from_node.clone())
            .collect();
    }
    WorkflowTemplate {
        id: id.to_owned(),
        name: name.to_owned(),
        description: description.to_owned(),
        nodes,
        edges,
        resource_versions: BTreeMap::from([("SKILL.md".to_owned(), "bundled".to_owned())]),
        allow_unvalidated_commit: false,
    }
}

fn model_node(
    id: &str,
    operation: &str,
    kind: WorkflowNodeKind,
    inputs: &[ArtifactKind],
    outputs: &[ArtifactKind],
    capability: &str,
) -> WorkflowDraftNode {
    let mut node = node(id, operation, kind, inputs, outputs, None, &[], &[]);
    node.parameters
        .insert("required_capability".to_owned(), json!(capability));
    node
}

#[allow(clippy::too_many_arguments)]
fn node(
    id: &str,
    node_type: &str,
    kind: WorkflowNodeKind,
    inputs: &[ArtifactKind],
    outputs: &[ArtifactKind],
    model_binding: Option<&str>,
    validators: &[&str],
    refiners: &[&str],
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: node_type.to_owned(),
        kind,
        depends_on: Vec::new(),
        inputs: ports("in", inputs),
        outputs: ports("out", outputs),
        model_binding: model_binding.map(str::to_owned),
        required_skills: vec![SKILL_ID.to_owned()],
        validators: validators.iter().map(|value| (*value).to_owned()).collect(),
        refiners: refiners.iter().map(|value| (*value).to_owned()).collect(),
        fallback: None,
        max_retries: 1,
        review_gate: false,
        parameters: BTreeMap::new(),
        retry_policy: RetryPolicy { max_attempts: 2 },
        fallback_policy: FallbackPolicy::default(),
        gate: ReviewGate::default(),
        resources: ResourceRequirements::default(),
    }
}

fn review_node(types: &[ArtifactKind]) -> WorkflowDraftNode {
    let mut node = node(
        "review",
        "review_gate",
        WorkflowNodeKind::HumanReview,
        types,
        &[],
        None,
        &[],
        &[],
    );
    node.review_gate = true;
    node.gate = ReviewGate {
        required: true,
        allow_manual_override: true,
    };
    node.parameters.insert(
        "routing".to_owned(),
        json!(
            "low risk may be approved automatically; conflicts and hard negatives require review"
        ),
    );
    node
}

fn commit_node() -> WorkflowDraftNode {
    node(
        "commit",
        "commit",
        WorkflowNodeKind::Commit,
        &[],
        &[],
        None,
        &[],
        &[],
    )
}

fn ports(prefix: &str, kinds: &[ArtifactKind]) -> Vec<NodePort> {
    kinds
        .iter()
        .map(|kind| NodePort {
            id: format!("{prefix}_{}", kind_name(*kind)),
            artifact_type: *kind,
            required: true,
            multiple: false,
        })
        .collect()
}

fn chain_edges(chains: &[(&str, &str, &[ArtifactKind])]) -> Vec<WorkflowEdge> {
    chains
        .iter()
        .flat_map(|(from, to, kinds)| edge_set(from, to, kinds))
        .collect()
}

fn edge_set(from: &str, to: &str, kinds: &[ArtifactKind]) -> Vec<WorkflowEdge> {
    kinds
        .iter()
        .map(|kind| WorkflowEdge {
            from_node: from.to_owned(),
            from_port: format!("out_{}", kind_name(*kind)),
            to_node: to.to_owned(),
            to_port: format!("in_{}", kind_name(*kind)),
            route: None,
        })
        .collect()
}

const fn kind_name(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Image => "image",
        ArtifactKind::DetectionSet => "detection_set",
        ArtifactKind::CropSet => "crop_set",
        ArtifactKind::ClassificationSet => "classification_set",
        ArtifactKind::AnnotationCandidateSet => "annotation_candidate_set",
        ArtifactKind::Classification => "classification",
        ArtifactKind::BoundingBox => "bounding_box",
        ArtifactKind::Keypoints => "keypoints",
        ArtifactKind::Polyline => "polyline",
        ArtifactKind::Polygon => "polygon",
        ArtifactKind::SemanticMask => "semantic_mask",
        ArtifactKind::InstanceMask => "instance_mask",
        ArtifactKind::Attributes => "attributes",
        ArtifactKind::Relations => "relations",
    }
}
