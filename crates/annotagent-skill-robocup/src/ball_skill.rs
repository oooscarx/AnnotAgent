use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    AgentBudget, AnnotationRefiner, AnnotationValidator, ArtifactKind, CoreError, CoreResult,
    CorrectionKind, DetectionRecoveryPolicy, NodePort, ReviewPolicy, Skill, SkillManifest,
    SkillResource, SkillResourceRequest, TaskId, TaskTemplate, WorkflowDraftNode, WorkflowEdge,
    WorkflowNodeKind, WorkflowTemplate,
};

use crate::{
    RoboCupBallFieldRelationValidator, RoboCupBallForegroundRefiner,
    RoboCupBallHardNegativeValidator, RoboCupReviewPolicy,
};

pub const ROBOCUP_PACK_ID: &str = "robocup";
pub const ROBOCUP_BALL_SKILL_ID: &str = "robocup.ball";
const BALL_PROMPT_RESOURCE_VERSION: &str = "2.0.0";
const BALL_DETECTION_PROMPT_SHA256: &str =
    "5530dcdecf14a07f5bcfada7ae03660ad0013631ad0037ded6eac17b48bd9fce";
const BALL_LOCAL_RELOCALIZATION_PROMPT_SHA256: &str =
    "c10efdd07415809ecbcff79f640ea619aaa293bbeb98b5f35972bb647ed09a9e";
const BALL_CROP_VERIFICATION_PROMPT_SHA256: &str =
    "7d8c2980b405cb1ab2f37e176c4b5f8a9983873d17d7c404e0f30230b57b9516";
const BALL_DETECTION_PROMPT: &str =
    include_str!("../../../skills/robocup/ball/resources/detection-prompt.md");
const BALL_LOCAL_RELOCALIZATION_PROMPT: &str =
    include_str!("../../../skills/robocup/ball/resources/local-relocalization-prompt.md");
const BALL_CROP_VERIFICATION_PROMPT: &str =
    include_str!("../../../skills/robocup/ball/resources/crop-verification-prompt.md");

pub struct RoboCupPackSkill {
    manifest: SkillManifest,
}

impl RoboCupPackSkill {
    pub fn new() -> CoreResult<Self> {
        Ok(Self {
            manifest: SkillManifest::from_yaml(include_str!(
                "../../../skills/robocup/manifest.yaml"
            ))
            .map_err(|error| CoreError::InvalidManifest(error.to_string()))?,
        })
    }
}

impl Skill for RoboCupPackSkill {
    fn id(&self) -> &str {
        ROBOCUP_PACK_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        match request.resource_name.as_deref() {
            None => Ok(vec![
                resource("SKILL.md", include_str!("../../../skills/robocup/SKILL.md")),
                resource(
                    "resources/advisor.md",
                    include_str!("../../../skills/robocup/resources/advisor.md"),
                ),
            ]),
            Some("SKILL.md") => Ok(vec![resource(
                "SKILL.md",
                include_str!("../../../skills/robocup/SKILL.md"),
            )]),
            Some("resources/advisor.md") => Ok(vec![resource(
                "resources/advisor.md",
                include_str!("../../../skills/robocup/resources/advisor.md"),
            )]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown RoboCup Pack resource {other:?}"
            ))),
        }
    }
}

pub struct RoboCupBallSkill {
    manifest: SkillManifest,
    refiners: Vec<Arc<dyn AnnotationRefiner>>,
}

impl RoboCupBallSkill {
    pub fn new() -> CoreResult<Self> {
        Ok(Self {
            manifest: SkillManifest::from_yaml(include_str!(
                "../../../skills/robocup/ball/manifest.yaml"
            ))
            .map_err(|error| CoreError::InvalidManifest(error.to_string()))?,
            refiners: vec![Arc::new(RoboCupBallForegroundRefiner::default())],
        })
    }
}

impl Skill for RoboCupBallSkill {
    fn id(&self) -> &str {
        ROBOCUP_BALL_SKILL_ID
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn node_templates(&self) -> Vec<TaskTemplate> {
        vec![TaskTemplate {
            id: TaskId::from("robocup.ball.validate"),
            description: "Validate ball candidates against RoboCup hard negatives".to_owned(),
        }]
    }

    fn validators(&self) -> Vec<Arc<dyn AnnotationValidator>> {
        vec![
            Arc::new(RoboCupBallHardNegativeValidator::default()),
            Arc::new(RoboCupBallFieldRelationValidator),
        ]
    }

    fn refiners(&self) -> Vec<Arc<dyn AnnotationRefiner>> {
        self.refiners.clone()
    }
    fn review_policies(&self) -> Vec<(String, Arc<dyn ReviewPolicy>)> {
        vec![(
            "robocup.ball.review".to_owned(),
            Arc::new(RoboCupReviewPolicy),
        )]
    }

    fn workflow_templates(&self) -> Vec<WorkflowTemplate> {
        ball_templates()
    }

    fn resources(&self, request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        let requested = request.resource_name.as_deref();
        match requested {
            None => Ok(vec![
                resource(
                    "ball/SKILL.md",
                    include_str!("../../../skills/robocup/ball/SKILL.md"),
                ),
                resource(
                    "ball/resources/hard-negatives.md",
                    include_str!("../../../skills/robocup/ball/resources/hard-negatives.md"),
                ),
                resource(
                    "ball/resources/advisor.md",
                    include_str!("../../../skills/robocup/resources/advisor.md"),
                ),
                resource("ball/resources/detection-prompt.md", BALL_DETECTION_PROMPT),
                resource(
                    "ball/resources/local-relocalization-prompt.md",
                    BALL_LOCAL_RELOCALIZATION_PROMPT,
                ),
                resource(
                    "ball/resources/crop-verification-prompt.md",
                    BALL_CROP_VERIFICATION_PROMPT,
                ),
            ]),
            Some("ball/SKILL.md") => Ok(vec![resource(
                "ball/SKILL.md",
                include_str!("../../../skills/robocup/ball/SKILL.md"),
            )]),
            Some("ball/resources/hard-negatives.md") => Ok(vec![resource(
                "ball/resources/hard-negatives.md",
                include_str!("../../../skills/robocup/ball/resources/hard-negatives.md"),
            )]),
            Some("ball/resources/advisor.md") => Ok(vec![resource(
                "ball/resources/advisor.md",
                include_str!("../../../skills/robocup/resources/advisor.md"),
            )]),
            Some("ball/resources/detection-prompt.md") => Ok(vec![resource(
                "ball/resources/detection-prompt.md",
                BALL_DETECTION_PROMPT,
            )]),
            Some("ball/resources/local-relocalization-prompt.md") => Ok(vec![resource(
                "ball/resources/local-relocalization-prompt.md",
                BALL_LOCAL_RELOCALIZATION_PROMPT,
            )]),
            Some("ball/resources/crop-verification-prompt.md") => Ok(vec![resource(
                "ball/resources/crop-verification-prompt.md",
                BALL_CROP_VERIFICATION_PROMPT,
            )]),
            Some(other) => Err(CoreError::Validation(format!(
                "unknown RoboCup Ball resource {other:?}"
            ))),
        }
    }

    fn correction_taxonomy(&self) -> Vec<CorrectionKind> {
        self.manifest
            .correction_taxonomy
            .iter()
            .map(|code| CorrectionKind {
                code: code.clone(),
                description: code.replace('_', " "),
            })
            .collect()
    }
}

fn resource(name: &str, content: &str) -> SkillResource {
    SkillResource {
        name: name.to_owned(),
        media_type: "text/markdown".to_owned(),
        content: content.to_owned(),
    }
}

fn port(id: &str, kind: ArtifactKind) -> NodePort {
    NodePort {
        id: id.to_owned(),
        artifact_type: kind,
        required: true,
        multiple: false,
    }
}

fn optional_port(id: &str, kind: ArtifactKind) -> NodePort {
    NodePort {
        required: false,
        ..port(id, kind)
    }
}

fn multiple_port(id: &str, kind: ArtifactKind) -> NodePort {
    NodePort {
        multiple: true,
        ..port(id, kind)
    }
}

fn bind_prompt_resource(
    node: &mut WorkflowDraftNode,
    resource_id: &str,
    sha256: &str,
    content: &str,
) {
    node.parameters.extend([
        (
            "prompt_resource_id".to_owned(),
            serde_json::json!(resource_id),
        ),
        (
            "prompt_resource_version".to_owned(),
            serde_json::json!(BALL_PROMPT_RESOURCE_VERSION),
        ),
        (
            "prompt_resource_sha256".to_owned(),
            serde_json::json!(sha256),
        ),
        ("target_description".to_owned(), serde_json::json!(content)),
    ]);
}

fn node(
    id: &str,
    operation: &str,
    kind: WorkflowNodeKind,
    inputs: Vec<NodePort>,
    outputs: Vec<NodePort>,
) -> WorkflowDraftNode {
    WorkflowDraftNode {
        id: id.to_owned(),
        node_type: operation.to_owned(),
        kind,
        inputs,
        outputs,
        required_skills: vec![ROBOCUP_BALL_SKILL_ID.to_owned()],
        ..WorkflowDraftNode::default()
    }
}

fn edge(from: &str, from_port: &str, to: &str, to_port: &str, route: Option<&str>) -> WorkflowEdge {
    WorkflowEdge {
        from_node: from.to_owned(),
        from_port: from_port.to_owned(),
        to_node: to.to_owned(),
        to_port: to_port.to_owned(),
        route: route.map(ToOwned::to_owned),
    }
}

fn ball_templates() -> Vec<WorkflowTemplate> {
    vec![
        vlm_bootstrap_template(),
        small_object_recovery_template(),
        specialist_fallback_template(),
    ]
}

fn vlm_bootstrap_template() -> WorkflowTemplate {
    let mut detector = node(
        "detector",
        "vlm_detection.detect",
        WorkflowNodeKind::VisionLanguageModel,
        vec![port("image", ArtifactKind::Image)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    detector.model_binding = Some("default-vision".to_owned());
    detector
        .parameters
        .insert("labels".to_owned(), serde_json::json!(["ball"]));
    bind_prompt_resource(
        &mut detector,
        "ball/resources/detection-prompt.md",
        BALL_DETECTION_PROMPT_SHA256,
        BALL_DETECTION_PROMPT,
    );
    let mut select = node(
        "select_football",
        "core.filter",
        WorkflowNodeKind::Transform,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    select.required_skills.clear();
    select.parameters.extend([
        ("labels".to_owned(), serde_json::json!(["ball", "football"])),
        (
            "class_mapping".to_owned(),
            serde_json::json!({"ball": "ball", "football": "ball", "sports ball": "ball"}),
        ),
        ("minimum_confidence".to_owned(), serde_json::json!(0.0)),
    ]);
    let mut validator = node(
        "validate_ball",
        "static_validator",
        WorkflowNodeKind::Validator,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    validator.validators = vec![
        "robocup.ball.ball_hard_negative".to_owned(),
        "robocup.ball.robocup_ball_field_relation".to_owned(),
    ];
    validator
        .parameters
        .insert("task_id".to_owned(), serde_json::json!("objects"));
    let mut gate = node(
        "gate",
        "core.confidence_gate",
        WorkflowNodeKind::Gate,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    gate.parameters
        .insert("threshold".to_owned(), serde_json::json!(0.92));
    WorkflowTemplate {
        id: "robocup.ball.vlm-bootstrap".to_owned(),
        name: "RoboCup Ball · VLM bootstrap".to_owned(),
        description: "Image → one VLM detector → Select football candidates → RoboCup validation → Decision → review or save".to_owned(),
        nodes: vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
            ),
            detector,
            select,
            validator,
            gate,
            {
                let mut review = node(
                    "review",
                    "review_gate",
                    WorkflowNodeKind::HumanReview,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    vec![port("detections", ArtifactKind::DetectionSet)],
                );
                review.inputs[0].multiple = true;
                review
            },
            {
                let mut commit = node(
                    "commit",
                    "commit",
                    WorkflowNodeKind::Commit,
                    vec![port("detections", ArtifactKind::DetectionSet)],
                    Vec::new(),
                );
                commit.inputs[0].multiple = true;
                commit
            },
        ],
        edges: vec![
            edge("image", "image", "detector", "image", None),
            edge("detector", "detections", "select_football", "detections", None),
            edge("select_football", "detections", "validate_ball", "detections", None),
            edge("validate_ball", "detections", "gate", "detections", None),
            edge("gate", "detections", "review", "detections", Some("pass")),
            edge("gate", "detections", "review", "detections", Some("review")),
            edge("review", "detections", "commit", "detections", None),
        ],
        resource_versions: ball_resources(),
        allow_unvalidated_commit: false,
    }
}

fn small_object_recovery_template() -> WorkflowTemplate {
    let mut coarse = node(
        "coarse_localization",
        "vlm_detection.detect",
        WorkflowNodeKind::VisionLanguageModel,
        vec![port("image", ArtifactKind::Image)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    coarse
        .parameters
        .insert("labels".to_owned(), serde_json::json!(["ball"]));
    bind_prompt_resource(
        &mut coarse,
        "ball/resources/detection-prompt.md",
        BALL_DETECTION_PROMPT_SHA256,
        BALL_DETECTION_PROMPT,
    );

    let mut select = node(
        "select_coarse_ball",
        "core.select_and_map",
        WorkflowNodeKind::Transform,
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
    );
    select.required_skills.clear();
    select.parameters.extend([
        ("labels".to_owned(), serde_json::json!(["ball"])),
        (
            "class_mapping".to_owned(),
            serde_json::json!({"ball": "ball", "football": "ball", "sports ball": "ball"}),
        ),
        ("minimum_confidence".to_owned(), serde_json::json!(0.0)),
    ]);

    let mut expand = node(
        "infer_scale_and_expand_search",
        "core.expand_region",
        WorkflowNodeKind::Transform,
        vec![
            port("image", ArtifactKind::Image),
            multiple_port("detections", ArtifactKind::DetectionSet),
        ],
        vec![multiple_port("regions", ArtifactKind::DetectionSet)],
    );
    expand.required_skills.clear();
    expand.parameters.extend([
        (
            "policy".to_owned(),
            serde_json::json!({
                "kind": "relative_to_candidate",
                "width_factor": 4.0,
                "height_factor": 4.0,
                "minimum_width_px": 96,
                "minimum_height_px": 96,
                "maximum_image_fraction": 0.5
            }),
        ),
        ("tiny_max_dimension_px".to_owned(), serde_json::json!(20.0)),
        ("small_max_dimension_px".to_owned(), serde_json::json!(64.0)),
        (
            "medium_max_dimension_px".to_owned(),
            serde_json::json!(160.0),
        ),
    ]);

    let mut crop = node(
        "crop_original_search_region",
        "core.crop",
        WorkflowNodeKind::Transform,
        vec![
            port("image", ArtifactKind::Image),
            multiple_port("detections", ArtifactKind::DetectionSet),
        ],
        vec![
            multiple_port("crops", ArtifactKind::CropSet),
            multiple_port("images", ArtifactKind::Image),
        ],
    );
    crop.required_skills.clear();
    crop.parameters
        .insert("padding".to_owned(), serde_json::json!(0.0));

    let mut localize = node(
        "local_relocalization",
        "vlm_detection.detect",
        WorkflowNodeKind::VisionLanguageModel,
        vec![multiple_port("image", ArtifactKind::Image)],
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
    );
    localize.parameters.extend([
        ("labels".to_owned(), serde_json::json!(["ball"])),
        (
            "coordinate_space".to_owned(),
            serde_json::json!("local_crop"),
        ),
        ("maximum_model_calls".to_owned(), serde_json::json!(3)),
    ]);
    bind_prompt_resource(
        &mut localize,
        "ball/resources/local-relocalization-prompt.md",
        BALL_LOCAL_RELOCALIZATION_PROMPT_SHA256,
        BALL_LOCAL_RELOCALIZATION_PROMPT,
    );

    let mut verify = node(
        "independent_crop_verification",
        "vlm_detection.detect",
        WorkflowNodeKind::VisionLanguageModel,
        vec![multiple_port("image", ArtifactKind::Image)],
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
    );
    verify.parameters.extend([
        ("labels".to_owned(), serde_json::json!(["ball"])),
        (
            "coordinate_space".to_owned(),
            serde_json::json!("local_crop"),
        ),
        ("maximum_model_calls".to_owned(), serde_json::json!(3)),
    ]);
    bind_prompt_resource(
        &mut verify,
        "ball/resources/crop-verification-prompt.md",
        BALL_CROP_VERIFICATION_PROMPT_SHA256,
        BALL_CROP_VERIFICATION_PROMPT,
    );

    let project = |id: &str| {
        let mut project = node(
            id,
            "core.project_coordinates",
            WorkflowNodeKind::Transform,
            vec![
                multiple_port("images", ArtifactKind::Image),
                multiple_port("detections", ArtifactKind::DetectionSet),
            ],
            vec![multiple_port("detections", ArtifactKind::DetectionSet)],
        );
        project.required_skills.clear();
        project
    };

    let mut validator = node(
        "validate_relocalized_ball",
        "static_validator",
        WorkflowNodeKind::Validator,
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
    );
    validator.validators = vec![
        "robocup.ball.ball_hard_negative".to_owned(),
        "robocup.ball.robocup_ball_field_relation".to_owned(),
    ];
    validator
        .parameters
        .insert("task_id".to_owned(), serde_json::json!("objects"));

    let mut prompts = node(
        "relocalized_box_prompts",
        "core.detections_to_box_prompts",
        WorkflowNodeKind::Transform,
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
        vec![multiple_port("prompts", ArtifactKind::BoxPromptSet)],
    );
    prompts.required_skills.clear();
    prompts
        .parameters
        .insert("padding".to_owned(), serde_json::json!(0.0));

    let mut coverage = node(
        "prompt_coverage_gate",
        "core.prompt_coverage_gate",
        WorkflowNodeKind::Gate,
        vec![
            multiple_port("prompts", ArtifactKind::BoxPromptSet),
            multiple_port("candidates", ArtifactKind::DetectionSet),
            optional_port("evidence", ArtifactKind::DetectionSet),
        ],
        vec![
            multiple_port("prompts", ArtifactKind::BoxPromptSet),
            multiple_port("detections", ArtifactKind::DetectionSet),
            multiple_port("coverage", ArtifactKind::PromptCoverage),
        ],
    );
    coverage.required_skills.clear();

    let mut segment = node(
        "refine_validated_prompt",
        "capability.segment",
        WorkflowNodeKind::VisionModel,
        vec![
            port("images", ArtifactKind::Image),
            multiple_port("box_prompts", ArtifactKind::BoxPromptSet),
        ],
        vec![multiple_port("masks", ArtifactKind::MaskSet)],
    );
    segment.required_skills.clear();

    let mut mask_to_bbox = node(
        "project_mask_bbox",
        "core.mask_to_bbox",
        WorkflowNodeKind::Transform,
        vec![
            multiple_port("masks", ArtifactKind::MaskSet),
            multiple_port("box_prompts", ArtifactKind::BoxPromptSet),
        ],
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
    );
    mask_to_bbox.required_skills.clear();

    let mut geometry_quality = node(
        "evaluate_refiner_geometry",
        "core.geometry_quality_evaluation",
        WorkflowNodeKind::Validator,
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
    );
    geometry_quality.required_skills.clear();

    let mut decision = node(
        "geometry_decision",
        "core.geometry_decision",
        WorkflowNodeKind::Gate,
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
    );
    decision.required_skills.clear();

    let mut review = node(
        "review_final_ball",
        "review_gate",
        WorkflowNodeKind::HumanReview,
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
    );
    review.review_gate = true;
    let mut commit = node(
        "commit_final_ball",
        "commit",
        WorkflowNodeKind::Commit,
        vec![multiple_port("detections", ArtifactKind::DetectionSet)],
        Vec::new(),
    );
    commit.required_skills.clear();

    WorkflowTemplate {
        id: "robocup.ball.small-object-recovery".to_owned(),
        name: "RoboCup Ball · small-object localization recovery".to_owned(),
        description: "Coarse semantic localization → target-scale search crop → local re-localization → independent prompt-coverage evidence → prompted segmentation → geometry decision → review → commit".to_owned(),
        nodes: vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
            ),
            coarse,
            select,
            expand,
            crop,
            localize,
            project("project_local_detection"),
            validator,
            verify,
            project("project_verification_detection"),
            prompts,
            coverage,
            segment,
            mask_to_bbox,
            geometry_quality,
            decision,
            review,
            commit,
        ],
        edges: vec![
            edge("image", "image", "coarse_localization", "image", None),
            edge("coarse_localization", "detections", "select_coarse_ball", "detections", None),
            edge("image", "image", "infer_scale_and_expand_search", "image", None),
            edge("select_coarse_ball", "detections", "infer_scale_and_expand_search", "detections", None),
            edge("image", "image", "crop_original_search_region", "image", None),
            edge("infer_scale_and_expand_search", "regions", "crop_original_search_region", "detections", None),
            edge("crop_original_search_region", "images", "local_relocalization", "image", None),
            edge("crop_original_search_region", "images", "independent_crop_verification", "image", None),
            edge("crop_original_search_region", "images", "project_local_detection", "images", None),
            edge("local_relocalization", "detections", "project_local_detection", "detections", None),
            edge("project_local_detection", "detections", "validate_relocalized_ball", "detections", None),
            edge("crop_original_search_region", "images", "project_verification_detection", "images", None),
            edge("independent_crop_verification", "detections", "project_verification_detection", "detections", None),
            edge("validate_relocalized_ball", "detections", "relocalized_box_prompts", "detections", None),
            edge("relocalized_box_prompts", "prompts", "prompt_coverage_gate", "prompts", None),
            edge("validate_relocalized_ball", "detections", "prompt_coverage_gate", "candidates", None),
            edge("project_verification_detection", "detections", "prompt_coverage_gate", "evidence", None),
            edge("image", "image", "refine_validated_prompt", "images", None),
            edge("prompt_coverage_gate", "prompts", "refine_validated_prompt", "box_prompts", Some("refine")),
            edge("refine_validated_prompt", "masks", "project_mask_bbox", "masks", None),
            edge("prompt_coverage_gate", "prompts", "project_mask_bbox", "box_prompts", Some("refine")),
            edge("project_mask_bbox", "detections", "evaluate_refiner_geometry", "detections", None),
            edge("evaluate_refiner_geometry", "detections", "geometry_decision", "detections", None),
            edge("geometry_decision", "detections", "review_final_ball", "detections", Some("accept")),
            edge("geometry_decision", "detections", "review_final_ball", "detections", Some("review")),
            edge("prompt_coverage_gate", "detections", "review_final_ball", "detections", Some("relocalize")),
            edge("prompt_coverage_gate", "detections", "review_final_ball", "detections", Some("search_tiles")),
            edge("prompt_coverage_gate", "detections", "review_final_ball", "detections", Some("review")),
            edge("review_final_ball", "detections", "commit_final_ball", "detections", None),
        ],
        resource_versions: ball_resources(),
        allow_unvalidated_commit: false,
    }
}

fn specialist_fallback_template() -> WorkflowTemplate {
    let required = |node: &mut WorkflowDraftNode, skills: &[&str]| {
        node.required_skills = skills.iter().map(|skill| (*skill).to_owned()).collect();
    };
    let mut specialist = node(
        "specialist",
        "object_detection.detect",
        WorkflowNodeKind::VisionModel,
        vec![port("image", ArtifactKind::Image)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    required(&mut specialist, &["annotagent.detection"]);
    specialist.parameters.extend([
        ("target_labels".to_owned(), serde_json::json!(["football"])),
        (
            "recover_on_backend_error".to_owned(),
            serde_json::json!(true),
        ),
        (
            "class_mapping".to_owned(),
            serde_json::json!({"ball": "football", "football": "football"}),
        ),
    ]);

    let mut validate_primary = node(
        "validate_primary",
        "static_validator",
        WorkflowNodeKind::Validator,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    validate_primary.validators = vec![
        "robocup.ball.ball_hard_negative".to_owned(),
        "robocup.ball.robocup_ball_field_relation".to_owned(),
    ];
    validate_primary.parameters.extend([
        ("task_id".to_owned(), serde_json::json!("footballs")),
        (
            "correction_memory_skill_id".to_owned(),
            serde_json::json!(ROBOCUP_BALL_SKILL_ID),
        ),
    ]);

    let mut policy = DetectionRecoveryPolicy::default();
    policy.initial_gate.accept_when[0].minimum_score = Some(0.9);
    policy.initial_gate.fallback_when[1].specialist_score_below = Some(0.72);
    let mut recovery = node(
        "recovery",
        "agent.detection_recovery",
        WorkflowNodeKind::Gate,
        vec![
            port("image", ArtifactKind::Image),
            port("primary", ArtifactKind::DetectionSet),
        ],
        vec![port("candidates", ArtifactKind::CandidateClusterSet)],
    );
    required(
        &mut recovery,
        &[ROBOCUP_BALL_SKILL_ID, "annotagent.detection"],
    );
    recovery.review_gate = true;
    recovery.parameters.extend([
        (
            "queries".to_owned(),
            serde_json::json!([{
                "id": "football",
                "text": "the compact round football on the playing field, excluding white shoes, socks, penalty marks, and line intersections",
                "target_label": "football"
            }]),
        ),
        (
            "recovery_policy".to_owned(),
            serde_json::to_value(policy).expect("static Recovery policy serializes"),
        ),
        (
            "agent_budget".to_owned(),
            serde_json::to_value(AgentBudget {
                max_steps: 4,
                max_tool_calls: 4,
                max_tokens: None,
                max_cost: None,
            })
            .expect("static Agent budget serializes"),
        ),
    ]);

    let mut project_candidates = node(
        "project_candidates",
        "core.project_detection_candidates",
        WorkflowNodeKind::Transform,
        vec![port("candidates", ArtifactKind::CandidateClusterSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    project_candidates.required_skills.clear();
    let mut validate_recovered = node(
        "validate_recovered",
        "static_validator",
        WorkflowNodeKind::Validator,
        vec![port("detections", ArtifactKind::DetectionSet)],
        vec![port("detections", ArtifactKind::DetectionSet)],
    );
    validate_recovered
        .validators
        .clone_from(&validate_primary.validators);
    validate_recovered.parameters = validate_primary.parameters.clone();

    let mut crop = node(
        "crop_verify",
        "core.crop",
        WorkflowNodeKind::Transform,
        vec![
            port("image", ArtifactKind::Image),
            port("detections", ArtifactKind::DetectionSet),
        ],
        vec![port("crops", ArtifactKind::CropSet)],
    );
    crop.required_skills.clear();
    crop.parameters
        .insert("padding".to_owned(), serde_json::json!(0.12));
    let mut classifier = node(
        "classify_crop",
        "classification.classify",
        WorkflowNodeKind::VisionModel,
        vec![port("crops", ArtifactKind::CropSet)],
        vec![port("classifications", ArtifactKind::ClassificationSet)],
    );
    required(&mut classifier, &["annotagent.classification"]);
    classifier.parameters.insert(
        "labels".to_owned(),
        serde_json::json!(["football", "not_football"]),
    );
    let mut verify = node(
        "verify_crop",
        "classification.verify",
        WorkflowNodeKind::Validator,
        vec![
            port("classifications", ArtifactKind::ClassificationSet),
            optional_port("detections", ArtifactKind::DetectionSet),
        ],
        vec![port("classifications", ArtifactKind::ClassificationSet)],
    );
    required(
        &mut verify,
        &["annotagent.classification", ROBOCUP_BALL_SKILL_ID],
    );
    verify.parameters.extend([
        (
            "labels".to_owned(),
            serde_json::json!(["football", "not_football"]),
        ),
        ("accept_labels".to_owned(), serde_json::json!(["football"])),
        (
            "reject_labels".to_owned(),
            serde_json::json!(["not_football"]),
        ),
        ("minimum_confidence".to_owned(), serde_json::json!(0.72)),
        (
            "review_on_validation_issue".to_owned(),
            serde_json::json!(true),
        ),
    ]);
    let mut attach = node(
        "attach_verified",
        "core.attach_result",
        WorkflowNodeKind::CandidateMerge,
        vec![
            port("detections", ArtifactKind::DetectionSet),
            port("classifications", ArtifactKind::ClassificationSet),
        ],
        vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
    );
    attach.required_skills.clear();
    attach.inputs[1].multiple = true;
    attach.parameters.extend([
        ("task_id".to_owned(), serde_json::json!("footballs")),
        (
            "class_mapping".to_owned(),
            serde_json::json!({"football": "football"}),
        ),
    ]);
    let mut verified_gate = node(
        "verified_gate",
        "core.confidence_gate",
        WorkflowNodeKind::Gate,
        vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
        vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
    );
    verified_gate.required_skills.clear();
    verified_gate
        .parameters
        .insert("threshold".to_owned(), serde_json::json!(0.72));

    WorkflowTemplate {
        id: "robocup.ball.specialist_with_open_vocab_fallback".to_owned(),
        name: "RoboCup Ball · specialist with open-vocabulary fallback".to_owned(),
        description: "Image → specialist capability → domain evidence → bounded open-vocabulary fallback → candidate projection → Crop Verify → commit, reject, or review".to_owned(),
        nodes: vec![
            node(
                "image",
                "core.image_input",
                WorkflowNodeKind::ImageInput,
                Vec::new(),
                vec![port("image", ArtifactKind::Image)],
            ),
            specialist,
            validate_primary,
            recovery,
            project_candidates,
            validate_recovered,
            crop,
            classifier,
            verify,
            attach,
            verified_gate,
            {
                let mut review = node(
                    "review_evidence",
                    "review_gate",
                    WorkflowNodeKind::HumanReview,
                    vec![port("candidates", ArtifactKind::CandidateClusterSet)],
                    vec![port("candidates", ArtifactKind::CandidateClusterSet)],
                );
                review.inputs[0].multiple = true;
                review
            },
            {
                let mut review = node(
                    "review_verified",
                    "review_gate",
                    WorkflowNodeKind::HumanReview,
                    vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                    vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                );
                review.inputs[0].multiple = true;
                review
            },
            {
                let mut commit = node(
                    "commit_evidence",
                    "commit",
                    WorkflowNodeKind::Commit,
                    vec![port("candidates", ArtifactKind::CandidateClusterSet)],
                    Vec::new(),
                );
                commit.inputs[0].multiple = true;
                commit
            },
            {
                let mut commit = node(
                    "commit_verified",
                    "commit",
                    WorkflowNodeKind::Commit,
                    vec![port("candidates", ArtifactKind::AnnotationCandidateSet)],
                    Vec::new(),
                );
                commit.inputs[0].multiple = true;
                commit
            },
            {
                let mut reject = node(
                    "reject_hard_negative",
                    "core.reject_candidates",
                    WorkflowNodeKind::Export,
                    vec![port("classifications", ArtifactKind::ClassificationSet)],
                    vec![port("classifications", ArtifactKind::ClassificationSet)],
                );
                reject.parameters.insert(
                    "reason".to_owned(),
                    serde_json::json!("crop verification classified the candidate as a RoboCup hard negative"),
                );
                reject
            },
        ],
        edges: vec![
            edge("image", "image", "specialist", "image", None),
            edge("specialist", "detections", "validate_primary", "detections", None),
            edge("image", "image", "recovery", "image", None),
            edge("validate_primary", "detections", "recovery", "primary", None),
            edge("recovery", "candidates", "review_evidence", "candidates", Some("accept")),
            edge("recovery", "candidates", "review_evidence", "candidates", Some("review")),
            edge("recovery", "candidates", "project_candidates", "candidates", Some("verify")),
            edge("project_candidates", "detections", "validate_recovered", "detections", None),
            edge("image", "image", "crop_verify", "image", None),
            edge("validate_recovered", "detections", "crop_verify", "detections", None),
            edge("crop_verify", "crops", "classify_crop", "crops", None),
            edge("classify_crop", "classifications", "verify_crop", "classifications", None),
            edge("validate_recovered", "detections", "verify_crop", "detections", None),
            edge("validate_recovered", "detections", "attach_verified", "detections", None),
            edge("verify_crop", "classifications", "attach_verified", "classifications", Some("accept")),
            edge("verify_crop", "classifications", "attach_verified", "classifications", Some("review")),
            edge("verify_crop", "classifications", "reject_hard_negative", "classifications", Some("reject")),
            edge("attach_verified", "candidates", "verified_gate", "candidates", None),
            edge("verified_gate", "candidates", "review_verified", "candidates", Some("pass")),
            edge("verified_gate", "candidates", "review_verified", "candidates", Some("review")),
            edge("review_verified", "candidates", "commit_verified", "candidates", None),
            edge("review_evidence", "candidates", "commit_evidence", "candidates", None),
        ],
        resource_versions: hybrid_resources(),
        allow_unvalidated_commit: false,
    }
}

fn ball_resources() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("ball/SKILL.md".to_owned(), "1".to_owned()),
        (
            "ball/resources/hard-negatives.md".to_owned(),
            "1".to_owned(),
        ),
        ("ball/resources/advisor.md".to_owned(), "1".to_owned()),
        (
            "ball/resources/detection-prompt.md".to_owned(),
            format!("{BALL_PROMPT_RESOURCE_VERSION}+sha256:{BALL_DETECTION_PROMPT_SHA256}"),
        ),
        (
            "ball/resources/local-relocalization-prompt.md".to_owned(),
            format!(
                "{BALL_PROMPT_RESOURCE_VERSION}+sha256:{BALL_LOCAL_RELOCALIZATION_PROMPT_SHA256}"
            ),
        ),
        (
            "ball/resources/crop-verification-prompt.md".to_owned(),
            format!("{BALL_PROMPT_RESOURCE_VERSION}+sha256:{BALL_CROP_VERIFICATION_PROMPT_SHA256}"),
        ),
    ])
}

fn hybrid_resources() -> BTreeMap<String, String> {
    ball_resources()
        .into_iter()
        .map(|(resource, version)| (format!("{ROBOCUP_BALL_SKILL_ID}.{resource}"), version))
        .collect()
}

#[cfg(test)]
mod tests {
    use annotagent_core::{SkillKind, SkillResourceRequest};

    use super::*;

    #[test]
    fn pack_and_ball_domain_are_separate_and_templates_are_model_agnostic() {
        let pack = RoboCupPackSkill::new().expect("Pack");
        let ball = RoboCupBallSkill::new().expect("Ball Skill");
        assert_eq!(pack.manifest().kind, SkillKind::Pack);
        assert_eq!(ball.manifest().kind, SkillKind::Domain);
        assert_eq!(ball.validators().len(), 2);
        let templates = ball.workflow_templates();
        assert_eq!(templates.len(), 3);
        let hybrid = templates
            .iter()
            .find(|template| template.id == "robocup.ball.specialist_with_open_vocab_fallback")
            .expect("hybrid template");
        assert!(
            hybrid
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.kind,
                        WorkflowNodeKind::VisionModel | WorkflowNodeKind::VisionLanguageModel
                    ) || node.node_type == "agent.detection_recovery"
                })
                .all(|node| node.model_binding.is_none())
        );
        let serialized = serde_json::to_string(hybrid).expect("template JSON");
        assert!(!serialized.contains("rfdetr"));
        assert!(!serialized.contains("locate"));
        let recovery = templates
            .iter()
            .find(|template| template.id == "robocup.ball.small-object-recovery")
            .expect("small-object recovery template");
        let recovery_types = recovery
            .nodes
            .iter()
            .map(|node| node.node_type.as_str())
            .collect::<Vec<_>>();
        for required in [
            "core.expand_region",
            "core.crop",
            "core.project_coordinates",
            "core.prompt_coverage_gate",
            "capability.segment",
            "core.mask_to_bbox",
            "core.geometry_quality_evaluation",
            "core.geometry_decision",
        ] {
            assert!(recovery_types.contains(&required), "missing {required}");
        }
        assert!(
            recovery
                .nodes
                .iter()
                .filter(|node| node.node_type == "vlm_detection.detect")
                .all(|node| {
                    node.model_binding.is_none()
                        && node.parameters.contains_key("prompt_resource_version")
                        && node.parameters.contains_key("prompt_resource_sha256")
                })
        );
        let recovery_json = serde_json::to_string(recovery).expect("recovery JSON");
        assert!(recovery_json.contains("white shoes"));
        assert!(recovery_json.contains("local_crop"));
        assert!(!recovery_json.to_ascii_lowercase().contains("qwen"));
        assert!(!recovery_json.to_ascii_lowercase().contains("efficientsam"));
        assert_eq!(
            ball.manifest().requires.capabilities,
            ["detection", "human_review"]
        );
        let advisor = ball
            .resources(&SkillResourceRequest {
                task_id: None,
                resource_name: Some("ball/resources/advisor.md".to_owned()),
            })
            .expect("Advisor resource");
        assert!(advisor[0].content.contains("smallest Pipeline"));
        assert!(
            advisor[0]
                .content
                .contains("Never add an unavailable, Unknown, disabled")
        );
        assert!(
            ball.resources(&SkillResourceRequest {
                task_id: None,
                resource_name: Some("../secret".to_owned()),
            })
            .is_err()
        );
    }
}
