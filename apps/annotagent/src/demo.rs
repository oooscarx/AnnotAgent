use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    AgentBudget, Annotation, AnnotationId, AnnotationProvenance, AnnotationSource,
    AnnotationValidator, AnnotationValue, ArtifactId, ArtifactKind, ArtifactProvenance,
    ArtifactRef, ArtifactRole, ArtifactValidationState, CorrectionFeatures, CorrectionRecord,
    ImageArtifact, ImageFrame, ImageId, IssueSeverity, LabelId, MaskEncoding, ModelRegistry,
    NodePort, NormalizedRect, PipelineArtifact, ProjectId, ProjectSchema, ReviewStatus, RunId,
    Skill, SuggestedAction, TaskId, ValidationContext, ValidationEvidence, ValidationIssue,
    VisionArtifact, VisionArtifactValue, VisionCapability, VisionModelDescriptor,
    VisionNodeDescriptor, WorkflowDraftNode, WorkflowNodeKind, all_artifact_kinds,
};
use annotagent_provider::MockVisionBackend;
use annotagent_runtime::{
    CORE_CROP, CorePipelineRunner, DagNodeContext, DagNodeRunner, HybridExecutionRequest,
    HybridNodeAction, HybridWorkflowExecutor, HybridWorkflowNode, HybridWorkflowPlan,
    VisionArtifactValidator,
};
use annotagent_skill_classification::{
    CLASSIFICATION_OPERATION, ClassificationCapabilitySkill, ClassificationSkillRunner,
    MockClassificationBackend,
};
use annotagent_skill_robocup::{
    BallHardNegativeValidator, RecoveryDisposition, RoboCupBallRecoveryAgent,
    RoboCupBallRecoveryReport, RoboCupBallRecoveryRequest,
};
use annotagent_skill_yolo::{
    MockYoloBackend, YOLO_DETECTION_OPERATION, YoloCapabilitySkill, YoloDetectionSkillRunner,
};
use annotagent_storage::SqliteStore;
use anyhow::{Context, Result, bail};
use serde_json::json;
use tokio_util::sync::CancellationToken;

pub async fn run(name: &str) -> Result<()> {
    match name {
        "generic-classification" => generic_classification().await,
        "generic-detection-crop" => generic_detection_crop().await,
        "robocup-ball" => robocup_ball().await,
        "generic-workflow" => generic_workflow().await,
        "robocup-hybrid" | "robocup" => robocup_hybrid().await,
        other => bail!(
            "unknown demo {other:?}; available demos: generic-classification, \
             generic-detection-crop, robocup-ball, generic-workflow, robocup-hybrid"
        ),
    }
}

async fn generic_classification() -> Result<()> {
    let skill = ClassificationCapabilitySkill::default();
    let image_id = ImageId::new();
    let image = pipeline_image(image_id);
    let node = pipeline_node(
        "classifier",
        CLASSIFICATION_OPERATION,
        WorkflowNodeKind::VisionModel,
        vec![pipeline_port("image", ArtifactKind::Image)],
        vec![pipeline_port(
            "classifications",
            ArtifactKind::ClassificationSet,
        )],
        BTreeMap::from([
            ("labels".to_owned(), json!(["day", "night"])),
            ("mock_label".to_owned(), json!("day")),
            ("mock_confidence".to_owned(), json!(0.96)),
        ]),
    );
    let runner = ClassificationSkillRunner::new(
        Arc::new(MockClassificationBackend::new("offline-classifier")),
        "offline-classifier",
        None,
    )?;
    let output = runner
        .run(DagNodeContext {
            project_id: ProjectId::new(),
            run_id: RunId::new(),
            image_id,
            node: &node,
            input_artifacts: Vec::new(),
            input_pipeline_artifacts: vec![image],
            input_metadata: BTreeMap::new(),
            cancellation: CancellationToken::new(),
        })
        .await
        .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.summary))?;
    let PipelineArtifact::ClassificationSet(classifications) = &output.pipeline_artifacts[0] else {
        bail!("Classification Skill did not produce ClassificationSet");
    };
    let classification = classifications
        .classifications
        .first()
        .context("ClassificationSet was empty")?;
    println!("AnnotAgent Agent + Skill Alpha · Generic Classification · offline mock");
    println!(
        "Skill loaded: {}@{} ({:?})",
        skill.id(),
        skill.manifest().skill_version,
        skill.manifest().kind
    );
    println!("Workflow: Image → Classification Skill → Commit");
    println!(
        "result label={} confidence={:.2} subject={} parent={}",
        classification.label,
        classification.confidence,
        classification.subject.artifact_id,
        classification
            .parent
            .as_ref()
            .map_or("none", |parent| parent.artifact_id.as_str())
    );
    println!("status=completed model_calls=1 tokens=0 cost=0 stop=commit completed");
    Ok(())
}

async fn generic_detection_crop() -> Result<()> {
    let skill = YoloCapabilitySkill::default();
    let image_id = ImageId::new();
    let image = pipeline_image(image_id);
    let detector_node = pipeline_node(
        "detector",
        YOLO_DETECTION_OPERATION,
        WorkflowNodeKind::VisionModel,
        vec![pipeline_port("image", ArtifactKind::Image)],
        vec![pipeline_port("detections", ArtifactKind::DetectionSet)],
        BTreeMap::from([
            ("mock_count".to_owned(), json!(1)),
            ("mock_class_id".to_owned(), json!("0")),
            ("class_mapping".to_owned(), json!({"0": "target"})),
            ("confidence_threshold".to_owned(), json!(0.8)),
        ]),
    );
    let detector = YoloDetectionSkillRunner::new(
        Arc::new(MockYoloBackend::new("offline-detector")),
        "offline-detector",
        None,
    )?;
    let detected = detector
        .run(DagNodeContext {
            project_id: ProjectId::new(),
            run_id: RunId::new(),
            image_id,
            node: &detector_node,
            input_artifacts: Vec::new(),
            input_pipeline_artifacts: vec![image.clone()],
            input_metadata: BTreeMap::new(),
            cancellation: CancellationToken::new(),
        })
        .await
        .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.summary))?;
    let detection_set = detected
        .pipeline_artifacts
        .first()
        .cloned()
        .context("YOLO Detection Skill returned no DetectionSet")?;
    let crop_node = pipeline_node(
        "crop",
        CORE_CROP,
        WorkflowNodeKind::Transform,
        vec![
            pipeline_port("image", ArtifactKind::Image),
            pipeline_port("detections", ArtifactKind::DetectionSet),
        ],
        vec![pipeline_port("crops", ArtifactKind::CropSet)],
        BTreeMap::from([("padding".to_owned(), json!(0.05))]),
    );
    let cropped = CorePipelineRunner
        .run(DagNodeContext {
            project_id: ProjectId::new(),
            run_id: RunId::new(),
            image_id,
            node: &crop_node,
            input_artifacts: Vec::new(),
            input_pipeline_artifacts: vec![image, detection_set],
            input_metadata: BTreeMap::new(),
            cancellation: CancellationToken::new(),
        })
        .await
        .map_err(|error| anyhow::anyhow!("{}: {}", error.code, error.summary))?;
    let PipelineArtifact::CropSet(crops) = &cropped.pipeline_artifacts[0] else {
        bail!("Core Crop did not produce CropSet");
    };
    let crop = crops.crops.first().context("CropSet was empty")?;
    println!("AnnotAgent Agent + Skill Alpha · Generic Detection + Crop · offline mock");
    println!(
        "Skill loaded: {}@{} ({:?})",
        skill.id(),
        skill.manifest().skill_version,
        skill.manifest().kind
    );
    println!("Workflow: Image → YOLO DetectionSet → Core Crop → CropSet");
    println!(
        "result crops={} parent_artifact={} parent_item={} size={}x{} cache={}",
        crops.crops.len(),
        crop.parent.artifact_id,
        crop.parent.item_id.as_deref().unwrap_or("none"),
        crop.crop_width,
        crop.crop_height,
        crop.cache_key.as_deref().unwrap_or("none")
    );
    println!("status=completed model_calls=1 tokens=0 cost=0 stop=typed CropSet completed");
    Ok(())
}

async fn robocup_ball() -> Result<()> {
    let project_id = ProjectId::new();
    let project_root = std::env::temp_dir();
    let image = Arc::new(load_embedded_robocup_image()?);
    let candidate = ball_candidate();
    println!("AnnotAgent Agent + Skill Alpha · RoboCup Ball · offline mock");
    println!("Skill loaded: robocup.ball@1 (Domain) → classification@1");

    let normal = run_recovery_case(
        project_id,
        &project_root,
        &candidate,
        image.clone(),
        Vec::new(),
        Vec::new(),
    )
    .await?;
    print_recovery_case(
        "Case 1 · normal football · Validator pass → Gate → Commit",
        "pass",
        &normal,
    );
    if normal.disposition != RecoveryDisposition::Accept || !normal.fast_path {
        bail!("normal football did not use the deterministic fast path");
    }

    let shoe = run_recovery_case(
        project_id,
        &project_root,
        &candidate,
        image.clone(),
        vec![demo_issue("possible_white_shoe")],
        Vec::new(),
    )
    .await?;
    print_recovery_case(
        "Case 2 · white shoe · Validator → Memory → Crop verify",
        "possible_white_shoe",
        &shoe,
    );
    if shoe.disposition == RecoveryDisposition::Accept {
        bail!("white-shoe hard negative was incorrectly accepted");
    }

    let penalty = run_recovery_case(
        project_id,
        &project_root,
        &candidate,
        image.clone(),
        vec![demo_issue("possible_penalty_mark")],
        Vec::new(),
    )
    .await?;
    print_recovery_case(
        "Case 3 · penalty mark · Validator → Human Review",
        "possible_penalty_mark",
        &penalty,
    );
    if penalty.disposition != RecoveryDisposition::HumanReview {
        bail!("penalty-mark hard negative bypassed Human Review");
    }

    let first = run_recovery_case(
        project_id,
        &project_root,
        &candidate,
        image.clone(),
        vec![demo_issue("inaccurate_ball_bbox")],
        Vec::new(),
    )
    .await?;
    print_recovery_case(
        "Case 4a · first similar candidate · Review and operator rejection",
        "inaccurate_ball_bbox",
        &first,
    );
    if first.disposition != RecoveryDisposition::HumanReview {
        bail!("first uncertain candidate did not require operator review");
    }
    let store = SqliteStore::open_in_memory()?;
    let correction = demo_correction(project_id, &candidate, "white_shoe_as_ball");
    store.save_correction(&correction)?;
    let memory = store.query_corrections(
        project_id,
        "robocup.ball",
        &candidate.task_id,
        candidate.label.as_ref(),
        10,
    )?;
    println!(
        "Memory write: project={} skill=robocup.ball reason=white_shoe_as_ball records={}",
        project_id,
        memory.len()
    );
    let second = run_recovery_case(
        project_id,
        &project_root,
        &candidate,
        image,
        vec![demo_issue("inaccurate_ball_bbox")],
        memory,
    )
    .await?;
    print_recovery_case(
        "Case 4b · second similar candidate · Memory raises risk",
        "inaccurate_ball_bbox + scoped Memory",
        &second,
    );
    if second.disposition != RecoveryDisposition::Reject || !second.memory_changed_decision {
        bail!("scoped Correction Memory did not change the second decision");
    }
    println!("demo=passed cases=4 external_requests=0");
    Ok(())
}

fn pipeline_image(image_id: ImageId) -> PipelineArtifact {
    PipelineArtifact::Image(ImageArtifact {
        reference: ArtifactRef {
            artifact_id: format!("image:{image_id}"),
            source_node: "image".to_owned(),
            port: "image".to_owned(),
            artifact_type: ArtifactKind::Image,
            item_id: None,
        },
        image_id,
        width: 640,
        height: 480,
        mime_type: "image/png".to_owned(),
        blob_ref: "workspace://offline-demo.png".to_owned(),
    })
}

fn pipeline_port(id: &str, artifact_type: ArtifactKind) -> NodePort {
    NodePort {
        id: id.to_owned(),
        artifact_type,
        required: true,
        multiple: false,
    }
}

fn pipeline_node(
    id: &str,
    node_type: &str,
    kind: WorkflowNodeKind,
    inputs: Vec<NodePort>,
    outputs: Vec<NodePort>,
    parameters: BTreeMap<String, serde_json::Value>,
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: node_type.to_owned(),
        kind,
        inputs,
        outputs,
        parameters,
        ..WorkflowDraftNode::default()
    }
}

fn ball_candidate() -> Annotation {
    Annotation {
        id: AnnotationId::new(),
        image_id: ImageId::new(),
        task_id: TaskId::from("objects"),
        label: Some(LabelId::from("ball")),
        value: AnnotationValue::BoundingBox {
            rect: NormalizedRect::new(0.218, 0.615, 0.04, 0.03)
                .expect("embedded demo geometry is valid"),
        },
        attributes: BTreeMap::new(),
        confidence: Some(0.94),
        source: AnnotationSource::Model,
        review_status: ReviewStatus::Draft,
        provenance: AnnotationProvenance::default(),
        created_at: chrono::Utc::now(),
    }
}

fn demo_issue(code: &str) -> ValidationIssue {
    ValidationIssue {
        code: code.to_owned(),
        severity: IssueSeverity::Warning,
        annotation_ids: Vec::new(),
        message: code.replace('_', " "),
        suggested_action: SuggestedAction::HumanReview,
        evidence: ValidationEvidence::Rule {
            facts: BTreeMap::from([("offline_fixture".to_owned(), code.to_owned())]),
        },
    }
}

fn demo_correction(
    project_id: ProjectId,
    candidate: &Annotation,
    reason: &str,
) -> CorrectionRecord {
    CorrectionRecord {
        id: uuid::Uuid::new_v4(),
        project_id,
        skill_id: "robocup.ball".to_owned(),
        task_id: candidate.task_id.clone(),
        predicted_label: candidate.label.clone(),
        corrected_label: None,
        reason_code: reason.to_owned(),
        original_annotation: None,
        corrected_annotation: None,
        note: Some("operator rejected the hard negative".to_owned()),
        image_features: CorrectionFeatures {
            geometry: BTreeMap::new(),
            colors: BTreeMap::new(),
        },
        created_at: chrono::Utc::now(),
    }
}

async fn run_recovery_case(
    project_id: ProjectId,
    project_root: &std::path::Path,
    candidate: &Annotation,
    image: Arc<ImageFrame>,
    issues: Vec<ValidationIssue>,
    correction_memory: Vec<CorrectionRecord>,
) -> Result<RoboCupBallRecoveryReport> {
    RoboCupBallRecoveryAgent
        .run(RoboCupBallRecoveryRequest {
            project_id,
            project_root: project_root.to_path_buf(),
            candidate: candidate.clone(),
            related_annotations: Vec::new(),
            issues,
            correction_memory,
            image: Some(image),
            budget: AgentBudget::default(),
            cancellation: CancellationToken::new(),
        })
        .await
        .map_err(anyhow::Error::msg)
}

fn print_recovery_case(name: &str, validator: &str, report: &RoboCupBallRecoveryReport) {
    println!("\n{name}");
    println!("Validator: {validator}");
    println!(
        "Memory: matches={} changed_decision={}",
        report.memory_matches, report.memory_changed_decision
    );
    println!(
        "Decision: {:?} · reasons={}",
        report.disposition,
        report.reasons.join("; ")
    );
    if let Some(session) = &report.session {
        println!(
            "Agent steps={} tool_calls={} tokens={} cost={} stop={}",
            session.usage.steps,
            session.usage.tool_calls,
            session.usage.input_tokens + session.usage.output_tokens,
            session.usage.cost,
            session.stop_reason.as_deref().unwrap_or("running")
        );
        for step in &session.steps {
            println!(
                "  tool {} {} → success={}",
                step.sequence, step.tool_name, step.success
            );
        }
    } else {
        println!("Agent steps=0 tool_calls=0 tokens=0 cost=0 stop=deterministic fast path");
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
