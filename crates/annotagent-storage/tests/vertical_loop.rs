use std::{collections::BTreeMap, path::PathBuf, str::FromStr, sync::Arc};

use annotagent_core::{
    AgentTool, AnnotationRefiner, AnnotationValidator, Budget, CoreResult, CorrectionKind,
    DatasetConfig, DomainSkill, ExportConfig, ImageFrame, ImageId, ImageMetadata, IssueSeverity,
    PricingConfig, ProjectDescriptor, ProjectSchema, ReviewConfig, ReviewContext, ReviewDecision,
    ReviewPolicy, RuntimeConfig, SkillManifest, SkillResource, SkillResourceRequest,
    SuggestedAction, TaskConfig, TaskGraph, TaskId, TaskKind, TaskNode, TaskTemplate,
    ValidationContext, ValidationEvidence, ValidationIssue,
};
use annotagent_provider::{MockResponseSpec, MockScript, MockStep, MockUsage, MockVisionProvider};
use annotagent_runtime::{AgentLoopConfig, AgentRuntime, ImageRunRequest};
use annotagent_storage::SqliteStore;
use rust_decimal::Decimal;
use serde_json::json;

struct ConfidenceValidator;

impl AnnotationValidator for ConfidenceValidator {
    fn id(&self) -> &str {
        "minimum_confidence"
    }

    fn validate(&self, context: &ValidationContext<'_>) -> CoreResult<Vec<ValidationIssue>> {
        if context.candidate.confidence.unwrap_or(0.0) >= 0.8 {
            Ok(Vec::new())
        } else {
            Ok(vec![ValidationIssue {
                code: "low_confidence".to_owned(),
                severity: IssueSeverity::Warning,
                annotation_ids: vec![context.candidate.id],
                message: "candidate confidence is below the demo threshold".to_owned(),
                suggested_action: SuggestedAction::HumanReview,
                evidence: ValidationEvidence::Geometry {
                    metric: "confidence".to_owned(),
                    value: f64::from(context.candidate.confidence.unwrap_or(0.0)),
                    threshold: 0.8,
                },
            }])
        }
    }
}

struct AcceptCleanCandidates;

impl ReviewPolicy for AcceptCleanCandidates {
    fn decide(&self, context: &ReviewContext<'_>) -> ReviewDecision {
        if context.issues.is_empty() {
            ReviewDecision::AutoAccept {
                reasons: vec!["confidence and deterministic validation passed".to_owned()],
            }
        } else {
            ReviewDecision::HumanReview {
                reasons: context
                    .issues
                    .iter()
                    .map(|issue| issue.code.clone())
                    .collect(),
            }
        }
    }
}

struct BboxSkill {
    manifest: SkillManifest,
}

impl BboxSkill {
    fn new() -> Self {
        Self {
            manifest: SkillManifest {
                version: 1,
                id: "bbox_fixture".to_owned(),
                display_name: "BBox fixture".to_owned(),
                description: "A test-only bounding-box skill".to_owned(),
                rust_implementation: None,
                summary_resources: Vec::new(),
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
            },
        }
    }
}

impl DomainSkill for BboxSkill {
    fn id(&self) -> &str {
        &self.manifest.id
    }

    fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    fn task_templates(&self) -> Vec<TaskTemplate> {
        vec![TaskTemplate {
            id: TaskId::from("objects"),
            description: "detect objects".to_owned(),
        }]
    }

    fn workflow(&self) -> TaskGraph {
        TaskGraph {
            nodes: vec![TaskNode {
                id: TaskId::from("objects"),
                depends_on: Vec::new(),
            }],
        }
    }

    fn tool_factories(&self) -> Vec<Arc<dyn AgentTool>> {
        Vec::new()
    }

    fn validators(&self) -> Vec<Arc<dyn AnnotationValidator>> {
        vec![Arc::new(ConfidenceValidator)]
    }

    fn refiners(&self) -> Vec<Arc<dyn AnnotationRefiner>> {
        Vec::new()
    }

    fn prompt_resources(&self, _request: &SkillResourceRequest) -> CoreResult<Vec<SkillResource>> {
        Ok(vec![SkillResource {
            name: "objects".to_owned(),
            media_type: "text/markdown".to_owned(),
            content: "Submit checked bounding boxes.".to_owned(),
        }])
    }

    fn correction_taxonomy(&self) -> Vec<CorrectionKind> {
        Vec::new()
    }

    fn review_policy(&self) -> Arc<dyn ReviewPolicy> {
        Arc::new(AcceptCleanCandidates)
    }
}

fn project() -> ProjectSchema {
    ProjectSchema {
        version: 1,
        project: ProjectDescriptor {
            name: "Vertical loop".to_owned(),
            skill: "bbox_fixture".to_owned(),
            skill_version: "1".to_owned(),
            language: "en".to_owned(),
        },
        dataset: DatasetConfig {
            root: PathBuf::from("images"),
            include: vec!["**/*.png".to_owned()],
            recursive: true,
        },
        runtime: RuntimeConfig {
            max_parallel_images: 1,
            max_agent_steps_per_image: 3,
            max_retries_per_task: 1,
            auto_resume: true,
        },
        tasks: vec![TaskConfig {
            id: TaskId::from("objects"),
            kind: TaskKind::BoundingBox,
            labels: vec!["target".to_owned()],
            required: true,
            multi_label: false,
            depends_on: Vec::new(),
            validators: vec!["minimum_confidence".to_owned()],
            refiners: Vec::new(),
            target_task: None,
            target_labels: Vec::new(),
            attributes: BTreeMap::<String, annotagent_core::AttributeDefinition>::new(),
        }],
        review: ReviewConfig {
            auto_accept_confidence: 0.9,
            force_review_below: 0.7,
            force_review_on_warning_codes: Vec::new(),
        },
        export: ExportConfig {
            formats: vec!["native".to_owned()],
        },
    }
}

#[tokio::test]
async fn model_tool_validator_commit_event_sqlite_and_usage_form_one_loop() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![MockStep {
            expect_task: Some("objects".to_owned()),
            expect_message_contains: Some("Submit checked bounding boxes".to_owned()),
            response: MockResponseSpec::ToolCall {
                name: "submit_annotation_candidates".to_owned(),
                arguments: json!({
                    "annotations": [{
                        "label": "target",
                        "value": {"kind": "bounding_box", "rect": [0.2, 0.3, 0.2, 0.1]},
                        "attributes": {},
                        "confidence": 0.95
                    }]
                }),
            },
            usage: MockUsage {
                input_tokens: 100,
                output_tokens: 25,
            },
        }],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite store"));
    let runtime = AgentRuntime::new(
        Arc::new(BboxSkill::new()),
        provider,
        store.clone(),
        PricingConfig {
            currency: "USD".to_owned(),
            input_per_million_tokens: Decimal::from_str("1.0").expect("decimal"),
            output_per_million_tokens: Decimal::from_str("4.0").expect("decimal"),
            per_image: Decimal::ZERO,
            per_request: Decimal::ZERO,
            per_credit: Decimal::ZERO,
        },
        Budget {
            max_requests: Some(10),
            ..Budget::default()
        },
        AgentLoopConfig {
            max_steps_per_image: 3,
            ..AgentLoopConfig::default()
        },
    );
    let run_id = annotagent_core::RunId::new();
    let result = runtime
        .run_image(ImageRunRequest {
            run_id,
            project_id: annotagent_core::ProjectId::new(),
            project_root: PathBuf::from("."),
            project: Arc::new(project()),
            image_id: ImageId::new(),
            image: Arc::new(ImageFrame {
                metadata: ImageMetadata {
                    width: 1,
                    height: 1,
                    mime_type: "image/png".to_owned(),
                    sha256: "fixture".to_owned(),
                },
                rgb: vec![0, 128, 0],
            }),
            model_image: None,
        })
        .await
        .expect("vertical loop completes");

    assert_eq!(
        result.status,
        annotagent_core::RunStatus::Completed,
        "{result:?}"
    );
    assert_eq!(result.committed.len(), 1);
    assert_eq!(result.usage.input_tokens, 100);
    assert_eq!(result.usage.output_tokens, 25);
    assert_eq!(
        store.list_annotations(run_id).expect("annotations").len(),
        1
    );
    assert!(store.list_events(run_id).expect("events").len() >= 8);
    assert_eq!(
        store.run_status(run_id).expect("run status"),
        annotagent_core::RunStatus::Completed
    );

    let history = store.history(run_id).expect("versioned history");
    assert_eq!(
        history.schema_version,
        annotagent_core::HISTORY_SCHEMA_VERSION
    );
    assert_eq!(history.annotations.len(), 1);
    assert_eq!(history.usage.len(), 1);
    assert_eq!(history.tool_calls.len(), 1);
    let history_directory = tempfile::tempdir().expect("history directory");
    let history_path = history_directory.path().join("run.json");
    store
        .export_history(run_id, &history_path)
        .expect("history export");
    let serialized = std::fs::read_to_string(&history_path).expect("history file");
    assert!(!serialized.contains("Authorization"));
    assert!(!serialized.contains("data:image"));

    let imported = store
        .import_history(history)
        .expect("history import with ID collision");
    assert!(imported.ids_remapped);
    assert_ne!(imported.run_id, run_id);
    assert_eq!(
        store
            .history(imported.run_id)
            .expect("imported history")
            .annotations
            .len(),
        1
    );
    assert!(!imported.warnings.is_empty());
}

#[tokio::test]
async fn malformed_model_candidate_is_fed_back_and_retried_instead_of_crashing_run() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: None,
                response: MockResponseSpec::ToolCall {
                    name: "submit_annotation_candidates".to_owned(),
                    arguments: json!({"annotations": [{
                        "label": "objects",
                        "value": {"kind": "bounding_box", "rect": [0.2, 0.3, 0.2, 0.1]},
                        "attributes": {},
                        "confidence": 0.95
                    }]}),
                },
                usage: MockUsage {
                    input_tokens: 100,
                    output_tokens: 25,
                },
            },
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: Some("Candidate rejected before validation".to_owned()),
                response: MockResponseSpec::ToolCall {
                    name: "submit_annotation_candidates".to_owned(),
                    arguments: json!({"annotations": [{
                        "label": "target",
                        "value": {"kind": "bounding_box", "rect": [0.2, 0.3, 0.2, 0.1]},
                        "attributes": {},
                        "confidence": 0.95
                    }]}),
                },
                usage: MockUsage {
                    input_tokens: 100,
                    output_tokens: 25,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite store"));
    let runtime = AgentRuntime::new(
        Arc::new(BboxSkill::new()),
        provider,
        store.clone(),
        PricingConfig::default(),
        Budget {
            max_requests: Some(10),
            ..Budget::default()
        },
        AgentLoopConfig {
            max_steps_per_image: 3,
            ..AgentLoopConfig::default()
        },
    );
    let run_id = annotagent_core::RunId::new();
    let result = runtime
        .run_image(ImageRunRequest {
            run_id,
            project_id: annotagent_core::ProjectId::new(),
            project_root: PathBuf::from("."),
            project: Arc::new(project()),
            image_id: ImageId::new(),
            image: Arc::new(ImageFrame {
                metadata: ImageMetadata {
                    width: 1,
                    height: 1,
                    mime_type: "image/png".to_owned(),
                    sha256: "retry-fixture".to_owned(),
                },
                rgb: vec![0, 128, 0],
            }),
            model_image: None,
        })
        .await
        .expect("runtime retries malformed candidate");

    assert_eq!(
        result.status,
        annotagent_core::RunStatus::Completed,
        "{result:?}"
    );
    assert_eq!(result.committed.len(), 1);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.code == "invalid_candidate")
    );
    assert!(
        store
            .list_events(run_id)
            .expect("events")
            .iter()
            .any(|event| event.kind == annotagent_core::RunEventKind::RetryScheduled)
    );
}
