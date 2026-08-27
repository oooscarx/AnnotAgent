use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    Annotation, AnnotationId, AnnotationProvenance, AnnotationSource, AnnotationValidator,
    ArtifactId, ArtifactKind, ArtifactProvenance, ArtifactRole, ArtifactValidationState,
    ImageFrame, ImageId, LabelId, MaskEncoding, ModelRegistry, NormalizedRect, ProjectSchema,
    ReviewStatus, RunId, TaskId, ValidationContext, VisionArtifact, VisionArtifactValue,
    VisionCapability, VisionModelDescriptor, VisionNodeDescriptor, all_artifact_kinds,
};
use annotagent_provider::MockVisionBackend;
use annotagent_runtime::{
    HybridExecutionRequest, HybridNodeAction, HybridWorkflowExecutor, HybridWorkflowNode,
    HybridWorkflowPlan, VisionArtifactValidator,
};
use annotagent_skill_robocup::BallHardNegativeValidator;
use anyhow::{Result, bail};
use tokio_util::sync::CancellationToken;

pub async fn run(name: &str) -> Result<()> {
    match name {
        "generic-workflow" => generic_workflow().await,
        "robocup-hybrid" | "robocup" => robocup_hybrid().await,
        other => bail!("unknown demo {other:?}; available demos: generic-workflow, robocup-hybrid"),
    }
}

async fn generic_workflow() -> Result<()> {
    let detector = artifact(
        "detector",
        Some("component"),
        VisionArtifactValue::BoundingBox {
            rect: NormalizedRect::new(0.2, 0.2, 0.3, 0.25)?,
        },
    );
    let segmenter = artifact(
        "segmenter",
        Some("component"),
        VisionArtifactValue::InstanceMask {
            mask: MaskEncoding::CocoRle {
                width: 2,
                height: 2,
                counts: "4".to_owned(),
            },
        },
    );
    let mut models = ModelRegistry::new();
    register_mock_model(
        &mut models,
        "detector",
        VisionCapability::ObjectDetection,
        detector,
    )?;
    register_mock_model(
        &mut models,
        "segmenter",
        VisionCapability::PromptedSegmentation,
        segmenter,
    )?;
    let nodes = hybrid_nodes(&[
        (
            "object_detection",
            Some(VisionCapability::ObjectDetection),
            vec![ArtifactKind::BoundingBox],
        ),
        (
            "prompted_segmentation",
            Some(VisionCapability::PromptedSegmentation),
            vec![ArtifactKind::InstanceMask],
        ),
        ("static_validator", None, all_artifact_kinds().to_vec()),
        ("review_gate", None, all_artifact_kinds().to_vec()),
        ("commit", None, all_artifact_kinds().to_vec()),
    ])?;
    let plan = hybrid_plan(
        "generic-workflow",
        "detector",
        "object_detection",
        Some(("segmenter", "prompted_segmentation")),
        "generic_shape_validator",
    );
    let mut executor = HybridWorkflowExecutor::new(&models, &nodes);
    executor.register_validator(Arc::new(GenericShapeValidator))?;
    let result = executor
        .execute(
            &plan,
            HybridExecutionRequest {
                run_id: RunId::new(),
                image_id: ImageId::new(),
                task_id: TaskId::from("components"),
                image: None,
            },
            CancellationToken::new(),
        )
        .await?;
    if result.needs_review || result.committed.len() != 2 {
        bail!("generic demo did not reach its validated Commit node");
    }
    println!("AnnotAgent Workflow Alpha · Generic Workflow · offline mock");
    print_result(&result, "completed");
    Ok(())
}

async fn robocup_hybrid() -> Result<()> {
    let image = load_embedded_robocup_image()?;
    let ball = artifact(
        "detector",
        Some("ball"),
        VisionArtifactValue::BoundingBox {
            rect: NormalizedRect::new(0.218, 0.615, 0.04, 0.03)?,
        },
    );
    let robot = artifact(
        "detector",
        Some("robot"),
        VisionArtifactValue::BoundingBox {
            rect: NormalizedRect::new(0.225, 0.445, 0.07, 0.2)?,
        },
    );
    let semantics = artifact(
        "vlm_semantics",
        Some("possible_ball"),
        VisionArtifactValue::Classification {
            labels: vec![LabelId::from("possible_white_object")],
        },
    );
    let mut models = ModelRegistry::new();
    let detector_backend = Arc::new(MockVisionBackend::new(
        "detector-backend",
        vec![VisionCapability::ObjectDetection],
        vec![ball, robot],
    ));
    models.register_backend(detector_backend)?;
    models.register_model(VisionModelDescriptor {
        id: "detector".to_owned(),
        backend_id: "detector-backend".to_owned(),
        capabilities: vec![VisionCapability::ObjectDetection],
        ..VisionModelDescriptor::default()
    })?;
    register_mock_model(
        &mut models,
        "vlm",
        VisionCapability::VisionLanguage,
        semantics,
    )?;
    let nodes = hybrid_nodes(&[
        (
            "object_detection",
            Some(VisionCapability::ObjectDetection),
            vec![ArtifactKind::BoundingBox],
        ),
        (
            "vision_language",
            Some(VisionCapability::VisionLanguage),
            vec![ArtifactKind::Classification],
        ),
        ("static_validator", None, all_artifact_kinds().to_vec()),
        ("review_gate", None, all_artifact_kinds().to_vec()),
        ("commit", None, all_artifact_kinds().to_vec()),
    ])?;
    let plan = hybrid_plan(
        "robocup-hybrid",
        "detector",
        "object_detection",
        Some(("vlm", "vision_language")),
        "robocup_ball_hard_negative",
    );
    let mut executor = HybridWorkflowExecutor::new(&models, &nodes);
    executor.register_validator(Arc::new(RoboCupArtifactValidator {
        project: ProjectSchema::from_yaml(include_str!("../../../examples/robocup/project.yaml"))
            .map_err(|error| anyhow::anyhow!(error.to_string()))?,
        image,
    }))?;
    let result = executor
        .execute(
            &plan,
            HybridExecutionRequest {
                run_id: RunId::new(),
                image_id: ImageId::new(),
                task_id: TaskId::from("objects"),
                image: None,
            },
            CancellationToken::new(),
        )
        .await?;
    if !result.needs_review
        || !result
            .validation_issues
            .iter()
            .any(|issue| issue == "possible_white_shoe")
        || !result.committed.is_empty()
    {
        bail!("RoboCup demo did not route its hard negative to Review");
    }
    println!("AnnotAgent Workflow Alpha · RoboCup Hybrid Skill · offline mock");
    print_result(&result, "completed_with_review");
    Ok(())
}

fn print_result(result: &annotagent_runtime::HybridExecutionResult, status: &str) {
    println!(
        "status={status} artifacts={} committed={} review={} model_calls={}",
        result.artifacts.len(),
        result.committed.len(),
        result.needs_review,
        result.usage.model_calls
    );
    for node in &result.trace {
        println!("trace {node}");
    }
    for issue in &result.validation_issues {
        println!("validation {issue}");
    }
}

fn artifact(source_node: &str, label: Option<&str>, value: VisionArtifactValue) -> VisionArtifact {
    VisionArtifact {
        id: ArtifactId::new(),
        image_id: ImageId::new(),
        task_id: None,
        label: label.map(LabelId::from),
        role: ArtifactRole::Candidate,
        value,
        source_node: source_node.to_owned(),
        confidence: Some(0.95),
        metadata: BTreeMap::new(),
        validation_state: ArtifactValidationState::Unvalidated,
        provenance: ArtifactProvenance::default(),
        revision: 1,
        replaces_artifact_id: None,
        created_at: chrono::Utc::now(),
    }
}

fn register_mock_model(
    models: &mut ModelRegistry,
    id: &str,
    capability: VisionCapability,
    output: VisionArtifact,
) -> Result<()> {
    let backend_id = format!("{id}-backend");
    models.register_backend(Arc::new(MockVisionBackend::new(
        &backend_id,
        vec![capability],
        vec![output],
    )))?;
    models.register_model(VisionModelDescriptor {
        id: id.to_owned(),
        backend_id,
        capabilities: vec![capability],
        ..VisionModelDescriptor::default()
    })?;
    Ok(())
}

fn hybrid_nodes(
    descriptors: &[(&str, Option<VisionCapability>, Vec<ArtifactKind>)],
) -> Result<annotagent_core::NodeRegistry> {
    let mut nodes = annotagent_core::NodeRegistry::new();
    for (id, capability, produces) in descriptors {
        nodes.register(VisionNodeDescriptor {
            id: (*id).to_owned(),
            display_name: (*id).replace('_', " "),
            required_capabilities: capability.iter().copied().collect(),
            accepts: all_artifact_kinds().to_vec(),
            produces: produces.clone(),
            deterministic: capability.is_none(),
        })?;
    }
    Ok(nodes)
}

fn hybrid_plan(
    id: &str,
    first_model: &str,
    first_node_type: &str,
    second_model: Option<(&str, &str)>,
    validator_id: &str,
) -> HybridWorkflowPlan {
    let mut nodes = vec![HybridWorkflowNode {
        id: "candidate".to_owned(),
        node_type: first_node_type.to_owned(),
        depends_on: Vec::new(),
        action: HybridNodeAction::Model {
            model_id: first_model.to_owned(),
        },
        parameters: BTreeMap::new(),
    }];
    let validation_dependencies = if let Some((model, node_type)) = second_model {
        nodes.push(HybridWorkflowNode {
            id: "specialist_or_semantic".to_owned(),
            node_type: node_type.to_owned(),
            depends_on: vec!["candidate".to_owned()],
            action: HybridNodeAction::Model {
                model_id: model.to_owned(),
            },
            parameters: BTreeMap::from([(
                "geometry_policy".to_owned(),
                serde_json::json!("read_only"),
            )]),
        });
        vec!["candidate".to_owned(), "specialist_or_semantic".to_owned()]
    } else {
        vec!["candidate".to_owned()]
    };
    nodes.extend([
        HybridWorkflowNode {
            id: "validate".to_owned(),
            node_type: "static_validator".to_owned(),
            depends_on: validation_dependencies,
            action: HybridNodeAction::StaticValidator {
                validator_id: validator_id.to_owned(),
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
    ]);
    HybridWorkflowPlan {
        id: id.to_owned(),
        nodes,
    }
}

struct GenericShapeValidator;

impl VisionArtifactValidator for GenericShapeValidator {
    fn id(&self) -> &str {
        "generic_shape_validator"
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
            vec!["detector_and_segmenter_outputs_required".to_owned()]
        }
    }
}

struct RoboCupArtifactValidator {
    project: ProjectSchema,
    image: ImageFrame,
}

impl VisionArtifactValidator for RoboCupArtifactValidator {
    fn id(&self) -> &str {
        "robocup_ball_hard_negative"
    }

    fn validate(&self, artifacts: &[VisionArtifact]) -> Vec<String> {
        let annotations = artifacts
            .iter()
            .map(artifact_annotation)
            .collect::<Vec<_>>();
        annotations
            .iter()
            .filter(|annotation| {
                annotation
                    .label
                    .as_ref()
                    .is_some_and(|label| label.as_str() == "ball")
            })
            .flat_map(|candidate| {
                BallHardNegativeValidator::default()
                    .validate(&ValidationContext {
                        project: &self.project,
                        image: Some(&self.image),
                        candidate,
                        related_annotations: &annotations,
                        correction_risk: 0.0,
                    })
                    .unwrap_or_default()
                    .into_iter()
                    .map(|issue| issue.code)
            })
            .collect()
    }
}

fn artifact_annotation(artifact: &VisionArtifact) -> Annotation {
    Annotation {
        id: AnnotationId::new(),
        image_id: artifact.image_id,
        task_id: artifact
            .task_id
            .clone()
            .unwrap_or_else(|| TaskId::from("objects")),
        label: artifact.label.clone(),
        value: artifact.value.as_annotation_value(),
        attributes: BTreeMap::new(),
        confidence: artifact.confidence,
        source: AnnotationSource::Model,
        review_status: ReviewStatus::Draft,
        provenance: AnnotationProvenance::default(),
        created_at: artifact.created_at,
    }
}

fn load_embedded_robocup_image() -> Result<ImageFrame> {
    let path = std::env::temp_dir().join(format!(
        "annotagent-robocup-demo-{}.png",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(
        &path,
        include_bytes!("../../../examples/robocup/images/synthetic-robocup.png"),
    )?;
    let loaded = annotagent_image_tools::load_image(&path, 1_000_000)
        .map_err(|error| anyhow::anyhow!(error));
    let _ignored = std::fs::remove_file(&path);
    loaded
}
