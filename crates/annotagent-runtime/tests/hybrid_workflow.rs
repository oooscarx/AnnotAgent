use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    ArtifactId, ArtifactKind, ArtifactProvenance, ArtifactRole, ArtifactValidationState, ImageId,
    LabelId, ModelRegistry, NodeRegistry, NormalizedRect, ProjectSchema, RegistryWorkflowAdvisor,
    RunId, VisionArtifact, VisionArtifactValue, VisionCapability, VisionModelDescriptor,
    VisionNodeDescriptor, WorkflowAdvisor, WorkflowConstraints, all_artifact_kinds,
};
use annotagent_provider::MockVisionBackend;
use annotagent_runtime::{
    HybridExecutionRequest, HybridNodeAction, HybridWorkflowExecutor, HybridWorkflowNode,
    HybridWorkflowPlan, VisionArtifactValidator,
};
use tokio_util::sync::CancellationToken;

fn artifact(value: VisionArtifactValue, source: &str) -> VisionArtifact {
    VisionArtifact {
        id: ArtifactId::new(),
        image_id: ImageId::new(),
        task_id: None,
        label: Some(LabelId::from("target")),
        role: ArtifactRole::Candidate,
        value,
        source_node: source.to_owned(),
        confidence: Some(0.9),
        metadata: BTreeMap::new(),
        validation_state: ArtifactValidationState::Unvalidated,
        provenance: ArtifactProvenance::default(),
        revision: 1,
        replaces_artifact_id: None,
        created_at: chrono::Utc::now(),
    }
}

fn node(
    id: &str,
    capability: Option<VisionCapability>,
    produces: Vec<ArtifactKind>,
) -> VisionNodeDescriptor {
    VisionNodeDescriptor {
        id: id.to_owned(),
        display_name: id.to_owned(),
        required_capabilities: capability.into_iter().collect(),
        accepts: all_artifact_kinds().to_vec(),
        produces,
        deterministic: capability.is_none(),
    }
}

struct ShapeValidator;

impl VisionArtifactValidator for ShapeValidator {
    fn id(&self) -> &str {
        "shape_validator"
    }

    fn validate(&self, artifacts: &[VisionArtifact]) -> Vec<String> {
        let has_box = artifacts
            .iter()
            .any(|artifact| matches!(artifact.value, VisionArtifactValue::BoundingBox { .. }));
        let has_mask = artifacts
            .iter()
            .any(|artifact| matches!(artifact.value, VisionArtifactValue::InstanceMask { .. }));
        if has_box && has_mask {
            Vec::new()
        } else {
            vec!["detector and prompted segmenter outputs are both required".to_owned()]
        }
    }
}

#[tokio::test]
async fn generic_hybrid_detector_segmenter_validator_review_gate_and_commit() {
    let detector_artifact = artifact(
        VisionArtifactValue::BoundingBox {
            rect: NormalizedRect::new(0.2, 0.2, 0.3, 0.3).expect("box"),
        },
        "mock_detector",
    );
    let segmenter_artifact = artifact(
        VisionArtifactValue::InstanceMask {
            mask: annotagent_core::MaskEncoding::CocoRle {
                width: 2,
                height: 2,
                counts: "4".to_owned(),
            },
        },
        "mock_prompted_segmenter",
    );
    let mut models = ModelRegistry::new();
    models
        .register_backend(Arc::new(MockVisionBackend::new(
            "detector-backend",
            vec![VisionCapability::ObjectDetection],
            vec![detector_artifact],
        )))
        .expect("detector backend");
    models
        .register_backend(Arc::new(MockVisionBackend::new(
            "segmenter-backend",
            vec![VisionCapability::PromptedSegmentation],
            vec![segmenter_artifact],
        )))
        .expect("segmenter backend");
    models
        .register_model(VisionModelDescriptor {
            id: "detector".to_owned(),
            backend_id: "detector-backend".to_owned(),
            capabilities: vec![VisionCapability::ObjectDetection],
            configuration: BTreeMap::new(),
        })
        .expect("detector model");
    models
        .register_model(VisionModelDescriptor {
            id: "segmenter".to_owned(),
            backend_id: "segmenter-backend".to_owned(),
            capabilities: vec![VisionCapability::PromptedSegmentation],
            configuration: BTreeMap::new(),
        })
        .expect("segmenter model");

    let mut nodes = NodeRegistry::new();
    for descriptor in [
        node(
            "object_detection",
            Some(VisionCapability::ObjectDetection),
            vec![ArtifactKind::BoundingBox],
        ),
        node(
            "prompted_segmentation",
            Some(VisionCapability::PromptedSegmentation),
            vec![ArtifactKind::InstanceMask],
        ),
        node("static_validator", None, all_artifact_kinds().to_vec()),
        node("review_gate", None, all_artifact_kinds().to_vec()),
        node("commit", None, all_artifact_kinds().to_vec()),
    ] {
        nodes.register(descriptor).expect("node");
    }
    let plan = HybridWorkflowPlan {
        id: "generic-hybrid".to_owned(),
        nodes: vec![
            HybridWorkflowNode {
                id: "detect".to_owned(),
                node_type: "object_detection".to_owned(),
                depends_on: Vec::new(),
                action: HybridNodeAction::Model {
                    model_id: "detector".to_owned(),
                },
                parameters: BTreeMap::new(),
            },
            HybridWorkflowNode {
                id: "segment".to_owned(),
                node_type: "prompted_segmentation".to_owned(),
                depends_on: vec!["detect".to_owned()],
                action: HybridNodeAction::Model {
                    model_id: "segmenter".to_owned(),
                },
                parameters: BTreeMap::new(),
            },
            HybridWorkflowNode {
                id: "validate".to_owned(),
                node_type: "static_validator".to_owned(),
                depends_on: vec!["detect".to_owned(), "segment".to_owned()],
                action: HybridNodeAction::StaticValidator {
                    validator_id: "shape_validator".to_owned(),
                },
                parameters: BTreeMap::new(),
            },
            HybridWorkflowNode {
                id: "review".to_owned(),
                node_type: "review_gate".to_owned(),
                depends_on: vec!["validate".to_owned()],
                action: HybridNodeAction::ReviewGate,
                parameters: BTreeMap::new(),
            },
            HybridWorkflowNode {
                id: "commit".to_owned(),
                node_type: "commit".to_owned(),
                depends_on: vec!["review".to_owned()],
                action: HybridNodeAction::Commit,
                parameters: BTreeMap::new(),
            },
        ],
    };
    let mut executor = HybridWorkflowExecutor::new(&models, &nodes);
    executor
        .register_validator(Arc::new(ShapeValidator))
        .expect("validator");
    let result = executor
        .execute(
            &plan,
            HybridExecutionRequest {
                run_id: RunId::new(),
                image_id: ImageId::new(),
                task_id: annotagent_core::TaskId::from("target"),
                image: None,
            },
            CancellationToken::new(),
        )
        .await
        .expect("hybrid execution");
    assert!(result.validation_issues.is_empty());
    assert!(!result.needs_review);
    assert_eq!(result.committed.len(), 2);
    assert_eq!(result.trace.len(), 5);
}

#[test]
fn generic_project_suggestion_contains_no_domain_specific_logic() {
    let project = ProjectSchema::from_yaml(
        r#"
version: 1
project: {name: Generic Factory Dataset, skill: generic, skill_version: "1", language: en}
dataset: {root: images, include: ["**/*.png"], recursive: true}
runtime:
  max_parallel_images: 1
  max_model_turns_per_task: 4
  max_tool_calls_per_task: 6
  max_recovery_turns_per_task: 1
  task_timeout_seconds: 60
  provider_request_timeout_seconds: 30
  max_retries: 1
  auto_resume: false
tasks:
  - {id: parts, kind: bounding_box, labels: [part], required: true}
review: {auto_accept_confidence: 0.9, force_review_below: 0.5, force_review_on_warning_codes: []}
export: {formats: [native]}
"#,
    )
    .expect("generic project");
    let mut nodes = NodeRegistry::new();
    nodes
        .register(node(
            "vision_language",
            Some(VisionCapability::VisionLanguage),
            all_artifact_kinds().to_vec(),
        ))
        .expect("node");
    let mut models = ModelRegistry::new();
    models
        .register_backend(Arc::new(MockVisionBackend::new(
            "vlm-backend",
            vec![VisionCapability::VisionLanguage],
            Vec::new(),
        )))
        .expect("backend");
    models
        .register_model(VisionModelDescriptor {
            id: "generic-vlm".to_owned(),
            backend_id: "vlm-backend".to_owned(),
            capabilities: vec![VisionCapability::VisionLanguage],
            configuration: BTreeMap::new(),
        })
        .expect("model");
    let suggestion = RegistryWorkflowAdvisor.suggest_workflow(
        "factory",
        &project,
        &["generic".to_owned()],
        &nodes,
        &models,
        &WorkflowConstraints::default(),
    );
    let serialized = serde_json::to_string(&suggestion).expect("suggestion");
    for forbidden in [
        "robocup",
        "penalty_mark",
        "field_line",
        "ball_hard_negative",
    ] {
        assert!(!serialized.to_ascii_lowercase().contains(forbidden));
    }
}
