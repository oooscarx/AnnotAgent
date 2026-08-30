use std::{collections::BTreeMap, path::PathBuf, str::FromStr, sync::Arc, time::Duration};

use annotagent_core::{
    AgentTool, AnnotationRefiner, AnnotationValidator, Budget, CoreResult, CorrectionKind,
    DatasetConfig, DomainSkill, ExportConfig, ImageFrame, ImageId, ImageMetadata, IssueSeverity,
    ModelCapabilities, ModelRequest, ModelResponse, PricingConfig, ProjectDescriptor,
    ProjectSchema, ReviewConfig, ReviewContext, ReviewDecision, ReviewPolicy, RuntimeConfig,
    SkillManifest, SkillResource, SkillResourceRequest, SuggestedAction, TaskConfig, TaskGraph,
    TaskId, TaskKind, TaskNode, TaskTemplate, ValidationContext, ValidationEvidence,
    ValidationIssue, VisionModelProvider,
};
use annotagent_provider::{MockResponseSpec, MockScript, MockStep, MockUsage, MockVisionProvider};
use annotagent_runtime::{AgentLoopConfig, AgentRuntime, ImageRunRequest};
use annotagent_storage::SqliteStore;
use rust_decimal::Decimal;
use serde_json::json;
use tokio_util::sync::CancellationToken;

struct SlowProvider;

#[async_trait::async_trait]
impl VisionModelProvider for SlowProvider {
    fn name(&self) -> &str {
        "slow_fixture"
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            vision: true,
            tool_calls: true,
            json_schema: true,
            usage_reporting: false,
            multi_image: false,
        }
    }

    async fn complete(
        &self,
        _request: ModelRequest,
        _cancellation: CancellationToken,
    ) -> CoreResult<ModelResponse> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Err(annotagent_core::CoreError::Provider(
            "slow fixture unexpectedly completed".to_owned(),
        ))
    }
}

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
                kind: annotagent_core::SkillKind::Domain,
                skill_version: "1".to_owned(),
                display_name: "BBox fixture".to_owned(),
                description: "A test-only bounding-box skill".to_owned(),
                product_visibility: annotagent_core::SkillProductVisibility::Primary,
                deprecated_alias_for: None,
                rust_implementation: None,
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                capabilities: Vec::new(),
                requires: annotagent_core::SkillCapabilityRequirements::default(),
                optional_capabilities: Vec::new(),
                nodes: Vec::new(),
                tools: Vec::new(),
                validators: Vec::new(),
                policies: Vec::new(),
                templates: Vec::new(),
                summary_resources: Vec::new(),
                task_resources: BTreeMap::new(),
                correction_taxonomy: Vec::new(),
                visual_profile: BTreeMap::new(),
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
        vec![
            TaskTemplate {
                id: TaskId::from("objects"),
                description: "detect objects".to_owned(),
            },
            TaskTemplate {
                id: TaskId::from("optional_check"),
                description: "optional follow-up".to_owned(),
            },
        ]
    }

    fn workflow(&self) -> TaskGraph {
        TaskGraph {
            nodes: vec![
                TaskNode {
                    id: TaskId::from("objects"),
                    depends_on: Vec::new(),
                },
                TaskNode {
                    id: TaskId::from("optional_check"),
                    depends_on: Vec::new(),
                },
            ],
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
            enabled_skills: Vec::new(),
            language: "en".to_owned(),
        },
        dataset: DatasetConfig {
            root: PathBuf::from("images"),
            include: vec!["**/*.png".to_owned()],
            recursive: true,
        },
        runtime: RuntimeConfig {
            max_parallel_images: 1,
            max_model_turns_per_task: 3,
            max_tool_calls_per_task: 6,
            max_recovery_turns_per_task: 1,
            task_timeout_seconds: 60,
            provider_request_timeout_seconds: 30,
            max_retries: 1,
            auto_resume: true,
        },
        tasks: vec![TaskConfig {
            id: TaskId::from("objects"),
            display_name: None,
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
            max_model_turns_per_task: 3,
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
    assert_eq!(history.task_runs.len(), 1);
    assert_eq!(
        history.task_runs[0].status,
        annotagent_core::TaskRunStatus::Succeeded
    );
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
    let imported_history = store.history(imported.run_id).expect("imported history");
    assert_eq!(imported_history.annotations.len(), 1);
    assert_eq!(imported_history.task_runs.len(), 1);
    assert_eq!(imported_history.task_runs[0].run_id, imported.run_id);
    let assistant_call_id = imported_history
        .model_messages
        .iter()
        .flat_map(|entry| &entry.message.tool_calls)
        .next()
        .expect("imported assistant tool call")
        .id
        .clone();
    let tool_message_id = imported_history
        .model_messages
        .iter()
        .find_map(|entry| entry.message.tool_call_id.clone())
        .expect("imported tool result message");
    assert_eq!(assistant_call_id, tool_message_id);
    assert_eq!(assistant_call_id, imported_history.tool_calls[0].call_id);
    assert!(!imported.warnings.is_empty());
}

#[tokio::test]
async fn provider_failure_is_recorded_and_retried_without_consuming_the_agent_step() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: None,
                response: MockResponseSpec::Error {
                    message: "request timed out after 120 seconds".to_owned(),
                },
                usage: MockUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                },
            },
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: None,
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
            max_model_turns_per_task: 1,
            max_retries: 2,
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
                    sha256: "provider-retry-fixture".to_owned(),
                },
                rgb: vec![0, 128, 0],
            }),
            model_image: None,
        })
        .await
        .expect("provider retry completes");

    assert_eq!(result.status, annotagent_core::RunStatus::Completed);
    assert_eq!(result.usage.requests, 2);
    let history = store.history(run_id).expect("history");
    assert_eq!(history.usage.len(), 2);
    assert!(!history.usage[0].success);
    assert!(history.usage[1].success);
    assert!(history.events.iter().any(|event| {
        event.kind == annotagent_core::RunEventKind::ModelCallFailed
            && matches!(
                &event.payload,
                annotagent_core::RunEventPayload::ProviderFailure {
                    provider,
                    model,
                    retry_count: 1,
                    error_code,
                    ..
                } if provider == "mock" && model == "mock-vision" && error_code == "provider_error"
            )
    }));
}

#[tokio::test]
async fn task_timeout_is_structured_in_events_and_terminal_history() {
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite store"));
    let runtime = AgentRuntime::new(
        Arc::new(BboxSkill::new()),
        Arc::new(SlowProvider),
        store.clone(),
        PricingConfig::default(),
        Budget::default(),
        AgentLoopConfig {
            task_timeout: Duration::from_millis(10),
            provider_request_timeout: Duration::from_secs(1),
            max_retries: 0,
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
                    sha256: "task-timeout-fixture".to_owned(),
                },
                rgb: vec![0, 128, 0],
            }),
            model_image: None,
        })
        .await
        .expect("task timeout becomes a terminal result");

    assert_eq!(result.status, annotagent_core::RunStatus::Failed);
    let history = store.history(run_id).expect("history");
    let failure = history
        .events
        .iter()
        .find(|event| event.kind == annotagent_core::RunEventKind::TaskFailed)
        .expect("structured task failure event");
    assert!(matches!(
        &failure.payload,
        annotagent_core::RunEventPayload::TaskFailure {
            task_id,
            node_id,
            elapsed_ms,
            error_code,
            summary,
        } if task_id.as_str() == "objects"
            && node_id == "objects"
            && *elapsed_ms >= 1
            && error_code == "task_timeout"
            && summary.contains("elapsed_ms=")
    ));
    let reason = history.run.terminal_reason.expect("terminal reason");
    assert!(reason.contains("task_timeout"), "{reason}");
    assert!(reason.contains("elapsed_ms="), "{reason}");
}

#[tokio::test]
async fn provider_timeout_preserves_provider_model_task_retry_and_elapsed() {
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite store"));
    let runtime = AgentRuntime::new(
        Arc::new(BboxSkill::new()),
        Arc::new(SlowProvider),
        store.clone(),
        PricingConfig::default(),
        Budget::default(),
        AgentLoopConfig {
            task_timeout: Duration::from_secs(1),
            provider_request_timeout: Duration::from_millis(10),
            max_retries: 0,
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
                    sha256: "provider-timeout-fixture".to_owned(),
                },
                rgb: vec![0, 128, 0],
            }),
            model_image: None,
        })
        .await
        .expect("provider timeout becomes a terminal result");

    assert_eq!(result.status, annotagent_core::RunStatus::Failed);
    let history = store.history(run_id).expect("history");
    let failure = history
        .events
        .iter()
        .find(|event| event.kind == annotagent_core::RunEventKind::ModelCallFailed)
        .expect("structured provider timeout event");
    assert!(matches!(
        &failure.payload,
        annotagent_core::RunEventPayload::ProviderFailure {
            task_id,
            node_id,
            provider,
            model,
            elapsed_ms,
            retry_count: 1,
            error_code,
            summary,
        } if task_id.as_str() == "objects"
            && node_id == "objects"
            && provider == "slow_fixture"
            && model == "mock-vision"
            && *elapsed_ms >= 1
            && error_code == "provider_timeout"
            && summary.contains("retry=1")
    ));
    let reason = history.run.terminal_reason.expect("terminal reason");
    assert!(reason.contains("provider_timeout"), "{reason}");
    assert!(reason.contains("slow_fixture"), "{reason}");
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
            max_model_turns_per_task: 3,
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

#[tokio::test]
async fn required_success_and_optional_failure_produce_partial_run() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: None,
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
            MockStep {
                expect_task: Some("optional_check".to_owned()),
                expect_message_contains: None,
                response: MockResponseSpec::Error {
                    message: "optional provider failure".to_owned(),
                },
                usage: MockUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                },
            },
        ],
    }));
    let mut project = project();
    project.tasks.push(TaskConfig {
        id: TaskId::from("optional_check"),
        display_name: None,
        kind: TaskKind::BoundingBox,
        labels: vec!["target".to_owned()],
        required: false,
        multi_label: false,
        depends_on: Vec::new(),
        validators: Vec::new(),
        refiners: Vec::new(),
        target_task: None,
        target_labels: Vec::new(),
        attributes: BTreeMap::new(),
    });
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite store"));
    let runtime = AgentRuntime::new(
        Arc::new(BboxSkill::new()),
        provider,
        store.clone(),
        PricingConfig::default(),
        Budget::default(),
        AgentLoopConfig {
            max_retries: 0,
            ..AgentLoopConfig::default()
        },
    );
    let run_id = annotagent_core::RunId::new();
    let result = runtime
        .run_image(ImageRunRequest {
            run_id,
            project_id: annotagent_core::ProjectId::new(),
            project_root: PathBuf::from("."),
            project: Arc::new(project),
            image_id: ImageId::new(),
            image: Arc::new(ImageFrame {
                metadata: ImageMetadata {
                    width: 1,
                    height: 1,
                    mime_type: "image/png".to_owned(),
                    sha256: "partial-run".to_owned(),
                },
                rgb: vec![0, 128, 0],
            }),
            model_image: None,
        })
        .await
        .expect("optional failure is represented, not raised");
    assert_eq!(result.status, annotagent_core::RunStatus::Partial);
    assert_eq!(result.committed.len(), 1);
    let statuses = store.list_task_runs(run_id).expect("task statuses");
    assert!(statuses.iter().any(|task| {
        task.task_id == TaskId::from("objects")
            && task.status == annotagent_core::TaskRunStatus::Succeeded
    }));
    assert!(statuses.iter().any(|task| {
        task.task_id == TaskId::from("optional_check")
            && task.status == annotagent_core::TaskRunStatus::Failed
    }));
}
