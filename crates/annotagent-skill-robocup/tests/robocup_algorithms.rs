use std::collections::BTreeMap;

use annotagent_core::{
    AgentTool, Annotation, AnnotationId, AnnotationProvenance, AnnotationRefiner, AnnotationSource,
    AnnotationValidator, AnnotationValue, AttributeValue, DomainSkill, ImageFrame, ImageId,
    ImageMetadata, Keypoint, LabelId, NormalizedPoint, NormalizedRect, ProjectSchema,
    RefinementContext, ReviewContext, ReviewDecision, ReviewPolicy, ReviewStatus, RunId, TaskId,
    ToolContext, ValidationContext, WorkflowNodeKind,
};
use annotagent_image_tools::{generate_synthetic_robocup, load_image};
use annotagent_skill_robocup::{
    BallHardNegativeValidator, EvaluationGroundTruth, EvaluationPredictions, EvaluationThresholds,
    FieldContainmentValidator, RoboCupBallFieldRelationValidator, RoboCupBallForegroundRefiner,
    RoboCupFieldLineRefiner, RoboCupReviewPolicy, RoboCupSkill, RobotAttributeValidator,
    TeamColorEvidenceTool, evaluate, evaluate_with_thresholds,
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

#[test]
fn ball_field_relation_is_safe_inside_outside_and_without_field_evidence() {
    let validator = RoboCupBallFieldRelationValidator;
    let inside = annotation(
        "ball",
        "objects",
        AnnotationValue::BoundingBox {
            rect: NormalizedRect::new(0.5, 0.5, 0.04, 0.04).expect("inside"),
        },
    );
    let outside = annotation(
        "ball",
        "objects",
        AnnotationValue::BoundingBox {
            rect: NormalizedRect::new(0.0, 0.0, 0.005, 0.005).expect("outside"),
        },
    );
    let fields = [full_field()];
    let mut project = project();
    project.tasks.push(annotagent_core::TaskConfig {
        id: TaskId::from("field_region"),
        display_name: Some("Field region".to_owned()),
        kind: annotagent_core::TaskKind::Polygon,
        labels: vec!["field".to_owned()],
        required: false,
        multi_label: false,
        depends_on: Vec::new(),
        validators: Vec::new(),
        refiners: Vec::new(),
        target_task: None,
        target_labels: Vec::new(),
        attributes: BTreeMap::new(),
    });
    assert!(
        validator
            .validate(&ValidationContext {
                project: &project,
                image: None,
                candidate: &inside,
                related_annotations: &fields,
                correction_risk: 0.0,
            })
            .expect("inside relation")
            .is_empty()
    );
    assert_eq!(
        validator
            .validate(&ValidationContext {
                project: &project,
                image: None,
                candidate: &outside,
                related_annotations: &fields,
                correction_risk: 0.0,
            })
            .expect("outside relation")[0]
            .code,
        "ball_outside_field"
    );
    assert_eq!(
        validator
            .validate(&ValidationContext {
                project: &project,
                image: None,
                candidate: &inside,
                related_annotations: &[],
                correction_risk: 0.0,
            })
            .expect("missing evidence warning")[0]
            .code,
        "missing_field_evidence"
    );
}

fn candidate_bbox_center(annotation: &Annotation) -> NormalizedPoint {
    match annotation.value {
        AnnotationValue::BoundingBox { rect } => rect.center(),
        _ => panic!("test candidate is a bbox"),
    }
}

#[tokio::test]
async fn pixel_refiner_moves_coarse_line_toward_white_pixels() {
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
            run_id: RunId::new(),
            project: &project(),
            image: &image,
            candidate: &candidate,
            related_annotations: &related,
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("refinement");
    let AnnotationValue::Polyline { points } = result.annotation.value else {
        panic!("refiner preserves polyline type");
    };
    let average_y = points.iter().map(|point| point.y()).sum::<f32>() / points.len() as f32;
    assert!((average_y - 0.5).abs() < (0.47_f32 - 0.5).abs());
    assert!(result.confidence > 0.4);
}

#[tokio::test]
async fn ball_foreground_refiner_tightens_a_coarse_box_and_ignores_a_field_line() {
    let width = 120_u32;
    let height = 100_u32;
    let mut rgb = vec![0_u8; (width * height * 3) as usize];
    for pixel in rgb.chunks_exact_mut(3) {
        pixel.copy_from_slice(&[62, 142, 55]);
    }
    for x in 0..width {
        for y in 59..=61 {
            let offset = ((y * width + x) * 3) as usize;
            rgb[offset..offset + 3].copy_from_slice(&[230, 230, 225]);
        }
    }
    for y in 0..height {
        for x in 0..width {
            let dx = i64::from(x) - 60;
            let dy = i64::from(y) - 50;
            if dx * dx + dy * dy <= 14 * 14 {
                let offset = ((y * width + x) * 3) as usize;
                let color = if (x + y) % 9 < 3 {
                    [205, 38, 42]
                } else {
                    [218, 218, 212]
                };
                rgb[offset..offset + 3].copy_from_slice(&color);
            }
        }
    }
    let image = ImageFrame {
        metadata: ImageMetadata {
            width,
            height,
            mime_type: "image/rgb8".to_owned(),
            sha256: "synthetic-ball".to_owned(),
        },
        rgb,
    };
    let coarse = NormalizedRect::new(0.35, 0.31, 0.30, 0.38).expect("coarse box");
    let candidate = annotation(
        "ball",
        "objects",
        AnnotationValue::BoundingBox { rect: coarse },
    );
    let result = RoboCupBallForegroundRefiner::default()
        .refine(&RefinementContext {
            run_id: RunId::new(),
            project: &project(),
            image: &image,
            candidate: &candidate,
            related_annotations: &[],
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("foreground refinement");
    let AnnotationValue::BoundingBox { rect: refined } = result.annotation.value else {
        panic!("refiner preserves bounding-box type");
    };
    assert!(refined.area() < coarse.area() * 0.8);
    assert!((refined.center().x() - 0.5).abs() < 0.03);
    assert!((refined.center().y() - 0.5).abs() < 0.03);
    assert!(refined.width() / refined.height() < 1.25);
    assert!(result.issues.is_empty());
    assert!(result.confidence >= 0.45);
}

#[tokio::test]
async fn ball_foreground_refiner_preserves_original_box_when_evidence_is_missing() {
    let image = ImageFrame {
        metadata: ImageMetadata {
            width: 64,
            height: 64,
            mime_type: "image/rgb8".to_owned(),
            sha256: "green-only".to_owned(),
        },
        rgb: [55_u8, 135, 48].repeat(64 * 64),
    };
    let coarse = NormalizedRect::new(0.3, 0.3, 0.2, 0.2).expect("coarse box");
    let candidate = annotation(
        "ball",
        "objects",
        AnnotationValue::BoundingBox { rect: coarse },
    );
    let result = RoboCupBallForegroundRefiner::default()
        .refine(&RefinementContext {
            run_id: RunId::new(),
            project: &project(),
            image: &image,
            candidate: &candidate,
            related_annotations: &[],
            cancellation: CancellationToken::new(),
        })
        .await
        .expect("safe fallback");
    assert_eq!(result.annotation.value, candidate.value);
    assert_eq!(result.issues[0].code, "ball_foreground_refiner_fallback");
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
fn robocup_exposes_one_lean_default_ball_workflow_template() {
    let skill = RoboCupSkill::new().expect("RoboCup Skill");
    assert_eq!(skill.task_templates().len(), 1);
    assert_eq!(skill.workflow().nodes.len(), 1);
    assert_eq!(skill.tool_factories().len(), 1);
    assert_eq!(skill.validators().len(), 2);
    assert_eq!(skill.refiners().len(), 1);
    assert_eq!(skill.refiners()[0].id(), "ball_foreground_refiner");
    let schema = ProjectSchema::from_yaml(skill.project_template().expect("Ball project template"))
        .expect("Ball Project Schema");
    assert_eq!(schema.tasks.len(), 1);
    assert_eq!(schema.tasks[0].labels, ["ball"]);
    let templates = skill.workflow_templates();
    assert_eq!(
        templates
            .iter()
            .map(|template| template.id.as_str())
            .collect::<Vec<_>>(),
        ["robocup.ball.vlm-bootstrap"]
    );
    for template in &templates {
        assert!(
            template
                .nodes
                .iter()
                .any(|node| node.kind == WorkflowNodeKind::HumanReview)
        );
        assert!(
            template
                .nodes
                .iter()
                .any(|node| node.kind == WorkflowNodeKind::Commit)
        );
        assert!(template.nodes.iter().all(|node| {
            !node.id.contains("field") && !node.id.contains("robot") && !node.id.contains("penalty")
        }));
    }
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
