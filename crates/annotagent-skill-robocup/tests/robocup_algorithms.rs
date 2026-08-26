use std::collections::BTreeMap;

use annotagent_core::{
    AgentTool, Annotation, AnnotationId, AnnotationProvenance, AnnotationRefiner, AnnotationSource,
    AnnotationValidator, AnnotationValue, AttributeValue, DomainSkill, ImageFrame, ImageId,
    Keypoint, LabelId, NormalizedPoint, NormalizedRect, ProjectSchema, RefinementContext,
    ReviewContext, ReviewDecision, ReviewPolicy, ReviewStatus, RunId, TaskId, ToolContext,
    ValidationContext,
};
use annotagent_image_tools::{generate_synthetic_robocup, load_image};
use annotagent_skill_robocup::{
    BallHardNegativeValidator, EvaluationGroundTruth, EvaluationPredictions, EvaluationThresholds,
    FieldContainmentValidator, RoboCupFieldLineRefiner, RoboCupReviewPolicy, RoboCupSkill,
    RobotAttributeValidator, TeamColorEvidenceTool, evaluate, evaluate_with_thresholds,
};
use chrono::Utc;
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn project() -> ProjectSchema {
    ProjectSchema::from_yaml(include_str!("../../../examples/robocup/project.yaml"))
        .expect("example project parses")
}

fn fixture() -> (tempfile::TempDir, ImageFrame) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("field.png");
    generate_synthetic_robocup(&path).expect("synthetic image");
    let image = load_image(&path, 1_000_000).expect("load image");
    (directory, image)
}

fn annotation(label: &str, task: &str, value: AnnotationValue) -> Annotation {
    Annotation {
        id: AnnotationId::new(),
        image_id: ImageId::new(),
        task_id: TaskId::from(task),
        label: Some(LabelId::from(label)),
        value,
        attributes: BTreeMap::new(),
        confidence: Some(0.95),
        source: AnnotationSource::Model,
        review_status: ReviewStatus::Draft,
        provenance: AnnotationProvenance::default(),
        created_at: Utc::now(),
    }
}

fn point(x: f32, y: f32) -> NormalizedPoint {
    NormalizedPoint::new(x, y).expect("valid point")
}

fn full_field() -> Annotation {
    annotation(
        "field",
        "field_region",
        AnnotationValue::Polygon {
            rings: vec![vec![
                point(0.01, 0.01),
                point(0.99, 0.01),
                point(0.99, 0.99),
                point(0.01, 0.99),
            ]],
        },
    )
}

#[test]
fn containment_passes_inside_and_degrades_without_region() {
    let candidate = annotation(
        "ball",
        "objects",
        AnnotationValue::BoundingBox {
            rect: NormalizedRect::new(0.5, 0.5, 0.04, 0.04).expect("rect"),
        },
    );
    let field = full_field();
    let validator = FieldContainmentValidator;
    let with_field = validator
        .validate(&ValidationContext {
            project: &project(),
            image: None,
            candidate: &candidate,
            related_annotations: &[field],
            correction_risk: 0.0,
        })
        .expect("containment");
    assert!(with_field.is_empty());
    let without_field = validator
        .validate(&ValidationContext {
            project: &project(),
            image: None,
            candidate: &candidate,
            related_annotations: &[],
            correction_risk: 0.0,
        })
        .expect("downgraded containment");
    assert_eq!(without_field[0].code, "field_region_missing");
}

#[test]
fn white_shoe_and_penalty_mark_are_structured_ball_risks() {
    let (_temporary, image) = fixture();
    let candidate = annotation(
        "ball",
        "objects",
        AnnotationValue::BoundingBox {
            rect: NormalizedRect::new(0.218, 0.615, 0.04, 0.03).expect("shoe rect"),
        },
    );
    let robot = annotation(
        "robot",
        "objects",
        AnnotationValue::BoundingBox {
            rect: NormalizedRect::new(0.225, 0.445, 0.07, 0.2).expect("robot rect"),
        },
    );
    let penalty = annotation(
        "penalty_mark",
        "penalty_mark",
        AnnotationValue::Keypoints {
            points: vec![Keypoint {
                name: "center".to_owned(),
                point: candidate_bbox_center(&candidate),
                visible: true,
            }],
        },
    );
    let issues = BallHardNegativeValidator::default()
        .validate(&ValidationContext {
            project: &project(),
            image: Some(&image),
            candidate: &candidate,
            related_annotations: &[robot, penalty],
            correction_risk: 0.3,
        })
        .expect("ball risk");
    let codes: Vec<_> = issues.iter().map(|issue| issue.code.as_str()).collect();
    assert!(codes.contains(&"possible_white_shoe"));
    assert!(codes.contains(&"possible_penalty_mark"));
    assert!(codes.contains(&"frequent_ball_correction"));
    let decision = RoboCupReviewPolicy.decide(&ReviewContext {
        annotation: &candidate,
        issues: &issues,
        refiner_confidence: None,
        correction_risk: 0.3,
        evidence_conflict: true,
        retry_count: 0,
        max_retries: 2,
    });
    assert!(matches!(decision, ReviewDecision::Retry { .. }));
    let exhausted = RoboCupReviewPolicy.decide(&ReviewContext {
        annotation: &candidate,
        issues: &issues,
        refiner_confidence: None,
        correction_risk: 0.3,
        evidence_conflict: true,
        retry_count: 2,
        max_retries: 2,
    });
    assert!(matches!(exhausted, ReviewDecision::HumanReview { .. }));
}

fn candidate_bbox_center(annotation: &Annotation) -> NormalizedPoint {
    match annotation.value {
        AnnotationValue::BoundingBox { rect } => rect.center(),
        _ => panic!("test candidate is a bbox"),
    }
}

#[test]
fn pixel_refiner_moves_coarse_line_toward_white_pixels() {
    let (_temporary, image) = fixture();
    let candidate = annotation(
        "white_field_line",
        "field_line",
        AnnotationValue::Polyline {
            points: vec![point(0.08, 0.47), point(0.92, 0.47)],
        },
    );
    let related = vec![full_field()];
    let result = RoboCupFieldLineRefiner::default()
        .refine(&RefinementContext {
            project: &project(),
            image: &image,
            candidate: &candidate,
            related_annotations: &related,
        })
        .expect("refinement");
    let AnnotationValue::Polyline { points } = result.annotation.value else {
        panic!("refiner preserves polyline type");
    };
    let average_y = points.iter().map(|point| point.y()).sum::<f32>() / points.len() as f32;
    assert!((average_y - 0.5).abs() < (0.47_f32 - 0.5).abs());
    assert!(result.confidence > 0.4);
}

#[tokio::test]
async fn team_color_tool_and_validator_report_conflict() {
    let (temporary, image) = fixture();
    let rect = NormalizedRect::new(0.225, 0.445, 0.07, 0.2).expect("robot rect");
    let tool = TeamColorEvidenceTool;
    let tool_result = tool
        .execute(
            &ToolContext {
                project_root: temporary.path().to_path_buf(),
                run_id: RunId::new(),
                image_id: Some(ImageId::new()),
                image: Some(std::sync::Arc::new(image.clone())),
                task_id: Some(TaskId::from("robot_attributes")),
                cancellation: CancellationToken::new(),
            },
            json!({"bbox": rect}),
        )
        .await
        .expect("team color evidence");
    assert_eq!(tool_result.model_result["recommendation"], "red");

    let mut candidate = annotation(
        "robot",
        "robot_attributes",
        AnnotationValue::BoundingBox { rect },
    );
    candidate.attributes.insert(
        "team_color".to_owned(),
        AttributeValue::String("blue".to_owned()),
    );
    candidate.attributes.insert(
        "state".to_owned(),
        AttributeValue::String("standing".to_owned()),
    );
    let issues = RobotAttributeValidator
        .validate(&ValidationContext {
            project: &project(),
            image: Some(&image),
            candidate: &candidate,
            related_annotations: &[],
            correction_risk: 0.0,
        })
        .expect("robot validation");
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "team_color_conflict")
    );
}

#[test]
fn required_robot_attributes_and_memory_change_review_decision() {
    let candidate = annotation(
        "robot",
        "robot_attributes",
        AnnotationValue::BoundingBox {
            rect: NormalizedRect::new(0.2, 0.2, 0.1, 0.25).expect("robot rect"),
        },
    );
    let issues = RobotAttributeValidator
        .validate(&ValidationContext {
            project: &project(),
            image: None,
            candidate: &candidate,
            related_annotations: &[],
            correction_risk: 0.0,
        })
        .expect("required attributes");
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "missing_team_color")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "missing_robot_state")
    );

    let decision = RoboCupReviewPolicy.decide(&ReviewContext {
        annotation: &candidate,
        issues: &[],
        refiner_confidence: None,
        correction_risk: 0.4,
        evidence_conflict: false,
        retry_count: 0,
        max_retries: 2,
    });
    assert!(matches!(decision, ReviewDecision::HumanReview { .. }));
}

#[test]
fn robocup_owns_three_semantically_bounded_workflow_templates() {
    let templates = RoboCupSkill::new()
        .expect("RoboCup Skill")
        .workflow_templates();
    assert_eq!(
        templates
            .iter()
            .map(|template| template.id.as_str())
            .collect::<Vec<_>>(),
        ["vlm-bootstrap", "detector-first", "accurate-hybrid"]
    );
    for template in &templates {
        assert!(template.nodes.iter().any(|node| node.id == "review"));
        assert!(template.nodes.iter().any(|node| node.id == "commit"));
    }
    let hybrid = templates
        .iter()
        .find(|template| template.id == "accurate-hybrid")
        .expect("accurate hybrid");
    let verification = hybrid
        .nodes
        .iter()
        .find(|node| node.id == "vlm_verification")
        .expect("semantic verification node");
    assert_eq!(
        verification.inputs[0].artifact_type,
        annotagent_core::ArtifactKind::BoundingBox
    );
    assert_eq!(
        verification.outputs[0].artifact_type,
        annotagent_core::ArtifactKind::Classification
    );
    assert_eq!(
        verification.parameters["geometry_policy"],
        "read_only; output semantic verification only"
    );
    assert!(hybrid.nodes.iter().any(|node| {
        node.id == "field_line_refiner" && node.refiners == ["robocup_field_line_refiner"]
    }));
}

#[test]
fn synthetic_evaluation_reports_accuracy_and_operational_metrics() {
    let truth: EvaluationGroundTruth = serde_json::from_str(include_str!(
        "../../../examples/robocup/evaluation/ground-truth.synthetic.json"
    ))
    .expect("ground truth fixture");
    let predictions: EvaluationPredictions = serde_json::from_str(include_str!(
        "../../../examples/robocup/evaluation/predictions.synthetic.json"
    ))
    .expect("prediction fixture");
    let report = evaluate(&truth, &predictions, 0.5).expect("evaluation");
    assert_eq!(report.bbox.true_positive, 2);
    assert_eq!(report.bbox.false_positive, 1);
    assert_eq!(report.bbox.false_negative, 1);
    assert_eq!(report.mask_iou.value, Some(0.75));
    assert_eq!(report.classification_accuracy.value, Some(0.5));
    assert_eq!(report.attribute_accuracy.value, Some(0.5));
    assert_eq!(report.review_rate.value, Some(0.5));
    assert_eq!(report.failure_rate.value, Some(0.5));
    assert_eq!(report.cost_per_image.value, Some(0.01));
    assert_eq!(report.latency_ms_per_image.value, Some(50.0));
    assert_eq!(report.model_calls_per_image.value, Some(1.0));
    assert!(report.missing_prediction_images.is_empty());
    let gated = evaluate_with_thresholds(
        &truth,
        &predictions,
        EvaluationThresholds {
            bbox_iou: 0.5,
            minimum_field_region_mask_iou: Some(0.7),
        },
    )
    .expect("gated evaluation");
    assert_eq!(gated.quality_gates.field_region_mask_iou_passed, Some(true));
}

#[test]
fn unlabelled_data_cannot_claim_accuracy() {
    let mut truth: EvaluationGroundTruth = serde_json::from_str(include_str!(
        "../../../examples/robocup/evaluation/ground-truth.synthetic.json"
    ))
    .expect("ground truth fixture");
    truth.labeled = false;
    let predictions: EvaluationPredictions = serde_json::from_str(include_str!(
        "../../../examples/robocup/evaluation/predictions.synthetic.json"
    ))
    .expect("prediction fixture");
    assert!(evaluate(&truth, &predictions, 0.5).is_err());
}
