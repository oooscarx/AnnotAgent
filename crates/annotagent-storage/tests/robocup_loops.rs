use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{
    AnnotationSource, AnnotationValue, Budget, CorrectionFeatures, CorrectionRecord, LabelId,
    PricingConfig, ProjectId, ProjectSchema, RunEventKind, RunId, RunStatus, TaskId, TaskRunStatus,
};
use annotagent_image_tools::{generate_synthetic_robocup, load_image};
use annotagent_provider::{
    MockResponseSpec, MockScript, MockStep, MockToolCall, MockUsage, MockVisionProvider,
};
use annotagent_runtime::{AgentLoopConfig, AgentRuntime, ImageRunRequest, RuntimeStore};
use annotagent_skill_robocup::RoboCupSkill;
use annotagent_storage::SqliteStore;
use chrono::Utc;
use serde_json::json;

fn project_for(task_id: &str) -> ProjectSchema {
    let mut project =
        ProjectSchema::from_yaml(include_str!("../../../examples/robocup/project.yaml"))
            .expect("example project");
    project.tasks.retain(|task| task.id.as_str() == task_id);
    project.tasks[0].depends_on.clear();
    if task_id == "objects" {
        project.tasks[0].validators = vec!["ball_hard_negative".to_owned()];
    }
    if task_id == "field_line" {
        project.tasks[0].validators = vec![
            "white_line_appearance".to_owned(),
            "polyline_continuity".to_owned(),
        ];
    }
    project
}

fn fixture() -> (tempfile::TempDir, Arc<annotagent_core::ImageFrame>) {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let path = temporary.path().join("field.png");
    generate_synthetic_robocup(&path).expect("generate fixture");
    let image = Arc::new(load_image(&path, 1_000_000).expect("load fixture"));
    (temporary, image)
}

fn runtime(provider: Arc<MockVisionProvider>, store: Arc<SqliteStore>) -> AgentRuntime {
    runtime_with_steps(provider, store, 4)
}

fn runtime_with_steps(
    provider: Arc<MockVisionProvider>,
    store: Arc<SqliteStore>,
    max_model_turns_per_task: u32,
) -> AgentRuntime {
    AgentRuntime::new(
        Arc::new(RoboCupSkill::new().expect("RoboCup skill")),
        provider,
        store,
        PricingConfig::default(),
        Budget {
            max_requests: Some(10),
            ..Budget::default()
        },
        AgentLoopConfig {
            max_model_turns_per_task,
            max_retries: 2,
            ..AgentLoopConfig::default()
        },
    )
}

#[tokio::test]
async fn qwen_compatible_tool_result_is_followed_by_model_submit_and_commit() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: None,
                response: MockResponseSpec::ToolCall {
                    name: "evaluate_ball_hard_negative".to_owned(),
                    arguments: json!({"bbox": [0.2, 0.3, 0.2, 0.2]}),
                },
                usage: MockUsage {
                    input_tokens: 100,
                    output_tokens: 20,
                },
            },
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: Some("white_ratio".to_owned()),
                response: MockResponseSpec::ToolCall {
                    name: "submit_annotation_candidates".to_owned(),
                    arguments: json!({"annotations": [{
                        "label": "robot",
                        "value": {"kind": "bounding_box", "rect": [0.4, 0.25, 0.15, 0.4]},
                        "attributes": {},
                        "confidence": 0.97
                    }]}),
                },
                usage: MockUsage {
                    input_tokens: 140,
                    output_tokens: 35,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let (temporary, image) = fixture();
    let run_id = RunId::new();
    let result = runtime(provider, store.clone())
        .run_image(ImageRunRequest {
            run_id,
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project_for("objects")),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("evidence then submit");
    assert_eq!(result.status, RunStatus::Completed, "{result:#?}");
    assert_eq!(result.committed.len(), 1);
    assert_eq!(result.usage.requests, 2);
    let history = store.history(run_id).expect("history");
    let first_assistant = history
        .model_messages
        .iter()
        .position(|entry| {
            entry
                .message
                .tool_calls
                .iter()
                .any(|call| call.name == "evaluate_ball_hard_negative")
        })
        .expect("evidence tool call");
    let tool_result = &history.model_messages[first_assistant + 1].message;
    assert_eq!(tool_result.role, annotagent_core::ModelRole::Tool);
    assert!(tool_result.content.contains("white_ratio"));
    assert!(
        history.model_messages[first_assistant + 2..]
            .iter()
            .any(|entry| entry
                .message
                .tool_calls
                .iter()
                .any(|call| call.name == "submit_annotation_candidates"))
    );
}

#[tokio::test]
async fn absent_penalty_mark_is_a_succeeded_empty_task_and_run_completes() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![MockStep {
            expect_task: Some("penalty_mark".to_owned()),
            expect_message_contains: None,
            response: MockResponseSpec::ToolCall {
                name: "finish_task".to_owned(),
                arguments: json!({}),
            },
            usage: MockUsage {
                input_tokens: 60,
                output_tokens: 5,
            },
        }],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let (temporary, image) = fixture();
    let run_id = RunId::new();
    let result = runtime(provider, store.clone())
        .run_image(ImageRunRequest {
            run_id,
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project_for("penalty_mark")),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("valid empty task");
    assert_eq!(result.status, RunStatus::Completed, "{result:#?}");
    assert!(result.committed.is_empty());
    let task_runs = store.list_task_runs(run_id).expect("task status");
    assert_eq!(task_runs.len(), 1);
    assert_eq!(task_runs[0].status, TaskRunStatus::SucceededEmpty);
}

#[tokio::test]
async fn identical_deterministic_tool_call_reuses_cached_result() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: None,
                response: MockResponseSpec::ToolCalls {
                    calls: vec![
                        MockToolCall {
                            name: "evaluate_ball_hard_negative".to_owned(),
                            arguments: json!({"bbox": [0.2, 0.3, 0.2, 0.2]}),
                        },
                        MockToolCall {
                            name: "evaluate_ball_hard_negative".to_owned(),
                            arguments: json!({"bbox": [0.2, 0.3, 0.2, 0.2]}),
                        },
                    ],
                    content: None,
                },
                usage: MockUsage {
                    input_tokens: 100,
                    output_tokens: 30,
                },
            },
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: Some(
                    "identical deterministic tool call reused".to_owned(),
                ),
                response: MockResponseSpec::ToolCall {
                    name: "submit_annotation_candidates".to_owned(),
                    arguments: json!({"annotations": [{
                        "label": "robot",
                        "value": {"kind": "bounding_box", "rect": [0.4, 0.25, 0.15, 0.4]},
                        "attributes": {},
                        "confidence": 0.97
                    }]}),
                },
                usage: MockUsage {
                    input_tokens: 140,
                    output_tokens: 35,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let (temporary, image) = fixture();
    let run_id = RunId::new();
    let result = runtime(provider, store.clone())
        .run_image(ImageRunRequest {
            run_id,
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project_for("objects")),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("cached tool loop");
    assert_eq!(result.status, RunStatus::Completed, "{result:#?}");
    let history = store.history(run_id).expect("history");
    assert!(history.tool_calls.iter().any(|call| {
        call.result
            .as_ref()
            .is_some_and(|result| result.ui_summary.starts_with("cache hit"))
    }));
}

#[tokio::test]
async fn structured_tool_data_reaches_the_model_before_submission() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![MockStep {
            expect_task: Some("field_line".to_owned()),
            expect_message_contains: None,
            response: MockResponseSpec::ToolCall {
                name: "refine_robocup_field_line".to_owned(),
                arguments: json!({"points": [[0.08, 0.47], [0.92, 0.47]]}),
            },
            usage: MockUsage {
                input_tokens: 120,
                output_tokens: 30,
            },
        }],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let (temporary, image) = fixture();
    let run_id = RunId::new();
    let result = runtime_with_steps(provider, store.clone(), 2)
        .run_image(ImageRunRequest {
            run_id,
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project_for("field_line")),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("artifact-producing tool completes the task");

    assert_eq!(result.status, RunStatus::Completed, "{result:#?}");
    assert_eq!(result.usage.requests, 1);
    assert_eq!(result.committed.len(), 1);
    let history = store.history(run_id).expect("history");
    assert_eq!(history.artifacts.len(), 2);
    let assistant_index = history
        .model_messages
        .iter()
        .position(|entry| !entry.message.tool_calls.is_empty())
        .expect("assistant tool-call message");
    let assistant = &history.model_messages[assistant_index].message;
    let tool = &history.model_messages[assistant_index + 1].message;
    assert_eq!(tool.role, annotagent_core::ModelRole::Tool);
    assert_eq!(
        tool.tool_call_id.as_ref(),
        Some(&assistant.tool_calls[0].id)
    );
    assert!(tool.content.contains("artifact_references"));
    assert!(tool.content.contains("pixel_support"));
    assert!(!tool.content.contains("points"));
    assert!(
        history
            .events
            .iter()
            .any(|event| { event.kind == RunEventKind::ArtifactCommitted })
    );
}

#[tokio::test]
async fn white_shoe_candidate_is_detected_retried_and_removed() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: None,
                response: MockResponseSpec::ToolCall {
                    name: "submit_annotation_candidates".to_owned(),
                    arguments: json!({"annotations": [
                        {
                            "label": "robot",
                            "value": {"kind": "bounding_box", "rect": [0.225, 0.445, 0.07, 0.2]},
                            "attributes": {}, "confidence": 0.97
                        },
                        {
                            "label": "ball",
                            "value": {"kind": "bounding_box", "rect": [0.218, 0.615, 0.04, 0.03]},
                            "attributes": {}, "confidence": 0.94
                        }
                    ]}),
                },
                usage: MockUsage {
                    input_tokens: 200,
                    output_tokens: 60,
                },
            },
            MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: Some("possible_white_shoe".to_owned()),
                response: MockResponseSpec::ToolCall {
                    name: "submit_annotation_candidates".to_owned(),
                    arguments: json!({"annotations": []}),
                },
                usage: MockUsage {
                    input_tokens: 240,
                    output_tokens: 20,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let project_id = ProjectId::new();
    for _ in 0..4 {
        store
            .save_correction(&CorrectionRecord {
                id: uuid::Uuid::new_v4(),
                project_id,
                skill_id: "robocup".to_owned(),
                task_id: TaskId::from("objects"),
                predicted_label: Some(LabelId::from("ball")),
                corrected_label: None,
                reason_code: "white_shoe_as_ball".to_owned(),
                original_annotation: None,
                corrected_annotation: None,
                note: None,
                image_features: CorrectionFeatures {
                    geometry: BTreeMap::new(),
                    colors: BTreeMap::new(),
                },
                created_at: Utc::now(),
            })
            .expect("save correction");
    }
    assert!(
        store
            .correction_risk(
                project_id,
                "robocup",
                &TaskId::from("objects"),
                Some(&LabelId::from("ball"))
            )
            .await
            .expect("correction risk")
            >= 0.2
    );
    let (temporary, image) = fixture();
    let run_id = RunId::new();
    let result = runtime(provider, store.clone())
        .run_image(ImageRunRequest {
            run_id,
            project_id,
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project_for("objects")),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("hard-negative loop");
    assert_eq!(result.status, RunStatus::Completed);
    assert_eq!(result.usage.requests, 2);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.code == "possible_white_shoe")
    );
    assert_eq!(result.committed.len(), 1);
    assert_eq!(
        result.committed[0].label.as_ref().map(LabelId::as_str),
        Some("robot")
    );
    assert!(
        store
            .list_events(run_id)
            .expect("events")
            .iter()
            .any(|event| event.kind == RunEventKind::RetryScheduled)
    );
}

#[tokio::test]
async fn coarse_field_line_is_refined_validated_committed_and_revisioned() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            MockStep {
                expect_task: Some("field_line".to_owned()),
                expect_message_contains: Some("Rust pixel refiner".to_owned()),
                response: MockResponseSpec::ToolCall {
                    name: "submit_annotation_candidates".to_owned(),
                    arguments: json!({"annotations": [{
                        "label": "white_field_line",
                        "value": {"kind": "polyline", "points": [[0.08, 0.47], [0.92, 0.47]]},
                        "attributes": {}, "confidence": 0.96
                    }]}),
                },
                usage: MockUsage {
                    input_tokens: 180,
                    output_tokens: 44,
                },
            },
            MockStep {
                expect_task: Some("field_line".to_owned()),
                expect_message_contains: None,
                response: MockResponseSpec::ToolCall {
                    name: "submit_annotation_candidates".to_owned(),
                    arguments: json!({"annotations": []}),
                },
                usage: MockUsage {
                    input_tokens: 50,
                    output_tokens: 10,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let (temporary, image) = fixture();
    let run_id = RunId::new();
    let result = runtime(provider, store.clone())
        .run_image(ImageRunRequest {
            run_id,
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project_for("field_line")),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("field-line loop");
    assert_eq!(result.status, RunStatus::Completed);
    assert_eq!(result.usage.requests, 1);
    assert_eq!(result.committed.len(), 1, "issues: {:#?}", result.issues);
    assert_eq!(result.committed[0].source, AnnotationSource::ModelAndTool);
    let AnnotationValue::Polyline { points } = &result.committed[0].value else {
        panic!("committed annotation is a polyline");
    };
    let average_y = points.iter().map(|point| point.y()).sum::<f32>() / points.len() as f32;
    assert!((average_y - 0.5).abs() < 0.02);
    assert_eq!(
        store
            .list_revisions(result.committed[0].id)
            .expect("revisions")
            .len(),
        1
    );
    let event_kinds: Vec<_> = store
        .list_events(run_id)
        .expect("events")
        .into_iter()
        .map(|event| event.kind)
        .collect();
    assert!(event_kinds.contains(&RunEventKind::RefinementStarted));
    assert!(event_kinds.contains(&RunEventKind::RefinementCompleted));
}

#[tokio::test]
async fn multiple_tool_calls_are_persisted_and_answered_in_order() {
    let provider = Arc::new(MockVisionProvider::new(MockScript {
        steps: vec![
            MockStep {
                expect_task: Some("field_line".to_owned()),
                expect_message_contains: None,
                response: MockResponseSpec::ToolCalls {
                    calls: vec![
                        MockToolCall {
                            name: "refine_robocup_field_line".to_owned(),
                            arguments: json!({"points": [[0.08, 0.47], [0.92, 0.47]]}),
                        },
                        MockToolCall {
                            name: "refine_robocup_field_line".to_owned(),
                            arguments: json!({"points": [[0.1, 0.45], [0.9, 0.45]]}),
                        },
                        MockToolCall {
                            name: "refine_robocup_field_line".to_owned(),
                            arguments: json!({"points": [[0.1, 0.55], [0.9, 0.55]]}),
                        },
                    ],
                    content: None,
                },
                usage: MockUsage {
                    input_tokens: 120,
                    output_tokens: 80,
                },
            },
            MockStep {
                expect_task: Some("field_line".to_owned()),
                expect_message_contains: Some("pixel_support".to_owned()),
                response: MockResponseSpec::ToolCall {
                    name: "submit_annotation_candidates".to_owned(),
                    arguments: json!({"annotations": [{
                        "label": "white_field_line",
                        "value": {"kind": "polyline", "points": [[0.08, 0.47], [0.92, 0.47]]},
                        "attributes": {}, "confidence": 0.96
                    }]}),
                },
                usage: MockUsage {
                    input_tokens: 140,
                    output_tokens: 30,
                },
            },
        ],
    }));
    let store = Arc::new(SqliteStore::open_in_memory().expect("SQLite"));
    let (temporary, image) = fixture();
    let run_id = RunId::new();
    let result = runtime(provider, store.clone())
        .run_image(ImageRunRequest {
            run_id,
            project_id: ProjectId::new(),
            project_root: temporary.path().to_path_buf(),
            project: Arc::new(project_for("field_line")),
            image_id: annotagent_core::ImageId::new(),
            image,
            model_image: None,
        })
        .await
        .expect("multi-tool field-line loop");
    assert_eq!(result.status, RunStatus::Completed);
    assert_eq!(result.committed.len(), 1);
    let messages = store.history(run_id).expect("history").model_messages;
    let assistant_index = messages
        .iter()
        .position(|entry| entry.message.tool_calls.len() == 3)
        .expect("assistant with three tool calls");
    let calls = &messages[assistant_index].message.tool_calls;
    for (offset, call) in calls.iter().enumerate() {
        let tool = &messages[assistant_index + offset + 1].message;
        assert_eq!(tool.role, annotagent_core::ModelRole::Tool);
        assert_eq!(tool.tool_call_id.as_ref(), Some(&call.id));
    }
}
